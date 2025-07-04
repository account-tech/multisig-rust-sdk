pub mod actions;
pub mod dynamic_fields;
pub mod intents;
pub mod move_binding;
pub mod multisig;
pub mod owned_objects;
pub mod params;
pub mod utils;

use anyhow::{anyhow, Ok, Result};
use move_types::{Identifier, Key, MoveStruct, MoveType, StructTag, TypeTag};
use std::{str::FromStr, sync::Arc};
use sui_graphql_client::Client;
use sui_sdk_types::{Address, Argument, ObjectData};
use sui_transaction_builder::{unresolved::Input, TransactionBuilder};
use sui_transaction_builder::{Function, Serialized};

use crate::dynamic_fields::DynamicFields;
use crate::intents::{Intent, Intents};
use crate::move_binding::sui;
// use crate::move_binding::account_extensions as ae;
use crate::move_binding::account_actions as aa;
use crate::move_binding::account_multisig as am;
use crate::move_binding::account_protocol as ap;
use crate::multisig::Multisig;
use crate::owned_objects::OwnedObjects;
use crate::params::ParamsArgs;

// TODO: MultisigCreateBuilder
// TODO: intents, User

static ACCOUNT_PROTOCOL_PACKAGE: &str =
    "0x10c87c29ea5d5674458652ababa246742a763f9deafed11608b7f0baea296484";
static ACCOUNT_ACTIONS_PACKAGE: &str =
    "0xf477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94";
static ACCOUNT_MULTISIG_PACKAGE: &str =
    "0x460632ef4e9e708658788229531b99f1f3285de06e1e50e98a22633c7e494867";
static EXTENSIONS_OBJECT: &str =
    "0x698bc414f25a7036d9a72d6861d9d268e478492dc8bfef8b5c1c2f1eae769254";
static FEE_OBJECT: &str = "0xc27762578a0b1f37224550dcfd0442f37dc82744b802d3517822d1bd2718598f";
static CLOCK_OBJECT: &str = "0x0000000000000000000000000000000000000000000000000000000000000006";

pub struct MultisigClient {
    sui_client: Arc<Client>,
    multisig: Option<Multisig>,
}

impl MultisigClient {
    // === Constructors ===

    pub fn new_with_client(sui_client: Client) -> Self {
        Self {
            sui_client: Arc::new(sui_client),
            multisig: None,
        }
    }

    pub fn new_with_url(url: &str) -> Result<Self> {
        Ok(Self {
            sui_client: Arc::new(Client::new(url)?),
            multisig: None,
        })
    }

    pub fn new_testnet() -> Self {
        Self {
            sui_client: Arc::new(Client::new_testnet()),
            multisig: None,
        }
    }

    pub fn new_mainnet() -> Self {
        Self {
            sui_client: Arc::new(Client::new_mainnet()),
            multisig: None,
        }
    }

    // === Multisig ===

    pub async fn create_multisig(&self, builder: &mut TransactionBuilder) -> Result<()> {
        let fee_obj = utils::get_object(&self.sui_client, Address::from_hex(FEE_OBJECT)?).await?;
        let fee = if let ObjectData::Struct(obj) = fee_obj.data() {
            bcs::from_bytes::<am::fees::Fees>(obj.contents())
                .map_err(|e| anyhow!("Failed to parse fee object: {}", e))?
        } else {
            return Err(anyhow!("Fee object not a struct"));
        };

        let coin_amount = builder.input(Serialized(&fee.amount));
        let coin_arg = builder.split_coins(builder.gas(), vec![coin_amount]);
        let fee_arg = builder.input(Input::from(&fee_obj).by_ref());
        let extensions_arg =
            builder.input(self.obj(EXTENSIONS_OBJECT.parse().unwrap()).await?.by_ref());

        let account_obj = am::multisig::new_account(
            builder,
            extensions_arg.into(),
            fee_arg.into(),
            coin_arg.into(),
        );

        sui::transfer::public_share_object(builder, account_obj);

        Ok(())
    }

    pub async fn load_multisig(&mut self, id: Address) -> Result<()> {
        self.multisig = Some(Multisig::from_id(self.sui_client.clone(), id).await?);
        Ok(())
    }

    pub async fn refresh(&mut self) -> Result<()> {
        if let Some(multisig) = self.multisig.as_mut() {
            multisig.refresh().await?;
        }
        Ok(())
    }

    pub async fn approve_intent(
        &self,
        builder: &mut TransactionBuilder,
        intent_key: String,
    ) -> Result<()> {
        let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
        let key_arg = builder.input(Serialized(&intent_key));

        am::multisig::approve_intent(builder, ms_arg.into(), key_arg.into());

        Ok(())
    }

