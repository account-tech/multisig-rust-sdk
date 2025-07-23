use account_multisig_sdk::{
    MultisigClient,
    proposals::params::{ConfigDepsArgs, ParamsArgs},
};
use anyhow::{Result, anyhow};
use clap::Subcommand;
use sui_crypto::ed25519::Ed25519PrivateKey;
use sui_sdk_types::Address;

use crate::tx_utils;

#[derive(Debug, Subcommand)]
pub enum DepsCommands {
    #[command(name = "update-to-latest", about = "Update dependencies to latest")]
    UpdateToLatest,
    #[command(
        name = "propose-config-deps",
        about = "Create a proposal with new dependencies"
    )]
    ProposeConfigDeps {
        #[arg(long, short, help = "Name of the proposal")]
        name: String,
        #[arg(long, short, help = "Name of the package")]
        names: Vec<String>,
        #[arg(long, short, help = "Address of the package")]
        addresses: Vec<Address>,
        #[arg(long, short, help = "Version of the package")]
        versions: Vec<u64>,
    },
    #[command(
        name = "propose-toggle-unverified-allowed",
        about = "Propose to toggle unverified dependencies allowed"
    )]
    ProposeToggleUnverifiedAllowed {
        #[arg(long, short, help = "Name of the proposal")]
        name: String,
    },
}

impl DepsCommands {
    pub async fn run(&self, client: &mut MultisigClient, pk: &Ed25519PrivateKey) -> Result<()> {
        client.multisig().ok_or(anyhow!("Multisig not loaded"))?;
        match self {
            DepsCommands::UpdateToLatest => {
                let mut builder =
                    tx_utils::init(client.sui(), pk.public_key().derive_address()).await?;
                client.update_verified_deps_to_latest(&mut builder).await?;
                tx_utils::execute(client.sui(), builder, pk).await?;
                Ok(())
            }
            DepsCommands::ProposeConfigDeps {
                name,
                names,
                addresses,
                versions,
            } => {
                let mut builder =
                    tx_utils::init(client.sui(), pk.public_key().derive_address()).await?;

                let intent_args =
                    ParamsArgs::new(&mut builder, name.clone(), "".to_string(), vec![0], 0);
                let actions_args = ConfigDepsArgs::new(
                    &mut builder,
                    names.clone(),
                    addresses.clone(),
                    versions.clone(),
                );

                client
                    .request_config_deps(&mut builder, intent_args, actions_args)
                    .await?;

                tx_utils::execute(client.sui(), builder, pk).await?;
                Ok(())
            }
            DepsCommands::ProposeToggleUnverifiedAllowed { name } => {
                let mut builder =
                    tx_utils::init(client.sui(), pk.public_key().derive_address()).await?;

                let intent_args =
                    ParamsArgs::new(&mut builder, name.clone(), "".to_string(), vec![0], 0);

                client
                    .request_toggle_unverified_allowed(&mut builder, intent_args)
                    .await?;

                tx_utils::execute(client.sui(), builder, pk).await?;
                Ok(())
            }
        }
    }
}
