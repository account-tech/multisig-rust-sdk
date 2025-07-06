pub mod actions;
pub mod dynamic_fields;
pub mod intents;
pub mod move_binding;
pub mod multisig;
pub mod owned_objects;
pub mod params;
pub mod utils;

use anyhow::{anyhow, Ok, Result};
use move_types::functions::{Arg, MutRef, Ref};
use move_types::{Identifier, Key, MoveStruct, MoveType, StructTag, TypeTag};
use std::sync::Arc;
use sui_graphql_client::Client;
use sui_sdk_types::{Address, Argument, ObjectData};
use sui_transaction_builder::{unresolved::Input, TransactionBuilder};
use sui_transaction_builder::{Function, Serialized};

use crate::dynamic_fields::DynamicFields;
use crate::intents::{Intent, Intents};
use crate::move_binding::account_actions as aa;
use crate::move_binding::account_extensions as ae;
use crate::move_binding::account_multisig as am;
use crate::move_binding::account_protocol as ap;
use crate::move_binding::sui;
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
        intent_key: &str,
    ) -> Result<()> {
        let mut ms_arg = self.multisig_arg(builder).await?;
        let key_arg = self.key_arg(builder, &intent_key).await?;

        am::multisig::approve_intent(builder, ms_arg.borrow_mut(), key_arg);

        Ok(())
    }

    pub async fn disapprove_intent(
        &self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
    ) -> Result<()> {
        let mut ms_arg = self.multisig_arg(builder).await?;
        let key_arg = self.key_arg(builder, intent_key).await?;

        am::multisig::disapprove_intent(builder, ms_arg.borrow_mut(), key_arg);

        Ok(())
    }

    // === Commands ===

    pub async fn deposit_cap<CapType: Key>(
        &self,
        builder: &mut TransactionBuilder,
        cap_id: Address,
    ) -> Result<()> {
        let mut ms_arg = self.multisig_arg(builder).await?;
        let cap_arg = self.owned_arg::<CapType>(builder, cap_id).await?;

        let auth = am::multisig::authenticate(builder, ms_arg.borrow());
        aa::access_control::lock_cap(builder, auth, ms_arg.borrow_mut(), cap_arg);

        Ok(())
    }

    pub async fn replace_metadata(
        &self,
        builder: &mut TransactionBuilder,
        keys: Vec<String>,
        values: Vec<String>,
    ) -> Result<()> {
        let mut ms_arg = self.multisig_arg(builder).await?;
        let keys_arg = self.pure_arg(builder, keys).await?;
        let values_arg = self.pure_arg(builder, values).await?;

        let auth = am::multisig::authenticate(builder, ms_arg.borrow());
        ap::config::edit_metadata(builder, auth, ms_arg.borrow_mut(), keys_arg, values_arg);

        Ok(())
    }

    pub async fn update_verified_deps_to_latest(
        &self,
        builder: &mut TransactionBuilder,
    ) -> Result<()> {
        let mut ms_arg = self.multisig_arg(builder).await?;
        let extensions_arg = self.extensions_arg(builder).await?;

        let auth = am::multisig::authenticate(builder, ms_arg.borrow());
        ap::config::update_extensions_to_latest(
            builder,
            auth,
            ms_arg.borrow_mut(),
            extensions_arg.borrow(),
        );

        Ok(())
    }

    pub async fn deposit_treasury_cap<CoinType: MoveType>(
        &self,
        builder: &mut TransactionBuilder,
        max_supply: Option<u64>,
        cap_id: Address,
    ) -> Result<()> {
        let mut ms_arg = self.multisig_arg(builder).await?;
        let max_supply_arg = self.pure_arg(builder, max_supply).await?;
        let cap_arg = self
            .owned_arg::<sui::coin::TreasuryCap<CoinType>>(builder, cap_id)
            .await?;

        let auth = am::multisig::authenticate(builder, ms_arg.borrow());
        aa::currency::lock_cap(builder, auth, ms_arg.borrow_mut(), cap_arg, max_supply_arg);

        Ok(())
    }

    pub async fn merge_and_split<CoinType: MoveType>(
        &self,
        builder: &mut TransactionBuilder,
        coins_to_merge: Vec<Address>,
        amounts_to_split: Vec<u64>,
    ) -> Result<()> {
        let mut ms_arg = self.multisig_arg(builder).await?;
        let mut coin_inputs = Vec::new();
        for coin in coins_to_merge {
            coin_inputs.push(builder.input(self.obj(coin).await?.with_receiving_kind()));
        }

        let to_merge_arg = builder.make_move_vec(None, coin_inputs).into();
        let to_split_arg = self.pure_arg(builder, amounts_to_split).await?;

        let auth = am::multisig::authenticate(builder, ms_arg.borrow());
        ap::owned::merge_and_split::<_, CoinType>(
            builder,
            auth,
            ms_arg.borrow_mut(),
            to_merge_arg,
            to_split_arg,
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
        let mut ms_arg = self.multisig_arg(builder).await?;
        let package_name_arg = self.pure_arg(builder, package_name.to_string()).await?;
        let timelock_duration_arg = self.pure_arg(builder, timelock_duration).await?;
        let upgrade_cap_arg = self
            .owned_arg::<sui::package::UpgradeCap>(builder, cap_id)
            .await?;

        let auth = am::multisig::authenticate(builder, ms_arg.borrow());
        aa::package_upgrade::lock_cap(
            builder,
            auth,
            ms_arg.borrow_mut(),
            upgrade_cap_arg,
            package_name_arg,
            timelock_duration_arg,
        );

        Ok(())
    }

    pub async fn open_vault(
        &self,
        builder: &mut TransactionBuilder,
        vault_name: &str,
    ) -> Result<()> {
        let mut ms_arg = self.multisig_arg(builder).await?;
        let vault_name_arg = self.pure_arg(builder, vault_name.to_string()).await?;

        let auth = am::multisig::authenticate(builder, ms_arg.borrow());
        aa::vault::open(builder, auth, ms_arg.borrow_mut(), vault_name_arg);

        Ok(())
    }

    pub async fn deposit_from_wallet<CoinType: MoveType>(
        &self,
        builder: &mut TransactionBuilder,
        vault_name: &str,
        coin_arg: Arg<sui::coin::Coin<CoinType>>, // splitted in previous command
    ) -> Result<()> {
        let mut ms_arg = self.multisig_arg(builder).await?;
        let vault_name_arg = self.pure_arg(builder, vault_name.to_string()).await?;

        let auth = am::multisig::authenticate(builder, ms_arg.borrow());
        aa::vault::deposit(builder, auth, ms_arg.borrow_mut(), vault_name_arg, coin_arg);

        Ok(())
    }

    pub async fn close_vault(
        &self,
        builder: &mut TransactionBuilder,
        vault_name: &str,
    ) -> Result<()> {
        let mut ms_arg = self.multisig_arg(builder).await?;
        let vault_name_arg = self.pure_arg(builder, vault_name.to_string()).await?;

        let auth = am::multisig::authenticate(builder, ms_arg.borrow());
        aa::vault::close(builder, auth, ms_arg.borrow_mut(), vault_name_arg);

        Ok(())
    }

    pub async fn claim_vested<CoinType: MoveType>(
        &self,
        builder: &mut TransactionBuilder,
        vesting_id: Address,
        cap_id: Address,
    ) -> Result<()> {
        let mut vesting_arg = self
            .shared_mut_arg::<aa::vesting::Vesting<CoinType>>(builder, vesting_id)
            .await?;
        let cap_arg = self
            .owned_arg::<aa::vesting::ClaimCap>(builder, cap_id)
            .await?;
        let clock_arg = self.clock_arg(builder).await?;

        aa::vesting::claim(
            builder,
            vesting_arg.borrow_mut(),
            cap_arg.borrow(),
            clock_arg.borrow(),
        );

        Ok(())
    }

    pub async fn cancel_vesting<CoinType: MoveType>(
        &self,
        builder: &mut TransactionBuilder,
        vesting_id: Address,
        _coin_type: CoinType,
    ) -> Result<()> {
        let ms_arg = self.multisig_arg(builder).await?;
        let vesting_arg = self
            .shared_val_arg::<aa::vesting::Vesting<CoinType>>(builder, vesting_id)
            .await?;

        let auth = am::multisig::authenticate(builder, ms_arg.borrow());
        aa::vesting::cancel_payment(builder, auth, vesting_arg, ms_arg.borrow());

        Ok(())
    }

    pub async fn destroy_empty_vesting<CoinType: MoveType>(
        &self,
        builder: &mut TransactionBuilder,
        vesting_id: Address,
    ) -> Result<()> {
        let vesting_arg = self
            .shared_val_arg::<aa::vesting::Vesting<CoinType>>(builder, vesting_id)
            .await?;

        aa::vesting::destroy_empty(builder, vesting_arg);

        Ok(())
    }

    pub async fn destroy_claim_cap(
        &self,
        builder: &mut TransactionBuilder,
        cap_id: Address,
    ) -> Result<()> {
        let cap_arg = self
            .owned_arg::<aa::vesting::ClaimCap>(builder, cap_id)
            .await?;

        aa::vesting::destroy_cap(builder, cap_arg);

        Ok(())
    }

    // === Intents ===

    // define_intent_interface!(
    //     config_multisig,
    //     params::ConfigMultisigArgs,
    //     |builder, auth, multisig, params, outcome, args: params::ConfigMultisigArgs| {
    //         am::config::request_config_multisig(
    //             builder,
    //             auth,
    //             multisig,
    //             params,
    //             outcome,
    //             args.addresses,
    //             args.weights,
    //             args.roles,
    //             args.global,
    //             args.role_names,
    //             args.role_thresholds,
    //         )
    //     },
    //     |builder, executable, multisig| am::config::execute_config_multisig(
    //         builder, executable, multisig
    //     ),
    //     |builder, expired| am::config::delete_config_multisig(builder, expired),
    // );

    // define_intent_interface!(
    //     config_deps,
    //     params::ConfigDepsArgs,
    //     |builder, auth, multisig, params, outcome, args: params::ConfigDepsArgs| {
    //         ap::config::request_config_deps::<am::multisig::Multisig, am::multisig::Approvals>(
    //             builder,
    //             auth,
    //             multisig,
    //             params,
    //             outcome,
    //             args.extensions.into(),
    //             args.names,
    //             args.addresses,
    //             args.versions,
    //         )
    //     },
    //     |builder, executable, multisig| ap::config::execute_config_deps::<
    //         am::multisig::Multisig,
    //         am::multisig::Approvals,
    //     >(builder, executable, multisig),
    //     |builder, expired| ap::config::delete_config_deps(builder, expired),
    // );

    // define_intent_interface!(
    //     toggle_unverified_allowed,
    //     (),
    //     |builder, auth, multisig, params, outcome, _| {
    //         ap::config::request_toggle_unverified_allowed::<
    //             am::multisig::Multisig,
    //             am::multisig::Approvals,
    //         >(builder, auth, multisig, params, outcome)
    //     },
    //     |builder, executable, multisig| ap::config::execute_toggle_unverified_allowed::<
    //         am::multisig::Multisig,
    //         am::multisig::Approvals,
    //     >(builder, executable, multisig),
    //     |builder, expired| ap::config::delete_toggle_unverified_allowed(builder, expired),
    // );

    define_request_intent!(
        request_borrow_cap,
        (),
        |builder, auth, multisig, params, outcome, _| {
            aa::access_control_intents::request_borrow_cap::<
                am::multisig::Multisig,
                am::multisig::Approvals,
                CapType,
            >(builder, auth, multisig, params, outcome)
        },
        CapType,
    );

    // define_execute_intent!(
    //     execute_borrow_cap,
    //     |builder, executable, ms_arg, _| {
    //         aa::access_control_intents::execute_borrow_cap::<
    //             am::multisig::Multisig,
    //             am::multisig::Approvals,
    //             CapObject,
    //         >(builder, executable, ms_arg)
    //     },
    //     CapObject:Key,
    // );

    pub async fn execute_borrow_cap<Cap: Key>(
        &self,
        builder: &mut TransactionBuilder,
        key: &str,
    ) -> Result<(
        Arg<ap::executable::Executable<am::multisig::Approvals>>,
        Arg<Cap>,
    )> {
        let key_arg = self.key_arg(builder, key).await?;
        let clock_arg = self.clock_arg(builder).await?;
        let mut ms_arg = self.multisig_arg(builder).await?;

        let mut executable =
            am::multisig::execute_intent(builder, ms_arg.borrow_mut(), key_arg, clock_arg.borrow());

        let cap = aa::access_control_intents::execute_borrow_cap(
            builder,
            executable.borrow_mut(),
            ms_arg.borrow_mut(),
        );

        Ok((executable, cap))
    }

    // Use the Cap between borrow and return

    pub async fn execute_return_cap<Cap: Key>(
        &self,
        builder: &mut TransactionBuilder,
        mut executable: Arg<ap::executable::Executable<am::multisig::Approvals>>,
        cap: Arg<Cap>,
    ) -> Result<()> {
        let mut ms_arg = self.multisig_arg(builder).await?;

        aa::access_control_intents::execute_return_cap::<
            am::multisig::Multisig,
            am::multisig::Approvals,
            Cap,
        >(builder, executable.borrow_mut(), ms_arg.borrow_mut(), cap);

        ap::account::confirm_execution::<am::multisig::Multisig, am::multisig::Approvals>(
            builder,
            ms_arg.borrow_mut(),
            executable,
        );

        Ok(())
    }

    // pub async fn delete_borrow_cap<CapType: MoveType>(
    //     &self,
    //     builder: &mut TransactionBuilder,
    //     intent_key: &str,
    // ) -> Result<()> {
    //     let mut ms_arg = self.multisig_arg(builder).await?;
    //     let clock_arg = self.clock_arg(builder).await?;
    //     let key_arg = self.key_arg(builder, intent_key).await?;
    //     let mut expired = ap::account::delete_expired_intent::<
    //         am::multisig::Multisig,
    //         am::multisig::Approvals,
    //     >(builder, ms_arg.borrow_mut(), key_arg, clock_arg.borrow());
    //     aa::access_control::delete_borrow::<CapType>(builder, expired.borrow_mut());
    //     aa::access_control::delete_return::<CapType>(builder, expired.borrow_mut());
    //     Ok(())
    // }

    define_delete_intent!(
        delete_borrow_cap,
        |builder: &mut TransactionBuilder, expired: Argument| {
            aa::access_control::delete_borrow::<CapType>(builder, expired.into());
            aa::access_control::delete_return::<CapType>(builder, expired.into());
        },
        CapType,
    );

    // define_intent_interface!(
    //     disable_rules,
    //     params::DisableRulesArgs,
    //     |builder: &mut TransactionBuilder,
    //      auth,
    //      multisig,
    //      params,
    //      outcome,
    //      args: params::DisableRulesArgs,
    //      type_args| {
    //         builder.move_call(
    //             Function::new(
    //                 ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
    //                 "currency_intents".parse().unwrap(),
    //                 "request_disable_rules".parse().unwrap(),
    //                 type_args,
    //             ),
    //             vec![
    //                 auth,
    //                 multisig,
    //                 params,
    //                 outcome,
    //                 args.mint.into(),
    //                 args.burn.into(),
    //                 args.update_symbol.into(),
    //                 args.update_name.into(),
    //                 args.update_description.into(),
    //                 args.update_icon.into(),
    //             ],
    //         );
    //     },
    //     |builder: &mut TransactionBuilder, executable, multisig, _, type_args| {
    //         builder.move_call(
    //             Function::new(
    //                 ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
    //                 "currency_intents".parse().unwrap(),
    //                 "execute_disable_rules".parse().unwrap(),
    //                 type_args,
    //             ),
    //             vec![executable, multisig],
    //         );
    //     },
    //     |builder: &mut TransactionBuilder, expired, type_args| {
    //         builder.move_call(
    //             Function::new(
    //                 ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
    //                 "currency".parse().unwrap(),
    //                 "delete_disable".parse().unwrap(),
    //                 type_args,
    //             ),
    //             vec![expired],
    //         );
    //     },
    // );

    // define_intent_interface!(
    //     update_metadata,
    //     params::UpdateMetadataArgs,
    //     |builder: &mut TransactionBuilder,
    //      auth,
    //      multisig,
    //      params,
    //      outcome,
    //      args: params::UpdateMetadataArgs,
    //      type_args| {
    //         builder.move_call(
    //             Function::new(
    //                 ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
    //                 "currency_intents".parse().unwrap(),
    //                 "request_update_metadata".parse().unwrap(),
    //                 type_args,
    //             ),
    //             vec![
    //                 auth,
    //                 multisig,
    //                 params,
    //                 outcome,
    //                 args.symbol.into(),
    //                 args.name.into(),
    //                 args.description.into(),
    //                 args.icon_url.into(),
    //             ],
    //         );
    //     },
    //     |builder: &mut TransactionBuilder, executable, multisig, coin_metadata_arg, type_args| {
    //         builder.move_call(
    //             Function::new(
    //                 ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
    //                 "currency_intents".parse().unwrap(),
    //                 "execute_update_metadata".parse().unwrap(),
    //                 type_args,
    //             ),
    //             vec![executable, multisig, coin_metadata_arg],
    //         );
    //     },
    //     |builder: &mut TransactionBuilder, expired, type_args| {
    //         builder.move_call(
    //             Function::new(
    //                 ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
    //                 "currency".parse().unwrap(),
    //                 "delete_update".parse().unwrap(),
    //                 type_args,
    //             ),
    //             vec![expired],
    //         );
    //     },
    // );

    // define_intent_interface!(
    //     mint_and_transfer,
    //     params::MintAndTransferArgs,
    //     |builder: &mut TransactionBuilder,
    //      auth,
    //      multisig,
    //      params,
    //      outcome,
    //      args: params::MintAndTransferArgs,
    //      type_args| {
    //         builder.move_call(
    //             Function::new(
    //                 ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
    //                 "currency_intents".parse().unwrap(),
    //                 "request_mint_and_transfer".parse().unwrap(),
    //                 type_args,
    //             ),
    //             vec![
    //                 auth,
    //                 multisig,
    //                 params,
    //                 outcome,
    //                 args.amounts.into(),
    //                 args.recipients.into(),
    //             ],
    //         );
    //     },
    //     |builder: &mut TransactionBuilder, executable, multisig, _, type_args| {
    //         builder.move_call(
    //             Function::new(
    //                 ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
    //                 "currency_intents".parse().unwrap(),
    //                 "execute_mint_and_transfer".parse().unwrap(),
    //                 type_args,
    //             ),
    //             vec![executable, multisig],
    //         );
    //     },
    //     |builder: &mut TransactionBuilder, expired, type_args| {
    //         builder.move_call(
    //             Function::new(
    //                 ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
    //                 "currency".parse().unwrap(),
    //                 "delete_mint".parse().unwrap(),
    //                 type_args,
    //             ),
    //             vec![expired],
    //         );
    //         builder.move_call(
    //             Function::new(
    //                 ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
    //                 "transfer".parse().unwrap(),
    //                 "delete_transfer".parse().unwrap(),
    //                 vec![],
    //             ),
    //             vec![expired],
    //         );
    //     },
    // );

    // define_intent_interface!(
    //     mint_and_vest,
    //     params::MintAndVestArgs,
    //     |builder: &mut TransactionBuilder,
    //      auth,
    //      multisig,
    //      params,
    //      outcome,
    //      args: params::MintAndVestArgs,
    //      type_args| {
    //         builder.move_call(
    //             Function::new(
    //                 ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
    //                 "currency_intents".parse().unwrap(),
    //                 "request_mint_and_vest".parse().unwrap(),
    //                 type_args,
    //             ),
    //             vec![
    //                 auth,
    //                 multisig,
    //                 params,
    //                 outcome,
    //                 args.total_amount.into(),
    //                 args.start_timestamp.into(),
    //                 args.end_timestamp.into(),
    //                 args.recipient.into(),
    //             ],
    //         );
    //     },
    //     |builder: &mut TransactionBuilder, executable, multisig, _, type_args| {
    //         builder.move_call(
    //             Function::new(
    //                 ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
    //                 "currency_intents".parse().unwrap(),
    //                 "execute_mint_and_vest".parse().unwrap(),
    //                 type_args,
    //             ),
    //             vec![executable, multisig],
    //         );
    //     },
    //     |builder: &mut TransactionBuilder, expired, type_args| {
    //         builder.move_call(
    //             Function::new(
    //                 ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
    //                 "currency".parse().unwrap(),
    //                 "delete_mint".parse().unwrap(),
    //                 type_args,
    //             ),
    //             vec![expired],
    //         );
    //         builder.move_call(
    //             Function::new(
    //                 ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
    //                 "vesting".parse().unwrap(),
    //                 "delete_vest".parse().unwrap(),
    //                 vec![],
    //             ),
    //             vec![expired],
    //         );
    //     },
    // );

    // define_intent_interface!(
    //     withdraw_and_burn,
    //     params::WithdrawAndBurnArgs,
    //     |builder: &mut TransactionBuilder,
    //      auth,
    //      multisig,
    //      params,
    //      outcome,
    //      args: params::WithdrawAndBurnArgs,
    //      type_args| {
    //         builder.move_call(
    //             Function::new(
    //                 ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
    //                 "currency_intents".parse().unwrap(),
    //                 "request_withdraw_and_burn".parse().unwrap(),
    //                 type_args,
    //             ),
    //             vec![
    //                 auth,
    //                 multisig,
    //                 params,
    //                 outcome,
    //                 args.coin_id.into(),
    //                 args.amount.into(),
    //             ],
    //         );
    //     },
    //     |builder: &mut TransactionBuilder, executable, multisig, coin_object_arg, type_args| {
    //         builder.move_call(
    //             Function::new(
    //                 ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
    //                 "currency_intents".parse().unwrap(),
    //                 "execute_withdraw_and_burn".parse().unwrap(),
    //                 type_args,
    //             ),
    //             vec![executable, multisig, coin_object_arg],
    //         );
    //     },
    //     |builder: &mut TransactionBuilder, expired, type_args: Vec<TypeTag>| {
    //         builder.move_call(
    //             Function::new(
    //                 ACCOUNT_PROTOCOL_PACKAGE.parse().unwrap(),
    //                 "owned".parse().unwrap(),
    //                 "delete_withdraw".parse().unwrap(),
    //                 vec![type_args[0].clone()],
    //             ),
    //             vec![expired],
    //         );
    //         builder.move_call(
    //             Function::new(
    //                 ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
    //                 "currency".parse().unwrap(),
    //                 "delete_burn".parse().unwrap(),
    //                 vec![type_args[1].clone()],
    //             ),
    //             vec![expired],
    //         );
    //     },
    // );

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
        utils::get_object_as_input(&self.sui_client, id).await
    }

    async fn clock_timestamp(&self) -> Result<u64> {
        let clock_object = utils::get_object(&self.sui_client, CLOCK_OBJECT.parse().unwrap()).await?;
        if let ObjectData::Struct(obj) = clock_object.data() {
            let clock: sui::clock::Clock = bcs::from_bytes(obj.contents())
                .map_err(|e| anyhow!("Failed to parse clock object: {}", e))?;   
            Ok(clock.timestamp_ms)
        } else {
            Err(anyhow!("Clock object data is missing"))
        }
    }

    async fn pure_arg<Pure: serde::Serialize + MoveType>(
        &self,
        builder: &mut TransactionBuilder,
        value: Pure,
    ) -> Result<Arg<Pure>> {
        let value_arg = builder.input(Serialized(&value)).into();
        Ok(value_arg)
    }

    async fn owned_arg<Obj: MoveType + Key>(
        &self,
        builder: &mut TransactionBuilder,
        id: Address,
    ) -> Result<Arg<Obj>> {
        let object_input = self.obj(id).await?;
        let object_arg = builder.input(object_input).into();
        Ok(object_arg)
    }

    async fn shared_mut_arg<Obj: MoveType + Key>(
        &self,
        builder: &mut TransactionBuilder,
        id: Address,
    ) -> Result<Arg<Obj>> {
        let object_input = self.obj(id).await?;
        let object_arg = builder.input(object_input.by_mut()).into();
        Ok(object_arg)
    }

    async fn shared_val_arg<Obj: MoveType + Key>(
        &self,
        builder: &mut TransactionBuilder,
        id: Address,
    ) -> Result<Arg<Obj>> {
        let object_input = self.obj(id).await?;
        let object_arg = builder.input(object_input.by_val()).into();
        Ok(object_arg)
    }

    async fn key_arg(
        &self,
        builder: &mut TransactionBuilder,
        key: &str,
    ) -> Result<Arg<std::string::String>> {
        let as_owned = key.to_owned();
        let key_arg = builder.input(Serialized(&as_owned)).into();
        Ok(key_arg)
    }

    async fn clock_arg(&self, builder: &mut TransactionBuilder) -> Result<Arg<sui::clock::Clock>> {
        let clock_input = self.obj(CLOCK_OBJECT.parse().unwrap()).await?;
        let clock_arg = builder.input(clock_input.by_ref()).into();
        Ok(clock_arg)
    }

    async fn extensions_arg(
        &self,
        builder: &mut TransactionBuilder,
    ) -> Result<Arg<ae::extensions::Extensions>> {
        let extensions_input = self.obj(EXTENSIONS_OBJECT.parse().unwrap()).await?;
        let extensions_arg = builder.input(extensions_input.by_ref()).into();
        Ok(extensions_arg)
    }

    async fn multisig_arg(
        &self,
        builder: &mut TransactionBuilder,
    ) -> Result<Arg<ap::account::Account<am::multisig::Multisig>>> {
        let ms_input = self.obj(self.multisig_id()?).await?;
        let ms_arg = builder.input(ms_input.by_mut()).into();
        Ok(ms_arg)
    }
}

