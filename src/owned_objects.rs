use anyhow::{anyhow, Result};
use serde_json::{Map, Value};
use std::fmt;
use std::sync::Arc;
use sui_graphql_client::Client;
use sui_sdk_types::Address;

use crate::utils;

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
        let move_values = utils::get_objects(&self.sui_client, multisig_id).await?;

        for move_value in move_values {
            let fields = move_value
                .json
                .and_then(|json| json.as_object().cloned())
                .ok_or(anyhow!("Could not parse object"))?;

            let id = fields
                .get("id")
                .and_then(|id| id.as_str())
                .ok_or(anyhow!("Could not get object id"))?
                .parse::<Address>()?;

            let type_ = move_value.type_.repr;

            if type_.starts_with("0x0000000000000000000000000000000000000000000000000000000000000002::coin::Coin") {
                let balance = fields
                    .get("balance")
                    .and_then(|bal| bal.get("value"))
                    .and_then(|v| v.as_str())
                    .ok_or(anyhow!("Could not get coin balance"))?
                    .parse::<u64>()?;
                self.coins.push(Coin { type_, id, balance });
            } else {
                self.objects.push(Object { type_, id, fields });
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