    pub async fn disapprove_intent(
        &self,
        builder: &mut TransactionBuilder,
        intent_key: String,
    ) -> Result<()> {
        let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
        let key_arg = builder.input(Serialized(&intent_key));

        am::multisig::disapprove_intent(builder, ms_arg.into(), key_arg.into());

        Ok(())
    }

    // === Commands ===

    pub async fn deposit_cap<CapType: Key>(
        &self,
        builder: &mut TransactionBuilder,
        cap: CapType,
    ) -> Result<()> {
        let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
        let cap_arg = builder.input(self.obj(*cap.id().as_address()).await?);

        let auth = am::multisig::authenticate(builder, ms_arg.into());
        aa::access_control::lock_cap::<am::multisig::Multisig, CapType>(
            builder,
            auth,
            ms_arg.into(),
            cap_arg.into(),
        );

        Ok(())
    }

    pub async fn replace_metadata(
        &self,
        builder: &mut TransactionBuilder,
        keys: Vec<String>,
        values: Vec<String>,
    ) -> Result<()> {
        let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
        let keys_arg = builder.input(Serialized(&keys));
        let values_arg = builder.input(Serialized(&values));

        let auth = am::multisig::authenticate(builder, ms_arg.into());

        ap::config::edit_metadata::<am::multisig::Approvals>(
            builder,
            auth,
            ms_arg.into(),
            keys_arg.into(),
            values_arg.into(),
        );

        Ok(())
    }

    pub async fn update_verified_deps_to_latest(
        &self,
        builder: &mut TransactionBuilder,
    ) -> Result<()> {
        let extensions_arg =
            builder.input(self.obj(EXTENSIONS_OBJECT.parse().unwrap()).await?.by_ref());
        let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
        let auth = am::multisig::authenticate(builder, ms_arg.into());

        ap::config::update_extensions_to_latest::<am::multisig::Approvals>(
            builder,
            auth,
            ms_arg.into(),
            extensions_arg.into(),
        );

        Ok(())
    }

    pub async fn deposit_treasury_cap<TreasuryCapType: Key>(
        &self,
        builder: &mut TransactionBuilder,
        max_supply: Option<u64>,
        cap: TreasuryCapType,
    ) -> Result<()> {
        let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
        let max_supply_arg = builder.input(Serialized(&max_supply));
        let cap_arg = builder.input(self.obj(*cap.id().as_address()).await?);

        let auth = am::multisig::authenticate(builder, ms_arg.into());
        aa::currency::lock_cap::<am::multisig::Multisig, TreasuryCapType>(
            builder,
            auth,
            ms_arg.into(),
            cap_arg.into(),
            max_supply_arg.into(),
        );

        Ok(())
    }

    pub async fn merge_and_split<CoinType: MoveType>(
        &self,
        builder: &mut TransactionBuilder,
        coins_to_merge: Vec<Address>,
        amounts_to_split: Vec<u64>,
        _coin_type: CoinType,
    ) -> Result<()> {
        let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
        let mut coin_inputs = Vec::new();
        for coin in coins_to_merge {
            let input = Input::from(&utils::get_object(&self.sui_client, coin).await?);
            coin_inputs.push(builder.input(input.by_val().with_receiving_kind()));
        }

        let to_merge_arg = builder.make_move_vec(None, coin_inputs);
        let to_split_arg = builder.input(Serialized(&amounts_to_split));

        let auth = am::multisig::authenticate(builder, ms_arg.into());
        ap::owned::merge_and_split::<am::multisig::Approvals, CoinType>(
            builder,
            auth,
            ms_arg.into(),
            to_merge_arg.into(),
            to_split_arg.into(),
        );

        Ok(())
    }

    pub async fn deposit_upgrade_cap(
        &self,
        builder: &mut TransactionBuilder,
        cap_id: Address,
        package_name: &str,
        timelock_duration: u64, // can be 0
    ) -> Result<()> {
        let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
        let package_name_arg = builder.input(Serialized(&package_name));
        let timelock_duration_arg = builder.input(Serialized(&timelock_duration));
        let upgrade_cap_arg = builder.input(self.obj(cap_id).await?);

        let auth = am::multisig::authenticate(builder, ms_arg.into());
        aa::package_upgrade::lock_cap::<am::multisig::Approvals>(
            builder,
            auth,
            ms_arg.into(),
            upgrade_cap_arg.into(),
            package_name_arg.into(),
            timelock_duration_arg.into(),
        );

        Ok(())
    }

