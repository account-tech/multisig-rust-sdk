use std::sync::Arc;
use anyhow::Result;
use sui_graphql_client::Client;
use sui_sdk_types::Address;

use multisig_sdk::multisig::Multisig;

#[tokio::main]
async fn main() -> Result<()> {
    let client = Client::new_testnet();

    let mut multisig = Multisig::new(Arc::new(client), Address::from_hex("0x6de46a045f17ccb4ca0cd4c1051af3cb70ee54b385a86d5347b2eeb18c742bfb").unwrap());
    multisig.fetch().await?;

    println!("Multisig: {:#?}", multisig.id());
    println!("Multisig: {:#?}", multisig.metadata());
    println!("Multisig: {:#?}", multisig.deps());
    println!("Multisig: {:#?}", multisig.unverified_deps_allowed());
    println!("Multisig: {:#?}", multisig.intents_bag_id());
    println!("Multisig: {:#?}", multisig.locked_objects());
    println!("Members: {:#?}", multisig.config().members);
    println!("Global: {:#?}", multisig.config().global);
    println!("Fee Amount: {:#?}", multisig.fee_amount());
    println!("Fee Recipient: {:#?}", multisig.fee_recipient());

    Ok(())
}