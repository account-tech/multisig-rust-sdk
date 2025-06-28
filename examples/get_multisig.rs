use std::str::FromStr;
use anyhow::Result;
use move_types::Address;
use sui_sdk_types::TransactionDigest;

use account_multisig_sdk::MultisigClient;

#[tokio::main]
async fn main() -> Result<()> {
    let mut client = MultisigClient::new_testnet();
    client.load_multisig(Address::from_hex("0xbd4128161c82c7b58e320c2cf7ed10a0bffc3de1859593879c15875800bda672").unwrap()).await?;

    if let Some(objects) = client.owned_objects() { println!("{:#?}", objects) };
    // println!("{}", multisig.fee_recipient());
    // if let Some(intents) = multisig.intents() { println!("{:#?}", intents.get_intent("config_multisig").unwrap().get_actions_args().await?) };

    Ok(())
}