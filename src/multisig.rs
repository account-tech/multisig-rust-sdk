use anyhow::{Ok, Result, anyhow};
use std::collections::HashMap;
use std::sync::Arc;
use std::fmt;

use sui_graphql_client::Client;
use sui_sdk_types::{ObjectData, Address};

use crate::move_binding::{account_protocol as ap, account_multisig as am};
use crate::assets::{dynamic_fields::DynamicFields, owned_objects::OwnedObjects};
use crate::intents::intents::Intents;
use crate::utils;
use crate::FEE_OBJECT;

pub struct Multisig {
    pub sui_client: Arc<Client>,
    pub fee_amount: u64,
    pub fee_recipient: Address,
    pub id: Address,
    pub metadata: HashMap<String, String>,
    pub deps: Vec<Dep>,
    pub unverified_deps_allowed: bool,
    pub intents_bag_id: Address,
    pub locked_objects: Vec<Address>,
    pub config: Config,
    pub intents: Option<Intents>, // if None then not fetched yet
    pub owned_objects: Option<OwnedObjects>, // if None then not fetched yet
    pub dynamic_fields: Option<DynamicFields>, // if None then not fetched yet
}

#[derive(Debug)]
pub struct Dep {
    pub name: String,
    pub addr: Address,
    pub version: u64,
}

#[derive(Debug, Default)]
pub struct Config {
    pub members: Vec<Member>,
    pub global: Role,
    pub roles: HashMap<String, Role>,
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
    pub async fn from_id(sui_client: Arc<Client>, id: Address) -> Result<Self> {
        let mut multisig = Self {
            sui_client: sui_client.clone(),
            fee_amount: 0,
            fee_recipient: Address::ZERO,
            id,
            metadata: HashMap::new(),
            deps: Vec::new(),
            unverified_deps_allowed: false,
            intents_bag_id: Address::ZERO,
            locked_objects: Vec::new(),
            config: Config::default(),
            intents: None,
            owned_objects: None,
            dynamic_fields: None,
        };

        multisig.refresh().await?;
        Ok(multisig)
    }

    pub async fn refresh(&mut self) -> Result<()> {

        // --- Account<Multisig> ---

        // fetch Account<Multisig> object
        let multisig_obj = utils::get_object(&self.sui_client, self.id).await?;

        // parse the Account<Multisig> object
        if let ObjectData::Struct(obj) = multisig_obj.data() {
            let multisig: ap::account::Account<am::multisig::Multisig> = bcs::from_bytes(obj.contents())
                .map_err(|e| anyhow!("Failed to parse multisig object: {}", e))?;

            // get the metadata map
            self.metadata = multisig.metadata.inner.contents
                .iter()
                .map(|entry| (entry.key.to_string(), entry.value.to_string()))
                .collect::<HashMap<String, String>>();

            // get the deps array and unverified toggle
            self.deps = multisig.deps.inner
                .iter()
                .map(|dep| Dep {
                    name: dep.name.to_string(),
                    addr: dep.addr,
                    version: dep.version,
                })
                .collect();
            self.unverified_deps_allowed = multisig.deps.unverified_allowed;

            // get the intents bag id and locked objects array
            self.intents_bag_id = multisig.intents.inner.id.into();
            self.locked_objects = multisig.intents.locked.contents.iter().map(|id| *id.as_address()).collect();

            // get the fields from the config/multisig struct
            self.config = Config {
                members: multisig.config.members.iter().map(|member| Member {
                    username: String::new(),
                    avatar: String::new(),
                    address: member.addr.to_string(),
                    weight: member.weight,
                    roles: member.roles.contents.iter().map(|role| role.to_string()).collect(),
                }).collect(),
                global: Role {
                    threshold: multisig.config.global,
                    total_weight: 0,
                },
                roles: multisig.config.roles.iter().map(|role| {
                    (role.name.to_string(), Role {
                        threshold: role.threshold,
                        total_weight: 0,
                    })
                }).collect(),
            };

            // calculate the total weight of the global and role thresholds
            self.config.global.total_weight = self.config.members
                .iter()
                .fold(0, |acc, member| acc + member.weight);

            for member in &self.config.members {
                for role in member.roles.iter() {
                    self.config.roles
                        .get_mut(role)
                        .ok_or_else(|| anyhow!("Role {} not found", role))?
                        .total_weight += member.weight;
                }
            }
        }

        // --- Intents ---

        let intents = Intents::from_bag_id(self.sui_client.clone(), self.intents_bag_id).await?;
        self.intents = Some(intents);

        // --- Owned Objects ---

        let owned_objects = OwnedObjects::from_multisig_id(self.sui_client.clone(), self.id).await?;
        self.owned_objects = Some(owned_objects);

        // --- Dynamic Fields ---

        let dynamic_fields = DynamicFields::from_multisig_id(self.sui_client.clone(), self.id).await?;
        self.dynamic_fields = Some(dynamic_fields);

        // --- Fees ---

        // fetch the Fees object
        let fee_obj = utils::get_object(&self.sui_client, Address::from_hex(FEE_OBJECT).unwrap()).await?;

        // parse the Fees object
        if let ObjectData::Struct(obj) = fee_obj.data() {
            let fees: am::fees::Fees = bcs::from_bytes(obj.contents())
                .map_err(|e| anyhow!("Failed to parse fees object: {}", e))?;   
        
                self.fee_amount = fees.amount;
                self.fee_recipient = fees.recipient;
        }

        Ok(())
    }
}

impl fmt::Debug for Multisig {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Multisig")
            .field("fee_amount", &self.fee_amount)
            .field("fee_recipient", &self.fee_recipient)
            .field("id", &self.id)
            .field("metadata", &self.metadata)
            .field("deps", &self.deps)
            .field("unverified_deps_allowed", &self.unverified_deps_allowed)
            .field("intents_bag_id", &self.intents_bag_id)
            .field("locked_objects", &self.locked_objects)
            .field("config", &self.config)
            .field("intents", &self.intents)
            .field("owned_objects", &self.owned_objects)
            .field("dynamic_fields", &self.dynamic_fields)
            .finish()
    }
}