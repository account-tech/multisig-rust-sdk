use account_multisig_sdk::{
    MultisigClient,
    proposals::params::{ParamsArgs, RestrictPolicyArgs, UpgradePackageArgs},
};
use anyhow::{Result, anyhow};
use clap::Subcommand;
use sui_crypto::ed25519::Ed25519PrivateKey;
use sui_sdk_types::Address;

use crate::tx_utils;

#[derive(Debug, Subcommand)]
pub enum PackageCommands {
    #[command(
        name = "deposit-upgrade-cap",
        about = "Deposit an upgrade cap for a package"
    )]
    DepositUpgradeCap {
        #[arg(long, help = "Upgrade cap object id")]
        cap_id: Address,
        #[arg(long, help = "Package name")]
        package_name: String,
        #[arg(long, help = "Timelock duration in ms (0: No timelock)")]
        timelock_duration: u64,
    },
    #[command(
        name = "propose-upgrade-package",
        about = "Propose to upgrade a package"
    )]
    ProposeUpgradePackage {
        #[arg(long, help = "Name of the proposal")]
        name: String,
        #[arg(long, help = "Package name")]
        package_name: String,
        #[arg(long, help = "Package build digest")]
        digest: Vec<u8>,
    },
    #[command(
        name = "propose-restrict-policy",
        about = "Propose to restrict a package policy"
    )]
    ProposeRestrictPolicy {
        #[arg(long, help = "Name of the proposal")]
        name: String,
        #[arg(long, help = "Package name")]
        package_name: String,
        #[arg(long, help = "Policy (128: Additive, 192: DepOnly, 255: Immutable)")]
        policy: u8,
    },
}

impl PackageCommands {
    pub async fn run(&self, client: &mut MultisigClient, pk: &Ed25519PrivateKey) -> Result<()> {
        client.multisig().ok_or(anyhow!("Multisig not loaded"))?;
        match self {
            PackageCommands::DepositUpgradeCap {
                cap_id,
                package_name,
                timelock_duration,
            } => {
                let mut builder =
                    tx_utils::init(client.sui(), pk.public_key().derive_address()).await?;
                client
                    .deposit_upgrade_cap(&mut builder, *cap_id, package_name, *timelock_duration)
                    .await?;
                tx_utils::execute(client.sui(), builder, pk).await?;
                Ok(())
            }
            PackageCommands::ProposeUpgradePackage {
                name,
                package_name,
                digest,
            } => {
                let mut builder =
                    tx_utils::init(client.sui(), pk.public_key().derive_address()).await?;
                let intent_args =
                    ParamsArgs::new(&mut builder, name.clone(), "".to_string(), vec![0], 0);
                let actions_args =
                    UpgradePackageArgs::new(&mut builder, package_name.clone(), digest.clone());
                client
                    .request_upgrade_package(&mut builder, intent_args, actions_args)
                    .await?;
                tx_utils::execute(client.sui(), builder, pk).await?;
                Ok(())
            }
            PackageCommands::ProposeRestrictPolicy {
                name,
                package_name,
                policy,
            } => {
                let mut builder =
                    tx_utils::init(client.sui(), pk.public_key().derive_address()).await?;
                let intent_args =
                    ParamsArgs::new(&mut builder, name.clone(), "".to_string(), vec![0], 0);
                let actions_args =
                    RestrictPolicyArgs::new(&mut builder, package_name.clone(), *policy);
                client
                    .request_restrict_policy(&mut builder, intent_args, actions_args)
                    .await?;
                tx_utils::execute(client.sui(), builder, pk).await?;
                Ok(())
            }
        }
    }
}
