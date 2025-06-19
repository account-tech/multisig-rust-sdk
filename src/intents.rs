use std::collections::HashMap;
use std::sync::Arc;
use std::fmt;
use anyhow::{anyhow, Ok, Result};
use serde::{Serialize};
use sui_graphql_client::{Client, PaginationFilter, Direction};
use sui_sdk_types::Address;

use crate::move_binding::account_protocol as ap;
use crate::move_binding::account_multisig as am;

pub struct Intents {
    pub sui_client: Arc<Client>,
    pub bag_id: Address,
    pub intents: HashMap<String, Intent>,
}

#[derive(Serialize)]
pub struct Intent {
    #[serde(skip)]
    pub sui_client: Arc<Client>,
    pub type_: String,
    pub key: String,
    pub description: String,
    pub account: Address,
    pub creator: Address,
    pub creation_time: u64,
    pub execution_times: Vec<u64>,
    pub expiration_time: u64,
    pub role: String,
    pub actions_bag_id: Address,
    pub actions_bcs: Vec<Vec<u8>>,
    pub outcome: Approvals,
}

#[derive(Serialize)]
pub struct Approvals {
    pub total_weight: u64,
    pub role_weight: u64,
    pub approved: Vec<Address>,
}

impl Intents {
    pub fn new(sui_client: Arc<Client>, bag_id: Address) -> Self {
        Self {
            sui_client,
            bag_id,
            intents: HashMap::new(),
        }
    }
    
    pub async fn fetch(&mut self) -> Result<()> {
        let mut cursor = None;
        let mut has_next_page = true;

        while has_next_page {
            let filter = PaginationFilter {
                direction: Direction::Forward,
                cursor: cursor.clone(),
                limit: Some(50),
            };

            let resp = self.sui_client.dynamic_fields(self.bag_id, filter).await?;
            for df_output in resp.data() {
                match &df_output.value {
                    Some(value) => {
                        let intent: ap::intents::Intent<am::multisig::Approvals> = bcs::from_bytes(&value.1)?;
                        self.intents.insert(intent.key.clone(), Intent { 
                            sui_client: self.sui_client.clone(), 
                            type_: intent.type_, 
                            key: intent.key, 
                            description: intent.description, 
                            account: intent.account, 
                            creator: intent.creator, 
                            creation_time: intent.creation_time, 
                            execution_times: intent.execution_times, 
                            expiration_time: intent.expiration_time, 
                            role: intent.role, 
                            actions_bag_id: intent.actions.id.into(), 
                            actions_bcs: Vec::new(), 
                            outcome: Approvals { 
                                total_weight: intent.outcome.total_weight, 
                                role_weight: intent.outcome.role_weight, 
                                approved: intent.outcome.approved.contents 
                            } 
                        });
                    },
                    None => Err(anyhow!("Intent not found"))?
                }
            }

            cursor = resp.page_info().end_cursor.clone();
            has_next_page = resp.page_info().has_next_page;
        }

        Ok(())
    }
}

impl Intent {
    pub async fn fetch_actions_bcs(&mut self) -> Result<()> {
        let mut dfs = Vec::<Vec<u8>>::new();
        let mut cursor = None;
        let mut has_next_page = true;

        while has_next_page {
            let filter = PaginationFilter {
                direction: Direction::Forward,
                cursor: cursor.clone(),
                limit: Some(50),
            };

            let resp = self.sui_client.dynamic_fields(self.actions_bag_id, filter).await?;
            for df_output in resp.data() {
                if let Some(value) = &df_output.value {
                    dfs.push(value.1.clone());
                }
            }

            cursor = resp.page_info().end_cursor.clone();
            has_next_page = resp.page_info().has_next_page;
        }

        Ok(())
    }

}

impl fmt::Display for Intents {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (key, intent) in &self.intents {
            writeln!(f, "{}: {}", key, intent)?;
        }
        fmt::Result::Ok(())
    }
}

impl fmt::Display for Intent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match serde_json::to_string_pretty(self) {
            std::result::Result::Ok(json) => write!(f, "{}", json),
            std::result::Result::Err(e) => write!(f, "<failed to serialize intent: {}>", e),
        }
    }
}