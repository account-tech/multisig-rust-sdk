use anyhow::{Ok, Result, anyhow};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use serde_json::Value;
use sui_sdk::SuiClient;
use sui_sdk::rpc_types::{SuiObjectDataOptions, SuiParsedData};
use sui_sdk::types::base_types::{ObjectID, SuiAddress};

pub struct Multisig {
    sui: Arc<SuiClient>,
    id: ObjectID,
    metadata: HashMap<String, String>,
    deps: Vec<Dep>,
    unverified_deps_allowed: bool,
    intents_bag_id: ObjectID,
    locked_objects: Vec<ObjectID>,
    config: Config,
}

#[derive(Debug, Default)]
pub struct Config {
    pub members: Vec<Member>,
    pub global: Role,
    pub roles: HashMap<String, Role>,
}

#[derive(Debug)]
pub struct Dep {
    pub name: String,
    pub addr: SuiAddress,
    pub version: u64,
}

#[derive(Debug, Default)]
pub struct Member {
    // social data
    pub username: String,
    pub avatar: String,
    // member data
    pub address: String,
    pub weight: u64,
    pub roles: Vec<String>,
}

#[derive(Debug, Default)]
pub struct Role {
    // threshold to reach for the role
    pub threshold: u64,
    // sum of the weight of the members with the role
    pub total_weight: u64,
}

impl Multisig {
    pub fn new(sui: Arc<SuiClient>, id: ObjectID) -> Self {
        Self {
            sui,
            id,
            metadata: HashMap::new(),
            deps: Vec::new(),
            unverified_deps_allowed: false,
            intents_bag_id: ObjectID::from_hex_literal("0x0").unwrap(),
            locked_objects: Vec::new(),
            config: Config::default(),
        }
    }

    pub async fn fetch(&mut self) -> Result<()> {
        let resp = self
            .sui
            .read_api()
            .get_object_with_options(self.id, SuiObjectDataOptions::new().with_content())
            .await?;

        let obj = resp.data.ok_or(anyhow!("Object not found"))?;
        if let SuiParsedData::MoveObject(content) = obj.content.unwrap() {
            let json = content.fields.to_json_value();

            // get the metadata map
            let metadata_struct = json.get("metadata").ok_or(anyhow!("No metadata field"))?;
            self.metadata = Self::get_metadata_field(metadata_struct)?;

            // get the deps array and unverified toggle
            let deps_struct = json.get("deps").ok_or(anyhow!("No deps field"))?;
            self.deps = Self::get_deps_field(deps_struct)?;
            self.unverified_deps_allowed = deps_struct.get("unverified_allowed").and_then(|v| v.as_bool()).ok_or(anyhow!("No unverified_deps_allowed field"))?;

            // get the intents bag id and locked objects array
            let intents_struct = json.get("intents").ok_or(anyhow!("No intents field"))?;
            self.intents_bag_id = Self::get_intents_bag_field(intents_struct)?;
            self.locked_objects = Self::get_locked_objects_field(intents_struct)?;

            // get the fields from the config/multisig struct
            let config_struct = json.get("config").ok_or(anyhow!("No config field"))?;
            self.config = Self::get_config_field(config_struct)?;

        } else {
            return Err(anyhow!("Not a MoveObject"));
        }

        Ok(())
    }

    // === Getters ===

    pub fn id(&self) -> ObjectID {
        self.id
    }

    pub fn metadata(&self) -> &HashMap<String, String> {
        &self.metadata
    }

    pub fn deps(&self) -> &Vec<Dep> {
        &self.deps
    }

    pub fn unverified_deps_allowed(&self) -> bool {
        self.unverified_deps_allowed
    }

    pub fn intents_bag_id(&self) -> ObjectID {
        self.intents_bag_id
    }

