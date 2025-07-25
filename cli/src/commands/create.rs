use super::super::tx_utils;
use account_multisig_sdk::{MultisigBuilder, MultisigClient};
use anyhow::Result;
use sui_crypto::ed25519::Ed25519PrivateKey;
use crate::parsers::{Member, Role};

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
