pub mod actions;
pub mod intents;
pub mod move_binding;
pub mod multisig;
pub mod params;

use std::sync::Arc;
use anyhow::{anyhow, Ok, Result};
use sui_graphql_client::Client;
use sui_sdk_types::{Address, ObjectData};
use sui_transaction_builder::Serialized;
use sui_transaction_builder::{unresolved::Input, TransactionBuilder};

use crate::move_binding::sui;
// use crate::move_binding::account_extensions as ae;
use crate::move_binding::account_multisig as am;
use crate::move_binding::account_protocol as ap;
use crate::multisig::Multisig;
use crate::params::{ConfigMultisigArgs, ParamsArgs};

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

    // === Intent request ===

    pub async fn request_config_multisig(
        &self,
        builder: &mut TransactionBuilder,
        params_args: ParamsArgs,
        config_multisig_args: ConfigMultisigArgs,
    ) -> Result<()> {
        intent_builder!(
            builder,
            self.multisig_as_input(true).await?,
            self.clock_as_input().await?,
            params_args,
            |builder, auth, multisig_input, params, outcome| {
                am::config::request_config_multisig(
                    builder, 
                    auth, 
                    multisig_input, 
                    params, 
                    outcome, 
                    config_multisig_args.addresses, 
                    config_multisig_args.weights, 
                    config_multisig_args.roles, 
                    config_multisig_args.global, 
                    config_multisig_args.role_names, 
                    config_multisig_args.role_thresholds
                )
            }
        );
        Ok(())
    }

    // === Intent execution ===

    pub async fn execute_config_multisig(
        &self,
        builder: &mut TransactionBuilder,
        intent_key: String,
        clear: bool,
    ) -> Result<()> {
        intent_executor!(
            builder,
            self.multisig_as_input(true).await?,
            self.clock_as_input().await?,
            intent_key,
            |executable, multisig| am::config::execute_config_multisig(
                builder, executable, multisig
            ),
            |expired| am::config::delete_config_multisig(builder, expired),
            clear
        );
        Ok(())
    }

    // === Intent deletion ===

    pub async fn delete_config_multisig(
        &self,
        builder: &mut TransactionBuilder,
        intent_key: String,
    ) -> Result<()> {
        intent_cleaner!(
            builder, 
            self.multisig_as_input(true).await?, 
            self.clock_as_input().await?, 
            intent_key, 
            |expired| am::config::delete_config_multisig(builder, expired)
        );
        Ok(())
    }

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
        self.multisig.as_ref().map(|m| m.id())
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
macro_rules! intent_builder {
    (
        $builder:expr,
        $multisig_input:expr,
        $clock_input:expr,
        $params_args:expr,
        $request:expr
    ) => {
        let multisig_input = $builder.input($multisig_input);
        let clock_input = $builder.input($clock_input);

        let auth = am::multisig::authenticate($builder, multisig_input.into());

        let params = ap::intents::new_params(
            $builder,
            $params_args.key,
            $params_args.description,
            $params_args.execution_times,
            $params_args.expiration_time,
            clock_input.into(),
        );

        let outcome = am::multisig::empty_outcome($builder);

        $request($builder, auth, multisig_input.into(), params, outcome);
    };
}

#[macro_export]
macro_rules! intent_executor {
    (
        $builder:expr, 
        $multisig_input:expr, 
        $clock_input:expr, 
        $intent_key:expr, 
        $execute:expr, 
        $delete:expr, 
        $clear:expr
    ) => {
        let multisig_input = $builder.input($multisig_input);
        let clock_input = $builder.input($clock_input);
        let key_input = $builder.input(Serialized(&$intent_key));

        let mut executable = am::multisig::execute_intent(
            $builder,
            multisig_input.into(),
            key_input.into(),
            clock_input.into(),
        );

        $execute(executable.borrow_mut(), multisig_input.into());

        ap::account::confirm_execution::<am::multisig::Multisig, am::multisig::Approvals>(
            $builder,
            multisig_input.into(),
            executable,
        );

        if $clear {
            let mut expired = ap::account::destroy_empty_intent::<
                am::multisig::Multisig,
                am::multisig::Approvals,
            >($builder, multisig_input.into(), key_input.into());

            $delete(expired.borrow_mut());
            ap::intents::destroy_empty_expired($builder, expired);
        }
    };
}

#[macro_export]
macro_rules! intent_cleaner {
    (
        $builder:expr, 
        $multisig_input:expr, 
        $clock_input:expr, 
        $intent_key:expr, 
        $delete:expr
    ) => {
        let multisig_input = $builder.input($multisig_input);
        let clock_input = $builder.input($clock_input);
        let key_input = $builder.input(Serialized(&$intent_key));

        let mut expired = ap::account::delete_expired_intent::<
            am::multisig::Multisig,
            am::multisig::Approvals,
        >($builder, multisig_input.into(), key_input.into(), clock_input.into());

        $delete(expired.borrow_mut());
        ap::intents::destroy_empty_expired($builder, expired);
    };
}
