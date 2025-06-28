use anyhow::{anyhow, Ok, Result};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use sui_graphql_client::{Client, Direction, PaginationFilter};
use sui_sdk_types::{Address, TypeTag};

use crate::actions::{deserialize_actions, IntentActionsType};
use crate::move_binding::account_multisig as am;
use crate::move_binding::account_protocol as ap;

pub struct Intents {
    pub sui_client: Arc<Client>,
    pub bag_id: Address,
    pub intents: HashMap<String, Intent>,
}

pub struct Intent {
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

#[derive(Debug)]
pub struct Approvals {
    pub total_weight: u64,
    pub role_weight: u64,
    pub approved: Vec<Address>,
}

impl Intents {
    pub async fn from_bag_id(sui_client: Arc<Client>, bag_id: Address) -> Result<Self> {
        let mut intents = Self {
            sui_client,
            bag_id,
            intents: HashMap::new(),
        };
        intents.refresh().await?;
        Ok(intents)
    }

    pub async fn refresh(&mut self) -> Result<()> {
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
                        let intent: ap::intents::Intent<am::multisig::Approvals> =
                            bcs::from_bytes(&value.1)?;
                        self.intents.insert(
                            intent.key.clone(),
                            Intent {
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
                                    approved: intent.outcome.approved.contents,
                                },
                            },
                        );
                    }
                    None => Err(anyhow!("Intent not found"))?,
                }
            }

            cursor = resp.page_info().end_cursor.clone();
            has_next_page = resp.page_info().has_next_page;
        }

        Ok(())
    }

    pub fn get_intent(&self, key: &str) -> Option<&Intent> {
        self.intents.get(key)
    }
}

impl fmt::Display for Intents {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for intent in self.intents.values() {
            writeln!(f, "{}", intent)?;
        }

        fmt::Result::Ok(())
    }
}

impl fmt::Debug for Intents {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Intents")
            .field("bag_id", &self.bag_id)
            .field("intents_count", &self.intents.len())
            .field("intents", &self.intents)
            .finish()
    }
}

impl Intent {
    pub async fn get_actions_args(&self) -> Result<IntentActionsType> {
        let actions_bcs = self.fetch_actions_generics_and_bcs_contents().await?;
        deserialize_actions(&self.type_, &actions_bcs)
    }

    async fn fetch_actions_generics_and_bcs_contents(
        &self,
    ) -> Result<Vec<(Vec<TypeTag>, Vec<u8>)>> {
        let mut dfs = Vec::<(Vec<TypeTag>, Vec<u8>)>::new();
        let mut cursor = None;
        let mut has_next_page = true;

        while has_next_page {
            let filter = PaginationFilter {
                direction: Direction::Forward,
                cursor: cursor.clone(),
                limit: Some(50),
            };

            let resp = self
                .sui_client
                .dynamic_fields(self.actions_bag_id, filter)
                .await?;
            for df_output in resp.data() {
                if let Some(value) = &df_output.value {
                    let type_params = match &value.0 {
                        TypeTag::Struct(struct_tag) => struct_tag.type_params.clone(),
                        _ => vec![],
                    };
                    dfs.push((type_params, value.1.clone())); // generics + contents bcs
                }
            }

            cursor = resp.page_info().end_cursor.clone();
            has_next_page = resp.page_info().has_next_page;
        }

        Ok(dfs)
    }
}

impl fmt::Display for Intent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Intent: {}", self.key)
    }
}

impl fmt::Debug for Intent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Intent")
            .field("key", &self.key)
            .field("type", &self.type_)
            .field("description", &self.description)
            .field("account", &self.account)
            .field("creator", &self.creator)
            .field("creation_time", &self.creation_time)
            .field("execution_times", &self.execution_times)
            .field("expiration_time", &self.expiration_time)
            .field("role", &self.role)
            .field("actions_bag_id", &self.actions_bag_id)
            .field("actions_bcs", &self.actions_bcs)
            .field("outcome", &self.outcome)
            .finish()
    }
}