    pub async fn open_vault(
        &self,
        builder: &mut TransactionBuilder,
        vault_name: &str,
    ) -> Result<()> {
        let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
        let vault_name_arg = builder.input(Serialized(&vault_name));

        let auth = am::multisig::authenticate(builder, ms_arg.into());
        aa::vault::open::<am::multisig::Approvals>(
            builder,
            auth,
            ms_arg.into(),
            vault_name_arg.into(),
        );

        Ok(())
    }

    pub async fn deposit_from_wallet<Coin: Key>(
        &self,
        builder: &mut TransactionBuilder,
        vault_name: &str,
        coin: Coin,
    ) -> Result<()> {
        let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
        let vault_name_arg = builder.input(Serialized(&vault_name));
        let coin_arg = builder.input(self.obj(*coin.id().as_address()).await?);

        let auth = am::multisig::authenticate(builder, ms_arg.into());
        aa::vault::deposit::<am::multisig::Approvals, Coin>(
            builder,
            auth,
            ms_arg.into(),
            vault_name_arg.into(),
            coin_arg.into(),
        );

        Ok(())
    }

    pub async fn close_vault(
        &self,
        builder: &mut TransactionBuilder,
        vault_name: &str,
    ) -> Result<()> {
        let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
        let vault_name_arg = builder.input(Serialized(&vault_name));

        let auth = am::multisig::authenticate(builder, ms_arg.into());
        aa::vault::close::<am::multisig::Approvals>(
            builder,
            auth,
            ms_arg.into(),
            vault_name_arg.into(),
        );

        Ok(())
    }

    pub async fn claim_vested<CoinType: MoveType>(
        &self,
        builder: &mut TransactionBuilder,
        vesting_id: Address,
        cap_id: Address,
        _coin_type: CoinType,
    ) -> Result<()> {
        let vesting_arg = builder.input(self.obj(vesting_id).await?);
        let cap_arg = builder.input(self.obj(cap_id).await?);
        let clock_arg = builder.input(self.obj(CLOCK_OBJECT.parse().unwrap()).await?);

        aa::vesting::claim::<CoinType>(
            builder,
            vesting_arg.into(),
            cap_arg.into(),
            clock_arg.into(),
        );

        Ok(())
    }

    pub async fn cancel_vesting<CoinType: MoveType>(
        &self,
        builder: &mut TransactionBuilder,
        vesting_id: Address,
        _coin_type: CoinType,
    ) -> Result<()> {
        let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_ref());
        let vesting_arg = builder.input(self.obj(vesting_id).await?);

        let auth = am::multisig::authenticate(builder, ms_arg.into());
        aa::vesting::cancel_payment::<am::multisig::Multisig, CoinType>(
            builder,
            auth,
            ms_arg.into(),
            vesting_arg.into(),
        );

