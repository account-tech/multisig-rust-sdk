use account_multisig_sdk::{
    MultisigClient,
    proposals::params::{
        DisableRulesArgs, MintAndTransferArgs, MintAndVestArgs, ParamsArgs, UpdateMetadataArgs,
        WithdrawAndBurnArgs,
    },
};
use anyhow::{Result, anyhow};
use clap::Subcommand;
use sui_crypto::ed25519::Ed25519PrivateKey;
use sui_sdk_types::{Address, ObjectId};

use crate::tx_utils;

#[derive(Debug, Subcommand)]
pub enum CurrencyCommands {
    #[command(
        name = "deposit-treasury-cap",
        about = "Deposit a TreasuryCap into the multisig"
    )]
    DepositTreasuryCap {
        #[arg(long, help = "Max supply (optional)")]
        max_supply: Option<u64>,
        #[arg(long, help = "Address of the TreasuryCap object")]
        cap_id: Address,
        #[arg(long, help = "Coin type (e.g. <addr>::<module>::<Coin>)")]
        coin_type: String,
    },
    #[command(
        name = "propose-disable-rules",
        about = "Propose to disable currency rules"
    )]
    ProposeDisableRules {
        #[arg(long, help = "Name of the proposal")]
        name: String,
        #[arg(long, help = "Coin type (e.g. <addr>::<module>::<Coin>)")]
        coin_type: String,
        #[arg(long, help = "Disable minting")]
        mint: bool,
        #[arg(long, help = "Disable burning")]
        burn: bool,
        #[arg(long, help = "Disable symbol updates")]
        update_symbol: bool,
        #[arg(long, help = "Disable name updates")]
        update_name: bool,
        #[arg(long, help = "Disable description updates")]
        update_description: bool,
        #[arg(long, help = "Disable icon updates")]
        update_icon: bool,
    },
    #[command(
        name = "propose-update-metadata",
        about = "Propose to update currency metadata"
    )]
    ProposeUpdateMetadata {
        #[arg(long, help = "Name of the proposal")]
        name: String,
        #[arg(long, help = "Coin type (e.g. <addr>::<module>::<Coin>)")]
        coin_type: String,
        #[arg(long, help = "Symbol (optional)")]
        symbol: Option<String>,
        #[arg(long, help = "Name (optional)")]
        name_field: Option<String>,
        #[arg(long, help = "Description (optional)")]
        description: Option<String>,
        #[arg(long, help = "Icon URL (optional)")]
        icon_url: Option<String>,
    },
    #[command(
        name = "propose-mint-and-transfer",
        about = "Propose to mint and transfer coins"
    )]
    ProposeMintAndTransfer {
        #[arg(long, help = "Name of the proposal")]
        name: String,
        #[arg(long, help = "Coin type (e.g. <addr>::<module>::<Coin>)")]
        coin_type: String,
        #[arg(long, help = "Amounts to mint")]
        amounts: Vec<u64>,
        #[arg(long, help = "Recipients")]
        recipients: Vec<Address>,
    },
    #[command(
        name = "propose-mint-and-vest",
        about = "Propose to mint and vest coins"
    )]
    ProposeMintAndVest {
        #[arg(long, help = "Name of the proposal")]
        name: String,
        #[arg(long, help = "Coin type (e.g. <addr>::<module>::<Coin>)")]
        coin_type: String,
        #[arg(long, help = "Total amount to mint")]
        total_amount: u64,
        #[arg(long, help = "Vesting start timestamp (ms since epoch)")]
        start_timestamp: u64,
        #[arg(long, help = "Vesting end timestamp (ms since epoch)")]
        end_timestamp: u64,
        #[arg(long, help = "Recipient address")]
        recipient: Address,
    },
    #[command(
        name = "propose-withdraw-and-burn",
        about = "Propose to withdraw and burn coins"
    )]
    ProposeWithdrawAndBurn {
        #[arg(long, help = "Name of the proposal")]
        name: String,
        #[arg(long, help = "Coin type (e.g. <addr>::<module>::<Coin>)")]
        coin_type: String,
        #[arg(long, help = "Coin object id")]
        coin_id: ObjectId,
        #[arg(long, help = "Amount to burn")]
        amount: u64,
    },
}

