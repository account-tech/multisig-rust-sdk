use anyhow::Result;
use move_types::Address;
use sui_graphql_client::Client;

use account_multisig_sdk::MultisigClient;

#[tokio::main]
async fn main() -> Result<()> {
    let mut client = MultisigClient::new_testnet();
    client.load_user(Address::from_hex("0x3c00d56434d581fdfd6e280626f7c8ee75cc9dac134d84290491e65f9b8b7161").unwrap()).await?;

    println!("{:#?}", client.user().unwrap());
    Ok(())
}