        Ok(())
    }

    pub async fn destroy_empty_vesting<CoinType: MoveType>(
        &self,
        builder: &mut TransactionBuilder,
        vesting_id: Address,
        _coin_type: CoinType,
    ) -> Result<()> {
        let vesting_arg = builder.input(self.obj(vesting_id).await?);

        aa::vesting::destroy_empty::<CoinType>(builder, vesting_arg.into());

        Ok(())
    }

    pub async fn destroy_claim_cap(
        &self,
        builder: &mut TransactionBuilder,
        cap_id: Address,
    ) -> Result<()> {
        let cap_arg = builder.input(self.obj(cap_id).await?);

        aa::vesting::destroy_cap(builder, cap_arg.into());

        Ok(())
    }

    // === Intents ===

    define_intent_interface!(
        config_multisig,
        params::ConfigMultisigArgs,
        |builder, auth, multisig_input, params, outcome, args: params::ConfigMultisigArgs| {
            am::config::request_config_multisig(
                builder,
                auth,
                multisig_input,
                params,
                outcome,
                args.addresses,
                args.weights,
                args.roles,
                args.global,
                args.role_names,
                args.role_thresholds,
            )
        },
        |builder, executable, multisig| am::config::execute_config_multisig(
            builder, executable, multisig
        ),
        |builder, expired| am::config::delete_config_multisig(builder, expired),
    );

    define_intent_interface!(
        config_deps,
        params::ConfigDepsArgs,
        |builder, auth, multisig_input, params, outcome, args: params::ConfigDepsArgs| {
            ap::config::request_config_deps::<am::multisig::Multisig, am::multisig::Approvals>(
                builder,
                auth,
                multisig_input,
                params,
                outcome,
                args.extensions.into(),
                args.names,
                args.addresses,
                args.versions,
            )
        },
        |builder, executable, multisig| ap::config::execute_config_deps::<
            am::multisig::Multisig,
            am::multisig::Approvals,
        >(builder, executable, multisig),
        |builder, expired| ap::config::delete_config_deps(builder, expired),
    );

    define_intent_interface!(
        toggle_unverified_allowed,
        (),
        |builder, auth, multisig_input, params, outcome, _args: ()| {
            ap::config::request_toggle_unverified_allowed::<
                am::multisig::Multisig,
                am::multisig::Approvals,
            >(builder, auth, multisig_input, params, outcome)
        },
        |builder, executable, multisig| ap::config::execute_toggle_unverified_allowed::<
            am::multisig::Multisig,
            am::multisig::Approvals,
        >(builder, executable, multisig),
        |builder, expired| ap::config::delete_toggle_unverified_allowed(builder, expired),
    );

    define_intent_interface!(
        borrow_cap,
        CapType: Key,
        (),
        |builder, auth, multisig_input, params, outcome, _args: ()| {
            aa::access_control_intents::request_borrow_cap::<
                am::multisig::Multisig,
                am::multisig::Approvals,
                CapType,
            >(builder, auth, multisig_input, params, outcome)
        },
        |builder, executable, multisig, _none_obj| aa::access_control_intents::execute_borrow_cap::<
            am::multisig::Multisig,
            am::multisig::Approvals,
            CapType,
        >(builder, executable, multisig),
        |builder: &mut TransactionBuilder, expired: Argument| {
            aa::access_control::delete_borrow::<CapType>(builder, expired.into());
            aa::access_control::delete_return::<CapType>(builder, expired.into());
        },
    );

    define_intent_interface!(
        disable_rules,
        CoinType,
        params::DisableRulesArgs,
        |builder, auth, multisig_input, params, outcome, args: params::DisableRulesArgs| {
            aa::currency_intents::request_disable_rules::<
                am::multisig::Multisig,
                am::multisig::Approvals,
                CoinType,
            >(
                builder,
                auth,
                multisig_input,
                params,
                outcome,
                args.mint,
                args.burn,
                args.update_symbol,
                args.update_name,
                args.update_description,
                args.update_icon,
            )
        },
        |builder, executable, multisig, _none_obj| aa::currency_intents::execute_disable_rules::<
            am::multisig::Multisig,
            am::multisig::Approvals,
            CoinType,
        >(builder, executable, multisig),
        |builder, expired| aa::currency::delete_disable::<CoinType>(builder, expired),
    );

    define_intent_interface!(
        update_metadata,
        CoinType,
        params::UpdateMetadataArgs,
        |builder, auth, multisig_input, params, outcome, args: params::UpdateMetadataArgs| {
            aa::currency_intents::request_update_metadata::<
                am::multisig::Multisig,
                am::multisig::Approvals,
                CoinType,
            >(
                builder,
                auth,
                multisig_input,
                params,
                outcome,
                args.symbol,
                args.name,
                args.description,
                args.icon_url,
            )
        },
        |builder, executable, multisig, some_coin_metadata: Option<Argument>| {
            assert!(some_coin_metadata.is_some(), "Coin metadata is required");
            aa::currency_intents::execute_update_metadata::<
                am::multisig::Multisig,
                am::multisig::Approvals,
                CoinType,
            >(
                builder,
                executable,
                multisig,
                some_coin_metadata.unwrap().into(),
            )
        },
        |builder, expired| aa::currency::delete_update::<CoinType>(builder, expired),
    );

    define_intent_interface!(
        mint_and_transfer,
        CoinType,
        params::MintAndTransferArgs,
        |builder, auth, multisig_input, params, outcome, args: params::MintAndTransferArgs| {
            aa::currency_intents::request_mint_and_transfer::<
                am::multisig::Multisig,
                am::multisig::Approvals,
                CoinType,
            >(
                builder,
                auth,
                multisig_input,
                params,
                outcome,
                args.amounts,
                args.recipients,
            )
        },
        |builder, executable, multisig, _none_obj| aa::currency_intents::execute_mint_and_transfer::<
            am::multisig::Multisig,
            am::multisig::Approvals,
            CoinType,
        >(builder, executable, multisig),
        |builder: &mut TransactionBuilder, expired: Argument| {
            aa::currency::delete_mint::<CoinType>(builder, expired.into());
            aa::transfer::delete_transfer(builder, expired.into());
        },
    );

    define_intent_interface!(
        mint_and_vest,
        CoinType,
        params::MintAndVestArgs,
        |builder, auth, multisig_input, params, outcome, args: params::MintAndVestArgs| {
            aa::currency_intents::request_mint_and_vest::<
                am::multisig::Multisig,
                am::multisig::Approvals,
                CoinType,
            >(
                builder,
                auth,
                multisig_input,
                params,
                outcome,
                args.total_amount,
                args.start_timestamp,
                args.end_timestamp,
                args.recipient,
            )
        },
        |builder, executable, multisig, _none_obj| aa::currency_intents::execute_mint_and_vest::<
            am::multisig::Multisig,
            am::multisig::Approvals,
            CoinType,
        >(builder, executable, multisig),
        |builder: &mut TransactionBuilder, expired: Argument| {
            aa::currency::delete_mint::<CoinType>(builder, expired.into());
            aa::vesting::delete_vest(builder, expired.into());
        },
    );

    define_intent_interface!(
        withdraw_and_burn,
        CoinType,
        Coin: Key,
        params::WithdrawAndBurnArgs,
        |builder, auth, multisig_input, params, outcome, args: params::WithdrawAndBurnArgs| {
            aa::currency_intents::request_withdraw_and_burn::<
                am::multisig::Multisig,
                am::multisig::Approvals,
                CoinType,
            >(
                builder,
                auth,
                multisig_input,
                params,
                outcome,
                args.coin_id,
                args.amount,
            )
        },
        |builder, executable, multisig, some_receiving_coin: Option<Argument>| {
            aa::currency_intents::execute_withdraw_and_burn::<
                am::multisig::Multisig,
                am::multisig::Approvals,
                CoinType,
            >(builder, executable, multisig, some_receiving_coin.unwrap().into())
        },
        |builder: &mut TransactionBuilder, multisig: Argument, expired: Argument| {
            ap::owned::delete_withdraw::<Coin>(builder, multisig.into(), expired.into());
            aa::currency::delete_burn::<CoinType>(builder, expired.into());
        },
    );

    // === Getters ===

    pub fn sui(&self) -> &Client {
        &self.sui_client
    }

    pub fn multisig(&self) -> Option<&Multisig> {
        self.multisig.as_ref()
    }

    pub fn multisig_mut(&mut self) -> Option<&mut Multisig> {
        self.multisig.as_mut()
    }

    pub fn multisig_id(&self) -> Result<Address> {
        self.multisig
            .as_ref()
            .map(|m| m.id)
            .ok_or(anyhow!("Multisig not loaded"))
    }

    pub fn intents(&self) -> Option<&Intents> {
        self.multisig.as_ref()?.intents.as_ref()
    }

    pub fn intent(&self, key: &str) -> Option<&Intent> {
        self.intents().and_then(|i| i.get_intent(key))
    }

    pub fn owned_objects(&self) -> Option<&OwnedObjects> {
        self.multisig.as_ref()?.owned_objects.as_ref()
    }

    pub fn dynamic_fields(&self) -> Option<&DynamicFields> {
        self.multisig.as_ref()?.dynamic_fields.as_ref()
    }

    // === Helpers ===

    async fn obj(&self, id: Address) -> Result<Input> {
        utils::get_object_as_input_owned(&self.sui_client, id).await
    }
}

