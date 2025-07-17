pub mod assets;
pub mod proposals;
pub mod move_binding;
pub mod multisig;
pub mod user;
pub mod utils;
pub mod multisig_builder;

pub use multisig_builder::MultisigBuilder;

use std::{fmt, sync::Arc};
use anyhow::{anyhow, Ok, Result};
use move_types::{Key, MoveType, functions::Arg};
use sui_graphql_client::Client;
use sui_sdk_types::{Address, Argument, ObjectData, ObjectId};
use sui_transaction_builder::{unresolved::Input, TransactionBuilder, Function, Serialized};

use crate::move_binding::{account_actions as aa, account_extensions as ae, account_multisig as am, account_protocol as ap, sui};
use crate::proposals::{actions::IntentActions, params::{self, ParamsArgs}, intents::{Intent, Intents}};
use crate::assets::{dynamic_fields::DynamicFields, owned_objects::OwnedObjects};
use crate::multisig::Multisig;
use crate::user::User;

static ACCOUNT_MULTISIG_PACKAGE: &str =
    "0x460632ef4e9e708658788229531b99f1f3285de06e1e50e98a22633c7e494867";
static ACCOUNT_PROTOCOL_PACKAGE: &str =
    "0x10c87c29ea5d5674458652ababa246742a763f9deafed11608b7f0baea296484";
static ACCOUNT_ACTIONS_PACKAGE: &str =
    "0xf477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94";
static EXTENSIONS_OBJECT: &str =
    "0x698bc414f25a7036d9a72d6861d9d268e478492dc8bfef8b5c1c2f1eae769254";
static FEE_OBJECT: &str = "0xc27762578a0b1f37224550dcfd0442f37dc82744b802d3517822d1bd2718598f";
static CLOCK_OBJECT: &str = "0x0000000000000000000000000000000000000000000000000000000000000006";

pub struct MultisigClient {
    sui_client: Arc<Client>,
    multisig: Option<Multisig>,
    user: Option<User>,
}

impl MultisigClient {
    // === Constructors ===

    pub fn new_with_client(sui_client: Client) -> Self {
        Self {
            sui_client: Arc::new(sui_client),
            multisig: None,
            user: None,
        }
    }

    pub fn new_with_url(url: &str) -> Result<Self> {
        Ok(Self {
            sui_client: Arc::new(Client::new(url)?),
            multisig: None,
            user: None,
        })
    }

    pub fn new_testnet() -> Self {
        Self {
            sui_client: Arc::new(Client::new_testnet()),
            multisig: None,
            user: None,
        }
    }

    pub fn new_mainnet() -> Self {
        Self {
            sui_client: Arc::new(Client::new_mainnet()),
            multisig: None,
            user: None,
        }
    }

    // === Multisig ===

    pub async fn create_multisig(
        &self,
        builder: &mut TransactionBuilder,
    ) -> Result<Arg<ap::account::Account<am::multisig::Multisig>>> {
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
        let extensions =
            builder.input(self.obj(EXTENSIONS_OBJECT.parse().unwrap()).await?.by_ref());

        let account_obj = am::multisig::new_account(
            builder,
            extensions.into(),
            fee_arg.into(),
            coin_arg.into(),
        );

        Ok(account_obj)
    }

    pub fn share_multisig(
        &self, 
        builder: &mut TransactionBuilder, 
        multisig: Arg<ap::account::Account<am::multisig::Multisig>>
    ) {
        sui::transfer::public_share_object(builder, multisig);
    }

    pub async fn load_multisig(&mut self, id: Address) -> Result<()> {
        self.multisig = Some(Multisig::from_id(self.sui_client.clone(), id).await?);
        Ok(())
    }

    pub async fn load_user(&mut self, address: Address) -> Result<()> {
        self.user = Some(User::from_address(self.sui_client.clone(), address).await?);
        Ok(())
    }

    pub async fn refresh(&mut self) -> Result<()> {
        if let Some(multisig) = self.multisig.as_mut() {
            multisig.refresh().await?;
        }
        if let Some(user) = self.user.as_mut() {
            user.refresh().await?;
        }
        Ok(())
    }

    pub async fn switch_multisig(&mut self, id: Address) -> Result<()> {
        if let Some(multisig) = self.multisig.as_mut() {
            multisig.switch_multisig(id).await?;
        }
        Ok(())
    }

    pub async fn approve_intent(
        &self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
    ) -> Result<()> {
        let mut multisig = self.multisig_arg(builder).await?;
        let key = self.key_arg(builder, intent_key)?;

        am::multisig::approve_intent(builder, multisig.borrow_mut(), key);

        Ok(())
    }

    pub async fn disapprove_intent(
        &self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
    ) -> Result<()> {
        let mut multisig = self.multisig_arg(builder).await?;
        let key = self.key_arg(builder, intent_key)?;

        am::multisig::disapprove_intent(builder, multisig.borrow_mut(), key);

        Ok(())
    }

    // === Commands ===

    pub async fn replace_metadata(
        &self,
        builder: &mut TransactionBuilder,
        keys: Vec<String>,
        values: Vec<String>,
    ) -> Result<()> {
        let mut multisig = self.multisig_arg(builder).await?;
        let keys = self.pure_arg(builder, keys)?;
        let values = self.pure_arg(builder, values)?;

        let auth = am::multisig::authenticate(builder, multisig.borrow());
        ap::config::edit_metadata(builder, auth, multisig.borrow_mut(), keys, values);

        Ok(())
    }

    pub async fn update_verified_deps_to_latest(
        &self,
        builder: &mut TransactionBuilder,
    ) -> Result<()> {
        let mut multisig = self.multisig_arg(builder).await?;
        let extensions = self.extensions_arg(builder).await?;

        let auth = am::multisig::authenticate(builder, multisig.borrow());
        ap::config::update_extensions_to_latest(
            builder,
            auth,
            multisig.borrow_mut(),
            extensions.borrow(),
        );

        Ok(())
    }