    pub fn locked_objects(&self) -> &Vec<ObjectID> {
        &self.locked_objects
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    // === Helpers ===

    fn get_metadata_field(metadata_struct: &Value) -> Result<HashMap<String, String>> {
        let metadata_field = metadata_struct
            .get("inner")
            .and_then(|inner| inner.get("contents"))
            .and_then(|contents| contents.as_array())
            .ok_or(anyhow!("Invalid metadata array"))?
            .iter()
            .map(|entry| {
                Ok((
                    entry
                        .get("key")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .ok_or(anyhow!("Invalid key"))?,
                    entry
                        .get("value")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .ok_or(anyhow!("Invalid value"))?
                    ))
            })
            .collect::<Result<HashMap<String, String>>>()?;
        
        Ok(metadata_field)
    }

    fn get_deps_field(deps_struct: &Value) -> Result<Vec<Dep>> {
        let deps_field = deps_struct
            .get("inner")
            .and_then(|inner| inner.as_array())
            .ok_or(anyhow!("Invalid deps array"))?
            .iter()
            .map(|entry| {
                Ok(Dep {
                    name: entry
                        .get("name")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .ok_or(anyhow!("Invalid name"))?,
                    addr: SuiAddress::from_str(entry
                        .get("addr")
                        .and_then(|v| v.as_str())
                        .ok_or(anyhow!("Invalid address"))?)?,
                    version: entry
                        .get("version")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<u64>().ok())
                        .ok_or(anyhow!("Invalid version"))?,
                })
            })
            .collect::<Result<Vec<Dep>>>()?;
        
        Ok(deps_field)
    }

    fn get_intents_bag_field(intents_struct: &Value) -> Result<ObjectID> {
        let intents_bag_field = intents_struct
            .get("inner")
            .and_then(|inner| inner.get("id"))
            .and_then(|uid| uid.get("id"))
            .and_then(|id| id.as_str())
            .ok_or(anyhow!("Invalid intents bag id"))?;

        Ok(ObjectID::from_hex_literal(intents_bag_field)?)
    }

    fn get_locked_objects_field(intents_struct: &Value) -> Result<Vec<ObjectID>> {
        let locked_objects_field = intents_struct
            .get("locked")
            .and_then(|locked| locked.get("contents"))
            .and_then(|contents| contents.as_array())
            .ok_or(anyhow!("Invalid locked array"))?
            .iter()
            .map(|id| {
                Ok(ObjectID::from_hex_literal(id.as_str().ok_or(anyhow!("Invalid locked id"))?)?)
            })
            .collect::<Result<Vec<ObjectID>>>()?;
        
        Ok(locked_objects_field)
    }

    fn get_config_field(config_struct: &Value) -> Result<Config> {
        let members_field = config_struct
            .get("members")
            .and_then(|v| v.as_array())
            .ok_or(anyhow!("Invalid members array"))?
            .iter()
            .map(|member| {
                Ok(Member {
                    username: "".to_string(),
                    avatar: "".to_string(),
                    address: member
                        .get("addr")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .ok_or(anyhow!("Invalid address"))?,
                    weight: member
                        .get("weight")
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<u64>().ok())
                        .ok_or(anyhow!("Invalid weight"))?,
                    roles: member
                        .get("roles")
                        .and_then(|v| v.get("contents"))
                        .and_then(|v| v.as_array())
                        .ok_or(anyhow!("Invalid roles"))?
                        .iter()
                        .map(|v| {
                            v.as_str()
                                .map(|s| s.to_string())
                                .ok_or(anyhow!("Invalid name"))
                        })
                        .collect::<Result<Vec<String>>>()?,
                })
            })
            .collect::<Result<Vec<Member>>>()?;

        let global_field = config_struct
            .get("global")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or(anyhow!("Invalid global field"))?;

        let roles_field = config_struct
            .get("roles")
            .and_then(|v| v.as_array())
            .ok_or(anyhow!("Invalid roles field"))?
            .iter()
            .map(|role| {
                Ok((
                    role
                        .get("name")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .ok_or(anyhow!("Invalid name"))?,
                    Role {
                        threshold: role
                            .get("threshold")
                            .and_then(|v| v.as_str())
                            .and_then(|s| s.parse::<u64>().ok())
                            .ok_or(anyhow!("Invalid threshold"))?,
                        total_weight: 0,
                    }
                ))
            })
            .collect::<Result<HashMap<String, Role>>>()?;

        Ok(Config {
            members: members_field,
            global: Role { threshold: global_field, total_weight: 0 },
            roles: roles_field,
        })
    }
}