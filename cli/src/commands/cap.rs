use account_multisig_sdk::{MultisigClient, proposals::params::ParamsArgs};
use anyhow::{Result, anyhow};
use clap::Subcommand;
use sui_crypto::ed25519::Ed25519PrivateKey;
use sui_sdk_types::Address;

use crate::tx_utils;

#[derive(Debug, Subcommand)]
pub enum CapCommands {
    #[command(name = "deposit-cap", about = "Deposit a Cap into the multisig")]
    DepositCap {
        #[arg(long, short, help = "Address of the Cap object")]
        cap_id: Address,
        #[arg(
            long,
            short,
            help = "Type of the Cap (e.g. <addr>::<module>::<CapType>)"
        )]
        cap_type: String,
    },
    #[command(name = "propose-borrow-cap", about = "Propose to borrow a Cap")]
    ProposeBorrowCap {
        #[arg(long, short, help = "Name of the proposal")]
        name: String,
        #[arg(
            long,
            short,
            help = "Type of the Cap (e.g. <addr>::<module>::<CapType>)"
        )]
        cap_type: String,
    },
}

impl CapCommands {
    pub async fn run(&self, client: &mut MultisigClient, pk: &Ed25519PrivateKey) -> Result<()> {
        client.multisig().ok_or(anyhow!("Multisig not loaded"))?;
        match self {
            CapCommands::DepositCap { cap_id, cap_type } => {
                let mut builder =
                    tx_utils::init(client.sui(), pk.public_key().derive_address()).await?;
                client.deposit_cap(&mut builder, *cap_id, cap_type).await?;
                tx_utils::execute(client.sui(), builder, pk).await?;
                Ok(())
            }
            CapCommands::ProposeBorrowCap { name, cap_type } => {
                let mut builder =
                    tx_utils::init(client.sui(), pk.public_key().derive_address()).await?;
                let intent_args =
                    ParamsArgs::new(&mut builder, name.clone(), "".to_string(), vec![0], 0);
                client
                    .request_borrow_cap(&mut builder, intent_args, cap_type)
                    .await?;
                tx_utils::execute(client.sui(), builder, pk).await?;
                Ok(())
            }
        }
    }
}
