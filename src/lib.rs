pub mod actions;
pub mod dynamic_fields;
pub mod intents;
pub mod move_binding;
pub mod multisig;
pub mod owned_objects;
pub mod params;
pub mod utils;

use anyhow::{anyhow, Ok, Result};
use move_types::TypeTag;
use std::{str::FromStr, sync::Arc};
use sui_graphql_client::Client;
use sui_sdk_types::{Address, Argument, ObjectData};
use sui_transaction_builder::{unresolved::Input, TransactionBuilder};
use sui_transaction_builder::{Function, Serialized};

use crate::move_binding::sui;
use crate::dynamic_fields::DynamicFields;
use crate::intents::{Intent, Intents};
// use crate::move_binding::account_extensions as ae;
use crate::move_binding::account_actions as aa;
use crate::move_binding::account_multisig as am;
use crate::move_binding::account_protocol as ap;
use crate::multisig::Multisig;
use crate::owned_objects::OwnedObjects;
use crate::params::{ParamsArgs};

// TODO: MultisigCreateBuilder
// TODO: params, intents, User

pub struct MultisigClient {
    sui_client: Arc<Client>,
    multisig: Option<Multisig>,
}

impl MultisigClient {
    pub const ACCOUNT_PROTOCOL_PACKAGE: &str = 
        "0x10c87c29ea5d5674458652ababa246742a763f9deafed11608b7f0baea296484";
    pub const ACCOUNT_ACTIONS_PACKAGE: &str = 
        "0xf477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94";
    pub const ACCOUNT_MULTISIG_PACKAGE: &str = 
        "0x460632ef4e9e708658788229531b99f1f3285de06e1e50e98a22633c7e494867";
    pub const EXTENSIONS_OBJECT: &str =
        "0x698bc414f25a7036d9a72d6861d9d268e478492dc8bfef8b5c1c2f1eae769254";
    pub const FEE_OBJECT: &str =
        "0xc27762578a0b1f37224550dcfd0442f37dc82744b802d3517822d1bd2718598f";
    pub const CLOCK_OBJECT: &str =
        "0x0000000000000000000000000000000000000000000000000000000000000006";

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
        let fee_obj = utils::get_object(
            &self.sui_client,
            Address::from_hex(Self::FEE_OBJECT)?,
        ).await?;
        let fee = if let ObjectData::Struct(obj) = fee_obj.data() {
            bcs::from_bytes::<am::fees::Fees>(obj.contents())
                .map_err(|e| anyhow!("Failed to parse fee object: {}", e))?
        } else {
            return Err(anyhow!("Fee object not a struct"));
        };

        let coin_amount = utils::pure_as_argument(builder, &fee.amount);
        let coin_arg = builder.split_coins(builder.gas(), vec![coin_amount]);
        let fee_arg = builder.input(Input::from(&fee_obj).by_ref());
        let extensions_arg = utils::object_ref_as_argument(
            &self.sui_client,
            builder,
            Address::from_hex(Self::EXTENSIONS_OBJECT)?,
        ).await?;

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
        let multisig_argument = self.multisig_mut_argument(builder).await?;
        let key_argument = utils::pure_as_argument(builder, &intent_key);

        am::multisig::approve_intent(builder, multisig_argument.into(), key_argument.into());

