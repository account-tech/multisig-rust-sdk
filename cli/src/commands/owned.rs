use account_multisig_sdk::{
    MultisigClient,
    proposals::params::{ParamsArgs, WithdrawAndTransferArgs, WithdrawAndVestArgs},
};
use anyhow::{Result, anyhow};
use clap::Subcommand;
use sui_crypto::ed25519::Ed25519PrivateKey;
use sui_sdk_types::{Address, ObjectId};

use crate::tx_utils;

#[derive(Debug, Subcommand)]
pub enum OwnedCommands {
    #[command(
        name = "propose-withdraw-and-transfer",
        about = "Propose to withdraw and transfer owned objects"
    )]
    ProposeWithdrawAndTransfer {
        #[arg(long, help = "Name of the proposal")]
        name: String,
        #[arg(long, help = "Object IDs to withdraw")]
        object_ids: Vec<ObjectId>,
        #[arg(long, help = "Recipient addresses")]
        recipients: Vec<Address>,
    },
    #[command(
        name = "propose-withdraw-and-vest",
        about = "Propose to withdraw and vest a coin"
    )]
    ProposeWithdrawAndVest {
        #[arg(long, help = "Name of the proposal")]
        name: String,
        #[arg(long, help = "Coin object id")]
        coin_id: ObjectId,
        #[arg(long, help = "Vesting start timestamp in ms")]
        start_timestamp: u64,
        #[arg(long, help = "Vesting end timestamp in ms")]
        end_timestamp: u64,
        #[arg(long, help = "Recipient address")]
        recipient: Address,
    },
}

impl OwnedCommands {
    pub async fn run(&self, client: &mut MultisigClient, pk: &Ed25519PrivateKey) -> Result<()> {
        client.multisig().ok_or(anyhow!("Multisig not loaded"))?;
        match self {
            OwnedCommands::ProposeWithdrawAndTransfer {
                name,
                object_ids,
                recipients,
            } => {
                let mut builder =
                    tx_utils::init(client.sui(), pk.public_key().derive_address()).await?;
                let intent_args =
                    ParamsArgs::new(&mut builder, name.clone(), "".to_string(), vec![0], 0);
                let actions_args = WithdrawAndTransferArgs::new(
                    &mut builder,
                    object_ids.clone(),
                    recipients.clone(),
                );
                client
                    .request_withdraw_and_transfer(&mut builder, intent_args, actions_args)
                    .await?;
                tx_utils::execute(client.sui(), builder, pk).await?;
                Ok(())
            }
            OwnedCommands::ProposeWithdrawAndVest {
                name,
                coin_id,
                start_timestamp,
                end_timestamp,
                recipient,
            } => {
                let mut builder =
                    tx_utils::init(client.sui(), pk.public_key().derive_address()).await?;
                let intent_args =
                    ParamsArgs::new(&mut builder, name.clone(), "".to_string(), vec![0], 0);
                let actions_args = WithdrawAndVestArgs::new(
                    &mut builder,
                    *coin_id,
                    *start_timestamp,
                    *end_timestamp,
                    *recipient,
                );
                client
                    .request_withdraw_and_vest(&mut builder, intent_args, actions_args)
                    .await?;
                tx_utils::execute(client.sui(), builder, pk).await?;
                Ok(())
            }
        }
    }
}
