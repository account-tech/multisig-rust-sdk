use account_multisig_sdk::{MultisigClient, proposals::actions::IntentType};
use anyhow::{Result, anyhow};
use clap::Subcommand;
use sui_crypto::ed25519::Ed25519PrivateKey;

use crate::tx_utils;

#[derive(Debug, Subcommand)]
pub enum ProposalCommands {
    #[command(name = "approve", about = "Approve a proposal")]
    Approve,
    #[command(name = "disapprove", about = "Remove approval from a proposal")]
    Disapprove,
    #[command(name = "execute", about = "Execute a proposal")]
    Execute,
    #[command(name = "delete", about = "Delete a proposal")]
    Delete,
}

impl ProposalCommands {
    pub async fn run(
        &self,
        client: &mut MultisigClient,
        pk: &Ed25519PrivateKey,
        key: &str,
    ) -> Result<()> {
        match self {
            ProposalCommands::Approve => self.approve(client, pk, key).await,
            ProposalCommands::Disapprove => self.disapprove(client, pk, key).await,
            ProposalCommands::Execute => self.execute(client, pk, key).await,
            ProposalCommands::Delete => self.delete(client, pk, key).await,
        }
    }

    async fn approve(
        &self,
        client: &MultisigClient,
        pk: &Ed25519PrivateKey,
        key: &str,
    ) -> Result<()> {
        let addr = pk.public_key().derive_address();
        let mut builder = tx_utils::init(client.sui(), addr).await?;
        client.approve_intent(&mut builder, key).await?;
        tx_utils::execute(client.sui(), builder, pk).await?;
        Ok(())
    }

    async fn disapprove(
        &self,
        client: &MultisigClient,
        pk: &Ed25519PrivateKey,
        key: &str,
    ) -> Result<()> {
        let addr = pk.public_key().derive_address();
        let mut builder = tx_utils::init(client.sui(), addr).await?;
        client.disapprove_intent(&mut builder, key).await?;
        tx_utils::execute(client.sui(), builder, pk).await?;
        Ok(())
    }

    pub async fn execute(
        &self,
        client: &mut MultisigClient,
        pk: &Ed25519PrivateKey,
        key: &str,
    ) -> Result<()> {
        let addr = pk.public_key().derive_address();
        let mut builder = tx_utils::init(client.sui(), addr).await?;

        let intent_type: IntentType = client.intent(key)?.type_.as_str().try_into()?;
        match intent_type {
            IntentType::ConfigMultisig => client.execute_config_multisig(&mut builder, key).await?,
            IntentType::ConfigDeps => client.execute_config_deps(&mut builder, key).await?,
            IntentType::ToggleUnverifiedAllowed => {
                client
                    .execute_toggle_unverified_allowed(&mut builder, key)
                    .await?
            }
            IntentType::BorrowCap => return Err(anyhow!("Cannot be used via the CLI")),
            IntentType::DisableRules => return Err(anyhow!("Not implemented")),
            IntentType::UpdateMetadata => return Err(anyhow!("Not implemented")),
            IntentType::MintAndTransfer => return Err(anyhow!("Not implemented")),
            IntentType::MintAndVest => return Err(anyhow!("Not implemented")),
            IntentType::WithdrawAndBurn => return Err(anyhow!("Not implemented")),
            IntentType::TakeNfts => return Err(anyhow!("Not implemented")),
            IntentType::ListNfts => return Err(anyhow!("Not implemented")),
            IntentType::WithdrawAndTransferToVault => return Err(anyhow!("Not implemented")),
            IntentType::WithdrawAndTransfer => return Err(anyhow!("Not implemented")),
            IntentType::WithdrawAndVest => return Err(anyhow!("Not implemented")),
            IntentType::SpendAndTransfer => return Err(anyhow!("Not implemented")),
            IntentType::SpendAndVest => return Err(anyhow!("Not implemented")),
            IntentType::UpgradePackage => return Err(anyhow!("Not implemented")),
            IntentType::RestrictPolicy => client.execute_restrict_policy(&mut builder, key).await?,
        }

        tx_utils::execute(client.sui(), builder, pk).await?;
        Ok(())
    }

    pub async fn delete(
        &self,
        client: &mut MultisigClient,
        pk: &Ed25519PrivateKey,
        key: &str,
    ) -> Result<()> {
        let addr = pk.public_key().derive_address();
        let mut builder = tx_utils::init(client.sui(), addr).await?;

        let intent_type: IntentType = client.intent(key)?.type_.as_str().try_into()?;
        match intent_type {
            IntentType::ConfigMultisig => client.delete_config_multisig(&mut builder, key).await?,
            IntentType::ConfigDeps => client.delete_config_deps(&mut builder, key).await?,
            IntentType::ToggleUnverifiedAllowed => client.delete_toggle_unverified_allowed(&mut builder, key).await?,
            IntentType::BorrowCap => return Err(anyhow!("Cannot be used via the CLI")),
            IntentType::DisableRules => return Err(anyhow!("Cannot be used via the CLI")),
            IntentType::UpdateMetadata => return Err(anyhow!("Cannot be used via the CLI")),
            IntentType::MintAndTransfer => return Err(anyhow!("Cannot be used via the CLI")),
            IntentType::MintAndVest => return Err(anyhow!("Cannot be used via the CLI")),
            IntentType::WithdrawAndBurn => return Err(anyhow!("Cannot be used via the CLI")),
            IntentType::TakeNfts => return Err(anyhow!("Cannot be used via the CLI")),
            IntentType::ListNfts => return Err(anyhow!("Cannot be used via the CLI")),
            IntentType::WithdrawAndTransferToVault => return Err(anyhow!("Cannot be used via the CLI")),
            IntentType::WithdrawAndTransfer => return Err(anyhow!("Cannot be used via the CLI")),
            IntentType::WithdrawAndVest => return Err(anyhow!("Cannot be used via the CLI")),
            IntentType::SpendAndTransfer => return Err(anyhow!("Cannot be used via the CLI")),
            IntentType::SpendAndVest => return Err(anyhow!("Cannot be used via the CLI")),
            IntentType::UpgradePackage => return Err(anyhow!("Cannot be used via the CLI")),
            IntentType::RestrictPolicy => return Err(anyhow!("Cannot be used via the CLI")),
        }

        tx_utils::execute(client.sui(), builder, pk).await?;
        Ok(())
    }
}
