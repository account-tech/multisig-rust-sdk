use account_multisig_sdk::{
    MultisigClient,
    proposals::params::{
        ParamsArgs, SpendAndTransferArgs, SpendAndVestArgs, WithdrawAndTransferToVaultArgs,
    },
    utils::get_owned_coins,
};
use anyhow::{Result, anyhow};
use clap::Subcommand;
use sui_crypto::ed25519::Ed25519PrivateKey;
use sui_sdk_types::{Address, ObjectId};

use crate::tx_utils;

#[derive(Debug, Subcommand)]
pub enum VaultCommands {
    #[command(name = "open-vault", about = "Open a new vault")]
    OpenVault {
        #[arg(long, help = "Vault name")]
        vault_name: String,
    },
    #[command(
        name = "deposit-from-wallet",
        about = "Deposit a coin from wallet into a vault"
    )]
    DepositFromWallet {
        #[arg(long, help = "Vault name")]
        vault_name: String,
        #[arg(long, help = "Coin amount in the smallest unit")]
        amount: u64,
        #[arg(long, help = "Coin type (e.g. <addr>::<module>::<COIN_TYPE>)")]
        coin_type: String,
    },
    #[command(name = "close-vault", about = "Close a vault")]
    CloseVault {
        #[arg(long, help = "Vault name")]
        vault_name: String,
    },
    #[command(
        name = "propose-withdraw-and-transfer-to-vault",
        about = "Propose to withdraw and transfer to vault"
    )]
    ProposeWithdrawAndTransferToVault {
        #[arg(long, help = "Name of the proposal")]
        name: String,
        #[arg(long, help = "Coin type (e.g. <addr>::<module>::<COIN_TYPE>)")]
        coin_type: String,
        #[arg(long, help = "Coin object id")]
        coin_id: ObjectId,
        #[arg(long, help = "Coin amount")]
        coin_amount: u64,
        #[arg(long, help = "Vault name")]
        vault_name: String,
    },
    #[command(
        name = "propose-spend-and-transfer",
        about = "Propose to spend and transfer from vault"
    )]
    ProposeSpendAndTransfer {
        #[arg(long, help = "Name of the proposal")]
        name: String,
        #[arg(long, help = "Coin type (e.g. <addr>::<module>::<COIN_TYPE>)")]
        coin_type: String,
        #[arg(long, help = "Vault name")]
        vault_name: String,
        #[arg(long, help = "Amounts to transfer")]
        amounts: Vec<u64>,
        #[arg(long, help = "Recipients")]
        recipients: Vec<Address>,
    },
    #[command(
        name = "propose-spend-and-vest",
        about = "Propose to spend and vest from vault"
    )]
    ProposeSpendAndVest {
        #[arg(long, help = "Name of the proposal")]
        name: String,
        #[arg(long, help = "Coin type (e.g. <addr>::<module>::<COIN_TYPE>)")]
        coin_type: String,
        #[arg(long, help = "Vault name")]
        vault_name: String,
        #[arg(long, help = "Coin amount")]
        coin_amount: u64,
        #[arg(long, help = "Vesting start timestamp in ms")]
        start_timestamp: u64,
        #[arg(long, help = "Vesting end timestamp in ms")]
        end_timestamp: u64,
        #[arg(long, help = "Recipient address")]
        recipient: Address,
    },
}

impl VaultCommands {
    pub async fn run(&self, client: &mut MultisigClient, pk: &Ed25519PrivateKey) -> Result<()> {
        client.multisig().ok_or(anyhow!("Multisig not loaded"))?;
        match self {
            VaultCommands::OpenVault { vault_name } => {
                let mut builder =
                    tx_utils::init(client.sui(), pk.public_key().derive_address()).await?;
                client.open_vault(&mut builder, vault_name).await?;
                tx_utils::execute(client.sui(), builder, pk).await?;
                Ok(())
            }
            VaultCommands::DepositFromWallet {
                vault_name,
                amount,
                coin_type,
            } => {
                let owner = pk.public_key().derive_address();
                let mut builder = tx_utils::init(client.sui(), owner).await?;
                
                let coins = get_owned_coins(client.sui(), owner, Some(coin_type)).await?;
                let to_merge = coins.iter().map(|coin| *coin.id().as_address()).collect();

                let coin = client.merge_and_split(&mut builder, to_merge, vec![*amount], coin_type).await?;
                client.deposit_from_wallet(&mut builder, vault_name.clone(), coin, coin_type).await?;

                tx_utils::execute(client.sui(), builder, pk).await?;
                Ok(())
            }
            VaultCommands::CloseVault { vault_name } => {
                let mut builder =
                    tx_utils::init(client.sui(), pk.public_key().derive_address()).await?;
                client.close_vault(&mut builder, vault_name).await?;
                tx_utils::execute(client.sui(), builder, pk).await?;
                Ok(())
            }
            VaultCommands::ProposeWithdrawAndTransferToVault {
                name,
                coin_type,
                coin_id,
                coin_amount,
                vault_name,
            } => {
                let mut builder =
                    tx_utils::init(client.sui(), pk.public_key().derive_address()).await?;
                let intent_args =
                    ParamsArgs::new(&mut builder, name.clone(), "".to_string(), vec![0], 0);
                let actions_args = WithdrawAndTransferToVaultArgs::new(
                    &mut builder,
                    *coin_id,
                    *coin_amount,
                    vault_name.clone(),
                );
                client
                    .request_withdraw_and_transfer_to_vault(
                        &mut builder,
                        intent_args,
                        actions_args,
                        coin_type,
                    )
                    .await?;
                tx_utils::execute(client.sui(), builder, pk).await?;
                Ok(())
            }
            VaultCommands::ProposeSpendAndTransfer {
                name,
                coin_type,
                vault_name,
                amounts,
                recipients,
            } => {
                let mut builder =
                    tx_utils::init(client.sui(), pk.public_key().derive_address()).await?;
                let intent_args =
                    ParamsArgs::new(&mut builder, name.clone(), "".to_string(), vec![0], 0);
                let actions_args = SpendAndTransferArgs::new(
                    &mut builder,
                    vault_name.clone(),
                    amounts.clone(),
                    recipients.clone(),
                );
                client
                    .request_spend_and_transfer(&mut builder, intent_args, actions_args, coin_type)
                    .await?;
                tx_utils::execute(client.sui(), builder, pk).await?;
                Ok(())
            }
            VaultCommands::ProposeSpendAndVest {
                name,
                coin_type,
                vault_name,
                coin_amount,
                start_timestamp,
                end_timestamp,
                recipient,
            } => {
                let mut builder =
                    tx_utils::init(client.sui(), pk.public_key().derive_address()).await?;
                let intent_args =
                    ParamsArgs::new(&mut builder, name.clone(), "".to_string(), vec![0], 0);
                let actions_args = SpendAndVestArgs::new(
                    &mut builder,
                    vault_name.clone(),
                    *coin_amount,
                    *start_timestamp,
                    *end_timestamp,
                    *recipient,
                );
                client
                    .request_spend_and_vest(&mut builder, intent_args, actions_args, coin_type)
                    .await?;
                tx_utils::execute(client.sui(), builder, pk).await?;
                Ok(())
            }
        }
    }
}
