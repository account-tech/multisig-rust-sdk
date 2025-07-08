use anyhow::Result;
use move_types::Address;

use account_multisig_sdk::MultisigClient;

#[tokio::main]
async fn main() -> Result<()> {
    let mut client = MultisigClient::new_testnet();
    client.load_multisig(Address::from_hex("0xbd4128161c82c7b58e320c2cf7ed10a0bffc3de1859593879c15875800bda672").unwrap()).await?;

    println!("{:#?}", client.multisig().unwrap().config);
    if let Some(intents) = client.intents() { println!("{:#?}", intents.get_intent("borrow_cap_again").unwrap()) };

    Ok(())
}