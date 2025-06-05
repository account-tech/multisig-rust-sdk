use sui_sdk::SuiClientBuilder;
use anyhow::Result;

use multisig_sdk::fees::Fees;

#[tokio::main]
async fn main() -> Result<()> {
    let client = SuiClientBuilder::default().build_testnet().await?;

    let mut fees = Fees::default();
    fees.fetch(&client).await?;

    println!("Fees: {:?}", fees);

    Ok(())
}