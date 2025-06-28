use anyhow::Result;
use cynic::QueryBuilder;
use serde_json::{Map, Value};
use std::fmt;
use std::sync::Arc;

use sui_graphql_client::query_types::{ObjectFilter, ObjectsQuery, ObjectsQueryArgs};
use sui_graphql_client::Client;
use sui_sdk_types::Address;

pub struct OwnedObjects {
    pub sui_client: Arc<Client>,
    pub coins: Vec<Coin>,
    pub objects: Vec<Object>,
}

#[derive(Debug)]
pub struct Coin {
    pub type_: String,
    pub id: Address,
    pub balance: u64,
}

#[derive(Debug)]
pub struct Object {
    pub type_: String,
    pub id: Address,
    pub fields: Map<String, Value>,
}

impl OwnedObjects {
    pub async fn from_multisig_id(sui_client: Arc<Client>, multisig_id: Address) -> Result<Self> {
        let mut owned_objects = Self {
            sui_client,
            coins: Vec::new(),
            objects: Vec::new(),
        };
        owned_objects.refresh(multisig_id).await?;
        Ok(owned_objects)
    }

    pub async fn refresh(&mut self, multisig_id: Address) -> Result<()> {
        let mut cursor = None;
        let mut has_next_page = true;

        while has_next_page {
            let operation = ObjectsQuery::build(ObjectsQueryArgs {
                after: cursor.as_deref(),
                before: None,
                filter: Some(ObjectFilter {
                    owner: Some(multisig_id),
                    ..Default::default()
                }),
                first: Some(50),
                last: None,
            });

            let response = self.sui_client.run_query(&operation).await?;
            if let Some(errors) = response.errors {
                return Err(anyhow::anyhow!("GraphQL error: {:?}", errors));
            }

            if let Some(objects) = response.data {
                for object in objects.objects.nodes {
                    let contents = object
                        .as_move_object
                        .and_then(|move_object| move_object.contents)
                        .ok_or(anyhow::anyhow!("Could not get object type"))?;

                    let fields = contents
                        .json
                        .and_then(|json| json.as_object().cloned())
                        .ok_or(anyhow::anyhow!("Could not parse object"))?;

                    let id = fields
                        .get("id")
                        .and_then(|id| id.as_str())
                        .ok_or(anyhow::anyhow!("Could not get object id"))?
                        .parse::<Address>()?;

                    let type_ = contents.type_.repr;

                    if type_.starts_with("0x0000000000000000000000000000000000000000000000000000000000000002::coin::Coin") {
                        let balance = fields
                            .get("balance")
                            .and_then(|bal| bal.get("value"))
                            .and_then(|v| v.as_str())
                            .ok_or(anyhow::anyhow!("Could not get coin balance"))?
                            .parse::<u64>()?;
                        self.coins.push(Coin { type_, id, balance });
                    } else {
                        self.objects.push(Object { type_, id, fields });
                    }
                }

                cursor = objects.objects.page_info.end_cursor;
                has_next_page = objects.objects.page_info.has_next_page;
            }
        }

        Ok(())
    }
}

impl fmt::Debug for OwnedObjects {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OwnedObjects")
            .field("coins", &self.coins)
            .field("objects", &self.objects)
            .finish()
    }
}
