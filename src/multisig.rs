use anyhow::{Ok, Result, anyhow};
use std::collections::HashMap;
use std::sync::Arc;

use sui_graphql_client::Client;
use sui_sdk_types::{ObjectData, Address};

use crate::move_binding::account_protocol as ap;
use crate::move_binding::account_multisig as am;
use crate::constants::FEE_ID;

pub struct Multisig {
    sui_client: Arc<Client>,
    fee_amount: u64,
    fee_recipient: Address,
    id: Address,
    metadata: HashMap<String, String>,
    deps: Vec<ap::deps::Dep>,
    unverified_deps_allowed: bool,
    intents_bag_id: Address,
    locked_objects: Vec<Address>,
    config: Config,
}

#[derive(Debug, Default)]
pub struct Config {
    pub members: Vec<am::multisig::Member>,
    pub global: Role,
    pub roles: HashMap<String, Role>,
}

// #[derive(Debug, Default)]
// pub struct Member {
//     // social data
//     pub username: String,
//     pub avatar: String,
//     // member data
//     pub address: String,
//     pub weight: u64,
//     pub roles: Vec<String>,
// }

#[derive(Debug, Default)]
pub struct Role {
    // threshold to reach for the role
    pub threshold: u64,
    // sum of the weight of the members with the role
    pub total_weight: u64,
}

impl Multisig {
    pub fn new(sui_client: Arc<Client>, id: Address) -> Self {
        Self {
            sui_client,
            fee_amount: 0,
            fee_recipient: Address::from_hex("0x0").unwrap(),
            id,
            metadata: HashMap::new(),
            deps: Vec::new(),
            unverified_deps_allowed: false,
            intents_bag_id: Address::from_hex("0x0").unwrap(),
            locked_objects: Vec::new(),
            config: Config::default(),
        }
    }

    pub async fn fetch(&mut self) -> Result<()> {
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
            self.deps = multisig.deps.inner;
            self.unverified_deps_allowed = multisig.deps.unverified_allowed;

            // get the intents bag id and locked objects array
            self.intents_bag_id = multisig.intents.inner.id.into();
            self.locked_objects = multisig.intents.locked.contents.iter().map(|id| *id.as_address()).collect();

            // get the fields from the config/multisig struct
            self.config = Config {
                members: multisig.config.members,
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
                for role in member.roles.contents.iter() {
                    self.config.roles
                        .get_mut(role)
                        .ok_or_else(|| anyhow!("Role {} not found", role))?
                        .total_weight += member.weight;
                }
            }
        } else {
            return Err(anyhow!("Multisig not a MoveObject"));
        }

        // fetch the Fees object
        let resp = self
            .sui_client
            .object(Address::from_hex(FEE_ID).unwrap(), None)
            .await
            .map_err(|e| anyhow!("Failed to fetch fees object: {}", e))?
            .ok_or_else(|| anyhow!("Fees object not found"))?;

        // parse the Fees object
        if let ObjectData::Struct(obj) = resp.data() {
            let fees: am::fees::Fees = bcs::from_bytes(obj.contents())
                .map_err(|e| anyhow!("Failed to parse fees object: {}", e))?;   

            self.fee_amount = fees.amount;
            self.fee_recipient = fees.recipient;
        } else {
            return Err(anyhow!("Fees not a MoveObject"));
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

    pub fn deps(&self) -> &Vec<ap::deps::Dep> {
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
}