    pub async fn deposit_cap(
        &self,
        builder: &mut TransactionBuilder,
        cap_id: Address,
        cap_type: &str,
    ) -> Result<()> {
        let mut multisig = self.multisig_arg(builder).await?;
        let cap = self.owned_argument(builder, cap_id).await?;

        let auth = am::multisig::authenticate(builder, multisig.borrow());
        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "access_control".parse().unwrap(),
                "lock_cap".parse().unwrap(),
                vec![cap_type.parse().unwrap()],
            ),
            vec![auth.into(), multisig.borrow_mut().into(), cap],
        );

        Ok(())
    }

    pub async fn deposit_treasury_cap(
        &self,
        builder: &mut TransactionBuilder,
        max_supply: Option<u64>,
        cap_id: Address,
        coin_type: &str,
    ) -> Result<()> {
        let mut multisig = self.multisig_arg(builder).await?;
        let max_supply = self.pure_arg(builder, max_supply)?;
        let cap = self.owned_argument(builder, cap_id).await?;

        let auth = am::multisig::authenticate(builder, multisig.borrow());
        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "currency".parse().unwrap(),
                "lock_cap".parse().unwrap(),
                vec![coin_type.parse().unwrap()],
            ),
            vec![auth.into(), multisig.borrow_mut().into(), cap, max_supply.into()],
        );

        Ok(())
    }

    pub async fn merge_and_split(
        &self,
        builder: &mut TransactionBuilder,
        coins_to_merge: Vec<Address>,
        amounts_to_split: Vec<u64>,
        coin_type: &str,
    ) -> Result<Argument> {
        let mut multisig = self.multisig_arg(builder).await?;
        let mut coin_inputs = Vec::new();
        for coin in coins_to_merge {
            coin_inputs.push(builder.input(self.obj(coin).await?.with_receiving_kind()));
        }

        let to_merge = builder.make_move_vec(None, coin_inputs);
        let to_split = builder.input(Serialized(&amounts_to_split));

        let auth = am::multisig::authenticate(builder, multisig.borrow());
        let ids = builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_PROTOCOL_PACKAGE.parse().unwrap(),
                "owned".parse().unwrap(),
                "merge_and_split".parse().unwrap(),
                vec![coin_type.parse().unwrap()],
            ),
            vec![auth.into(), multisig.borrow_mut().into(), to_merge, to_split],
        );

        Ok(ids)
    }

    pub async fn deposit_upgrade_cap(
        &self,
        builder: &mut TransactionBuilder,
        cap_id: Address,
        package_name: &str,
        timelock_duration: u64, // can be 0
    ) -> Result<()> {
        let mut multisig = self.multisig_arg(builder).await?;
        let package_name = self.pure_arg(builder, package_name.to_string())?;
        let timelock_duration = self.pure_arg(builder, timelock_duration)?;
        let upgrade_cap = self
            .owned_arg::<sui::package::UpgradeCap>(builder, cap_id)
            .await?;

        let auth = am::multisig::authenticate(builder, multisig.borrow());
        aa::package_upgrade::lock_cap(
            builder,
            auth,
            multisig.borrow_mut(),
            upgrade_cap,
            package_name,
            timelock_duration,
        );

        Ok(())
    }

    pub async fn open_vault(
        &self,
        builder: &mut TransactionBuilder,
        vault_name: &str,
    ) -> Result<()> {
        let mut multisig = self.multisig_arg(builder).await?;
        let vault_name = self.pure_arg(builder, vault_name.to_string())?;

        let auth = am::multisig::authenticate(builder, multisig.borrow());
        aa::vault::open(builder, auth, multisig.borrow_mut(), vault_name);

        Ok(())
    }

    pub async fn deposit_from_wallet(
        &self,
        builder: &mut TransactionBuilder,
        vault_name: String,
        coin: Argument, // splitted in previous command
        coin_type: &str,
    ) -> Result<()> {
        let mut multisig = self.multisig_arg(builder).await?;
        let vault_name = builder.input(Serialized(&vault_name));

        let auth = am::multisig::authenticate(builder, multisig.borrow());
        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "vault".parse().unwrap(),
                "deposit".parse().unwrap(),
                vec![coin_type.parse().unwrap()],
            ),
            vec![auth.into(), multisig.borrow_mut().into(), vault_name, coin],
        );

        Ok(())
    }

    pub async fn close_vault(
        &self,
        builder: &mut TransactionBuilder,
        vault_name: &str,
    ) -> Result<()> {
        let mut multisig = self.multisig_arg(builder).await?;
        let vault_name = self.pure_arg(builder, vault_name.to_string())?;

        let auth = am::multisig::authenticate(builder, multisig.borrow());
        aa::vault::close(builder, auth, multisig.borrow_mut(), vault_name);

        Ok(())
    }

    pub async fn claim_vested(
        &self,
        builder: &mut TransactionBuilder,
        vesting_id: Address,
        cap_id: Address,
        coin_type: &str,
    ) -> Result<()> {
        let vesting = self.shared_mut_argument(builder, vesting_id).await?;
        let cap = self
            .owned_arg::<aa::vesting::ClaimCap>(builder, cap_id)
            .await?;
        let clock = self.clock_arg(builder).await?;

        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "vesting".parse().unwrap(),
                "claim".parse().unwrap(),
                vec![coin_type.parse().unwrap()],
            ),
            vec![vesting, cap.borrow().into(), clock.borrow().into()],
        );

        Ok(())
    }

    pub async fn cancel_vesting(
        &self,
        builder: &mut TransactionBuilder,
        vesting_id: Address,
        coin_type: &str,
    ) -> Result<()> {
        let multisig = self.multisig_arg(builder).await?;
        let vesting = self.shared_mut_argument(builder, vesting_id).await?;

        let auth = am::multisig::authenticate(builder, multisig.borrow());
        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "vesting".parse().unwrap(),
                "cancel_payment".parse().unwrap(),
                vec![coin_type.parse().unwrap()],
            ),
            vec![auth.into(), vesting, multisig.borrow().into()],
        );

        Ok(())
    }

    pub async fn destroy_empty_vesting(
        &self,
        builder: &mut TransactionBuilder,
        vesting_id: Address,
        coin_type: &str,
    ) -> Result<()> {
        let vesting = self.shared_mut_argument(builder, vesting_id).await?;

        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "vesting".parse().unwrap(),
                "destroy_empty".parse().unwrap(),
                vec![coin_type.parse().unwrap()],
            ),
            vec![vesting],
        );

        Ok(())
    }

    pub async fn destroy_claim_cap(
        &self,
        builder: &mut TransactionBuilder,
        cap_id: Address,
    ) -> Result<()> {
        let cap = self
            .owned_arg::<aa::vesting::ClaimCap>(builder, cap_id)
            .await?;

        aa::vesting::destroy_cap(builder, cap);

        Ok(())
    }

    // === Intents ===

    pub async fn request_config_multisig(
        &self,
        builder: &mut TransactionBuilder,
        intent_args: ParamsArgs,
        actions_args: params::ConfigMultisigArgs,
    ) -> Result<()> {
        let (
            mut multisig, 
            auth, 
            params, 
            outcome
        ) = self.prepare_request(builder, intent_args).await?;

        am::config::request_config_multisig(
            builder, auth, multisig.borrow_mut(), params, outcome, 
            actions_args.addresses, actions_args.weights, actions_args.roles, 
            actions_args.global, actions_args.role_names, actions_args.role_thresholds,
        );

        Ok(())
    }

    pub async fn execute_config_multisig(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
    ) -> Result<()> {
        let (
            mut multisig, 
            mut executable, 
            is_last_execution, 
            _executions_count
        ) = self.prepare_execute(builder, intent_key).await?;

        am::config::execute_config_multisig(builder, executable.borrow_mut(), multisig.borrow_mut());
        ap::account::confirm_execution(builder, multisig.borrow_mut(), executable);

        if is_last_execution {
            let key = self.key_arg(builder, intent_key)?;
            let mut expired = ap::account::destroy_empty_intent::<
                am::multisig::Multisig,
                am::multisig::Approvals,
            >(builder, multisig.borrow_mut(), key);

            am::config::delete_config_multisig(builder, expired.borrow_mut());
            ap::intents::destroy_empty_expired(builder, expired);
        }

        Ok(())
    }

    pub async fn delete_config_multisig(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
    ) -> Result<()> {
        let (
            _multisig, 
            mut expired, 
            _executions_count
        ) = self.prepare_delete(builder, intent_key).await?;

        am::config::delete_config_multisig(builder, expired.borrow_mut());
        ap::intents::destroy_empty_expired(builder, expired);

        Ok(())
    }

    pub async fn request_config_deps(
        &self,
        builder: &mut TransactionBuilder,
        intent_args: ParamsArgs,
        actions_args: params::ConfigDepsArgs,
    ) -> Result<()> {
        let extensions = self.extensions_arg(builder).await?;
        let (
            mut multisig, 
            auth, 
            params, 
            outcome
        ) = self.prepare_request(builder, intent_args).await?;

        ap::config::request_config_deps(
            builder, auth, multisig.borrow_mut(), params, outcome, extensions.borrow(), 
            actions_args.names, actions_args.addresses, actions_args.versions,
        );

        Ok(())
    }

    pub async fn execute_config_deps(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
    ) -> Result<()> {
        let (
            mut multisig, 
            mut executable, 
            is_last_execution, 
            _executions_count
        ) = self.prepare_execute(builder, intent_key).await?;

        ap::config::execute_config_deps(builder, executable.borrow_mut(), multisig.borrow_mut());
        ap::account::confirm_execution(builder, multisig.borrow_mut(), executable);

        if is_last_execution {
            let key = self.key_arg(builder, intent_key)?;
            let mut expired = ap::account::destroy_empty_intent::<
                am::multisig::Multisig,
                am::multisig::Approvals,
            >(builder, multisig.borrow_mut(), key);

            ap::config::delete_config_deps(builder, expired.borrow_mut());
            ap::intents::destroy_empty_expired(builder, expired);
        }

        Ok(())
    }

    pub async fn delete_config_deps(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
    ) -> Result<()> {
        let (
            _multisig, 
            mut expired, 
            _executions_count
        ) = self.prepare_delete(builder, intent_key).await?;

        ap::config::delete_config_deps(builder, expired.borrow_mut());
        ap::intents::destroy_empty_expired(builder, expired);

        Ok(())
    }

    pub async fn request_toggle_unverified_allowed(
        &self,
        builder: &mut TransactionBuilder,
        intent_args: ParamsArgs,
    ) -> Result<()> {
        let (
            mut multisig, 
            auth, 
            params, 
            outcome
        ) = self.prepare_request(builder, intent_args).await?;

        ap::config::request_toggle_unverified_allowed(
            builder,
            auth,
            multisig.borrow_mut(),
            params,
            outcome,
        );

        Ok(())
    }

    pub async fn execute_toggle_unverified_allowed(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
    ) -> Result<()> {
        let (
            mut multisig, 
            mut executable, 
            is_last_execution, 
            _executions_count
        ) = self.prepare_execute(builder, intent_key).await?;

        ap::config::execute_toggle_unverified_allowed(builder, executable.borrow_mut(), multisig.borrow_mut());
        ap::account::confirm_execution(builder, multisig.borrow_mut(), executable);

        if is_last_execution {
            let key = self.key_arg(builder, intent_key)?;
            let mut expired = ap::account::destroy_empty_intent::<
                am::multisig::Multisig,
                am::multisig::Approvals,
            >(builder, multisig.borrow_mut(), key);

            ap::config::delete_toggle_unverified_allowed(builder, expired.borrow_mut());
            ap::intents::destroy_empty_expired(builder, expired);
        }

        Ok(())
    }

    pub async fn delete_toggle_unverified_allowed(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
    ) -> Result<()> {
        let (
            _multisig, 
            mut expired, 
            _executions_count
        ) = self.prepare_delete(builder, intent_key).await?;

        ap::config::delete_toggle_unverified_allowed(builder, expired.borrow_mut());
        ap::intents::destroy_empty_expired(builder, expired);

        Ok(())
    }

    pub async fn request_borrow_cap(
        &self,
        builder: &mut TransactionBuilder,
        intent_args: ParamsArgs,
        cap_type: &str,
    ) -> Result<()> {
        let (
            mut multisig, 
            auth, 
            params, 
            outcome
        ) = self.prepare_request(builder, intent_args).await?;

        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "access_control_intents".parse().unwrap(),
                "request_borrow_cap".parse().unwrap(),
                vec![cap_type.parse().unwrap()],
            ),
            vec![auth.into(), multisig.borrow_mut().into(), params.into(), outcome.into()],
        );

        Ok(())
    }

    pub async fn execute_borrow_cap(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
        cap_type: &str,
    ) -> Result<(
        Arg<ap::account::Account<am::multisig::Multisig>>,
        Arg<ap::executable::Executable<am::multisig::Approvals>>,
        Argument, // Cap
    )> {
        let (
            mut multisig, 
            mut executable, 
            _is_last_execution, 
            _executions_count
        ) = self.prepare_execute(builder, intent_key).await?;

        let cap = builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "access_control_intents".parse().unwrap(),
                "execute_borrow_cap".parse().unwrap(),
                vec![cap_type.parse().unwrap()],
            ),
            vec![executable.borrow_mut().into(), multisig.borrow_mut().into()],
        );

        Ok((multisig, executable, cap))
    }
    
    // Use the Cap between borrow and return
    pub async fn execute_return_cap(
        &self,
        builder: &mut TransactionBuilder,
        mut multisig: Arg<ap::account::Account<am::multisig::Multisig>>,
        mut executable: Arg<ap::executable::Executable<am::multisig::Approvals>>,
        cap: Argument,
        intent_key: &str,
        cap_type: &str,
    ) -> Result<()> {
        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "access_control_intents".parse().unwrap(),
                "execute_return_cap".parse().unwrap(),
                vec![cap_type.parse().unwrap()],
            ),
            vec![executable.borrow_mut().into(), multisig.borrow_mut().into(), cap],
        );
        ap::account::confirm_execution(builder, multisig.borrow_mut(), executable);

        if self.intent(intent_key)?.execution_times.len() == 1 {
            let key_arg = self.key_arg(builder, intent_key)?;
            let mut expired = ap::account::destroy_empty_intent::<
                am::multisig::Multisig,
                am::multisig::Approvals,
            >(builder, multisig.borrow_mut(), key_arg);

            builder.move_call(
                sui_transaction_builder::Function::new(
                    ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                    "access_control".parse().unwrap(),
                    "delete_borrow".parse().unwrap(),
                    vec![cap_type.parse().unwrap()],
                ),
                vec![expired.borrow_mut().into()],
            );
            builder.move_call(
                sui_transaction_builder::Function::new(
                    ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                    "access_control".parse().unwrap(),
                    "delete_return".parse().unwrap(),
                    vec![cap_type.parse().unwrap()],
                ),
                vec![expired.borrow_mut().into()],
            );
            
            ap::intents::destroy_empty_expired(builder, expired);
        }

        Ok(())
    }

    pub async fn delete_borrow_cap(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
        cap_type: &str,
    ) -> Result<()> {
        let (
            _multisig, 
            mut expired, 
            _executions_count
        ) = self.prepare_delete(builder, intent_key).await?;

        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "access_control".parse().unwrap(),
                "delete_borrow".parse().unwrap(),
                vec![cap_type.parse().unwrap()],
            ),
            vec![expired.borrow_mut().into()],
        );
        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "access_control".parse().unwrap(),
                "delete_return".parse().unwrap(),
                vec![cap_type.parse().unwrap()],
            ),
            vec![expired.borrow_mut().into()],
        );
        ap::intents::destroy_empty_expired(builder, expired);

        Ok(())
    }

    pub async fn request_disable_rules(
        &self,
        builder: &mut TransactionBuilder,
        intent_args: ParamsArgs,
        actions_args: params::DisableRulesArgs,
        coin_type: &str,
    ) -> Result<()> {
        let (
            mut multisig, 
            auth, 
            params, 
            outcome
        ) = self.prepare_request(builder, intent_args).await?;

        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "currency_intents".parse().unwrap(),
                "request_disable_rules".parse().unwrap(),
                vec![coin_type.parse().unwrap()],
            ),
            vec![
                auth.into(), multisig.borrow_mut().into(), params.into(), outcome.into(),
                actions_args.mint.into(), actions_args.burn.into(), actions_args.update_symbol.into(), 
                actions_args.update_name.into(), actions_args.update_description.into(), actions_args.update_icon.into(),
            ],
        );

        Ok(())
    }

    pub async fn execute_disable_rules(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
        coin_type: &str,
    ) -> Result<()> {
        let (
            mut multisig, 
            mut executable, 
            is_last_execution, 
            _executions_count
        ) = self.prepare_execute(builder, intent_key).await?;

        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "currency_intents".parse().unwrap(),
                "execute_disable_rules".parse().unwrap(),
                vec![coin_type.parse().unwrap()],
            ),
            vec![executable.borrow_mut().into(), multisig.borrow_mut().into()],
        );
        ap::account::confirm_execution(builder, multisig.borrow_mut(), executable);

        if is_last_execution {
            let key = self.key_arg(builder, intent_key)?;
            let mut expired = ap::account::destroy_empty_intent::<
                am::multisig::Multisig,
                am::multisig::Approvals,
            >(builder, multisig.borrow_mut(), key);

            builder.move_call(
                sui_transaction_builder::Function::new(
                    ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                    "currency".parse().unwrap(),
                    "delete_disable".parse().unwrap(),
                    vec![coin_type.parse().unwrap()],
                ),
                vec![expired.borrow_mut().into()],
            );
            ap::intents::destroy_empty_expired(builder, expired);
        }

        Ok(())
    }

    pub async fn delete_disable_rules(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
        coin_type: &str,
    ) -> Result<()> {
        let (
            _multisig, 
            mut expired, 
            _executions_count
        ) = self.prepare_delete(builder, intent_key).await?;

        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "currency".parse().unwrap(),
                "delete_disable".parse().unwrap(),
                vec![coin_type.parse().unwrap()],
            ),
            vec![expired.borrow_mut().into()],
        );
        ap::intents::destroy_empty_expired(builder, expired);

        Ok(())
    }

    pub async fn request_update_metadata(
        &self,
        builder: &mut TransactionBuilder,
        intent_args: ParamsArgs,
        actions_args: params::UpdateMetadataArgs,
        coin_type: &str,
    ) -> Result<()> {
        let (
            mut multisig, 
            auth, 
            params, 
            outcome
        ) = self.prepare_request(builder, intent_args).await?;

        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "currency_intents".parse().unwrap(),
                "request_update_metadata".parse().unwrap(),
                vec![coin_type.parse().unwrap()],
            ),
            vec![
                auth.into(), multisig.borrow_mut().into(), params.into(), outcome.into(),
                actions_args.symbol.into(), actions_args.name.into(), actions_args.description.into(), actions_args.icon_url.into(),
            ],
        );

        Ok(())
    }

    pub async fn execute_update_metadata(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
        coin_type: &str,
    ) -> Result<()> {
        let (
            mut multisig, 
            mut executable, 
            is_last_execution, 
            _executions_count
        ) = self.prepare_execute(builder, intent_key).await?;

        let coin_metadata_object = utils::coin_metadata(self.sui(), coin_type)
            .await?
            .ok_or(anyhow!("Coin metadata object not found"))?;
        let coin_metadata = self.shared_mut_argument(builder, coin_metadata_object.address).await?;
        
        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "currency_intents".parse().unwrap(),
                "execute_update_metadata".parse().unwrap(),
                vec![coin_type.parse().unwrap()],
            ),
            vec![executable.borrow_mut().into(), multisig.borrow_mut().into(), coin_metadata],
        );
        ap::account::confirm_execution(builder, multisig.borrow_mut(), executable);

        if is_last_execution {
            let key = self.key_arg(builder, intent_key)?;
            let mut expired = ap::account::destroy_empty_intent::<
                am::multisig::Multisig,
                am::multisig::Approvals,
            >(builder, multisig.borrow_mut(), key);

            builder.move_call(
                sui_transaction_builder::Function::new(
                    ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                    "currency".parse().unwrap(),
                    "delete_update".parse().unwrap(),
                    vec![coin_type.parse().unwrap()],
                ),
                vec![expired.borrow_mut().into()],
            );
            ap::intents::destroy_empty_expired(builder, expired);
        }

        Ok(())
    }

    pub async fn delete_update_metadata(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
        coin_type: &str,
    ) -> Result<()> {
        let (
            _multisig, 
            mut expired, 
            _executions_count
        ) = self.prepare_delete(builder, intent_key).await?;

        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "currency".parse().unwrap(),
                "delete_update".parse().unwrap(),
                vec![coin_type.parse().unwrap()],
            ),
            vec![expired.borrow_mut().into()],
        );
        ap::intents::destroy_empty_expired(builder, expired);

        Ok(())
    }

    pub async fn request_mint_and_transfer(
        &self,
        builder: &mut TransactionBuilder,
        intent_args: ParamsArgs,
        actions_args: params::MintAndTransferArgs,
        coin_type: &str,
    ) -> Result<()> {
        let (
            mut multisig, 
            auth, 
            params, 
            outcome
        ) = self.prepare_request(builder, intent_args).await?;

        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "currency_intents".parse().unwrap(),
                "request_mint_and_transfer".parse().unwrap(),
                vec![coin_type.parse().unwrap()],
            ),
            vec![
                auth.into(), multisig.borrow_mut().into(), params.into(), outcome.into(),
                actions_args.amounts.into(), actions_args.recipients.into(),
            ],
        );

        Ok(())
    }

    pub async fn execute_mint_and_transfer(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
        coin_type: &str,
    ) -> Result<()> {
        let (
            mut multisig, 
            mut executable, 
            is_last_execution, 
            executions_count
        ) = self.prepare_execute(builder, intent_key).await?;

        for _ in 0..executions_count {
            builder.move_call(
                sui_transaction_builder::Function::new(
                    ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                    "currency_intents".parse().unwrap(),
                    "execute_mint_and_transfer".parse().unwrap(),
                    vec![coin_type.parse().unwrap()],
                ),
                vec![executable.borrow_mut().into(), multisig.borrow_mut().into()],
            );
        }
        ap::account::confirm_execution(builder, multisig.borrow_mut(), executable);

        if is_last_execution {
            let key = self.key_arg(builder, intent_key)?;
            let mut expired = ap::account::destroy_empty_intent::<
                am::multisig::Multisig,
                am::multisig::Approvals,
            >(builder, multisig.borrow_mut(), key);

            for _ in 0..executions_count {
                builder.move_call(
                    sui_transaction_builder::Function::new(
                        ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                        "currency".parse().unwrap(),
                        "delete_mint".parse().unwrap(),
                        vec![coin_type.parse().unwrap()],
                    ),
                    vec![expired.borrow_mut().into()],
                );
                aa::transfer::delete_transfer(builder, expired.borrow_mut());
            }
            ap::intents::destroy_empty_expired(builder, expired);
        }

        Ok(())
    }

    pub async fn delete_mint_and_transfer(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
        coin_type: &str,
    ) -> Result<()> {
        let (
            _multisig, 
            mut expired, 
            executions_count
        ) = self.prepare_delete(builder, intent_key).await?;

        for _ in 0..executions_count {
            builder.move_call(
                sui_transaction_builder::Function::new(
                    ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                    "currency".parse().unwrap(),
                    "delete_mint".parse().unwrap(),
                    vec![coin_type.parse().unwrap()],
                ),
                vec![expired.borrow_mut().into()],
            );
            aa::transfer::delete_transfer(builder, expired.borrow_mut());
        }
        ap::intents::destroy_empty_expired(builder, expired);

        Ok(())
    }

    pub async fn request_mint_and_vest(
        &self,
        builder: &mut TransactionBuilder,
        intent_args: ParamsArgs,
        actions_args: params::MintAndVestArgs,
        coin_type: &str,
    ) -> Result<()> {
        let (
            mut multisig, 
            auth, 
            params, 
            outcome
        ) = self.prepare_request(builder, intent_args).await?;

        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "currency_intents".parse().unwrap(),
                "request_mint_and_vest".parse().unwrap(),
                vec![coin_type.parse().unwrap()],
            ),
            vec![
                auth.into(), multisig.borrow_mut().into(), params.into(), outcome.into(),
                actions_args.total_amount.into(), actions_args.start_timestamp.into(), 
                actions_args.end_timestamp.into(), actions_args.recipient.into(),
            ],
        );

        Ok(())
    }

    pub async fn execute_mint_and_vest(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
        coin_type: &str,
    ) -> Result<()> {
        let (
            mut multisig, 
            mut executable, 
            is_last_execution, 
            _executions_count
        ) = self.prepare_execute(builder, intent_key).await?;

        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "currency_intents".parse().unwrap(),
                "execute_mint_and_vest".parse().unwrap(),
                vec![coin_type.parse().unwrap()],
            ),
            vec![executable.borrow_mut().into(), multisig.borrow_mut().into()],
        );
        ap::account::confirm_execution(builder, multisig.borrow_mut(), executable);

        if is_last_execution {
            let key = self.key_arg(builder, intent_key)?;
            let mut expired = ap::account::destroy_empty_intent::<
                am::multisig::Multisig,
                am::multisig::Approvals,
            >(builder, multisig.borrow_mut(), key);

            builder.move_call(
                sui_transaction_builder::Function::new(
                    ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                    "currency".parse().unwrap(),
                    "delete_mint".parse().unwrap(),
                    vec![coin_type.parse().unwrap()],
                ),
                vec![expired.borrow_mut().into()],
            );
            aa::vesting::delete_vest(builder, expired.borrow_mut());
            ap::intents::destroy_empty_expired(builder, expired);
        }

        Ok(())
    }

    pub async fn delete_mint_and_vest(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
        coin_type: &str,
    ) -> Result<()> {
        let (
            _multisig, 
            mut expired, 
            _executions_count
        ) = self.prepare_delete(builder, intent_key).await?;

        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "currency".parse().unwrap(),
                "delete_mint".parse().unwrap(),
                vec![coin_type.parse().unwrap()],
            ),
            vec![expired.borrow_mut().into()],
        );
        aa::vesting::delete_vest(builder, expired.borrow_mut());
        ap::intents::destroy_empty_expired(builder, expired);

        Ok(())
    }

    pub async fn request_withdraw_and_burn(
        &self,
        builder: &mut TransactionBuilder,
        intent_args: ParamsArgs,
        actions_args: params::WithdrawAndBurnArgs,
        coin_type: &str,
    ) -> Result<()> {
        let (
            mut multisig, 
            auth, 
            params, 
            outcome
        ) = self.prepare_request(builder, intent_args).await?;

        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "currency_intents".parse().unwrap(),
                "request_withdraw_and_burn".parse().unwrap(),
                vec![coin_type.parse().unwrap()],
            ),
            vec![
                auth.into(), multisig.borrow_mut().into(), params.into(), outcome.into(),
                actions_args.coin_id.into(), actions_args.amount.into(),
            ],
        );

        Ok(())
    }

    pub async fn execute_withdraw_and_burn(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
        coin_type: &str,
    ) -> Result<()> {
        let (
            mut multisig, 
            mut executable, 
            is_last_execution, 
            _executions_count
        ) = self.prepare_execute(builder, intent_key).await?;

        let actions_args = self.intent_mut(intent_key)?.get_actions_args().await?;
        let coin_id = match actions_args {
            IntentActions::WithdrawAndBurn(actions_args) => actions_args.coin_id,
            _ => return Err(anyhow!("Intent {} is not a WithdrawAndBurn intent", intent_key)),
        };

        let receive_coin = self.receive_argument(builder, coin_id).await?;
        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "currency_intents".parse().unwrap(),
                "execute_withdraw_and_burn".parse().unwrap(),
                vec![coin_type.parse().unwrap()],
            ),
            vec![executable.borrow_mut().into(), multisig.borrow_mut().into(), receive_coin],
        );
        ap::account::confirm_execution(builder, multisig.borrow_mut(), executable);

        if is_last_execution {
            let key = self.key_arg(builder, intent_key)?;
            let mut expired = ap::account::destroy_empty_intent::<
                am::multisig::Multisig,
                am::multisig::Approvals,
            >(builder, multisig.borrow_mut(), key);

            ap::owned::delete_withdraw(builder, expired.borrow_mut(), multisig.borrow_mut());
            builder.move_call(
                sui_transaction_builder::Function::new(
                    ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                    "currency".parse().unwrap(),
                    "delete_burn".parse().unwrap(),
                    vec![coin_type.parse().unwrap()],
                ),
                vec![expired.borrow_mut().into()],
            );
            ap::intents::destroy_empty_expired(builder, expired);
        }

        Ok(())
    }

    pub async fn delete_withdraw_and_burn(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
        coin_type: &str,
    ) -> Result<()> {
        let (
            mut multisig, 
            mut expired, 
            _executions_count
        ) = self.prepare_delete(builder, intent_key).await?;

        ap::owned::delete_withdraw(builder, expired.borrow_mut(), multisig.borrow_mut());
        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "currency".parse().unwrap(),
                "delete_burn".parse().unwrap(),
                vec![coin_type.parse().unwrap()],
            ),
            vec![expired.borrow_mut().into()],
        );
        ap::intents::destroy_empty_expired(builder, expired);

        Ok(())
    }

    pub async fn request_withdraw_and_transfer_to_vault(
        &self,
        builder: &mut TransactionBuilder,
        intent_args: ParamsArgs,
        actions_args: params::WithdrawAndTransferToVaultArgs,
        coin_type: &str,
    ) -> Result<()> {
        let (
            mut multisig, 
            auth, 
            params, 
            outcome
        ) = self.prepare_request(builder, intent_args).await?;

        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "currency_intents".parse().unwrap(),
                "request_withdraw_and_transfer_to_vault".parse().unwrap(),
                vec![coin_type.parse().unwrap()],
            ),
            vec![
                auth.into(), multisig.borrow_mut().into(), params.into(), outcome.into(),
                actions_args.coin_id.into(), actions_args.coin_amount.into(), actions_args.vault_name.into(),
            ],
        );

        Ok(())
    }

    pub async fn execute_withdraw_and_transfer_to_vault(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
        coin_type: &str,
    ) -> Result<()> {
        let (
            mut multisig, 
            mut executable, 
            is_last_execution, 
            _executions_count
        ) = self.prepare_execute(builder, intent_key).await?;

        let actions_args = self.intent_mut(intent_key)?.get_actions_args().await?;
        let coin_id = match actions_args {
            IntentActions::WithdrawAndTransferToVault(actions_args) => actions_args.coin_id,
            _ => return Err(anyhow!("Intent {} is not a WithdrawAndTransferToVault intent", intent_key)),
        };

        let receive_coin = self.receive_argument(builder, coin_id).await?;
        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "currency_intents".parse().unwrap(),
                "execute_withdraw_and_transfer_to_vault".parse().unwrap(),
                vec![coin_type.parse().unwrap()],
            ),
            vec![executable.borrow_mut().into(), multisig.borrow_mut().into(), receive_coin],
        );
        ap::account::confirm_execution(builder, multisig.borrow_mut(), executable);

        if is_last_execution {
            let key = self.key_arg(builder, intent_key)?;
            let mut expired = ap::account::destroy_empty_intent::<
                am::multisig::Multisig,
                am::multisig::Approvals,
            >(builder, multisig.borrow_mut(), key);

            ap::owned::delete_withdraw(builder, expired.borrow_mut(), multisig.borrow_mut());
            builder.move_call(
                sui_transaction_builder::Function::new(
                    ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                    "vault".parse().unwrap(),
                    "delete_deposit".parse().unwrap(),
                    vec![coin_type.parse().unwrap()],
                ),
                vec![expired.borrow_mut().into()],
            );
            ap::intents::destroy_empty_expired(builder, expired);
        }

        Ok(())
    }

    pub async fn delete_withdraw_and_transfer_to_vault(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
        coin_type: &str,
    ) -> Result<()> {
        let (
            mut multisig, 
            mut expired, 
            _executions_count
        ) = self.prepare_delete(builder, intent_key).await?;

        ap::owned::delete_withdraw(builder, expired.borrow_mut(), multisig.borrow_mut());
        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "vault".parse().unwrap(),
                "delete_deposit".parse().unwrap(),
                vec![coin_type.parse().unwrap()],
            ),
            vec![expired.borrow_mut().into()],
        );
        ap::intents::destroy_empty_expired(builder, expired);

        Ok(())
    }

    pub async fn request_withdraw_and_transfer(
        &self,
        builder: &mut TransactionBuilder,
        intent_args: ParamsArgs,
        actions_args: params::WithdrawAndTransferArgs,
    ) -> Result<()> {
        let (
            mut multisig, 
            auth, 
            params, 
            outcome
        ) = self.prepare_request(builder, intent_args).await?;

        aa::owned_intents::request_withdraw_and_transfer(
            builder, auth, multisig.borrow_mut(), params, outcome, 
            actions_args.object_ids, actions_args.recipients,
        );

        Ok(())
    }

    pub async fn execute_withdraw_and_transfer(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
    ) -> Result<()> {
        let (
            mut multisig, 
            mut executable, 
            is_last_execution, 
            executions_count
        ) = self.prepare_execute(builder, intent_key).await?;

        let actions_args = self.intent_mut(intent_key)?.get_actions_args().await?;
        let transfers = match actions_args {
            IntentActions::WithdrawAndTransfer(actions_args) => actions_args.transfers.clone(),
            _ => return Err(anyhow!("Intent {} is not a WithdrawAndTransfer intent", intent_key)),
        };

        for (id, _recipient) in transfers {
            let receive_id = builder.input(self.obj(id).await?.with_receiving_kind());
            let obj_type = self
                .owned_objects()
                .and_then(|o| o.get_type_by_id(id))
                .ok_or(anyhow!("Object type not found"))?;

            builder.move_call(
                Function::new(
                    ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                    "owned_intents".parse().unwrap(),
                    "execute_withdraw_and_transfer".parse().unwrap(),
                    vec![obj_type.parse().unwrap()],
                ),
                vec![executable.borrow_mut().into(), multisig.borrow_mut().into(), receive_id],
            );
        }
        ap::account::confirm_execution(builder, multisig.borrow_mut(), executable);

        if is_last_execution {
            let key = self.key_arg(builder, intent_key)?;
            let mut expired = ap::account::destroy_empty_intent::<
                am::multisig::Multisig,
                am::multisig::Approvals,
            >(builder, multisig.borrow_mut(), key);

            for _ in 0..executions_count {
                ap::owned::delete_withdraw(builder, expired.borrow_mut(), multisig.borrow_mut());
                aa::transfer::delete_transfer(builder, expired.borrow_mut());
            }
            ap::intents::destroy_empty_expired(builder, expired);
        }

        Ok(())
    }

    pub async fn delete_withdraw_and_transfer(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
    ) -> Result<()> {
        let (
            mut multisig, 
            mut expired, 
            _executions_count
        ) = self.prepare_delete(builder, intent_key).await?;

        ap::owned::delete_withdraw(builder, expired.borrow_mut(), multisig.borrow_mut());
        aa::transfer::delete_transfer(builder, expired.borrow_mut());
        ap::intents::destroy_empty_expired(builder, expired);

        Ok(())
    }

    pub async fn request_withdraw_and_vest(
        &self,
        builder: &mut TransactionBuilder,
        intent_args: ParamsArgs,
        actions_args: params::WithdrawAndVestArgs,
    ) -> Result<()> {
        let (
            mut multisig, 
            auth, 
            params, 
            outcome
        ) = self.prepare_request(builder, intent_args).await?;

        aa::owned_intents::request_withdraw_and_vest(
            builder,
            auth, multisig.borrow_mut(), params, outcome, 
            actions_args.coin_id, actions_args.start_timestamp, actions_args.end_timestamp, actions_args.recipient,
        );

        Ok(())
    }

    pub async fn execute_withdraw_and_vest(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
        coin_type: &str,
    ) -> Result<()> {
        let (
            mut multisig, 
            mut executable, 
            is_last_execution, 
            _executions_count
        ) = self.prepare_execute(builder, intent_key).await?;

        let actions_args = self.intent_mut(intent_key)?.get_actions_args().await?;
        let coin_id = match actions_args {
            IntentActions::WithdrawAndVest(actions_args) => actions_args.coin_id,
            _ => return Err(anyhow!("Intent {} is not a WithdrawAndTransfer intent", intent_key)),
        };
        let receive_id = self.receive_argument(builder, coin_id).await?;
        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "owned_intents".parse().unwrap(),
                "execute_withdraw_and_vest".parse().unwrap(),
                vec![format!("0x2::coin::Coin<{}>", coin_type).parse().unwrap()],
            ),
            vec![executable.borrow_mut().into(), multisig.borrow_mut().into(), receive_id],
        );
        ap::account::confirm_execution(builder, multisig.borrow_mut(), executable);

        if is_last_execution {
            let key = self.key_arg(builder, intent_key)?;
            let mut expired = ap::account::destroy_empty_intent::<
                am::multisig::Multisig,
                am::multisig::Approvals,
            >(builder, multisig.borrow_mut(), key);

            ap::owned::delete_withdraw(builder, expired.borrow_mut(), multisig.borrow_mut());
            aa::vesting::delete_vest(builder, expired.borrow_mut());
            ap::intents::destroy_empty_expired(builder, expired);
        }

        Ok(())
    }

    pub async fn delete_withdraw_and_vest(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
    ) -> Result<()> {
        let (
            mut multisig, 
            mut expired, 
            _executions_count
        ) = self.prepare_delete(builder, intent_key).await?;

        ap::owned::delete_withdraw(builder, expired.borrow_mut(), multisig.borrow_mut());
        aa::vesting::delete_vest(builder, expired.borrow_mut());
        ap::intents::destroy_empty_expired(builder, expired);

        Ok(())
    }

    pub async fn request_upgrade_package(
        &self,
        builder: &mut TransactionBuilder,
        intent_args: ParamsArgs,
        actions_args: params::UpgradePackageArgs,
    ) -> Result<()> {
        let (
            mut multisig, 
            auth, 
            params, 
            outcome
        ) = self.prepare_request(builder, intent_args).await?;

        aa::package_upgrade_intents::request_upgrade_package(
            builder,auth,multisig.borrow_mut(),params,outcome,
            actions_args.package_name,actions_args.digest,
        );

        Ok(())
    }

    pub async fn execute_upgrade_package(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
        package_id: ObjectId,
        modules: Vec<Vec<u8>>,
        dependencies: Vec<ObjectId>,
    ) -> Result<()> {
        let mut multisig = self.multisig_arg(builder).await?;
        let clock = self.clock_arg(builder).await?;
        let key = self.key_arg(builder, intent_key)?;
        
        let intent = self.intent(intent_key)?;
        let current_timestamp = self.clock_timestamp().await?;
        if current_timestamp < *intent.execution_times.first().unwrap() {
            return Err(anyhow!("Intent cannot be executed"));
        }
        
        let mut executable = am::multisig::execute_intent(
            builder, multisig.borrow_mut(), key, clock.borrow(),
        );

        let ticket = aa::package_upgrade_intents::execute_upgrade_package(
            builder, executable.borrow_mut(), multisig.borrow_mut(), clock.borrow()
        );
        let receipt = builder.upgrade(modules, dependencies, package_id, ticket.into());
        aa::package_upgrade_intents::execute_commit_upgrade(
            builder, executable.borrow_mut(), multisig.borrow_mut(), receipt.into()
        );
        ap::account::confirm_execution(builder, multisig.borrow_mut(), executable);

        if intent.execution_times.len() == 1 {
            let key = self.key_arg(builder, intent_key)?;
            let mut expired = ap::account::destroy_empty_intent::<
                am::multisig::Multisig,
                am::multisig::Approvals,
            >(builder, multisig.borrow_mut(), key);

            ap::owned::delete_withdraw(builder, expired.borrow_mut(), multisig.borrow_mut());
            aa::vesting::delete_vest(builder, expired.borrow_mut());
            ap::intents::destroy_empty_expired(builder, expired);
        }

        Ok(())
    }

    pub async fn delete_upgrade_package(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
    ) -> Result<()> {
        let (
            _multisig, 
            mut expired, 
            _executions_count
        ) = self.prepare_delete(builder, intent_key).await?;

        aa::package_upgrade::delete_upgrade(builder, expired.borrow_mut());
        aa::package_upgrade::delete_commit(builder, expired.borrow_mut());
        ap::intents::destroy_empty_expired(builder, expired);

        Ok(())
    }

    pub async fn request_restrict_policy(
        &self,
        builder: &mut TransactionBuilder,
        intent_args: ParamsArgs,
        actions_args: params::RestrictPolicyArgs,
    ) -> Result<()> {
        let (
            mut multisig, 
            auth, 
            params, 
            outcome
        ) = self.prepare_request(builder, intent_args).await?;

        aa::package_upgrade_intents::request_restrict_policy(
            builder,auth,multisig.borrow_mut(),params,outcome,
            actions_args.package_name,actions_args.policy,
        );

        Ok(())
    }

    pub async fn execute_restrict_policy(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
    ) -> Result<()> {
        let mut multisig = self.multisig_arg(builder).await?;
        let clock = self.clock_arg(builder).await?;
        let key = self.key_arg(builder, intent_key)?;
        
        let intent = self.intent(intent_key)?;
        let current_timestamp = self.clock_timestamp().await?;
        if current_timestamp < *intent.execution_times.first().unwrap() {
            return Err(anyhow!("Intent cannot be executed"));
        }
        
        let mut executable = am::multisig::execute_intent(
            builder, multisig.borrow_mut(), key, clock.borrow(),
        );

        aa::package_upgrade_intents::execute_restrict_policy(
            builder, executable.borrow_mut(), multisig.borrow_mut()
        );
        ap::account::confirm_execution(builder, multisig.borrow_mut(), executable);

        if intent.execution_times.len() == 1 {
            let key = self.key_arg(builder, intent_key)?;
            let mut expired = ap::account::destroy_empty_intent::<
                am::multisig::Multisig,
                am::multisig::Approvals,
            >(builder, multisig.borrow_mut(), key);

            ap::owned::delete_withdraw(builder, expired.borrow_mut(), multisig.borrow_mut());
            aa::vesting::delete_vest(builder, expired.borrow_mut());
            ap::intents::destroy_empty_expired(builder, expired);
        }

        Ok(())
    }

    pub async fn delete_restrict_policy(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
    ) -> Result<()> {
        let (
            _multisig, 
            mut expired, 
            _executions_count
        ) = self.prepare_delete(builder, intent_key).await?;

        aa::package_upgrade::delete_restrict(builder, expired.borrow_mut());
        ap::intents::destroy_empty_expired(builder, expired);

        Ok(())
    }

    pub async fn request_spend_and_transfer(
        &self,
        builder: &mut TransactionBuilder,
        intent_args: ParamsArgs,
        actions_args: params::SpendAndTransferArgs,
        coin_type: &str,
    ) -> Result<()> {
        let (
            mut multisig, 
            auth, 
            params, 
            outcome
        ) = self.prepare_request(builder, intent_args).await?;

        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "vault_intents".parse().unwrap(),
                "request_spend_and_transfer".parse().unwrap(),
                vec![coin_type.parse().unwrap()],
            ),
            vec![
                auth.into(), multisig.borrow_mut().into(), params.into(), outcome.into(),
            actions_args.vault_name.into(), actions_args.amounts.into(), actions_args.recipients.into(),
            ],
        );

        Ok(())
    }

    pub async fn execute_spend_and_transfer(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
        coin_type: &str,
    ) -> Result<()> {
        let (
            mut multisig, 
            mut executable, 
            is_last_execution, 
            executions_count
        ) = self.prepare_execute(builder, intent_key).await?;

        for _ in 0..executions_count {
            builder.move_call(
                sui_transaction_builder::Function::new(
                    ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                    "vault_intents".parse().unwrap(),
                    "execute_spend_and_transfer".parse().unwrap(),
                    vec![coin_type.parse().unwrap()],
                ),
                vec![executable.borrow_mut().into(), multisig.borrow_mut().into()],
            );
        }
        ap::account::confirm_execution(builder, multisig.borrow_mut(), executable);

        if is_last_execution {
            let key = self.key_arg(builder, intent_key)?;
            let mut expired = ap::account::destroy_empty_intent::<
                am::multisig::Multisig,
                am::multisig::Approvals,
            >(builder, multisig.borrow_mut(), key);

            for _ in 0..executions_count {
                builder.move_call(
                    sui_transaction_builder::Function::new(
                        ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                        "vault".parse().unwrap(),
                        "delete_spend".parse().unwrap(),
                        vec![coin_type.parse().unwrap()],
                    ),
                    vec![expired.borrow_mut().into()],
                );
                aa::transfer::delete_transfer(builder, expired.borrow_mut());
            }
            ap::intents::destroy_empty_expired(builder, expired);
        }

        Ok(())
    }

    pub async fn delete_spend_and_transfer(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
        coin_type: &str,
    ) -> Result<()> {
        let (
            _multisig, 
            mut expired, 
            executions_count
        ) = self.prepare_delete(builder, intent_key).await?;

        for _ in 0..executions_count {
            builder.move_call(
                sui_transaction_builder::Function::new(
                    ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                    "vault".parse().unwrap(),
                    "delete_spend".parse().unwrap(),
                    vec![coin_type.parse().unwrap()],
                ),
                vec![expired.borrow_mut().into()],
            );
            aa::transfer::delete_transfer(builder, expired.borrow_mut());
        }
        ap::intents::destroy_empty_expired(builder, expired);

        Ok(())
    }

    pub async fn request_spend_and_vest(
        &self,
        builder: &mut TransactionBuilder,
        intent_args: ParamsArgs,
        actions_args: params::SpendAndVestArgs,
        coin_type: &str,
    ) -> Result<()> {
        let (
            mut multisig, 
            auth, 
            params, 
            outcome
        ) = self.prepare_request(builder, intent_args).await?;

        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "vault_intents".parse().unwrap(),
                "request_spend_and_vest".parse().unwrap(),
                vec![coin_type.parse().unwrap()],
            ),
            vec![
                auth.into(), multisig.borrow_mut().into(), params.into(), outcome.into(),
                actions_args.vault_name.into(), actions_args.coin_amount.into(), 
                actions_args.start_timestamp.into(), actions_args.end_timestamp.into(), actions_args.recipient.into(),
            ],
        );

        Ok(())
    }

    pub async fn execute_spend_and_vest(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
        coin_type: &str,
    ) -> Result<()> {
        let (
            mut multisig, 
            mut executable, 
            is_last_execution, 
            _executions_count
        ) = self.prepare_execute(builder, intent_key).await?;

        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "vault_intents".parse().unwrap(),
                "execute_spend_and_vest".parse().unwrap(),
                vec![coin_type.parse().unwrap()],
            ),
            vec![executable.borrow_mut().into(), multisig.borrow_mut().into()],
        );
        ap::account::confirm_execution(builder, multisig.borrow_mut(), executable);

        if is_last_execution {
            let key = self.key_arg(builder, intent_key)?;
            let mut expired = ap::account::destroy_empty_intent::<
                am::multisig::Multisig,
                am::multisig::Approvals,
            >(builder, multisig.borrow_mut(), key);

            builder.move_call(
                sui_transaction_builder::Function::new(
                    ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                    "vault".parse().unwrap(),
                    "delete_spend".parse().unwrap(),
                    vec![coin_type.parse().unwrap()],
                ),
                vec![expired.borrow_mut().into()],
            );
            aa::vesting::delete_vest(builder, expired.borrow_mut());
            ap::intents::destroy_empty_expired(builder, expired);
        }

        Ok(())
    }

    pub async fn delete_spend_and_vest(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
        coin_type: &str,
    ) -> Result<()> {
        let (
            _multisig, 
            mut expired, 
            _executions_count
        ) = self.prepare_delete(builder, intent_key).await?;

        builder.move_call(
            sui_transaction_builder::Function::new(
                ACCOUNT_ACTIONS_PACKAGE.parse().unwrap(),
                "vault".parse().unwrap(),
                "delete_spend".parse().unwrap(),
                vec![coin_type.parse().unwrap()],
            ),
            vec![expired.borrow_mut().into()],
        );
        aa::vesting::delete_vest(builder, expired.borrow_mut());
        ap::intents::destroy_empty_expired(builder, expired);

        Ok(())
    }

    // === Getters ===

    pub fn sui(&self) -> &Client {
        &self.sui_client
    }

    pub fn user(&self) -> Option<&User> {
        self.user.as_ref()
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

    pub fn intents_mut(&mut self) -> Option<&mut Intents> {
        self.multisig.as_mut()?.intents.as_mut()
    }

    pub fn intent(&self, key: &str) -> Result<&Intent> {
        self.intents().and_then(|i| i.get_intent(key)).ok_or(anyhow!("Intent not found"))
    }

    pub fn intent_mut(&mut self, key: &str) -> Result<&mut Intent> {
        self.intents_mut().and_then(|i| i.get_intent_mut(key)).ok_or(anyhow!("Intent not found"))
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

    pub async fn clock_timestamp(&self) -> Result<u64> {
        let clock_object = utils::get_object(&self.sui_client, CLOCK_OBJECT.parse().unwrap()).await?;
        if let ObjectData::Struct(obj) = clock_object.data() {
            let clock: sui::clock::Clock = bcs::from_bytes(obj.contents())
                .map_err(|e| anyhow!("Failed to parse clock object: {}", e))?;   
            Ok(clock.timestamp_ms)
        } else {
            Err(anyhow!("Clock object data is missing"))
        }
    }

    pub fn pure_arg<Pure: serde::Serialize + MoveType>(
        &self,
        builder: &mut TransactionBuilder,
        value: Pure,
    ) -> Result<Arg<Pure>> {
        let value_arg = builder.input(Serialized(&value)).into();
        Ok(value_arg)
    }

    pub async fn owned_arg<Obj: MoveType + Key>(
        &self,
        builder: &mut TransactionBuilder,
        id: Address,
    ) -> Result<Arg<Obj>> {
        let object_input = self.obj(id).await?;
        let object_arg = builder.input(object_input).into();
        Ok(object_arg)
    }

    pub async fn owned_argument(
        &self,
        builder: &mut TransactionBuilder,
        id: Address,
    ) -> Result<Argument> {
        let object_input = self.obj(id).await?;
        let object_arg = builder.input(object_input);
        Ok(object_arg)
    }

    pub async fn receive_argument(
        &self,
        builder: &mut TransactionBuilder,
        id: Address,
    ) -> Result<Argument> {
        let object_input = self.obj(id).await?;
        let object_arg = builder.input(object_input.with_receiving_kind());
        Ok(object_arg)
    }

    pub async fn shared_mut_argument(
        &self,
        builder: &mut TransactionBuilder,
        id: Address,
    ) -> Result<Argument> {
        let object_input = self.obj(id).await?;
        let object_arg = builder.input(object_input.by_mut());
        Ok(object_arg)
    }

    pub fn key_arg(
        &self,
        builder: &mut TransactionBuilder,
        key: &str,
    ) -> Result<Arg<std::string::String>> {
        let as_owned = key.to_owned();
        let key_arg = builder.input(Serialized(&as_owned)).into();
        Ok(key_arg)
    }

    pub async fn clock_arg(&self, builder: &mut TransactionBuilder) -> Result<Arg<sui::clock::Clock>> {
        let clock_input = self.obj(CLOCK_OBJECT.parse().unwrap()).await?;
        let clock = builder.input(clock_input.by_ref()).into();
        Ok(clock)
    }

    pub async fn extensions_arg(
        &self,
        builder: &mut TransactionBuilder,
    ) -> Result<Arg<ae::extensions::Extensions>> {
        let extensions_input = self.obj(EXTENSIONS_OBJECT.parse().unwrap()).await?;
        let extensions = builder.input(extensions_input.by_ref()).into();
        Ok(extensions)
    }

    pub async fn multisig_arg(
        &self,
        builder: &mut TransactionBuilder,
    ) -> Result<Arg<ap::account::Account<am::multisig::Multisig>>> {
        let ms_input = self.obj(self.multisig_id()?).await?;
        let multisig = builder.input(ms_input.by_mut()).into();
        Ok(multisig)
    }

    pub async fn prepare_request(
        &self,
        builder: &mut TransactionBuilder,
        params_args: ParamsArgs,
    ) -> Result<(
        Arg<ap::account::Account<am::multisig::Multisig>>,
        Arg<ap::account::Auth>,
        Arg<ap::intents::Params>,
        Arg<am::multisig::Approvals>,
    )> {
        let multisig = self.multisig_arg(builder).await?;
        let clock = self.clock_arg(builder).await?;

        let auth = am::multisig::authenticate(builder, multisig.borrow());
        let params = ap::intents::new_params(
            builder,
            params_args.key,
            params_args.description,
            params_args.execution_times,
            params_args.expiration_time,
            clock.borrow(),
        );
        let outcome = am::multisig::empty_outcome(builder);
        
        Ok((multisig, auth, params, outcome))
    }

    pub async fn prepare_execute(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
    ) -> Result<(
        Arg<ap::account::Account<am::multisig::Multisig>>,
        Arg<ap::executable::Executable<am::multisig::Approvals>>,
        bool,
        usize,
    )> {
        let mut multisig = self.multisig_arg(builder).await?;
        let clock = self.clock_arg(builder).await?;
        let key = self.key_arg(builder, intent_key)?;
        
        let executions_count = self.intent_mut(intent_key)?.get_executions_count().await?;
        
        let intent = self.intent(intent_key)?;
        let current_timestamp = self.clock_timestamp().await?;
        if current_timestamp < *intent.execution_times.first().unwrap() {
            return Err(anyhow!("Intent cannot be executed"));
        }
        let is_last_execution = intent.execution_times.len() == 1;
        
        let executable = am::multisig::execute_intent(
            builder,
            multisig.borrow_mut(),
            key,
            clock.borrow(),
        );

        Ok((multisig, executable, is_last_execution, executions_count))
    }

    pub async fn prepare_delete(
        &mut self,
        builder: &mut TransactionBuilder,
        intent_key: &str,
    ) -> Result<(
        Arg<ap::account::Account<am::multisig::Multisig>>,
        Arg<ap::intents::Expired>,
        usize,
    )> {
        let mut multisig = self.multisig_arg(builder).await?;
        let clock = self.clock_arg(builder).await?;
        let key = self.key_arg(builder, intent_key)?;

        let current_timestamp = self.clock_timestamp().await?;
        let intent = self.intent_mut(intent_key)?;
        
        let expired = if current_timestamp > intent.expiration_time {
            ap::account::delete_expired_intent::<
                am::multisig::Multisig,
                am::multisig::Approvals,
            >(builder, multisig.borrow_mut(), key, clock.borrow())
        } else if intent.execution_times.is_empty() {
            ap::account::destroy_empty_intent::<
                am::multisig::Multisig,
                am::multisig::Approvals,
            >(builder, multisig.borrow_mut(), key)
        } else {
            return Err(anyhow!("Intent cannot be deleted"));
        };

        let executions_count = intent.get_executions_count().await?;

        Ok((multisig, expired, executions_count))
    }
}

impl fmt::Debug for MultisigClient {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("MultisigClient")
            .field("user", &self.user)
            .field("multisig", &self.multisig)
            .finish()
    }
}

