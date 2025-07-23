use account_multisig_cli::commands::{
    cap::CapCommands, config::ConfigCommands, create::create_multisig, currency::CurrencyCommands,
    deps::DepsCommands, owned::OwnedCommands, proposal::ProposalCommands, user::UserCommands,
    vault::VaultCommands,
};
use account_multisig_sdk::MultisigClient;
use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use colored::*;
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
    #[command(name = "user", about = "Manage user")]
    User {
        #[command(subcommand)]
        command: UserCommands,
    },
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
    #[command(
        name = "proposals",
        about = "Display proposals, pass key to operate on"
    )]
    Proposals {
        /// Proposal key to operate on. If not provided, lists all proposals.
        /// If provided without a subcommand, shows proposal details.
        key: Option<String>,
        #[command(subcommand)]
        proposal_command: Option<ProposalCommands>,
    },
    #[command(name = "config", about = "Manage multisig config")]
    Config {
        #[command(subcommand)]
        command: Option<ConfigCommands>,
    },
    #[command(name = "deps", about = "Manage dependencies")]
    Deps {
        #[command(subcommand)]
        command: Option<DepsCommands>,
    },
    #[command(name = "cap", about = "Manage Caps")]
    Cap {
        #[command(subcommand)]
        command: Option<CapCommands>,
    },
    #[command(name = "currency", about = "Manage currencies")]
    Currency {
        #[command(subcommand)]
        command: Option<CurrencyCommands>,
    },
    #[command(name = "owned", about = "Manage owned objects")]
    Owned {
        #[command(subcommand)]
        command: Option<OwnedCommands>,
    },
    #[command(name = "vault", about = "Manage vaults")]
    Vault {
        #[command(subcommand)]
        command: Option<VaultCommands>,
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

    println!("{}", "Loading user...".yellow().italic());
    client.load_user(active_addr.to_inner().into()).await?;

    if let Some(id) = std::env::args().nth(2) {
        println!("{}", "Loading multisig...".yellow().italic());
        client
            .load_multisig(id.parse().map_err(|_| anyhow!("Invalid multisig id"))?)
            .await?;
    }

    loop {
        print!("{}", "\nmultisig> ".cyan());
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
            Ok(app) => match app.command {
                Commands::Exit => {
                    break;
                }
                Commands::User { command } => {
                    command.run(&mut client, &ed25519_pk).await?;
                }
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
                } => match (key, proposal_command) {
                    (Some(key), Some(proposal_command)) => {
                        proposal_command
                            .run(&mut client, &ed25519_pk, key.as_str())
                            .await?;
                    }
                    (Some(key), None) => {
                        let intent = client.intent_mut(key.as_str())?;
                        println!("\n{}", "=== PROPOSAL ===".bold());
                        println!("\n{}", "Details:".underline());
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
                        println!("\n{}", "Current outcome:".underline());
                        println!("Total weight: {}", intent.outcome.total_weight);
                        println!("Role weight: {}", intent.outcome.role_weight);
                        print!("Approved by: ");
                        for address in &intent.outcome.approved {
                            print!("{}", address);
                        }
                        let actions = intent.get_actions_args().await?;
                        println!("\n\n{}", "Actions:".underline());
                        println!("{:#?}", actions);
                    }
                    (None, None) => {
                        println!("\n{}\n", "=== PROPOSALS ===".bold());
                        let intents = client.intents().ok_or(anyhow!("Intents not loaded"))?;
                        for (key, intent) in &intents.intents {
                            println!("{} - {}", key, intent.type_);
                        }
                    }
                    _ => {
                        eprintln!("Invalid command");
                    }
                },
                Commands::Config { command } => match command {
                    Some(command) => {
                        command.run(&mut client, &ed25519_pk).await?;
                    }
                    None => {
                        let multisig = client.multisig().ok_or(anyhow!("Multisig not loaded"))?;
                        println!("\n{}", "=== MULTISIG CONFIG ===".bold());
                        println!("\n{} ", "Name:".underline());
                        println!(
                            "{}",
                            multisig.metadata.get("name").unwrap_or(&"".to_string())
                        );
                        println!("\n{}", "Members:".underline());
                        for member in &multisig.config.members {
                            println!(
                                "{} - {} - [{}]",
                                member.address,
                                member.weight,
                                member.roles.join(", ")
                            );
                        }
                        println!("\n{}", "Thresholds:".underline());
                        println!("Global: {}", multisig.config.global.threshold);
                        for (name, role) in &multisig.config.roles {
                            println!("{}: {}", name, role.threshold);
                        }
                    }
                },
                Commands::Deps { command } => match command {
                    Some(command) => {
                        command.run(&mut client, &ed25519_pk).await?;
                    }
                    None => {
                        let multisig = client.multisig().ok_or(anyhow!("Multisig not loaded"))?;
                        println!("\n{}\n", "=== DEPENDENCIES ===".bold());
                        for dep in &multisig.deps {
                            println!("{} - V{} - {}", dep.addr, dep.version, dep.name);
                        }
                    }
                },
                Commands::Cap { command } => match command {
                    Some(command) => {
                        command.run(&mut client, &ed25519_pk).await?;
                    }
                    None => {
                        let multisig = client.multisig().ok_or(anyhow!("Multisig not loaded"))?;
                        println!("\n{}\n", "=== CAPS ===".bold());
                        for cap in &multisig.dynamic_fields.as_ref().unwrap().caps {
                            println!("{}", cap.type_);
                        }
                    }
                },
                Commands::Currency { command } => match command {
                    Some(command) => {
                        command.run(&mut client, &ed25519_pk).await?;
                    }
                    None => {
                        let multisig = client.multisig().ok_or(anyhow!("Multisig not loaded"))?;
                        println!("\n{}\n", "=== CURRENCIES ===".bold());
                        for currency in &multisig.dynamic_fields.as_ref().unwrap().currencies {
                            println!("{}:", currency.0.underline());
                            println!(
                                "Max supply: {}",
                                currency
                                    .1
                                    .max_supply
                                    .map_or("None".to_string(), |max| max.to_string())
                            );
                            let mut enabled = vec![];
                            let mut disabled = vec![];
                            if currency.1.can_mint {
                                enabled.push("mint")
                            } else {
                                disabled.push("mint")
                            };
                            if currency.1.can_burn {
                                enabled.push("burn")
                            } else {
                                disabled.push("burn")
                            };
                            if currency.1.can_update_symbol {
                                enabled.push("update_symbol")
                            } else {
                                disabled.push("update_symbol")
                            };
                            if currency.1.can_update_name {
                                enabled.push("update_name")
                            } else {
                                disabled.push("update_name")
                            };
                            if currency.1.can_update_description {
                                enabled.push("update_description")
                            } else {
                                disabled.push("update_description")
                            };
                            if currency.1.can_update_icon {
                                enabled.push("update_icon")
                            } else {
                                disabled.push("update_icon")
                            };
                            println!("Enabled: {}", enabled.join(", "));
                            println!("Disabled: {}", disabled.join(", "));
                        }
                    }
                },
                Commands::Owned { command } => match command {
                    Some(command) => {
                        command.run(&mut client, &ed25519_pk).await?;
                    }
                    None => {
                        let multisig = client.multisig().ok_or(anyhow!("Multisig not loaded"))?;
                        println!("\n{}\n", "=== OWNED OBJECTS ===".bold());
                        println!("\n{}", "Coins:".underline());
                        let mut coins = multisig.owned_objects.as_ref().unwrap().coins.clone();
                        coins.sort_by(|a, b| a.type_.cmp(&b.type_));
                        for coin in coins {
                            println!("{} - {} - {}", coin.type_, coin.balance, coin.id);
                        }
                        println!("\n{}", "Objects:".underline());
                        let mut objects = multisig.owned_objects.as_ref().unwrap().objects.clone();
                        objects.sort_by(|a, b| a.type_.cmp(&b.type_));
                        for object in objects {
                            println!("{} - {}", object.type_, object.id);
                        }
                    }
                },
                Commands::Vault { command } => match command {
                    Some(command) => {
                        command.run(&mut client, &ed25519_pk).await?;
                    }
                    None => {
                        let multisig = client.multisig().ok_or(anyhow!("Multisig not loaded"))?;
                        println!("\n{}\n", "=== VAULTS ===".bold());
                        if let Some(dynamic_fields) = multisig.dynamic_fields.as_ref() {
                            for (vault_name, vault) in &dynamic_fields.vaults {
                                println!("{}:", vault_name.underline());
                                for (coin_type, amount) in &vault.coins {
                                    println!("{} - {}", coin_type, amount);
                                }
                            }
                        }
                    }
                },
            },
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }

    Ok(())
}
