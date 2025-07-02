use anyhow::{anyhow, Result};
use sui_sdk_types::{Address, Argument, Object};
use sui_graphql_client::Client;
use sui_transaction_builder::{TransactionBuilder, Serialized, unresolved::Input};

pub async fn get_object(sui_client: &Client, id: Address) -> Result<Object> {
    sui_client
        .object(id, None)
        .await?
        .ok_or(anyhow!("Object not found {}", id))
}

pub fn pure_as_argument<T: serde::Serialize + move_types::MoveType>(
    builder: &mut TransactionBuilder,
    pure_value: &T
) -> Argument {
    builder.input(Serialized(pure_value))
}

pub async fn object_ref_as_argument(
    sui_client: &Client,
    builder: &mut TransactionBuilder,
    id: Address
) -> Result<Argument> {
    let object = get_object(sui_client, id).await?;
    let argument = builder.input(Input::from(&object).by_ref());
    
    Ok(argument)
}

pub async fn object_mut_as_argument(
    sui_client: &Client,
    builder: &mut TransactionBuilder,
    id: Address
) -> Result<Argument> {
    let object = get_object(sui_client, id).await?;
    let argument = builder.input(Input::from(&object).by_mut());
    
    Ok(argument)
}

pub async fn object_val_as_argument(
    sui_client: &Client,
    builder: &mut TransactionBuilder,
    id: Address
) -> Result<Argument> {
    let object = get_object(sui_client, id).await?;
    let argument = builder.input(Input::from(&object).by_val());
    
    Ok(argument)
}