use sui_sdk::{rpc_types::SuiParsedData, SuiClient};
use sui_sdk::types::base_types::ObjectID;
use sui_sdk::rpc_types::SuiObjectDataOptions;
use anyhow::{anyhow, Result};

use crate::constants::FEE_ID;

#[derive(Debug)]
pub struct Fees {
    id: ObjectID,
    amount: u64,
    recipient: String,
}

impl Default for Fees {
    fn default() -> Self {
        Self::new(ObjectID::from_hex_literal(FEE_ID).unwrap())
    }
}

impl Fees {
    pub fn new(id: ObjectID) -> Self {
        Self { id, amount: 0, recipient: String::new() }
    }

    pub async fn fetch(&mut self, client: &SuiClient) -> Result<&mut Self> {
        let resp = client
            .read_api()
            .get_object_with_options(
                self.id, 
                SuiObjectDataOptions::new().with_content()
            )
            .await?;

        let obj = resp.data.ok_or(anyhow!("Fees object not found"))?;
        if let SuiParsedData::MoveObject(content) = obj.content.unwrap() {
            let json = content.fields.to_json_value();
            self.amount = json.get("amount").unwrap().as_str().unwrap().parse::<u64>().unwrap();
            self.recipient = json.get("recipient").unwrap().as_str().unwrap().to_string();
        }

        Ok(self)
    }

    pub fn get_amount(&self) -> u64 {
        self.amount
    }

    pub fn get_recipient(&self) -> &str {
        &self.recipient
    }
}