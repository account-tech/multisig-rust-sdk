use anyhow::{anyhow, Result};
use move_types::Address;
use sui_transaction_builder::TransactionBuilder;

use crate::{
    move_binding::{account_multisig as am, account_protocol as ap, sui},
    proposals::params::{ConfigMultisigArgs, ParamsArgs},
    MultisigClient,
};

pub struct MultisigBuilder {
    pub client: MultisigClient,
    pub builder: TransactionBuilder,
    pub name: Option<String>,
    pub config: Option<Config>,
}

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub addresses: Vec<String>,
    pub weights: Vec<u64>,
    pub roles: Vec<Vec<String>>,
    pub global_threshold: u64,
    pub role_names: Vec<String>,
    pub role_thresholds: Vec<u64>,
}

impl MultisigBuilder {
    pub fn new(client: MultisigClient, builder: TransactionBuilder) -> Self {
        Self {
            client,
            builder,
            name: None,
            config: None,
        }
    }

    pub fn set_name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    pub fn set_global_threshold(mut self, threshold: u64) -> Self {
        if self.config.is_none() {
            self.config = Some(Config::default());
        }
        self.config.as_mut().unwrap().global_threshold = threshold;
        self
    }

    pub fn add_member(mut self, address: &str, weight: u64, roles: Vec<&str>) -> Self {
        if self.config.is_none() {
            self.config = Some(Config::default());
        }

        self.config
            .as_mut()
            .unwrap()
            .addresses
            .push(address.to_string());
        self.config.as_mut().unwrap().weights.push(weight);
        self.config
            .as_mut()
            .unwrap()
            .roles
            .push(roles.iter().map(|r| r.to_string()).collect());

        self
    }

    pub fn add_role(mut self, role: &str, threshold: u64) -> Self {
        if self.config.is_none() {
            self.config = Some(Config::default());
        }
        self.config
            .as_mut()
            .unwrap()
            .role_names
            .push(role.to_string());
        self.config
            .as_mut()
            .unwrap()
            .role_thresholds
            .push(threshold);

        self
    }

    pub async fn build(self) -> Result<(MultisigClient, TransactionBuilder)> {
        let Self {
            client,
            mut builder,
            name,
            config,
        } = self;

        if client.user().is_none() {
            return Err(anyhow!("User not loaded"));
        }

        let mut user = if client.user().unwrap().id.is_none() {
            client.user().unwrap().create_user(&mut builder).await?
        } else {
            client
                .user()
                .unwrap()
                .user_arg(
                    &mut builder,
                    *client.user().unwrap().id.unwrap().as_address(),
                )
                .await?
        };

        let mut multisig = client.create_multisig(&mut builder).await?;

        // set name if provided
        if let Some(name) = name {
            let keys_arg = client.pure_arg(&mut builder, vec![String::from("name")])?;
            let values_arg = client.pure_arg(&mut builder, vec![name.to_string()])?;

            let auth = am::multisig::authenticate(&mut builder, multisig.borrow());
            ap::config::edit_metadata(
                &mut builder,
                auth,
                multisig.borrow_mut(),
                keys_arg,
                values_arg,
            );
        }

        // set config if provided
        if let Some(config) = config {
            let Config {
                addresses,
                weights,
                roles,
                global_threshold,
                role_names,
                role_thresholds,
            } = config;

            let clock = client.clock_arg(&mut builder).await?;
            let params = ParamsArgs::new(
                &mut builder,
                "config_multisig".to_string(),
                "".to_string(),
                vec![0],
                0,
            );

            let auth = am::multisig::authenticate(&mut builder, multisig.borrow());
            let params = ap::intents::new_params(
                &mut builder,
                params.key,
                params.description,
                params.execution_times,
                params.expiration_time,
                clock.borrow(),
            );
            let outcome = am::multisig::empty_outcome(&mut builder);

            let action_args = ConfigMultisigArgs::new(
                &mut builder,
                addresses
                    .clone()
                    .iter()
                    .map(|a| Address::from_hex(a).unwrap())
                    .collect(),
                weights,
                roles,
                global_threshold,
                role_names,
                role_thresholds,
            );

            am::config::request_config_multisig(
                &mut builder,
                auth,
                multisig.borrow_mut(),
                params,
                outcome,
                action_args.addresses,
                action_args.weights,
                action_args.roles,
                action_args.global,
                action_args.role_names,
                action_args.role_thresholds,
            );

            let key = client.key_arg(&mut builder, "config_multisig")?;
            am::multisig::approve_intent(&mut builder, multisig.borrow_mut(), key);

            let key = client.key_arg(&mut builder, "config_multisig")?;
            let mut executable = am::multisig::execute_intent(
                &mut builder,
                multisig.borrow_mut(),
                key,
                clock.borrow(),
            );
            am::config::execute_config_multisig(
                &mut builder,
                executable.borrow_mut(),
                multisig.borrow_mut(),
            );
            ap::account::confirm_execution(&mut builder, multisig.borrow_mut(), executable);

            let key = client.key_arg(&mut builder, "config_multisig")?;
            let mut expired = ap::account::destroy_empty_intent::<
                am::multisig::Multisig,
                am::multisig::Approvals,
            >(&mut builder, multisig.borrow_mut(), key);

            am::config::delete_config_multisig(&mut builder, expired.borrow_mut());
            ap::intents::destroy_empty_expired(&mut builder, expired);

            for addr in addresses {
                if addr == client.user().unwrap().address.to_string() {
                    // add multisig to User object
                    am::multisig::join(&mut builder, user.borrow_mut(), multisig.borrow());
                } else {    
                    // send invite to other addresses
                    client
                        .user()
                        .unwrap()
                        .send_invite(&mut builder, &multisig, addr.parse().unwrap())
                        .await?;
                }
            }
        }
        // transfer and share objects
        sui::transfer::public_share_object(&mut builder, multisig);
        if client.user().unwrap().id.is_none() {
            client
                .user()
                .unwrap()
                .transfer_user(&mut builder, user)
                .await?;
        }

        Ok((client, builder))
    }
}
