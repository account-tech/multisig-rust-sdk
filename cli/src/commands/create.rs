use super::super::tx_utils;
use account_multisig_sdk::{MultisigBuilder, MultisigClient};
use anyhow::Result;
use sui_crypto::ed25519::Ed25519PrivateKey;

#[derive(Debug, Clone)]
pub struct Member {
    pub address: String,
    pub weight: u64,
    pub roles: Vec<String>,
}

impl std::str::FromStr for Member {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Format: address:weight:role1,role2
        let mut parts = s.splitn(3, ':');
        let address = parts.next().ok_or("Missing address")?.to_string();
        let weight = parts
            .next()
            .ok_or("Missing weight")?
            .parse()
            .map_err(|_| "Invalid weight")?;
        let roles = parts
            .next()
            .map(|r| r.split(',').map(|s| s.to_string()).collect())
            .unwrap_or_else(Vec::new);
        Ok(Member {
            address,
            weight,
            roles,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Role {
    pub name: String,
    pub threshold: u64,
}

impl std::str::FromStr for Role {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Format: name:threshold
        let mut parts = s.splitn(2, ':');
        let name = parts.next().ok_or("Missing name")?.to_string();
        let threshold = parts
            .next()
            .ok_or("Missing threshold")?
            .parse()
            .map_err(|_| "Invalid threshold")?;
        Ok(Role { name, threshold })
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn create_multisig(
    client: &MultisigClient,
    pk: &Ed25519PrivateKey,
    name: Option<String>,
    global_threshold: Option<u64>,
    members: Option<Vec<Member>>,
    roles: Option<Vec<Role>>,
) -> Result<()> {
    let address = pk.public_key().derive_address();
    let mut builder = tx_utils::init(client.sui(), address).await?;

    let mut multisig = MultisigBuilder::new(client, &mut builder);
    if let Some(name) = name {
        multisig = multisig.set_name(name.as_str());
    }
    if let Some(global_threshold) = global_threshold {
        multisig = multisig.set_global_threshold(global_threshold);
    }

    if let Some(members) = members {
        for member in members {
            multisig = multisig.add_member(
                member.address.as_str(),
                member.weight,
                member.roles.iter().map(|r| r.as_str()).collect(),
            );
        }
    }

    if let Some(roles) = roles {
        for role in roles {
            multisig = multisig.add_role(role.name.as_str(), role.threshold);
        }
    }

    multisig.build().await?;
    tx_utils::execute(client.sui(), builder, pk).await?;

    Ok(())
}
