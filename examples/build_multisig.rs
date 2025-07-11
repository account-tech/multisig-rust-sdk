use anyhow::Result;
use base64ct::{Base64, Encoding};
use sui_crypto::{ed25519::Ed25519PrivateKey, SuiSigner};
use sui_graphql_client::{Client, PaginationFilter};
use sui_sdk_types::ExecutionStatus;
use sui_transaction_builder::{unresolved::Input, TransactionBuilder};

use account_multisig_sdk::{MultisigBuilder, MultisigClient};

#[tokio::main]
async fn main() -> Result<()> {
    let mut client = MultisigClient::new_testnet();
    let mut builder = init_tx(client.sui()).await;
    client.load_user("0x3c00d56434d581fdfd6e280626f7c8ee75cc9dac134d84290491e65f9b8b7161".parse().unwrap()).await?;

    MultisigBuilder::new(&mut client, &mut builder)
        .set_name("test")
        .set_global_threshold(1)
        .add_member("0x3c00d56434d581fdfd6e280626f7c8ee75cc9dac134d84290491e65f9b8b7161", 1, vec!["f477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::currency_intents"])
        .add_member("0xbd4128161c82c7b58e320c2cf7ed10a0bffc3de1859593879c15875800bda672", 2, vec![])
        .add_role("f477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::currency_intents", 1)
        .build()
        .await?;

    execute_tx(client.sui(), builder).await;

    Ok(())
}

async fn init_tx(sui_client: &Client) -> TransactionBuilder {
    let pk = Ed25519PrivateKey::new(
        (&Base64::decode_vec("AM06bExREdFceWiExfSacTJ+64AQtFl7SRkSiTmAqh6F").unwrap()[1..])
            .try_into()
            .unwrap(),
    );
    let address = pk.public_key().derive_address();

    let mut builder = TransactionBuilder::new();

    let gas_coin = sui_client
        .coins(
            address,
            Some("0x2::coin::Coin<0x2::sui::SUI>"),
            PaginationFilter::default(),
        )
        .await
        .unwrap()
        .data()
        .first()
        .unwrap()
        .to_owned();
    let gas_input: Input = (&sui_client
        .object(gas_coin.id().to_owned().into(), None)
        .await
        .unwrap()
        .unwrap())
        .into();

    builder.add_gas_objects(vec![gas_input.with_owned_kind()]);
    builder.set_gas_budget(100000000);
    builder.set_gas_price(1000);
    builder.set_sender(address);

    builder
}

async fn execute_tx(sui_client: &Client, builder: TransactionBuilder) {
    let pk = Ed25519PrivateKey::new(
        (&Base64::decode_vec("AM06bExREdFceWiExfSacTJ+64AQtFl7SRkSiTmAqh6F").unwrap()[1..])
            .try_into()
            .unwrap(),
    );
    let tx = builder.finish().unwrap();
    let sig = pk.sign_transaction(&tx).unwrap();

    let effects = sui_client.execute_tx(vec![sig], &tx).await;
    assert!(effects.is_ok(), "Execution failed. Effects: {:?}", effects);
    // wait for the transaction to be finalized
    while sui_client.transaction(tx.digest()).await.unwrap().is_none() {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    // check that it succeeded
    println!("Effects: {:#?}", &effects);
    let status = effects.unwrap();
    let expected_status = ExecutionStatus::Success;
    assert_eq!(&expected_status, status.as_ref().unwrap().status());

    println!("Transaction executed successfully");
}
