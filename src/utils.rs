use anyhow::{anyhow, Result};
use cynic::QueryBuilder;
use sui_sdk_types::{Address, Argument, Object};
use sui_graphql_client::{query_types::{MoveValue, ObjectFilter, ObjectsQuery, ObjectsQueryArgs}, Client, Direction, DynamicFieldOutput, PaginationFilter};
use sui_transaction_builder::{TransactionBuilder, Serialized, unresolved::Input};

pub async fn get_object(sui_client: &Client, id: Address) -> Result<Object> {
    sui_client
        .object(id, None)
        .await?
        .ok_or(anyhow!("Object not found {}", id))
}

// gets `MoveValue`s from sui-graphql-client (to get the fields json)
pub async fn get_objects(sui_client: &Client, id: Address) -> Result<Vec<MoveValue>> {
    let mut move_values = Vec::new();

    let mut cursor = None;
    let mut has_next_page = true;
    while has_next_page {
        let operation = ObjectsQuery::build(ObjectsQueryArgs {
            after: cursor.as_deref(),
            before: None,
            filter: Some(ObjectFilter {
                owner: Some(id),
                ..Default::default()
            }),
            first: Some(50),
            last: None,
        });

        let response = sui_client.run_query(&operation).await?;
        if let Some(errors) = response.errors {
            return Err(anyhow!("GraphQL error: {:?}", errors));
        }

        if let Some(objects) = response.data {
            for object in objects.objects.nodes {
                let move_value = object
                    .as_move_object
                    .and_then(|move_object| move_object.contents)
                    .ok_or(anyhow!("Could not get object type"))?;
                move_values.push(move_value);
            }

            cursor = objects.objects.page_info.end_cursor;
            has_next_page = objects.objects.page_info.has_next_page;
        }
    }

    Ok(move_values)
}

pub async fn get_dynamic_fields(sui_client: &Client, id: Address) -> Result<Vec<DynamicFieldOutput>> {
    let mut df_outputs = Vec::new();
    let mut cursor = None;
    let mut has_next_page = true;

    while has_next_page {
        let filter = PaginationFilter {
            direction: Direction::Forward,
            cursor: cursor.clone(),
            limit: Some(50),
        };

        let resp = sui_client
            .dynamic_fields(id, filter)
            .await?;
        df_outputs.extend(resp.data().iter().cloned());

        cursor = resp.page_info().end_cursor.clone();
        has_next_page = resp.page_info().has_next_page;
    }

    Ok(df_outputs)
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