        Ok(())
    }

    pub async fn disapprove_intent(
        &self,
        builder: &mut TransactionBuilder,
        intent_key: String,
    ) -> Result<()> {
        let multisig_argument = self.multisig_mut_argument(builder).await?;
        let key_argument = utils::pure_as_argument(builder, &intent_key);

        am::multisig::disapprove_intent(builder, multisig_argument.into(), key_argument.into());

        Ok(())
    }

    // === Commands ===

    pub async fn deposit_cap(
        &self,
        builder: &mut TransactionBuilder,
        cap_id: Address,
        cap_type: &str,
    ) -> Result<()> {
        let multisig_argument = self.multisig_mut_argument(builder).await?;
        let cap_argument = utils::object_val_as_argument(
            &self.sui_client,
            builder,
            cap_id,
        ).await?;

        let auth = am::multisig::authenticate(builder, multisig_argument.into());

        builder.move_call(
            Function::new(
                Self::ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "access_control".parse().unwrap(),
                "lock_cap".parse().unwrap(),
                vec![TypeTag::from_str(cap_type)?],
            ),
            vec![auth.into(), multisig_argument, cap_argument],
        );

        Ok(())
    }

    pub async fn replace_metadata(
        &self,
        builder: &mut TransactionBuilder,
        keys: Vec<String>,
        values: Vec<String>,
    ) -> Result<()> {
        let multisig_argument = self.multisig_mut_argument(builder).await?;
        let keys_argument = utils::pure_as_argument(builder, &keys);
        let values_argument = utils::pure_as_argument(builder, &values);

        let auth = am::multisig::authenticate(builder, multisig_argument.into());

        ap::config::edit_metadata::<am::multisig::Approvals>(
            builder,
            auth,
            multisig_argument.into(),
            keys_argument.into(),
            values_argument.into(),
        );

        Ok(())
    }

    pub async fn update_verified_deps_to_latest(
        &self,
        builder: &mut TransactionBuilder,
    ) -> Result<()> {
        let extensions_argument = utils::object_ref_as_argument(
            &self.sui_client,
            builder,
            Address::from_hex(Self::EXTENSIONS_OBJECT)?,
        ).await?;
        let multisig_argument = self.multisig_mut_argument(builder).await?;
        let auth = am::multisig::authenticate(builder, multisig_argument.into());

        ap::config::update_extensions_to_latest::<am::multisig::Approvals>(
            builder,
            auth,
            multisig_argument.into(),
            extensions_argument.into(),
        );

        Ok(())
    }

    pub async fn deposit_treasury_cap(
        &self,
        builder: &mut TransactionBuilder,
        cap_id: Address,
        coin_type: &str,
        max_supply: Option<u64>,
    ) -> Result<()> {
        let multisig_argument = self.multisig_mut_argument(builder).await?;
        let cap_argument = utils::object_val_as_argument(
            &self.sui_client,
            builder,
            cap_id,
        ).await?;
        let max_supply_argument = utils::pure_as_argument(builder, &max_supply);

        let auth = am::multisig::authenticate(builder, multisig_argument.into());

        builder.move_call(
            Function::new(
                Self::ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "currency".parse().unwrap(),
                "lock_cap".parse().unwrap(),
                vec![TypeTag::from_str(coin_type)?],
            ),
            vec![auth.into(), multisig_argument, cap_argument, max_supply_argument],
        );
        
        Ok(())
    }

    pub async fn merge_and_split(
        &self,
        builder: &mut TransactionBuilder,
        coins_to_merge: Vec<Address>,
        amounts_to_split: Vec<u64>,
        coin_type: &str,
    ) -> Result<()> {
        let multisig_argument = self.multisig_mut_argument(builder).await?;
        let mut coin_inputs = Vec::new();
        for coin in coins_to_merge {
            let input = Input::from(&utils::get_object(&self.sui_client, coin).await?);
            coin_inputs.push(builder.input(input.by_val().with_receiving_kind()));
        }

        let to_merge_argument = builder.make_move_vec(None, coin_inputs);
        let to_split_argument = builder.input(Serialized(&amounts_to_split));

        let auth = am::multisig::authenticate(builder, multisig_argument.into());

        builder.move_call(
            Function::new(
                Self::ACCOUNT_PROTOCOL_PACKAGE.parse().unwrap(),
                "owned".parse().unwrap(),
                "merge_and_split".parse().unwrap(),
                vec![TypeTag::from_str(coin_type)?],
            ),
            vec![auth.into(), multisig_argument, to_merge_argument, to_split_argument],
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
        let package_name_argument = utils::pure_as_argument(builder, &package_name);
        let timelock_duration_argument = utils::pure_as_argument(builder, &timelock_duration);
        let upgrade_cap_argument = utils::object_val_as_argument(
            &self.sui_client,
            builder,
            cap_id,
        ).await?;
        let multisig_argument = self.multisig_mut_argument(builder).await?;

        let auth = am::multisig::authenticate(builder, multisig_argument.into());

        aa::package_upgrade::lock_cap::<am::multisig::Approvals>(
            builder,
            auth,
            multisig_argument.into(),
            upgrade_cap_argument.into(),
            package_name_argument.into(),
            timelock_duration_argument.into(),
        );

        Ok(())
    }

    pub async fn open_vault(
        &self,
        builder: &mut TransactionBuilder,
        vault_name: &str,
    ) -> Result<()> {
        let vault_name_argument = utils::pure_as_argument(builder, &vault_name);
        let multisig_argument = self.multisig_mut_argument(builder).await?;

        let auth = am::multisig::authenticate(builder, multisig_argument.into());

        aa::vault::open::<am::multisig::Approvals>(
            builder,
            auth,
            multisig_argument.into(),
            vault_name_argument.into(),
        );

        Ok(())
    }

    pub async fn deposit_from_wallet(
        &self,
        builder: &mut TransactionBuilder,
        coin_id: Argument,
        coin_type: &str,
        vault_name: &str,
    ) -> Result<()> {
        let multisig_argument = self.multisig_mut_argument(builder).await?;
        let vault_name_argument = utils::pure_as_argument(builder, &vault_name);

        let auth = am::multisig::authenticate(builder, multisig_argument.into());

        builder.move_call(
            Function::new(
                Self::ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "vault".parse().unwrap(),
                "deposit".parse().unwrap(),
                vec![TypeTag::from_str(coin_type)?],
            ),
            vec![auth.into(), multisig_argument, vault_name_argument, coin_id],
        );
        
        Ok(())
    }

    pub async fn close_vault(
        &self,
        builder: &mut TransactionBuilder,
        vault_name: &str,
    ) -> Result<()> {
        let vault_name_argument = utils::pure_as_argument(builder, &vault_name);
        let multisig_argument = self.multisig_mut_argument(builder).await?;

        let auth = am::multisig::authenticate(builder, multisig_argument.into());

        aa::vault::close::<am::multisig::Approvals>(
            builder,
            auth,
            multisig_argument.into(),
            vault_name_argument.into(),
        );

        Ok(())
    }

    pub async fn claim_vested(
        &self,
        builder: &mut TransactionBuilder,
        vesting_id: Address,
        cap_id: Address,
        coin_type: &str,
    ) -> Result<()> {
        let vesting_argument = utils::object_mut_as_argument(
            &self.sui_client,
            builder,
            vesting_id,
        ).await?;
        let cap_argument = utils::object_ref_as_argument(
            &self.sui_client,
            builder,
            cap_id,
        ).await?;
        let clock_argument = utils::object_ref_as_argument(
            &self.sui_client,
            builder,
            Address::from_hex(Self::CLOCK_OBJECT)?,
        ).await?;

        builder.move_call(
            Function::new(
                Self::ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "vesting".parse().unwrap(),
                "claim".parse().unwrap(),
                vec![TypeTag::from_str(coin_type)?],
            ),
            vec![vesting_argument, cap_argument, clock_argument],
        );
        
        Ok(())
    }

    pub async fn cancel_vesting(
        &self,
        builder: &mut TransactionBuilder,
        vesting_id: Address,
        coin_type: &str,
    ) -> Result<()> {
        let vesting_argument = utils::object_mut_as_argument(
            &self.sui_client,
            builder,
            vesting_id,
        ).await?;
        let multisig_argument = utils::object_ref_as_argument(
            &self.sui_client,
            builder,
            self.multisig_id()?,
        ).await?;

        let auth = am::multisig::authenticate(builder, multisig_argument.into());

        builder.move_call(
            Function::new(
                Self::ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "vesting".parse().unwrap(),
                "cancel_payment".parse().unwrap(),
                vec![
                    format!("{}::{}::{}", Self::ACCOUNT_MULTISIG_PACKAGE, "multisig", "Multisig").parse().unwrap(),
                    TypeTag::from_str(coin_type)?
                ],
            ),
            vec![auth.into(), vesting_argument, multisig_argument],
        );
        
        Ok(())
    }

    pub async fn destroy_empty_vesting(
        &self,
        builder: &mut TransactionBuilder,
        vesting_id: Address,
        coin_type: &str,
    ) -> Result<()> {
        let vesting_argument = utils::object_val_as_argument(
            &self.sui_client,
            builder,
            vesting_id,
        ).await?;

        builder.move_call(
            Function::new(
                Self::ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "vesting".parse().unwrap(),
                "destroy_empty".parse().unwrap(),
                vec![TypeTag::from_str(coin_type)?],
            ),
            vec![vesting_argument],
        );
        
        Ok(())
    }

    pub async fn destroy_claim_cap(
        &self,
        builder: &mut TransactionBuilder,
        cap_id: Address,
    ) -> Result<()> {
        let cap_argument = utils::object_val_as_argument(
            &self.sui_client,
            builder,
            cap_id,
        ).await?;

        aa::vesting::destroy_cap(builder, cap_argument.into());
        
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

    // define_intent_interface!(
    //     config_multisig,
    //     ConfigMultisigArgs,
    //     |builder, auth, multisig_input, params, outcome, args: ConfigMultisigArgs| {
    //         ap::config::req(
    //             builder,
    //             auth,
    //             multisig_input,
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
        self.multisig.as_ref().map(|m| m.id).ok_or(anyhow!("Multisig not loaded"))
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

    async fn multisig_mut_argument(&self, builder: &mut TransactionBuilder) -> Result<Argument> {
        utils::object_mut_as_argument(
            &self.sui_client,
            builder,
            self.multisig_id()?,
        ).await
    }
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
                let multisig_argument = self.multisig_mut_argument(builder).await?;
                let clock_argument = utils::object_ref_as_argument(
                    &self.sui_client,
                    builder,
                    Address::from_hex(Self::CLOCK_OBJECT)?,
                ).await?;

                let auth = am::multisig::authenticate(builder, multisig_argument.into());
                let params = ap::intents::new_params(
                    builder,
                    params_args.key,
                    params_args.description,
                    params_args.execution_times,
                    params_args.expiration_time,
                    clock_argument.into(),
                );
                let outcome = am::multisig::empty_outcome(builder);

                $request_call(builder, auth, multisig_argument.into(), params, outcome, request_args);
                Ok(())
            }

            pub async fn [<execute_ $intent_name>](
                &self,
                builder: &mut TransactionBuilder,
                intent_key: String,
                clear: bool,
            ) -> Result<()> {
                let multisig_argument = self.multisig_mut_argument(builder).await?;
                let clock_argument = utils::object_ref_as_argument(
                    &self.sui_client,
                    builder,
                    Address::from_hex(Self::CLOCK_OBJECT)?,
                ).await?;
                let key_argument = utils::pure_as_argument(builder, &intent_key);

                let mut executable = am::multisig::execute_intent(
                    builder,
                    multisig_argument.into(),
                    key_argument.into(),
                    clock_argument.into(),
                );

                $execute_call(builder, executable.borrow_mut(), multisig_argument.into());

                ap::account::confirm_execution::<am::multisig::Multisig, am::multisig::Approvals>(
                    builder,
                    multisig_argument.into(),
                    executable,
                );

                if clear {
                    let mut expired = ap::account::destroy_empty_intent::<
                        am::multisig::Multisig,
                        am::multisig::Approvals,
                    >(builder, multisig_argument.into(), key_argument.into());

                    $delete_calls(builder, expired.borrow_mut());
                    ap::intents::destroy_empty_expired(builder, expired);
                }
                Ok(())
            }

            pub async fn [<delete_ $intent_name>](
                &self,
                builder: &mut TransactionBuilder,
                intent_key: String,
            ) -> Result<()> {
                let multisig_argument = self.multisig_mut_argument(builder).await?;
                let clock_argument = utils::object_ref_as_argument(
                    &self.sui_client,
                    builder,
                    Address::from_hex(Self::CLOCK_OBJECT)?,
                ).await?;
                let key_argument = utils::pure_as_argument(builder, &intent_key);

                let mut expired = ap::account::delete_expired_intent::<
                    am::multisig::Multisig,
                    am::multisig::Approvals,
                >(builder, multisig_argument.into(), key_argument.into(), clock_argument.into());

                $delete_calls(builder, expired.borrow_mut());
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
