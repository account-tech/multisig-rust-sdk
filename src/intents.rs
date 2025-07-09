use anyhow::{Ok, Result};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use sui_graphql_client::Client;
use sui_sdk_types::{Address, TypeTag};

use crate::actions::{IntentType, IntentActionsType};
use crate::move_binding::account_multisig as am;
use crate::move_binding::account_protocol as ap;
use crate::utils;

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
    pub actions_types_bcs: Vec<(Vec<TypeTag>, Vec<u8>)>,
    pub actions_args: Option<IntentActionsType>,
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
        let df_outputs = utils::get_dynamic_fields(&self.sui_client, self.bag_id).await?;

        for df_output in df_outputs {
            if let Some(value) = &df_output.value {
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
                        actions_types_bcs: Vec::new(),
                        actions_args: None,
                        outcome: Approvals {
                            total_weight: intent.outcome.total_weight,
                            role_weight: intent.outcome.role_weight,
                            approved: intent.outcome.approved.contents,
                        },
                    },
                );
            }
        }

        Ok(())
    }

    pub fn get_intent(&self, key: &str) -> Option<&Intent> {
        self.intents.get(key)
    }

    pub fn get_intent_mut(&mut self, key: &str) -> Option<&mut Intent> {
        self.intents.get_mut(key)
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
    pub async fn get_actions_args(&mut self) -> Result<&IntentActionsType> {
        if self.actions_args.is_none() {
            let mut df_types_with_bcs = Vec::new();
            let df_outputs = utils::get_dynamic_fields(&self.sui_client, self.actions_bag_id).await?;
    
            for df_output in df_outputs {
                if let Some(value) = &df_output.value {
                    let type_params = match &value.0 {
                        TypeTag::Struct(struct_tag) => struct_tag.type_params.clone(),
                        _ => vec![],
                    };
                    df_types_with_bcs.push((type_params, value.1.clone())); // generics + contents bcs
                }
            }
            self.actions_types_bcs = df_types_with_bcs;
    
            let intent_type = IntentType::try_from(self.type_.as_str())?;
            self.actions_args = Some(intent_type.deserialize_actions(&self.actions_types_bcs)?);
        }
        Ok(self.actions_args.as_ref().unwrap())
    }

    pub async fn get_executions_count(&mut self) -> Result<usize> {
        let _ = self.get_actions_args().await?; // fetch actions args
        let intent_type = IntentType::try_from(self.type_.as_str())?;
        Ok(intent_type.count_repetitions(&self.actions_types_bcs)?)
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
            .field("actions_types_bcs", &self.actions_types_bcs)
            .field("outcome", &self.outcome)
            .finish()
    }
}