#[macro_export]
macro_rules! define_move_type {
    (
        $move_type:ident,
        $full_path:expr $(,)?
    ) => {
        use move_types::{MoveType, StructTag, TypeTag};

        #[derive(serde::Serialize, serde::Deserialize)]
        pub struct $move_type {}

        impl MoveType for $move_type {
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
    };
}

#[macro_export]
macro_rules! define_move_object {
    (
        $move_object_name:ident, 
        $id:expr, 
        $full_path:expr $(,)?
    ) => {
        use move_types::{Key, MoveStruct, MoveType, ObjectId, StructTag, TypeTag};

        #[derive(serde::Serialize, serde::Deserialize)]
        pub struct $move_object_name {
            pub id: ObjectId,
        }

        impl MoveStruct for $move_object_name {
            fn struct_type() -> StructTag {
                $full_path.parse().unwrap()
            }
        }

        impl Key for $move_object_name {
            fn id(&self) -> &ObjectId {
                &self.id
            }
        }
    };
}

#[macro_export]
macro_rules! define_request_intent {
    (
        $request_intent_name:ident,
        $request_args_type:ty,
        $request_call:expr,
        $($generic_type:ident,)?
    ) => {
        pub async fn $request_intent_name$(<$generic_type: MoveType>)?(
            &self,
            builder: &mut TransactionBuilder,
            params_args: ParamsArgs,
            request_args: $request_args_type,
        ) -> Result<()> {
            let mut ms_arg = self.multisig_arg(builder).await?;
            let clock_arg = self.clock_arg(builder).await?;

            let auth = am::multisig::authenticate(builder, ms_arg.borrow());
            let params = ap::intents::new_params(
                builder,
                params_args.key,
                params_args.description,
                params_args.execution_times,
                params_args.expiration_time,
                clock_arg.borrow(),
            );
            let outcome = am::multisig::empty_outcome(builder);

            $request_call(
                builder,
                auth,
                ms_arg.borrow_mut(),
                params,
                outcome,
                request_args,
            );
            Ok(())
        }
    };
}

#[macro_export]
macro_rules! define_execute_intent {
    (
        $execute_intent_name:ident,
        $execute_call:expr,
        $($generic_type:ident $(:$trait_bound:path)?,)?
    ) => {
        pub async fn $execute_intent_name$(<$generic_type: MoveType + $($trait_bound)?>)?(
            &self,
            builder: &mut TransactionBuilder,
            intent_key: &str,
            repeat: u64,
        ) -> Result<()> {
            let mut ms_arg = self.multisig_arg(builder).await?;
            let clock_arg = self.clock_arg(builder).await?;
            let key_arg = self.key_arg(builder, intent_key).await?;

            let mut executable = am::multisig::execute_intent(
                builder,
                ms_arg.borrow_mut(),
                key_arg,
                clock_arg.borrow(),
            );

            for _ in 0..repeat {
                $execute_call(builder, executable.borrow_mut(), ms_arg.borrow_mut());
            }

            ap::account::confirm_execution::<am::multisig::Multisig, am::multisig::Approvals>(
                builder,
                ms_arg.borrow_mut(),
                executable,
            );
            Ok(())
        }
    };
}

#[macro_export]
macro_rules! define_delete_intent {
    (
        $delete_intent_name:ident,
        $delete_calls:expr,
        $($generic_type:ident,)?
    ) => {
        pub async fn $delete_intent_name$(<$generic_type: MoveType>)?(
            &self,
            builder: &mut TransactionBuilder,
            intent_key: &str,
            repeat: u64,
        ) -> Result<()> {
            let mut ms_arg = self.multisig_arg(builder).await?;
            let clock_arg = self.clock_arg(builder).await?;
            let key_arg = self.key_arg(builder, intent_key).await?;

            if let Some(intent) = self.intents().and_then(|i| i.get_intent(intent_key)) {
                let current_timestamp = self.clock_timestamp().await?;
                if (current_timestamp < intent.expiration_time && intent.execution_times.len() > 0) {
                    return Err(anyhow!("Intent cannot be deleted"));
                }
            } else {
                return Err(anyhow!("Intent not found"));
            }

            let mut expired = ap::account::delete_expired_intent::<
                am::multisig::Multisig,
                am::multisig::Approvals,
            >(builder, ms_arg.borrow_mut(), key_arg, clock_arg.borrow());

            for _ in 0..repeat {
                $delete_calls(builder, expired.borrow_mut().into());
            }
            ap::intents::destroy_empty_expired(builder, expired);
            Ok(())
        }
    };
}

#[macro_export]
macro_rules! define_intent_interface {
    (
        $intent_name:ident,
        $request_args_type:ty,
        $request_call:expr,
        $execute_call:expr,
        $delete_calls:expr,
        $($generic_type:ident $(:$trait_bound:path)?,)?
    ) => {
        paste::paste! {
            define_request_intent!(
                [<request_ $intent_name>],
                $request_args_type,
                $request_call,
                $($generic_type,)?
            );

            define_execute_intent!(
                [<execute_ $intent_name>],
                $execute_call,
                $($generic_type:$trait_bound,)?
            );

            define_delete_intent!(
                [<delete_ $intent_name>],
                $delete_calls,
                $($generic_type,)?
            );
        }
    };
}

// #[macro_export]
// macro_rules! define_intent_interface {
//     (
//         $intent_name:ident,
//         $request_args_type:ty,
//         $request_call:expr,
//         $execute_call:expr,
//         $delete_calls:expr,
//     ) => {
//         paste::paste! {
//             pub async fn [<request_ $intent_name>](
//                 &self,
//                 builder: &mut TransactionBuilder,
//                 params_args: ParamsArgs,
//                 request_args: $request_args_type,
//                 type_args: Vec<&str>,
//             ) -> Result<()> {
//                 let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
//                 let clock_arg = builder.input(self.obj(CLOCK_OBJECT.parse().unwrap()).await?.by_ref());
//                 let mut all_type_args = vec![
//                     format!("{}::{}::{}", ACCOUNT_MULTISIG_PACKAGE, "multisig", "Multisig").parse::<TypeTag>().unwrap(),
//                     format!("{}::{}::{}", ACCOUNT_MULTISIG_PACKAGE, "multisig", "Approvals").parse::<TypeTag>().unwrap(),
//                 ];
//                 all_type_args.extend(type_args.iter().map(|s| s.parse::<TypeTag>().unwrap()).collect::<Vec<TypeTag>>());

//                 let auth = am::multisig::authenticate(builder, ms_arg.into());
//                 let params = ap::intents::new_params(
//                     builder,
//                     params_args.key,
//                     params_args.description,
//                     params_args.execution_times,
//                     params_args.expiration_time,
//                     clock_arg.into(),
//                 );
//                 let outcome = am::multisig::empty_outcome(builder);

//                 $request_call(builder, auth.into(), ms_arg.into(), params.into(), outcome.into(), request_args, all_type_args);
//                 Ok(())
//             }

//             pub async fn [<execute_ $intent_name>](
//                 &self,
//                 builder: &mut TransactionBuilder,
//                 mut args: Vec<Argument>,
//                 type_args: Vec<&str>,
//                 intent_key: String,
//                 repeat: u64,
//                 clear: bool,
//             ) -> Result<()> {
//                 let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
//                 let clock_arg = builder.input(self.obj(CLOCK_OBJECT.parse().unwrap()).await?.by_ref());
//                 let key_arg = builder.input(Serialized(&intent_key));
//                 let mut all_type_args = vec![
//                     format!("{}::{}::{}", ACCOUNT_MULTISIG_PACKAGE, "multisig", "Multisig").parse::<TypeTag>().unwrap(),
//                     format!("{}::{}::{}", ACCOUNT_MULTISIG_PACKAGE, "multisig", "Approvals").parse::<TypeTag>().unwrap(),
//                 ];
//                 all_type_args.extend(type_args.iter().map(|s| s.parse::<TypeTag>().unwrap()).collect::<Vec<TypeTag>>());

//                 let mut executable = am::multisig::execute_intent(
//                     builder,
//                     ms_arg.into(),
//                     key_arg.into(),
//                     clock_arg.into(),
//                 );

//                 for _ in 0..repeat {
//                     $execute_call(builder, executable.borrow_mut().into(), ms_arg.into(), args.remove(0), all_type_args.clone());
//                 }

//                 ap::account::confirm_execution::<am::multisig::Multisig, am::multisig::Approvals>(
//                     builder,
//                     ms_arg.into(),
//                     executable,
//                 );

//                 if clear {
//                     let mut expired = ap::account::destroy_empty_intent::<
//                         am::multisig::Multisig,
//                         am::multisig::Approvals,
//                     >(builder, ms_arg.into(), key_arg.into());

//                     for _ in 0..repeat {
//                         $delete_calls(builder, expired.borrow_mut().into(), all_type_args.clone());
//                     }
//                     ap::intents::destroy_empty_expired(builder, expired);
//                 }
//                 Ok(())
//             }

//             pub async fn [<delete_ $intent_name>](
//                 &self,
//                 builder: &mut TransactionBuilder,
//                 type_args: Vec<&str>,
//                 intent_key: String,
//                 repeat: u64,
//             ) -> Result<()> {
//                 let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
//                 let clock_arg = builder.input(self.obj(CLOCK_OBJECT.parse().unwrap()).await?.by_ref());
//                 let key_arg = builder.input(Serialized(&intent_key));
//                 let type_args = type_args.iter().map(|s| s.parse::<TypeTag>().unwrap()).collect::<Vec<TypeTag>>();

//                 let mut expired = ap::account::delete_expired_intent::<
//                     am::multisig::Multisig,
//                     am::multisig::Approvals,
//                 >(builder, ms_arg.into(), key_arg.into(), clock_arg.into());

//                 for _ in 0..repeat {
//                     $delete_calls(builder, expired.borrow_mut().into(), type_args.clone());
//                 }
//                 ap::intents::destroy_empty_expired(builder, expired);
//                 Ok(())
//             }
//         }
//     };

//     (
//         $intent_name:ident,
//         $generic_type:ident $(:$trait_bound:path)?,
//         $request_args_type:ty,
//         $request_call:expr,
//         $execute_call:expr,
//         $delete_calls:expr,
//     ) => {
//         paste::paste! {
//             pub async fn [<request_ $intent_name>]<$generic_type: MoveType>(
//                 &self,
//                 builder: &mut TransactionBuilder,
//                 params_args: ParamsArgs,
//                 request_args: $request_args_type,
//                 type_args: Vec<&str>,
//             ) -> Result<()> {
//                 let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
//                 let clock_arg = builder.input(self.obj(CLOCK_OBJECT.parse().unwrap()).await?.by_ref());
//                 let mut all_type_args = vec![
//                     format!("{}::{}::{}", ACCOUNT_MULTISIG_PACKAGE, "multisig", "Multisig").parse::<TypeTag>().unwrap(),
//                     format!("{}::{}::{}", ACCOUNT_MULTISIG_PACKAGE, "multisig", "Approvals").parse::<TypeTag>().unwrap(),
//                 ];
//                 all_type_args.extend(type_args.iter().map(|s| s.parse::<TypeTag>().unwrap()).collect::<Vec<TypeTag>>());

//                 let auth = am::multisig::authenticate(builder, ms_arg.into());
//                 let params = ap::intents::new_params(
//                     builder,
//                     params_args.key,
//                     params_args.description,
//                     params_args.execution_times,
//                     params_args.expiration_time,
//                     clock_arg.into(),
//                 );
//                 let outcome = am::multisig::empty_outcome(builder);

//                 $request_call(builder, auth.into(), ms_arg.into(), params.into(), outcome.into(), request_args, all_type_args);
//                 Ok(())
//             }

//             pub async fn [<execute_ $intent_name>]<$generic_type: MoveType + $($trait_bound)?>(
//                 &self,
//                 builder: &mut TransactionBuilder,
//                 mut args: Vec<Argument>,
//                 type_args: Vec<&str>,
//                 intent_key: String,
//                 repeat: u64,
//                 clear: bool,
//             ) -> Result<()> {
//                 let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
//                 let clock_arg = builder.input(self.obj(CLOCK_OBJECT.parse().unwrap()).await?.by_ref());
//                 let key_arg = builder.input(Serialized(&intent_key));
//                 let mut all_type_args = vec![
//                     format!("{}::{}::{}", ACCOUNT_MULTISIG_PACKAGE, "multisig", "Multisig").parse::<TypeTag>().unwrap(),
//                     format!("{}::{}::{}", ACCOUNT_MULTISIG_PACKAGE, "multisig", "Approvals").parse::<TypeTag>().unwrap(),
//                 ];
//                 all_type_args.extend(type_args.iter().map(|s| s.parse::<TypeTag>().unwrap()).collect::<Vec<TypeTag>>());

//                 let mut executable = am::multisig::execute_intent(
//                     builder,
//                     ms_arg.into(),
//                     key_arg.into(),
//                     clock_arg.into(),
//                 );

//                 for _ in 0..repeat {
//                     $execute_call(builder, executable.borrow_mut().into(), ms_arg.into(), args.remove(0), all_type_args.clone());
//                 }

//                 ap::account::confirm_execution::<am::multisig::Multisig, am::multisig::Approvals>(
//                     builder,
//                     ms_arg.into(),
//                     executable,
//                 );

//                 if clear {
//                     let mut expired = ap::account::destroy_empty_intent::<
//                         am::multisig::Multisig,
//                         am::multisig::Approvals,
//                     >(builder, ms_arg.into(), key_arg.into());

//                     for _ in 0..repeat {
//                         $delete_calls(builder, expired.borrow_mut().into(), all_type_args.clone());
//                     }
//                     ap::intents::destroy_empty_expired(builder, expired);
//                 }
//                 Ok(())
//             }

//             pub async fn [<delete_ $intent_name>]<$generic_type: MoveType>(
//                 &self,
//                 builder: &mut TransactionBuilder,
//                 type_args: Vec<&str>,
//                 intent_key: String,
//                 repeat: u64,
//             ) -> Result<()> {
//                 let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
//                 let clock_arg = builder.input(self.obj(CLOCK_OBJECT.parse().unwrap()).await?.by_ref());
//                 let key_arg = builder.input(Serialized(&intent_key));
//                 let type_args = type_args.iter().map(|s| s.parse::<TypeTag>().unwrap()).collect::<Vec<TypeTag>>();

//                 let mut expired = ap::account::delete_expired_intent::<
//                     am::multisig::Multisig,
//                     am::multisig::Approvals,
//                 >(builder, ms_arg.into(), key_arg.into(), clock_arg.into());

//                 for _ in 0..repeat {
//                     $delete_calls(builder, expired.borrow_mut().into(), type_args.clone());
//                 }
//                 ap::intents::destroy_empty_expired(builder, expired);
//                 Ok(())
//             }
//         }
//     };

//     // (
//     //     $intent_name:ident,
//     //     $generic_type_1:ident $(:$trait_bound_1:path)?,
//     //     $generic_type_2:ident $(:$trait_bound_2:path)?,
//     //     $request_args_type:ty,
//     //     $request_call:expr,
//     //     $execute_call:expr,
//     //     $delete_calls:expr,
//     // ) => {
//     //     paste::paste! {
//     //         pub async fn [<request_ $intent_name>]<$generic_type_1: MoveType>(
//     //             &self,
//     //             builder: &mut TransactionBuilder,
//     //             params_args: ParamsArgs,
//     //             request_args: $request_args_type,
//     //             _generic_type: $generic_type_1,
//     //         ) -> Result<()> {
//     //             let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
//     //             let clock_arg = builder.input(self.obj(CLOCK_OBJECT.parse().unwrap()).await?.by_ref());

//     //             let auth = am::multisig::authenticate(builder, ms_arg.into());
//     //             let params = ap::intents::new_params(
//     //                 builder,
//     //                 params_args.key,
//     //                 params_args.description,
//     //                 params_args.execution_times,
//     //                 params_args.expiration_time,
//     //                 clock_arg.into(),
//     //             );
//     //             let outcome = am::multisig::empty_outcome(builder);

//     //             $request_call(builder, auth, ms_arg.into(), params, outcome, request_args);
//     //             Ok(())
//     //         }

//     //         pub async fn [<execute_ $intent_name>]<$generic_type_1: MoveType + $($trait_bound_1)?, $generic_type_2: MoveType + $($trait_bound_2)?>(
//     //             &self,
//     //             builder: &mut TransactionBuilder,
//     //             intent_key: String,
//     //             repeat: u64,
//     //             clear: bool,
//     //             opt_obj: Option<Argument>,
//     //         ) -> Result<()> {
//     //             let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
//     //             let clock_arg = builder.input(self.obj(CLOCK_OBJECT.parse().unwrap()).await?.by_ref());
//     //             let key_arg = builder.input(Serialized(&intent_key));

//     //             let mut executable = am::multisig::execute_intent(
//     //                 builder,
//     //                 ms_arg.into(),
//     //                 key_arg.into(),
//     //                 clock_arg.into(),
//     //             );

//     //             for _ in 0..repeat {
//     //                 $execute_call(builder, executable.borrow_mut(), ms_arg.into(), opt_obj);
//     //             }

//     //             ap::account::confirm_execution::<am::multisig::Multisig, am::multisig::Approvals>(
//     //                 builder,
//     //                 ms_arg.into(),
//     //                 executable,
//     //             );

//     //             if clear {
//     //                 let mut expired = ap::account::destroy_empty_intent::<
//     //                     am::multisig::Multisig,
//     //                     am::multisig::Approvals,
//     //                 >(builder, ms_arg.into(), key_arg.into());

//     //                 for _ in 0..repeat {
//     //                     $delete_calls(builder, ms_arg, expired.borrow_mut().into());
//     //                 }
//     //                 ap::intents::destroy_empty_expired(builder, expired);
//     //             }
//     //             Ok(())
//     //         }

//     //         pub async fn [<delete_ $intent_name>]<$generic_type_1: MoveType + $($trait_bound_1)?, $generic_type_2: MoveType + $($trait_bound_2)?>(
//     //             &self,
//     //             builder: &mut TransactionBuilder,
//     //             intent_key: String,
//     //             repeat: u64,
//     //         ) -> Result<()> {
//     //             let ms_arg = builder.input(self.obj(self.multisig_id()?).await?.by_mut());
//     //             let clock_arg = builder.input(self.obj(CLOCK_OBJECT.parse().unwrap()).await?.by_ref());
//     //             let key_arg = builder.input(Serialized(&intent_key));

//     //             let mut expired = ap::account::delete_expired_intent::<
//     //                 am::multisig::Multisig,
//     //                 am::multisig::Approvals,
//     //             >(builder, ms_arg.into(), key_arg.into(), clock_arg.into());

//     //             for _ in 0..repeat {
//     //                 $delete_calls(builder, ms_arg, expired.borrow_mut().into());
//     //             }
//     //             ap::intents::destroy_empty_expired(builder, expired);
//     //             Ok(())
//     //         }
//     //     }
//     // };
// }

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
