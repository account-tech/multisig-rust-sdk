pub mod multisig;
pub mod intents;
pub mod constants;
pub mod move_binding;
pub mod intent_type;

use anyhow::{Ok, Result};
use sui_graphql_client::Client;

pub struct MultisigClient {
    sui_client: Client,
}

impl MultisigClient {
    pub fn new_with_client(sui_client: Client) -> Self {
        Self { sui_client }
    }

    pub fn new_with_url(url: &str) -> Result<Self> {
        Ok(Self { sui_client: Client::new(url)? })
    }

    pub fn new_with_testnet() -> Self {
        Self { sui_client: Client::new_testnet() }
    }

    pub fn new_with_mainnet() -> Self {
        Self { sui_client: Client::new_mainnet() }
    }
}