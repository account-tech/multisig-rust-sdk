use std::str::FromStr;
use anyhow::Result;
use sui_sdk_types::TransactionDigest;

use account_multisig_sdk::MultisigClient;

#[tokio::main]
async fn main() -> Result<()> {
    let client = MultisigClient::new_testnet();

    let resp = client.sui().transaction(TransactionDigest::from_str("9WofMkDHjFeMLmS56azAsiYLQZFsSNeh7UzdkpFADgRM").unwrap()).await?;
    println!("{:#?}", resp);
    // let multisig = Multisig::from_id(Arc::new(client), Address::from_hex("0xbd4128161c82c7b58e320c2cf7ed10a0bffc3de1859593879c15875800bda672").unwrap()).await?;

    // println!("{}", multisig.fee_recipient());
    // if let Some(intents) = multisig.intents() { println!("{:#?}", intents.get_intent("config_multisig").unwrap().get_actions_args().await?) };
    // if let Some(intents) = multisig.intents() { println!("{:#?}", intents) };

    Ok(())
}