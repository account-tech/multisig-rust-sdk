use account_multisig_cli::commands::{create::create_multisig, intent::IntentCommands};
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
    #[command(name = "load", about = "Load a multisig data and cache it")]
    Load { id: String },
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
    #[command(name = "intents", about = "Manage intents")]
    Intents {
        key: Option<String>,
        #[command(subcommand)]
        intent_command: Option<IntentCommands>,
    },
}
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    println!("Multisig CLI - Interactive Mode");
    println!("Type 'help' for commands, 'exit' to quit");

    let mut wallet_context =
        WalletContext::new(&sui_config_dir()?.join(SUI_CLIENT_CONFIG), None, None)?;
    let active_addr = wallet_context.active_address()?;
    let signer = wallet_context.config.keystore.get_key(&active_addr)?;

    let bytes = match signer {
        SuiKeyPair::Ed25519(kp) => Ok(kp.as_bytes()),
        _ => Err(anyhow!("Only ed25519 keys are supported")),
    };
    let ed25519_pk = Ed25519PrivateKey::new(bytes?.try_into()?);

    let network = std::env::args().nth(1).unwrap_or("mainnet".to_string());
    let mut client = match network.as_str() {
        "testnet" => MultisigClient::new_testnet(),
        "mainnet" => MultisigClient::new_mainnet(),
        url => MultisigClient::new_with_url(url)?,
    };
    client.load_user(active_addr.to_inner().into()).await?;

    loop {
        print!("multisig> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        if input == "exit" || input == "quit" {
            break;
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
                    Commands::Load { id } => {
                        client.load_multisig(id.parse().unwrap()).await?;
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
                    Commands::Intents {
                        key,
                        intent_command,
                    } => {
                        match (key, intent_command) {
                            (Some(key), Some(intent_command)) => {
                                // assert key
                                // match command
                            }
                            (Some(key), None) => {
                                // get intent
                            }
                            (None, None) => {
                                // list intents
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