// #[macro_export]
// macro_rules! define_move_type {
//     (
//         $move_type:ident,
//         $full_path:expr $(,)?
//     ) => {
//         use move_types::{MoveType, StructTag, TypeTag};

//         #[derive(serde::Serialize, serde::Deserialize)]
//         pub struct $move_type {}

//         impl MoveType for $move_type {
//             fn type_() -> TypeTag {
//                 let parts: Vec<&str> = $full_path.split("::").collect();
//                 if parts.len() != 3 {
//                     panic!(
//                         "Invalid coin type path: {}. Expected format: address::module::name",
//                         $full_path
//                     );
//                 }

//                 let address = parts[0];
//                 let module = parts[1];
//                 let name = parts[2];

//                 TypeTag::Struct(Box::new(StructTag {
//                     address: address.parse().unwrap(),
//                     module: module.parse().unwrap(),
//                     name: name.parse().unwrap(),
//                     type_params: vec![],
//                 }))
//             }
//         }
//     };
// }

// #[macro_export]
// macro_rules! define_move_object {
//     (
//         $move_object_name:ident, 
//         $id:expr, 
//         $full_path:expr $(,)?
//     ) => {
//         use move_types::{Key, MoveStruct, MoveType, ObjectId, StructTag, TypeTag};

//         #[derive(serde::Serialize, serde::Deserialize)]
//         pub struct $move_object_name {
//             pub id: ObjectId,
//         }

//         impl MoveStruct for $move_object_name {
//             fn struct_type() -> StructTag {
//                 $full_path.parse().unwrap()
//             }
//         }

//         impl Key for $move_object_name {
//             fn id(&self) -> &ObjectId {
//                 &self.id
//             }
//         }
//     };
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

        let multisig = client.create_multisig(&mut builder).await.unwrap();
        client.share_multisig(&mut builder, multisig);
        let effects = execute_tx(client.sui(), pk, builder).await;

        let multisig_id = get_created_multisig(&effects).await;
        client.load_multisig(multisig_id).await.unwrap();

        assert!(client.multisig().is_some());
        assert!(client.intents().is_some());
        assert!(client.owned_objects().is_some());
    }
}
