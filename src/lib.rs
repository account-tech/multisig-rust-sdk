pub mod account;
pub mod fees;
pub mod constants;

use sui_sdk::SuiClient;

pub struct MultisigClient {
    sui: SuiClient,
}

impl MultisigClient {
    pub fn new(sui: SuiClient) -> Self {
        Self { sui }
    }
}