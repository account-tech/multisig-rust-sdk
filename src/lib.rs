pub mod multisig;
pub mod intents;
pub mod move_binding;
pub mod actions;

use std::sync::Arc;
use anyhow::{anyhow, Ok, Result};
use sui_graphql_client::Client;
use sui_transaction_builder::Serialized;
use sui_transaction_builder::{TransactionBuilder, unresolved::Input};
use sui_sdk_types::{Address, ObjectData};

use crate::move_binding::sui;
// use crate::move_binding::account_extensions as ae;
// use crate::move_binding::account_protocol as ap;
use crate::move_binding::account_multisig as am;
use crate::multisig::Multisig;

pub struct MultisigClient {
    sui_client: Arc<Client>,
    multisig: Option<Multisig>,
}

impl MultisigClient {
    pub const EXTENSIONS_OBJECT: &str = "0x698bc414f25a7036d9a72d6861d9d268e478492dc8bfef8b5c1c2f1eae769254";
    pub const FEE_OBJECT: &str = "0xc27762578a0b1f37224550dcfd0442f37dc82744b802d3517822d1bd2718598f";

    // === Constructors ===

    pub fn new_with_client(sui_client: Client) -> Self {
        Self { sui_client: Arc::new(sui_client), multisig: None }
    }

    pub fn new_with_url(url: &str) -> Result<Self> {
        Ok(Self { sui_client: Arc::new(Client::new(url)?), multisig: None })
    }

    pub fn new_testnet() -> Self {
        Self { sui_client: Arc::new(Client::new_testnet()), multisig: None }
    }

    pub fn new_mainnet() -> Self {
        Self { sui_client: Arc::new(Client::new_mainnet()), multisig: None }
    }

    // === Multisig ===

    pub async fn create_multisig(&self, builder: &mut TransactionBuilder) -> Result<()> {
        let extensions_obj = &self.sui_client
            .object(Address::from_hex(Self::EXTENSIONS_OBJECT)?, None)
            .await?
            .ok_or(anyhow!("Extensions object not found"))?;
        
        let fee_obj = &self.sui_client.object(Address::from_hex(Self::FEE_OBJECT)?, None).await?.ok_or(anyhow!("Fee object not found"))?;
        let fee = if let ObjectData::Struct(obj) = fee_obj.data() {
            bcs::from_bytes::<am::fees::Fees>(obj.contents())
                .map_err(|e| anyhow!("Failed to parse fee object: {}", e))?
        } else {
            return Err(anyhow!("Fee object not a struct"));
        };
        
        let coin_amount = builder.input(Serialized(&fee.amount));
        let coin_arg = builder.split_coins(builder.gas(), vec![coin_amount]);
        let fee_arg = builder.input(Input::from(fee_obj).by_ref());
        let extensions_arg = builder.input(Input::from(extensions_obj).by_ref());

        let account_obj: move_types::functions::Arg<move_binding::account_protocol::account::Account<am::multisig::Multisig>> = am::multisig::new_account(
            builder, 
            extensions_arg.into(), 
            fee_arg.into(), 
            coin_arg.into()
        );

        sui::transfer::public_share_object(builder, account_obj);

        Ok(())
    }

    pub async fn load_multisig(&mut self, id: Address) -> Result<()> {
        self.multisig = Some(Multisig::from_id(self.sui_client.clone(), id).await?);
        Ok(())
    }

    // === Getters ===

    pub fn sui(&self) -> &Client {
        &self.sui_client
    }

    pub fn multisig(&self) -> Option<&Multisig> {
        self.multisig.as_ref()
    }

    pub fn multisig_mut(&mut self) -> Option<&mut Multisig> {
        self.multisig.as_mut()
    }
}