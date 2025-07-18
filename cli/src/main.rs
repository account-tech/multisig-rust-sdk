use account_multisig_cli::commands::{create::create_multisig, proposal::ProposalCommands};
use account_multisig_sdk::MultisigClient;
use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use std::io::{self, Write};
use sui_config::{SUI_CLIENT_CONFIG, sui_config_dir};
use sui_crypto::ed25519::Ed25519PrivateKey;
use sui_keys::keystore::AccountKeystore;
use sui_sdk::{
    types::crypto::{SuiKeyPair, ToFromBytes},
    wallet_context::WalletContext,
};

#[derive(Debug, Parser)]
#[command(name = "account-multisig", version, about, long_about = None)]
struct App {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    #[command(name = "exit", about = "Exit the CLI")]
    Exit,
    // #[command(name = "user", about = "Manage user")]
    // UserCommands
    #[command(name = "load", about = "Load a specific multisig or reload current")]
    Load { id: Option<String> },
    #[command(name = "create", about = "Create a new multisig")]
    Create {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        addresses: Option<Vec<String>>,
        #[arg(long)]
        weights: Option<Vec<u64>>,
        #[arg(long)]
        roles: Option<Vec<String>>,
        #[arg(long)]
        global_threshold: Option<u64>,
        #[arg(long)]
        role_names: Option<Vec<String>>,
        #[arg(long)]
        role_thresholds: Option<Vec<u64>>,
    },
    #[command(name = "proposals", about = "Display proposals, pass key to operate on")]
    Proposals {
        /// Proposal key to operate on. If not provided, lists all proposals.
        /// If provided without a subcommand, shows proposal details.
        key: Option<String>,
        #[command(subcommand)]
        proposal_command: Option<ProposalCommands>,
    },
}
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    println!("Multisig CLI - Interactive Mode");
    println!("Type 'help' for commands, 'exit' to quit");

    // get keypair from sui_config
    let mut wallet_context =
        WalletContext::new(&sui_config_dir()?.join(SUI_CLIENT_CONFIG), None, None)?;
    let active_addr = wallet_context.active_address()?;
    let signer = wallet_context.config.keystore.get_key(&active_addr)?;

    let bytes = match signer {
        SuiKeyPair::Ed25519(kp) => Ok(kp.as_bytes()),
        _ => Err(anyhow!("Only ed25519 keys are supported")),
    };
    let ed25519_pk = Ed25519PrivateKey::new(bytes?.try_into()?);

    // init cli with network and multisig id
    let network = std::env::args().nth(1).ok_or(anyhow!(
        "Network not specified: 'mainnet' 'testnet' or '<url>'"
    ))?;
    let mut client = match network.as_str() {
        "testnet" => MultisigClient::new_testnet(),
        "mainnet" => MultisigClient::new_mainnet(),
        url => MultisigClient::new_with_url(url)?,
    };
    client.load_user(active_addr.to_inner().into()).await?;
    if let Some(id) = std::env::args().nth(2) {
        client
            .load_multisig(id.parse().map_err(|_| anyhow!("Invalid multisig id"))?)
            .await?;
    }

    loop {
        print!("multisig> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        let args: Vec<&str> = input.split_whitespace().collect();
        let mut clap_args = vec!["acc-multisig"];
        clap_args.extend(args);
        match App::try_parse_from(clap_args) {
            Ok(app) => {
                match app.command {
                    Commands::Exit => {
                        break;
                    }
                    // Commands::U
                    Commands::Load { id } => {
                        if let Some(id) = id {
                            client.load_multisig(id.parse()?).await?;
                        } else {
                            client.refresh().await?;
                        }
                    }
                    Commands::Create {
                        name,
                        addresses,
                        weights,
                        roles,
                        global_threshold,
                        role_names,
                        role_thresholds,
                    } => {
                        create_multisig(
                            &client,
                            &ed25519_pk,
                            name,
                            addresses,
                            weights,
                            roles,
                            global_threshold,
                            role_names,
                            role_thresholds,
                        )
                        .await?;
                    }
                    Commands::Proposals {
                        key,
                        proposal_command,
                    } => {
                        match (key, proposal_command) {
                            (Some(key), Some(proposal_command)) => {
                                proposal_command.run(&mut client, &ed25519_pk, key.as_str()).await?;
                            }
                            (Some(key), None) => {
                                let intent = client.intent_mut(key.as_str())?;
                                println!("\n=== DETAILS ===\n");
                                println!("Name: {}", intent.key);
                                println!("Type: {}", intent.type_);
                                println!("Description: {}", intent.description);
                                println!("Multisig: {}", intent.account);
                                println!("Creator: {}", intent.creator);
                                println!("Creation time: {}", intent.creation_time);
                                print!("Execution times: ");
                                for time in &intent.execution_times {
                                    print!("{} ", time);
                                }
                                println!();
                                println!("Expiration time: {}", intent.expiration_time);
                                println!("Role: {}", intent.role);
                                println!("\n=== CURRENT OUTCOME ===\n");
                                println!("Total weight: {}", intent.outcome.total_weight);
                                println!("Role weight: {}", intent.outcome.role_weight);
                                print!("Approved by: ");
                                for address in &intent.outcome.approved {
                                    print!("{}", address);
                                }
                                let actions = intent.get_actions_args().await?;
                                println!("\n\n=== ACTIONS ===\n");
                                println!("{:#?}", actions);
                            }
                            (None, None) => {
                                println!("{}", client.intents().unwrap());
                            }
                            _ => {
                                eprintln!("Invalid command");
                            }
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }

    Ok(())
}
