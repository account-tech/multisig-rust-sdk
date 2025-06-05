use std::sync::Arc;
use sui_sdk::{rpc_types::SuiParsedData, SuiClient};
use sui_sdk::types::base_types::ObjectID;
use sui_sdk::rpc_types::SuiObjectDataOptions;
use anyhow::{anyhow, Result};

use crate::constants::FEE_ID;

pub struct Fees {
    sui: Arc<SuiClient>,
    id: ObjectID,
    amount: u64,
    recipient: String,
}

impl Fees {
    pub fn new(sui: Arc<SuiClient>) -> Self {
        Self { 
            sui, 
            id: ObjectID::from_hex_literal(FEE_ID).unwrap(), 
            amount: 0, 
            recipient: String::new() 
        }
    }

    pub async fn fetch(&mut self) -> Result<()> {
        let resp = self.sui
            .read_api()
            .get_object_with_options(
                self.id, 
                SuiObjectDataOptions::new().with_content()
            )
            .await?;

        let obj = resp.data.ok_or(anyhow!("Fees object not found"))?;
        if let SuiParsedData::MoveObject(content) = obj.content.unwrap() {
            let json = content.fields.to_json_value();

            self.amount = json.get("amount")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<u64>().ok())
                .ok_or(anyhow!("Invalid amount"))?;

            self.recipient = json.get("recipient")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .ok_or(anyhow!("Invalid recipient"))?;
        }

        Ok(())
    }

    pub async fn from_id(sui: Arc<SuiClient>, id: ObjectID) -> Result<Self> {
        let mut fees = Self::new(sui);
        fees.id = id;
        fees.fetch().await?;
        
        Ok(fees)
    }

    pub fn amount(&self) -> u64 {
        self.amount
    }

    pub fn recipient(&self) -> &str {
        &self.recipient
    }
}