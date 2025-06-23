use anyhow::{Ok, Result, anyhow};
use std::collections::HashMap;
use std::sync::Arc;
use std::fmt;

use sui_graphql_client::Client;
use sui_sdk_types::{ObjectData, Address};

use crate::move_binding::account_protocol as ap;
use crate::move_binding::account_multisig as am;
use crate::intents::Intents;

pub struct Multisig {
    sui_client: Arc<Client>,
    fee_amount: u64,
    fee_recipient: Address,
    id: Address,
    metadata: HashMap<String, String>,
    deps: Vec<Dep>,
    unverified_deps_allowed: bool,
    intents_bag_id: Address,
    locked_objects: Vec<Address>,
    config: Config,
    intents: Option<Intents>, // if None then not fetched yet
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
    pub const FEE_OBJECT: &str = "0xc27762578a0b1f37224550dcfd0442f37dc82744b802d3517822d1bd2718598f";

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
        };

        multisig.refresh().await?;
        Ok(multisig)
    }

    pub async fn refresh(&mut self) -> Result<()> {

        // --- Account<Multisig> ---

        // fetch Account<Multisig> object
        let resp = self
            .sui_client
            .object(self.id, None)
            .await
            .map_err(|e| anyhow!("Failed to fetch multisig object: {}", e))?
            .ok_or_else(|| anyhow!("Multisig object not found"))?;

        // parse the Account<Multisig> object
        if let ObjectData::Struct(obj) = resp.data() {
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
        } else {
            return Err(anyhow!("Multisig not a MoveObject"));
        }

        // --- Intents ---

        let mut intents = Intents::new(self.sui_client.clone(), self.intents_bag_id);
        intents.fetch().await?;
        self.intents = Some(intents);

        // --- Fees ---

        // fetch the Fees object
        let contents = self
            .sui_client
            .move_object_contents_bcs(Address::from_hex(Self::FEE_OBJECT).unwrap(), None)
            .await
            .map_err(|e| anyhow!("Failed to fetch fees object: {}", e))?;

        // parse the Fees object
        match contents {
            Some(contents) => {
                let fees: am::fees::Fees = bcs::from_bytes(&contents)
                    .map_err(|e| anyhow!("Failed to parse fees object: {}", e))?;   
        
                self.fee_amount = fees.amount;
                self.fee_recipient = fees.recipient;
            }
            None => return Err(anyhow!("Fees not a MoveObject"))
        }

        Ok(())
    }

    // === Getters ===

    pub fn fee_amount(&self) -> u64 {
        self.fee_amount
    }

    pub fn fee_recipient(&self) -> &Address {
        &self.fee_recipient
    }

    pub fn id(&self) -> Address {
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

    pub fn intents_bag_id(&self) -> Address {
        self.intents_bag_id
    }

    pub fn locked_objects(&self) -> &Vec<Address> {
        &self.locked_objects
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn intents(&self) -> &Option<Intents> {
        &self.intents
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
            .finish()
    }
}