use anyhow::Result;
use move_types::Address;

use account_multisig_sdk::MultisigClient;

#[tokio::main]
async fn main() -> Result<()> {
    let mut client = MultisigClient::new_testnet();
    client.load_multisig(Address::from_hex("0x0d78f55193c6be44b68cc7f8e8324d7166ef7d11b55031fa07c0f3a6e4bd1159").unwrap()).await?;

    // println!("{:#?}", client.multisig().unwrap().config);
    // if let Some(intents) = client.intents() { println!("{:#?}", intents.get_intent("config_multisig").unwrap()) };

    Ok(())
}