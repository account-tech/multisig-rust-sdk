use std::sync::Arc;
use anyhow::Result;
use sui_graphql_client::Client;
use sui_sdk_types::Address;

use account_multisig_sdk::multisig::Multisig;

#[tokio::main]
async fn main() -> Result<()> {
    let client = Client::new_testnet();

    let mut multisig = Multisig::new(Arc::new(client), Address::from_hex("0xfdad7ba77f88e7d082787cb8a3d517bc58b533bee5950024ae4c7a5799a8979f").unwrap());
    multisig.fetch().await?;

    // println!("{}", multisig.fee_recipient());
    if let Some(intents) = multisig.intents() { println!("{:#?}", intents.get_intent("borrow").unwrap().get_actions_args().await?) };
    // if let Some(intents) = multisig.intents() { println!("{:#?}", intents) };

    Ok(())
}