use anyhow::{anyhow, Result};
use cynic::QueryBuilder;
use sui_graphql_client::{
    query_types::{MoveValue, ObjectFilter, ObjectsQuery, ObjectsQueryArgs, BigInt, schema},
    Client, Direction, DynamicFieldOutput, PaginationFilter,
};
use sui_sdk_types::{Address, Object, Owner};
use sui_transaction_builder::unresolved::Input;

pub async fn get_object(sui_client: &Client, id: Address) -> Result<Object> {
    sui_client
        .object(id, None)
        .await?
        .ok_or(anyhow!("Object not found {}", id))
}

pub async fn get_object_as_input(sui_client: &Client, id: Address) -> Result<Input> {
    let object = get_object(sui_client, id).await?;
    let mut input = Input::from(&object);

    input = match object.owner() {
        Owner::Address(_) => input.with_owned_kind(),
        Owner::Object(_) => input.with_owned_kind(),
        _ => input,
    };

    Ok(input)
}

pub async fn get_owned_objects(
    sui_client: &Client,
    owner: Address,
    type_: Option<&str>,
) -> Result<Vec<Object>> {
    let mut objects = Vec::new();
    let mut cursor = None;
    let mut has_next_page = true;

    while has_next_page {
        let filter = PaginationFilter {
            direction: Direction::Forward,
            cursor: cursor.clone(),
            limit: Some(50),
        };

        let resp = sui_client
            .objects(
                Some(ObjectFilter {
                    owner: Some(owner),
                    type_,
                    object_ids: None,
                }),
                filter
            )
            .await?;
        objects.extend(resp.data().iter().cloned());

        cursor = resp.page_info().end_cursor.clone();
        has_next_page = resp.page_info().has_next_page;
    }

    Ok(objects)
}

pub async fn get_objects(
    sui_client: &Client,
    mut ids: Vec<Address>,
) -> Result<Vec<Object>> {
    let mut objects = Vec::new();
    let mut cursor = None;
    let mut has_next_page = true;

    while has_next_page {
        let filter = PaginationFilter {
            direction: Direction::Forward,
            cursor: cursor.clone(),
            limit: Some(50),
        };

        let mut object_ids = Some(ids.clone());
        if ids.len() > 50 {
            object_ids = Some(ids.split_off(50));
        }

        let resp = sui_client
            .objects(
                Some(ObjectFilter {
                    object_ids,
                    ..Default::default()
                }),
                filter
            )
            .await?;
        objects.extend(resp.data().iter().cloned());

        cursor = resp.page_info().end_cursor.clone();
        has_next_page = resp.page_info().has_next_page;
    }

    Ok(objects)
}

// gets `MoveValue`s from sui-graphql-client (to get the fields json)
pub async fn get_objects_with_fields(sui_client: &Client, owner: Address, type_: Option<&str>) -> Result<Vec<MoveValue>> {
    let mut move_values = Vec::new();

    let mut cursor = None;
    let mut has_next_page = true;
    while has_next_page {
        let operation = ObjectsQuery::build(ObjectsQueryArgs {
            after: cursor.as_deref(),
            before: None,
            filter: Some(ObjectFilter {
                owner: Some(owner),
                type_,
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

pub async fn get_dynamic_fields(
    sui_client: &Client,
    id: Address,
) -> Result<Vec<DynamicFieldOutput>> {
    let mut objects = Vec::new();
    let mut cursor = None;
    let mut has_next_page = true;

    while has_next_page {
        let filter = PaginationFilter {
            direction: Direction::Forward,
            cursor: cursor.clone(),
            limit: Some(50),
        };

        let resp = sui_client.dynamic_fields(id, filter).await?;
        objects.extend(resp.data().iter().cloned());

        cursor = resp.page_info().end_cursor.clone();
        has_next_page = resp.page_info().has_next_page;
    }

    Ok(objects)
}

// CoinMetadata helpers 

#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "rpc", graphql_type = "Query", variables = "CoinMetadataArgs")]
pub struct CoinMetadataQuery {
    #[arguments(coinType: $coin_type)]
    pub coin_metadata: Option<CoinMetadata>,
}

#[derive(cynic::QueryVariables, Debug)]
pub struct CoinMetadataArgs<'a> {
    pub coin_type: &'a str,
}

/// The coin metadata associated with the given coin type.
#[derive(cynic::QueryFragment, Debug)]
#[cynic(schema = "rpc", graphql_type = "CoinMetadata")]
pub struct CoinMetadata {
    pub address: Address,
    /// The number of decimal places used to represent the token.
    pub decimals: Option<i32>,
    /// Optional description of the token, provided by the creator of the token.
    pub description: Option<String>,
    /// Icon URL of the coin.
    pub icon_url: Option<String>,
    /// Full, official name of the token.
    pub name: Option<String>,
    /// The token's identifying abbreviation.
    pub symbol: Option<String>,
    /// The overall quantity of tokens that will be issued.
    pub supply: Option<BigInt>,
    /// Version of the token.
    pub version: u64,
}

pub async fn coin_metadata(sui_client: &Client, coin_type: &str) -> Result<Option<CoinMetadata>> {
    let operation = CoinMetadataQuery::build(CoinMetadataArgs { coin_type });
    let response = sui_client.run_query(&operation).await?;

    if let Some(errors) = response.errors {
        return Err(anyhow!("GraphQL error: {:?}", errors));
    }

    Ok(response.data.and_then(|x| x.coin_metadata))
}
