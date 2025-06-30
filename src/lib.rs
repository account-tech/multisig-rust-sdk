pub mod actions;
pub mod intents;
pub mod move_binding;
pub mod multisig;
pub mod params;
pub mod owned_objects;
pub mod dynamic_fields;

use anyhow::{anyhow, Ok, Result};
use std::sync::Arc;
use sui_graphql_client::Client;
use sui_sdk_types::{Address, ObjectData};
use sui_transaction_builder::Serialized;
use sui_transaction_builder::{unresolved::Input, TransactionBuilder};

use crate::move_binding::sui;
// use crate::move_binding::account_extensions as ae;
use crate::move_binding::account_multisig as am;
use crate::move_binding::account_protocol as ap;
use crate::multisig::Multisig;
use crate::intents::{Intent, Intents};
use crate::params::{ConfigMultisigArgs, ParamsArgs};
use crate::owned_objects::OwnedObjects;
use crate::dynamic_fields::DynamicFields;

// TODO: MultisigCreateBuilder 
// TODO: dfs, intents, commands, User

pub struct MultisigClient {
    sui_client: Arc<Client>,
    multisig: Option<Multisig>,
}

impl MultisigClient {
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
        let extensions_obj = &self
            .sui_client
            .object(Address::from_hex(Self::EXTENSIONS_OBJECT)?, None)
            .await?
            .ok_or(anyhow!("Extensions object not found"))?;

        let fee_obj = &self
            .sui_client
            .object(Address::from_hex(Self::FEE_OBJECT)?, None)
            .await?
            .ok_or(anyhow!("Fee object not found"))?;
        let fee = if let ObjectData::Struct(obj) = fee_obj.data() {
            bcs::from_bytes::<am::fees::Fees>(obj.contents())
                .map_err(|e| anyhow!("Failed to parse fee object: {}", e))?
        } else {
            return Err(anyhow!("Fee object not a struct"));
        };

        let coin_amount = builder.input(Serialized(&fee.amount));
        let coin_arg = builder.split_coins(builder.gas(), vec![coin_amount]);
        let fee_arg = builder.input(Input::from(fee_obj).by_ref());
        let extensions_arg = builder.input(Input::from(extensions_obj).by_ref());

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
        let multisig_input = builder.input(self.multisig_as_input(true).await?);
        let key_input = builder.input(Serialized(&intent_key));

        am::multisig::approve_intent(builder, multisig_input.into(), key_input.into());

        Ok(())
    }

    pub async fn disapprove_intent(
        &self,
        builder: &mut TransactionBuilder,
        intent_key: String,
    ) -> Result<()> {
        let multisig_input = builder.input(self.multisig_as_input(true).await?);
        let key_input = builder.input(Serialized(&intent_key));

        am::multisig::disapprove_intent(builder, multisig_input.into(), key_input.into());

        Ok(())
    }

    define_intent_interface!(
        config_multisig,
        ConfigMultisigArgs,
        |builder, auth, multisig_input, params, outcome, args: ConfigMultisigArgs| {
            am::config::request_config_multisig(
                builder, auth, multisig_input, params, outcome, 
                args.addresses, args.weights, args.roles, args.global, 
                args.role_names, args.role_thresholds
            )
        },
        |builder, executable, multisig| am::config::execute_config_multisig(
            builder, executable, multisig
        ),
        |builder, expired| am::config::delete_config_multisig(builder, expired),
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

    pub fn multisig_id(&self) -> Option<Address> {
        self.multisig.as_ref().map(|m| m.id)
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

    async fn multisig_as_input(&self, is_mut: bool) -> Result<Input> {
        let multisig_id = self.multisig_id().ok_or(anyhow!("Multisig not loaded"))?;

        let multisig_obj = &self
            .sui_client
            .object(multisig_id, None)
            .await?
            .ok_or(anyhow!("Multisig object not found"))?;

        if is_mut {
            Ok(Input::from(multisig_obj).by_mut())
        } else {
            Ok(Input::from(multisig_obj).by_ref())
        }
    }

    async fn clock_as_input(&self) -> Result<Input> {
        let clock_obj = &self
            .sui_client
            .object(Address::from_hex(Self::CLOCK_OBJECT)?, None)
            .await?
            .ok_or(anyhow!("Multisig object not found"))?;

        Ok(Input::from(clock_obj).by_ref())
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
                let multisig_input = builder.input(self.multisig_as_input(true).await?);
                let clock_input = builder.input(self.clock_as_input().await?);

                let auth = am::multisig::authenticate(builder, multisig_input.into());
                let params = ap::intents::new_params(
                    builder,
                    params_args.key,
                    params_args.description,
                    params_args.execution_times,
                    params_args.expiration_time,
                    clock_input.into(),
                );
                let outcome = am::multisig::empty_outcome(builder);

                $request_call(builder, auth, multisig_input.into(), params, outcome, request_args);
                Ok(())
            }

            pub async fn [<execute_ $intent_name>](
                &self,
                builder: &mut TransactionBuilder,
                intent_key: String,
                clear: bool,
            ) -> Result<()> {
                let multisig_input = builder.input(self.multisig_as_input(true).await?);
                let clock_input = builder.input(self.clock_as_input().await?);
                let key_input = builder.input(Serialized(&intent_key));

                let mut executable = am::multisig::execute_intent(
                    builder,
                    multisig_input.into(),
                    key_input.into(),
                    clock_input.into(),
                );

                $execute_call(builder, executable.borrow_mut(), multisig_input.into());

                ap::account::confirm_execution::<am::multisig::Multisig, am::multisig::Approvals>(
                    builder,
                    multisig_input.into(),
                    executable,
                );

                if clear {
                    let mut expired = ap::account::destroy_empty_intent::<
                        am::multisig::Multisig,
                        am::multisig::Approvals,
                    >(builder, multisig_input.into(), key_input.into());

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
                let multisig_input = builder.input(self.multisig_as_input(true).await?);
                let clock_input = builder.input(self.clock_as_input().await?);
                let key_input = builder.input(Serialized(&intent_key));

                let mut expired = ap::account::delete_expired_intent::<
                    am::multisig::Multisig,
                    am::multisig::Approvals,
                >(builder, multisig_input.into(), key_input.into(), clock_input.into());

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
