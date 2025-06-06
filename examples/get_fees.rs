use std::sync::Arc;
use anyhow::Result;
use sui_sdk::SuiClientBuilder;

use multisig_sdk::fees::Fees;

#[tokio::main]
async fn main() -> Result<()> {
    let client = SuiClientBuilder::default().build_testnet().await?;

    let mut fees = Fees::new(Arc::new(client));
    fees.fetch().await?;

    println!("Fees: {:?}", fees.amount());

    Ok(())
}