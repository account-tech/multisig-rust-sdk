use anyhow::{Result, anyhow};
use sui_crypto::{SuiSigner, ed25519::Ed25519PrivateKey};
use sui_graphql_client::{Client, PaginationFilter};
use sui_sdk_types::{Address, ExecutionStatus};
use sui_transaction_builder::{TransactionBuilder, unresolved::Input};

pub async fn init(sui_client: &Client, address: Address) -> Result<TransactionBuilder> {
    let mut builder = TransactionBuilder::new();

    let gas_coin = sui_client
        .coins(
            address,
            Some("0x2::coin::Coin<0x2::sui::SUI>"),
            PaginationFilter::default(),
        )
        .await?
        .data()
        .first()
        .ok_or(anyhow!("No SUI coin found"))?
        .to_owned();
    let gas_input: Input = (&sui_client
        .object(gas_coin.id().to_owned().into(), None)
        .await?
        .ok_or(anyhow!("Gas coin not found"))?)
        .into();

    builder.add_gas_objects(vec![gas_input.with_owned_kind()]);
    builder.set_gas_budget(100000000);
    builder.set_gas_price(1000);
    builder.set_sender(address);

    Ok(builder)
}

pub async fn execute(
    sui_client: &Client,
    builder: TransactionBuilder,
    pk: &Ed25519PrivateKey,
) -> Result<()> {
    let tx = builder.finish()?;
    let sig = pk.sign_transaction(&tx)?;

    let effects = sui_client.execute_tx(vec![sig], &tx).await;
    // check that it succeeded
    assert!(effects.is_ok(), "Execution failed. Effects: {:?}", effects);
    // wait for the transaction to be finalized
    while sui_client.transaction(tx.digest()).await?.is_none() {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    // display effects
    println!("Effects: {:#?}", &effects);
    assert_eq!(&ExecutionStatus::Success, effects?.unwrap().status());

    println!("Transaction executed successfully");
    Ok(())
}