#[macro_export]
macro_rules! move_type {
    ($full_path:expr) => {{
        use move_types::{MoveType, StructTag, TypeTag};

        #[derive(serde::Serialize, serde::Deserialize)]
        pub struct Generic {}

        impl MoveType for Generic {
            fn type_() -> TypeTag {
                let parts: Vec<&str> = $full_path.split("::").collect();
                if parts.len() != 3 {
                    panic!(
                        "Invalid coin type path: {}. Expected format: address::module::name",
                        $full_path
                    );
                }

                let address = parts[0];
                let module = parts[1];
                let name = parts[2];

                TypeTag::Struct(Box::new(StructTag {
                    address: address.parse().unwrap(),
                    module: module.parse().unwrap(),
                    name: name.parse().unwrap(),
                    type_params: vec![],
                }))
            }
        }

        Generic {}
    }};
}

#[macro_export]
macro_rules! move_object {
    ($id:expr, $full_path:expr) => {{
        use move_types::{Key, MoveStruct, MoveType, ObjectId, StructTag, TypeTag};
        use std::str::FromStr;

        #[derive(serde::Serialize, serde::Deserialize)]
        pub struct Object {
            pub id: ObjectId,
        }

        impl MoveStruct for Object {
            fn struct_type() -> StructTag {
                StructTag::from_str($full_path).unwrap()
            }
        }

        impl Key for Object {
            fn id(&self) -> &ObjectId {
                &self.id
            }
        }

        Object {
            id: $id.parse().unwrap(),
        }
    }};
}

