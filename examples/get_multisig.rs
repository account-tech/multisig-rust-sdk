use std::sync::Arc;
use anyhow::Result;
use sui_graphql_client::Client;
use sui_sdk_types::Address;

use account_multisig_sdk::multisig::Multisig;

#[tokio::main]
async fn main() -> Result<()> {
    let client = Client::new_testnet();

    let multisig = Multisig::from_id(Arc::new(client), Address::from_hex("0xbd4128161c82c7b58e320c2cf7ed10a0bffc3de1859593879c15875800bda672").unwrap()).await?;

    // println!("{}", multisig.fee_recipient());
    if let Some(intents) = multisig.intents() { println!("{:#?}", intents.get_intent("config_multisig").unwrap().get_actions_args().await?) };
    // if let Some(intents) = multisig.intents() { println!("{:#?}", intents) };

    Ok(())
}