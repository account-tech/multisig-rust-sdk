use account_multisig_sdk::{
    MultisigClient,
    proposals::params::{ConfigMultisigArgs, ParamsArgs},
};
use anyhow::{Result, anyhow};
use clap::Subcommand;
use sui_crypto::ed25519::Ed25519PrivateKey;
use std::str::FromStr;

use crate::parsers::{Member, Role};
use crate::tx_utils;

#[derive(Debug, Subcommand)]
pub enum ConfigCommands {
    #[command(name = "modify-name", about = "Modify multisig name")]
    ModifyName { name: String },
    #[command(
        name = "propose-config-multisig",
        about = "Create a proposal with a new config (overrides the current state with the new one)"
    )]
    ProposeConfigMultisig {
        #[arg(long, short, help = "Name of the proposal")]
        name: String,
        #[arg(long, value_parser = clap::builder::ValueParser::new(Member::from_str))]
        member: Option<Vec<Member>>,
        #[arg(long, value_parser = clap::builder::ValueParser::new(Role::from_str))]
        role: Option<Vec<Role>>,
        #[arg(long, short, help = "Global threshold")]
        global_threshold: u64,
    },
}

impl ConfigCommands {
    pub async fn run(&self, client: &mut MultisigClient, pk: &Ed25519PrivateKey) -> Result<()> {
        client.multisig().ok_or(anyhow!("Multisig not loaded"))?;
        match self {
            ConfigCommands::ModifyName { name } => {
                let mut builder =
                    tx_utils::init(client.sui(), pk.public_key().derive_address()).await?;
                client
                    .replace_metadata(&mut builder, vec!["name".to_string()], vec![name.clone()])
                    .await?;
                tx_utils::execute(client.sui(), builder, &pk).await?;
                Ok(())
            }
            ConfigCommands::ProposeConfigMultisig {
                name,
                member,
                role,
                global_threshold,
            } => {
                let mut builder =
                    tx_utils::init(client.sui(), pk.public_key().derive_address()).await?;

                let intent_args =
                    ParamsArgs::new(&mut builder, name.clone(), "".to_string(), vec![0], 0);

                // Convert Member and Role structs to the format expected by ConfigMultisigArgs
                let addresses = member
                    .as_ref()
                    .map(|m| {
                        m.iter()
                            .map(|member| member.address.parse().unwrap())
                            .collect()
                    })
                    .unwrap_or_default();

                let weights = member
                    .as_ref()
                    .map(|m| m.iter().map(|member| member.weight).collect())
                    .unwrap_or_default();

                let roles = member
                    .as_ref()
                    .map(|m| m.iter().map(|member| member.roles.clone()).collect())
                    .unwrap_or_default();

                let role_names = role
                    .as_ref()
                    .map(|r| r.iter().map(|role| role.name.clone()).collect())
                    .unwrap_or_default();

                let role_thresholds = role
                    .as_ref()
                    .map(|r| r.iter().map(|role| role.threshold).collect())
                    .unwrap_or_default();

                let actions_args = ConfigMultisigArgs::new(
                    &mut builder,
                    addresses,
                    weights,
                    roles,
                    global_threshold.clone(),
                    role_names,
                    role_thresholds,
                );

                client
                    .request_config_multisig(&mut builder, intent_args, actions_args)
                    .await?;

                tx_utils::execute(client.sui(), builder, pk).await?;
                Ok(())
            }
        }
    }
}
