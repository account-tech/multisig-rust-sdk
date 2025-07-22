use std::str::FromStr;

use account_multisig_sdk::{MultisigClient, proposals::actions::IntentType};
use anyhow::{Result, anyhow};
use clap::Subcommand;
use sui_crypto::ed25519::Ed25519PrivateKey;
use sui_sdk_types::ObjectId;

use crate::tx_utils;

#[derive(Debug, Subcommand)]
pub enum ProposalCommands {
    #[command(name = "approve", about = "Approve a proposal")]
    Approve,
    #[command(name = "disapprove", about = "Remove approval from a proposal")]
    Disapprove,
    #[command(name = "execute", about = "Execute a proposal")]
    Execute {
        #[arg(short, long)]
        package_id: Option<String>,
        #[arg(short, long)]
        modules: Option<String>,
        #[arg(short, long)]
        dependencies: Option<String>,
    },
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
            ProposalCommands::Execute {
                package_id,
                modules,
                dependencies,
            } => match (package_id, modules, dependencies) {
                (None, None, None) => self.execute(client, pk, key).await,
                (Some(package_id), Some(modules), Some(dependencies)) => {
                    self.execute_upgrade_package(client, pk, key, package_id, modules, dependencies)
                        .await
                }
                _ => Err(anyhow!("Invalid arguments")),
            },
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
            IntentType::DisableRules => client.execute_disable_rules(&mut builder, key).await?,
            IntentType::UpdateMetadata => client.execute_update_metadata(&mut builder, key).await?,
            IntentType::MintAndTransfer => {
                client.execute_mint_and_transfer(&mut builder, key).await?
            }
            IntentType::MintAndVest => client.execute_mint_and_vest(&mut builder, key).await?,
            IntentType::WithdrawAndBurn => {
                client.execute_withdraw_and_burn(&mut builder, key).await?
            }
            IntentType::TakeNfts => return Err(anyhow!("Not implemented")),
            IntentType::ListNfts => return Err(anyhow!("Not implemented")),
            IntentType::WithdrawAndTransferToVault => {
                client
                    .execute_withdraw_and_transfer_to_vault(&mut builder, key)
                    .await?
            }
            IntentType::WithdrawAndTransfer => {
                client
                    .execute_withdraw_and_transfer(&mut builder, key)
                    .await?
            }
            IntentType::WithdrawAndVest => {
                client.execute_withdraw_and_vest(&mut builder, key).await?
            }
            IntentType::SpendAndTransfer => {
                client.execute_spend_and_transfer(&mut builder, key).await?
            }
            IntentType::SpendAndVest => client.execute_spend_and_vest(&mut builder, key).await?,
            IntentType::UpgradePackage => return Err(anyhow!("Not implemented")),
            IntentType::RestrictPolicy => client.execute_restrict_policy(&mut builder, key).await?,
        }

        tx_utils::execute(client.sui(), builder, pk).await?;
        Ok(())
    }

    pub async fn execute_upgrade_package(
        &self,
        client: &mut MultisigClient,
        pk: &Ed25519PrivateKey,
        key: &str,
        package_id: &str,
        modules: &str,
        dependencies: &str,
    ) -> Result<()> {
        let addr = pk.public_key().derive_address();
        let mut builder = tx_utils::init(client.sui(), addr).await?;

        let package_id = ObjectId::from_str(package_id)?;
        let mut modules_parsed = Vec::new();
        for m in modules
            .trim()
            .trim_start_matches('[')
            .trim_end_matches(']')
            .trim()
            .split(',')
        {
            let m = m.trim();
            modules_parsed.push(m.as_bytes().to_vec());
        }
        let mut dependencies_parsed = Vec::new();
        for d in dependencies
            .trim()
            .trim_start_matches('[')
            .trim_end_matches(']')
            .trim()
            .split(',')
        {
            let d = ObjectId::from_str(d.trim())?;
            dependencies_parsed.push(d);
        }

        client
            .execute_upgrade_package(
                &mut builder,
                key,
                package_id,
                modules_parsed,
                dependencies_parsed,
            )
            .await?;

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
            IntentType::ToggleUnverifiedAllowed => {
                client
                    .delete_toggle_unverified_allowed(&mut builder, key)
                    .await?
            }
            IntentType::BorrowCap => client.delete_borrow_cap(&mut builder, key).await?,
            IntentType::DisableRules => client.delete_disable_rules(&mut builder, key).await?,
            IntentType::UpdateMetadata => client.delete_update_metadata(&mut builder, key).await?,
            IntentType::MintAndTransfer => {
                client.delete_mint_and_transfer(&mut builder, key).await?
            }
            IntentType::MintAndVest => client.delete_mint_and_vest(&mut builder, key).await?,
            IntentType::WithdrawAndBurn => {
                client.delete_withdraw_and_burn(&mut builder, key).await?
            }
            IntentType::TakeNfts => return Err(anyhow!("Not implemented")),
            IntentType::ListNfts => return Err(anyhow!("Not implemented")),
            IntentType::WithdrawAndTransferToVault => {
                client
                    .delete_withdraw_and_transfer_to_vault(&mut builder, key)
                    .await?
            }
            IntentType::WithdrawAndTransfer => {
                client
                    .delete_withdraw_and_transfer(&mut builder, key)
                    .await?
            }
            IntentType::WithdrawAndVest => {
                client.delete_withdraw_and_vest(&mut builder, key).await?
            }
            IntentType::SpendAndTransfer => {
                client.delete_spend_and_transfer(&mut builder, key).await?
            }
            IntentType::SpendAndVest => client.delete_spend_and_vest(&mut builder, key).await?,
            IntentType::UpgradePackage => client.delete_upgrade_package(&mut builder, key).await?,
            IntentType::RestrictPolicy => client.delete_restrict_policy(&mut builder, key).await?,
        }

        tx_utils::execute(client.sui(), builder, pk).await?;
        Ok(())
    }
}