#[macro_export]
macro_rules! define_intent_interface {
    (
        $intent_name:ident,
        $request_args_type:ty,
        $request_call:expr,
        $execute_call:expr,
        $delete_calls:expr,
    ) => {
        paste::paste! {
            pub async fn [<request_ $intent_name>](
                &self,
                builder: &mut TransactionBuilder,
                params_args: ParamsArgs,
                request_args: $request_args_type,
            ) -> Result<()> {
                let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
                let clock_arg = builder.input(self.obj(CLOCK_OBJECT.parse().unwrap()).await?.by_ref());

                let auth = am::multisig::authenticate(builder, ms_arg.into());
                let params = ap::intents::new_params(
                    builder,
                    params_args.key,
                    params_args.description,
                    params_args.execution_times,
                    params_args.expiration_time,
                    clock_arg.into(),
                );
                let outcome = am::multisig::empty_outcome(builder);

                $request_call(builder, auth, ms_arg.into(), params, outcome, request_args);
                Ok(())
            }

            pub async fn [<execute_ $intent_name>](
                &self,
                builder: &mut TransactionBuilder,
                intent_key: String,
                repeat: u64,
                clear: bool,
            ) -> Result<()> {
                let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
                let clock_arg = builder.input(self.obj(CLOCK_OBJECT.parse().unwrap()).await?.by_ref());
                let key_arg = builder.input(Serialized(&intent_key));

                let mut executable = am::multisig::execute_intent(
                    builder,
                    ms_arg.into(),
                    key_arg.into(),
                    clock_arg.into(),
                );

                for _ in 0..repeat {
                    $execute_call(builder, executable.borrow_mut(), ms_arg.into());
                }

                ap::account::confirm_execution::<am::multisig::Multisig, am::multisig::Approvals>(
                    builder,
                    ms_arg.into(),
                    executable,
                );

                if clear {
                    let mut expired = ap::account::destroy_empty_intent::<
                        am::multisig::Multisig,
                        am::multisig::Approvals,
                    >(builder, ms_arg.into(), key_arg.into());

                    for _ in 0..repeat {
                        $delete_calls(builder, expired.borrow_mut());
                    }
                    ap::intents::destroy_empty_expired(builder, expired);
                }
                Ok(())
            }

            pub async fn [<delete_ $intent_name>](
                &self,
                builder: &mut TransactionBuilder,
                intent_key: String,
                repeat: u64,
            ) -> Result<()> {
                let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
                let clock_arg = builder.input(self.obj(CLOCK_OBJECT.parse().unwrap()).await?.by_ref());
                let key_arg = builder.input(Serialized(&intent_key));

                let mut expired = ap::account::delete_expired_intent::<
                    am::multisig::Multisig,
                    am::multisig::Approvals,
                >(builder, ms_arg.into(), key_arg.into(), clock_arg.into());

                for _ in 0..repeat {
                    $delete_calls(builder, expired.borrow_mut());
                }
                ap::intents::destroy_empty_expired(builder, expired);
                Ok(())
            }
        }
    };

    (
        $intent_name:ident,
        $generic_type:ident $(:$trait_bound:path)?,
        $request_args_type:ty,
        $request_call:expr,
        $execute_call:expr,
        $delete_calls:expr,
    ) => {
        paste::paste! {
            pub async fn [<request_ $intent_name>]<$generic_type: MoveType>(
                &self,
                builder: &mut TransactionBuilder,
                params_args: ParamsArgs,
                request_args: $request_args_type,
                _generic_type: $generic_type,
            ) -> Result<()> {
                let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
                let clock_arg = builder.input(self.obj(CLOCK_OBJECT.parse().unwrap()).await?.by_ref());

                let auth = am::multisig::authenticate(builder, ms_arg.into());
                let params = ap::intents::new_params(
                    builder,
                    params_args.key,
                    params_args.description,
                    params_args.execution_times,
                    params_args.expiration_time,
                    clock_arg.into(),
                );
                let outcome = am::multisig::empty_outcome(builder);

                $request_call(builder, auth, ms_arg.into(), params, outcome, request_args);
                Ok(())
            }

            pub async fn [<execute_ $intent_name>]<$generic_type: MoveType + $($trait_bound)?>(
                &self,
                builder: &mut TransactionBuilder,
                intent_key: String,
                repeat: u64,
                clear: bool,
                opt_obj: Option<Argument>,
            ) -> Result<()> {
                let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
                let clock_arg = builder.input(self.obj(CLOCK_OBJECT.parse().unwrap()).await?.by_ref());
                let key_arg = builder.input(Serialized(&intent_key));

                let mut executable = am::multisig::execute_intent(
                    builder,
                    ms_arg.into(),
                    key_arg.into(),
                    clock_arg.into(),
                );

                for _ in 0..repeat {
                    $execute_call(builder, executable.borrow_mut(), ms_arg.into(), opt_obj);
                }

                ap::account::confirm_execution::<am::multisig::Multisig, am::multisig::Approvals>(
                    builder,
                    ms_arg.into(),
                    executable,
                );

                if clear {
                    let mut expired = ap::account::destroy_empty_intent::<
                        am::multisig::Multisig,
                        am::multisig::Approvals,
                    >(builder, ms_arg.into(), key_arg.into());

                    for _ in 0..repeat {
                        $delete_calls(builder, expired.borrow_mut().into());
                    }
                    ap::intents::destroy_empty_expired(builder, expired);
                }
                Ok(())
            }

            pub async fn [<delete_ $intent_name>]<$generic_type: MoveType>(
                &self,
                builder: &mut TransactionBuilder,
                intent_key: String,
                repeat: u64,
            ) -> Result<()> {
                let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
                let clock_arg = builder.input(self.obj(CLOCK_OBJECT.parse().unwrap()).await?.by_ref());
                let key_arg = builder.input(Serialized(&intent_key));

                let mut expired = ap::account::delete_expired_intent::<
                    am::multisig::Multisig,
                    am::multisig::Approvals,
                >(builder, ms_arg.into(), key_arg.into(), clock_arg.into());

                for _ in 0..repeat {
                    $delete_calls(builder, expired.borrow_mut().into());
                }
                ap::intents::destroy_empty_expired(builder, expired);
                Ok(())
            }
        }
    };

    (
        $intent_name:ident,
        $generic_type_1:ident $(:$trait_bound_1:path)?,
        $generic_type_2:ident $(:$trait_bound_2:path)?,
        $request_args_type:ty,
        $request_call:expr,
        $execute_call:expr,
        $delete_calls:expr,
    ) => {
        paste::paste! {
            pub async fn [<request_ $intent_name>]<$generic_type_1: MoveType>(
                &self,
                builder: &mut TransactionBuilder,
                params_args: ParamsArgs,
                request_args: $request_args_type,
                _generic_type: $generic_type_1,
            ) -> Result<()> {
                let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
                let clock_arg = builder.input(self.obj(CLOCK_OBJECT.parse().unwrap()).await?.by_ref());

                let auth = am::multisig::authenticate(builder, ms_arg.into());
                let params = ap::intents::new_params(
                    builder,
                    params_args.key,
                    params_args.description,
                    params_args.execution_times,
                    params_args.expiration_time,
                    clock_arg.into(),
                );
                let outcome = am::multisig::empty_outcome(builder);

                $request_call(builder, auth, ms_arg.into(), params, outcome, request_args);
                Ok(())
            }

            pub async fn [<execute_ $intent_name>]<$generic_type_1: MoveType + $($trait_bound_1)?, $generic_type_2: MoveType + $($trait_bound_2)?>(
                &self,
                builder: &mut TransactionBuilder,
                intent_key: String,
                repeat: u64,
                clear: bool,
                opt_obj: Option<Argument>,
            ) -> Result<()> {
                let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
                let clock_arg = builder.input(self.obj(CLOCK_OBJECT.parse().unwrap()).await?.by_ref());
                let key_arg = builder.input(Serialized(&intent_key));

                let mut executable = am::multisig::execute_intent(
                    builder,
                    ms_arg.into(),
                    key_arg.into(),
                    clock_arg.into(),
                );

                for _ in 0..repeat {
                    $execute_call(builder, executable.borrow_mut(), ms_arg.into(), opt_obj);
                }

                ap::account::confirm_execution::<am::multisig::Multisig, am::multisig::Approvals>(
                    builder,
                    ms_arg.into(),
                    executable,
                );

                if clear {
                    let mut expired = ap::account::destroy_empty_intent::<
                        am::multisig::Multisig,
                        am::multisig::Approvals,
                    >(builder, ms_arg.into(), key_arg.into());

                    for _ in 0..repeat {
                        $delete_calls(builder, ms_arg, expired.borrow_mut().into());
                    }
                    ap::intents::destroy_empty_expired(builder, expired);
                }
                Ok(())
            }

            pub async fn [<delete_ $intent_name>]<$generic_type_1: MoveType + $($trait_bound_1)?, $generic_type_2: MoveType + $($trait_bound_2)?>(
                &self,
                builder: &mut TransactionBuilder,
                intent_key: String,
                repeat: u64,
            ) -> Result<()> {
                let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
                let clock_arg = builder.input(self.obj(CLOCK_OBJECT.parse().unwrap()).await?.by_ref());
                let key_arg = builder.input(Serialized(&intent_key));

                let mut expired = ap::account::delete_expired_intent::<
                    am::multisig::Multisig,
                    am::multisig::Approvals,
                >(builder, ms_arg.into(), key_arg.into(), clock_arg.into());

                for _ in 0..repeat {
                    $delete_calls(builder, ms_arg, expired.borrow_mut().into());
                }
                ap::intents::destroy_empty_expired(builder, expired);
                Ok(())
            }
        }
    };
}

