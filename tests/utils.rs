use base64ct::{Base64, Encoding};
use sui_crypto::ed25519::Ed25519PrivateKey;
use sui_crypto::SuiSigner;
use sui_graphql_client::{Client, PaginationFilter};
use sui_sdk_types::{Address, ExecutionStatus, ObjectIn, ObjectOut, TransactionEffects};
use sui_transaction_builder::{unresolved::Input, TransactionBuilder};

/// Helper function to setup a transaction builder with a gas object and a sender address.
pub async fn init_tx(sui_client: &Client) -> (Ed25519PrivateKey, TransactionBuilder) {
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

    (pk, builder)
}

pub async fn execute_tx(
    sui_client: &Client,
    pk: Ed25519PrivateKey,
    builder: TransactionBuilder,
) -> TransactionEffects {
    // execute the transaction
    let tx = builder.finish().unwrap();
    let sig = pk.sign_transaction(&tx).unwrap();
    let effects = sui_client.execute_tx(vec![sig], &tx).await;
    assert!(effects.is_ok(), "Execution failed. Effects: {:?}", effects);
    // wait for the transaction to be finalized
    while sui_client.transaction(tx.digest()).await.unwrap().is_none() {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    assert_eq!(
        ExecutionStatus::Success,
        effects.as_ref().unwrap().as_ref().unwrap().status().clone()
    );
    effects.unwrap().unwrap()
}

pub async fn get_created_multisig(effects: &TransactionEffects) -> Address {
    match effects {
        TransactionEffects::V1(_) => panic!("V1 not supported"),
        TransactionEffects::V2(effects) => effects
            .changed_objects
            .iter()
            .filter(|obj| {
                obj.input_state == ObjectIn::NotExist && obj.output_state != ObjectOut::NotExist
            })
            .collect::<Vec<_>>()
            .first()
            .unwrap()
            .object_id
            .into(),
    }
}