impl CurrencyCommands {
    pub async fn run(&self, client: &mut MultisigClient, pk: &Ed25519PrivateKey) -> Result<()> {
        client.multisig().ok_or(anyhow!("Multisig not loaded"))?;
        match self {
            CurrencyCommands::DepositTreasuryCap {
                max_supply,
                cap_id,
                coin_type,
            } => {
                let mut builder =
                    tx_utils::init(client.sui(), pk.public_key().derive_address()).await?;
                client
                    .deposit_treasury_cap(&mut builder, *max_supply, *cap_id, coin_type)
                    .await?;
                tx_utils::execute(client.sui(), builder, pk).await?;
                Ok(())
            }
            CurrencyCommands::ProposeDisableRules {
                name,
                coin_type,
                mint,
                burn,
                update_symbol,
                update_name,
                update_description,
                update_icon,
            } => {
                let mut builder =
                    tx_utils::init(client.sui(), pk.public_key().derive_address()).await?;
                let intent_args =
                    ParamsArgs::new(&mut builder, name.clone(), "".to_string(), vec![0], 0);
                let actions_args = DisableRulesArgs::new(
                    &mut builder,
                    *mint,
                    *burn,
                    *update_symbol,
                    *update_name,
                    *update_description,
                    *update_icon,
                );
                client
                    .request_disable_rules(&mut builder, intent_args, actions_args, coin_type)
                    .await?;
                tx_utils::execute(client.sui(), builder, pk).await?;
                Ok(())
            }
            CurrencyCommands::ProposeUpdateMetadata {
                name,
                coin_type,
                symbol,
                name_field,
                description,
                icon_url,
            } => {
                let mut builder =
                    tx_utils::init(client.sui(), pk.public_key().derive_address()).await?;
                let intent_args =
                    ParamsArgs::new(&mut builder, name.clone(), "".to_string(), vec![0], 0);
                let actions_args = UpdateMetadataArgs::new(
                    &mut builder,
                    symbol.clone(),
                    name_field.clone(),
                    description.clone(),
                    icon_url.clone(),
                );
                client
                    .request_update_metadata(&mut builder, intent_args, actions_args, coin_type)
                    .await?;
                tx_utils::execute(client.sui(), builder, pk).await?;
                Ok(())
            }
            CurrencyCommands::ProposeMintAndTransfer {
                name,
                coin_type,
                amounts,
                recipients,
            } => {
                let mut builder =
                    tx_utils::init(client.sui(), pk.public_key().derive_address()).await?;
                let intent_args =
                    ParamsArgs::new(&mut builder, name.clone(), "".to_string(), vec![0], 0);
                let actions_args =
                    MintAndTransferArgs::new(&mut builder, amounts.clone(), recipients.clone());
                client
                    .request_mint_and_transfer(&mut builder, intent_args, actions_args, coin_type)
                    .await?;
                tx_utils::execute(client.sui(), builder, pk).await?;
                Ok(())
            }
            CurrencyCommands::ProposeMintAndVest {
                name,
                coin_type,
                total_amount,
                start_timestamp,
                end_timestamp,
                recipient,
            } => {
                let mut builder =
                    tx_utils::init(client.sui(), pk.public_key().derive_address()).await?;
                let intent_args =
                    ParamsArgs::new(&mut builder, name.clone(), "".to_string(), vec![0], 0);
                let actions_args = MintAndVestArgs::new(
                    &mut builder,
                    *total_amount,
                    *start_timestamp,
                    *end_timestamp,
                    *recipient,
                );
                client
                    .request_mint_and_vest(&mut builder, intent_args, actions_args, coin_type)
                    .await?;
                tx_utils::execute(client.sui(), builder, pk).await?;
                Ok(())
            }
            CurrencyCommands::ProposeWithdrawAndBurn {
                name,
                coin_type,
                coin_id,
                amount,
            } => {
                let mut builder =
                    tx_utils::init(client.sui(), pk.public_key().derive_address()).await?;
                let intent_args =
                    ParamsArgs::new(&mut builder, name.clone(), "".to_string(), vec![0], 0);
                let actions_args = WithdrawAndBurnArgs::new(&mut builder, *coin_id, *amount);
                client
                    .request_withdraw_and_burn(&mut builder, intent_args, actions_args, coin_type)
                    .await?;
                tx_utils::execute(client.sui(), builder, pk).await?;
                Ok(())
            }
        }
    }
}