//**************************************************************************************************//
// Tests                                                                              //
//**************************************************************************************************//

#[cfg(test)]
mod tests {
    use super::*;
    use base64ct::{Base64, Encoding};
    use sui_crypto::ed25519::Ed25519PrivateKey;
    use sui_crypto::SuiSigner;
    use sui_graphql_client::{Client, PaginationFilter};
    use sui_sdk_types::{ExecutionStatus, ObjectIn, ObjectOut, TransactionEffects};

    /// Helper function to setup a transaction builder with a gas object and a sender address.
    async fn init_tx(sui_client: &Client) -> (Ed25519PrivateKey, TransactionBuilder) {
        let pk = Ed25519PrivateKey::new(
            (&Base64::decode_vec("AM06bExREdFceWiExfSacTJ+64AQtFl7SRkSiTmAqh6F").unwrap()[1..])
                .try_into()
                .unwrap(),
        );
        let address = pk.public_key().derive_address();

        let mut builder = TransactionBuilder::new();

        let gas_coin = sui_client
            .coins(
                address,
                Some("0x2::coin::Coin<0x2::sui::SUI>"),
                PaginationFilter::default(),
            )
            .await
            .unwrap()
            .data()
            .first()
            .unwrap()
            .to_owned();
        let gas_input: Input = (&sui_client
            .object(gas_coin.id().to_owned().into(), None)
            .await
            .unwrap()
            .unwrap())
            .into();

        builder.add_gas_objects(vec![gas_input.with_owned_kind()]);
        builder.set_gas_budget(100000000);
        builder.set_gas_price(1000);
        builder.set_sender(address);

        (pk, builder)
    }

