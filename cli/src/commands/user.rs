use account_multisig_sdk::MultisigClient;
use anyhow::{Result, anyhow};
use clap::Subcommand;
use sui_crypto::ed25519::Ed25519PrivateKey;

use crate::tx_utils;

#[derive(Debug, Subcommand)]
pub enum UserCommands {
    #[command(
        name = "list-multisigs",
        about = "List all multisigs the user is a member of"
    )]
    ListMultisigs,
    #[command(
        name = "join-multisig",
        about = "Insert a multisig id to the user object"
    )]
    JoinMultisig { multisig_id: String },
    #[command(
        name = "leave-multisig",
        about = "Remove a multisig from the user object"
    )]
    LeaveMultisig { multisig_id: String },
    #[command(
        name = "list-invites",
        about = "List all invites the user has received"
    )]
    ListInvites,
    #[command(name = "accept-invite", about = "Accept an invite")]
    AcceptInvite { invite_id: String },
    #[command(name = "refuse-invite", about = "Refuse an invite")]
    RefuseInvite { invite_id: String },
}

impl UserCommands {
    pub async fn run(&self, client: &mut MultisigClient, pk: &Ed25519PrivateKey) -> Result<()> {
        let user = client.user().ok_or(anyhow!("User not found"))?;

        match self {
            UserCommands::ListMultisigs => {
                println!("\n=== MULTISIGS ===\n");
                for multisig in &user.multisigs {
                    println!("{} - {}", multisig.id, multisig.name);
                }
                Ok(())
            },
            UserCommands::JoinMultisig { multisig_id } => {
                let addr = pk.public_key().derive_address();
                let mut builder = tx_utils::init(client.sui(), addr).await?;
                user.join_multisig(&mut builder, multisig_id.parse().unwrap())
                    .await?;
                tx_utils::execute(client.sui(), builder, pk).await?;
                Ok(())
            },
            UserCommands::LeaveMultisig { multisig_id } => {
                let addr = pk.public_key().derive_address();
                let mut builder = tx_utils::init(client.sui(), addr).await?;
                user.leave_multisig(&mut builder, multisig_id.parse().unwrap())
                    .await?;
                tx_utils::execute(client.sui(), builder, pk).await?;
                Ok(())
            },
            UserCommands::ListInvites => {
                println!("\n=== INVITES ===");
                for invite in &user.invites {
                    println!("\nInvite: {}", invite.id);
                    println!("Multisig: {} - {}", invite.multisig_id, invite.multisig_name);
                }
                Ok(())
            },
            UserCommands::AcceptInvite { invite_id } => {
                let addr = pk.public_key().derive_address();
                let mut builder = tx_utils::init(client.sui(), addr).await?;
                user.accept_invite(&mut builder, invite_id.parse().unwrap())
                    .await?;
                tx_utils::execute(client.sui(), builder, pk).await?;
                Ok(())
            },
            UserCommands::RefuseInvite { invite_id } => {
                let addr = pk.public_key().derive_address();
                let mut builder = tx_utils::init(client.sui(), addr).await?;
                user.refuse_invite(&mut builder, invite_id.parse().unwrap())
                    .await?;
                tx_utils::execute(client.sui(), builder, pk).await?;
                Ok(())
            },
        }
    }
}
