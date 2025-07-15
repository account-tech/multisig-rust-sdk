use super::super::tx_utils;
use account_multisig_sdk::{MultisigBuilder, MultisigClient};
use anyhow::{Result, anyhow};
use sui_crypto::ed25519::Ed25519PrivateKey;

#[allow(clippy::too_many_arguments)]
pub async fn create_multisig(
    client: &MultisigClient,
    pk: &Ed25519PrivateKey,
    name: Option<String>,
    addresses: Option<Vec<String>>,
    weights: Option<Vec<u64>>,
    roles: Option<Vec<String>>,
    global_threshold: Option<u64>,
    role_names: Option<Vec<String>>,
    role_thresholds: Option<Vec<u64>>,
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

    if addresses.is_some() && weights.is_some() && roles.is_some() {
        if addresses.as_ref().unwrap().len() != weights.as_ref().unwrap().len()
            || addresses.as_ref().unwrap().len() != roles.as_ref().unwrap().len()
        {
            return Err(anyhow!("Addresses, weights and roles must have the same length"));
        }
        for (i, address) in addresses.unwrap().iter().enumerate() {
            multisig = multisig.add_member(
                address.as_str(),
                weights.as_ref().unwrap()[i],
                roles.as_ref().unwrap()[i].split(",").collect(),
            );
        }
    }

    if role_names.is_some() && role_thresholds.is_some() {
        if role_names.as_ref().unwrap().len() != role_thresholds.as_ref().unwrap().len() {
            return Err(anyhow!("Role names and thresholds must have the same length"));
        }
        for (i, role_name) in role_names.as_ref().unwrap().iter().enumerate() {
            multisig = multisig.add_role(role_name.as_str(), role_thresholds.as_ref().unwrap()[i]);
        }
    }

    multisig.build().await?;
    tx_utils::execute(client.sui(), builder, pk).await?;

    Ok(())
}