    async fn execute_tx(
        sui_client: &Client,
        pk: Ed25519PrivateKey,
        builder: TransactionBuilder,
    ) -> TransactionEffects {
        // execute the transaction
        let tx = builder.finish().unwrap();
        let sig = pk.sign_transaction(&tx).unwrap();
        let effects = sui_client.execute_tx(vec![sig], &tx).await;
        assert!(effects.is_ok(), "Execution failed. Effects: {:?}", effects);
        // wait for the transaction to be finalized
        while sui_client.transaction(tx.digest()).await.unwrap().is_none() {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        assert_eq!(
            ExecutionStatus::Success,
            effects.as_ref().unwrap().as_ref().unwrap().status().clone()
        );
        effects.unwrap().unwrap()
    }

    async fn get_created_multisig(effects: &TransactionEffects) -> Address {
        match effects {
            TransactionEffects::V1(_) => panic!("V1 not supported"),
            TransactionEffects::V2(effects) => effects
                .changed_objects
                .iter()
                .filter(|obj| {
                    obj.input_state == ObjectIn::NotExist && obj.output_state != ObjectOut::NotExist
                })
                .collect::<Vec<_>>()
                .first()
                .unwrap()
                .object_id
                .into(),
        }
    }

    #[tokio::test]
    async fn test_create_and_get_multisig() {
        let mut client = MultisigClient::new_testnet();
        let (pk, mut builder) = init_tx(client.sui()).await;

        client.create_multisig(&mut builder).await.unwrap();
        let effects = execute_tx(client.sui(), pk, builder).await;

        let multisig_id = get_created_multisig(&effects).await;
        client.load_multisig(multisig_id).await.unwrap();

        assert!(client.multisig().is_some());
        assert!(client.intents().is_some());
        assert!(client.owned_objects().is_some());
    }
}
