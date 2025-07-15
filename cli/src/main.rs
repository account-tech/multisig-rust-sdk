use std::io::{self, Write};
use anyhow::Result;
use clap::{Parser, Subcommand};
use account_multisig_sdk::{multisig_builder::Config, MultisigClient};
use account_multisig_cli::commands::intent::IntentCommands;

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
    #[command(name = "load", about = "Load a multisig data and cache it")]
    Load {
        id: String
    },
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
    }
    
}
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    
    println!("Multisig CLI - Interactive Mode");
    println!("Type 'help' for commands, 'exit' to quit");

    let mut client = MultisigClient::new_testnet();
    
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
        
        // Use Clap to parse the command
        let args: Vec<&str> = input.split_whitespace().collect();
        match App::try_parse_from(args) {
            Ok(app) => {
                match app.command {
                    Commands::Exit => {
                        break;
                    },
                    Commands::Load { id } => {
                        // load multisig
                    },
                    Commands::Create { name, addresses, weights, roles, global_threshold, role_names, role_thresholds } => {
                        // create multisig
                    },
                    Commands::Intents { key, intent_command } => {
                        match (key, intent_command) {
                            (Some(key), Some(intent_command)) => {
                                // assert key 
                                // match command 
                            },
                            (Some(key), None) => {
                                // get intent
                            },
                            (None, None) => {
                                // list intents
                            },
                            _ => {
                                eprintln!("Invalid command");
                            }
                        }
                    },
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }
    
    Ok(())
}