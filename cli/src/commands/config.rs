use account_multisig_sdk::{
    MultisigClient,
    proposals::params::{ConfigMultisigArgs, ParamsArgs},
};
use anyhow::{Result, anyhow};
use clap::Subcommand;
use sui_crypto::ed25519::Ed25519PrivateKey;
use sui_sdk_types::Address;

use crate::tx_utils;

#[derive(Debug, Subcommand)]
pub enum ConfigCommands {
    #[command(name = "modify-name", about = "Modify multisig name")]
    ModifyName { name: String },
    #[command(
        name = "propose-config-multisig",
        about = "Create a proposal with a new config"
    )]
    ProposeConfigMultisig {
        #[arg(long, short, help = "Name of the proposal")]
        name: String,
        #[arg(long, short, help = "Addresses of the members")]
        addresses: Vec<Address>,
        #[arg(long, short, help = "Weights of the members")]
        weights: Vec<u64>,
        #[arg(long, short, help = "Roles of the members (e.g. [package::module,package::module1,...])")]
        roles: Vec<String>,
        #[arg(long, short, help = "Global threshold")]
        global: u64,
        #[arg(long, short, help = "Names of the roles")]
        role_names: Vec<String>,
        #[arg(long, short, help = "Thresholds of the roles")]
        role_thresholds: Vec<u64>,
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
                addresses,
                weights,
                roles,
                global,
                role_names,
                role_thresholds,
            } => {
                let mut builder =
                    tx_utils::init(client.sui(), pk.public_key().derive_address()).await?;

                let intent_args =
                    ParamsArgs::new(&mut builder, name.clone(), "".to_string(), vec![], 0);
                let actions_args = ConfigMultisigArgs::new(
                    &mut builder,
                    addresses.clone(),
                    weights.clone(),
                    roles.iter().map(|r| r.split(",").map(|s| s.to_string()).collect()).collect(),
                    global.clone(),
                    role_names.clone(),
                    role_thresholds.clone(),
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
