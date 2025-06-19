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

    // println!("{}", multisig.fee_recipient());
    // if let Some(intents) = multisig.intents() { println!("{:#?}", intents.get_intent("config-multisig").unwrap().get_actions_args().await?) };
    if let Some(intents) = multisig.intents() { println!("{:#?}", intents) };

    Ok(())
}