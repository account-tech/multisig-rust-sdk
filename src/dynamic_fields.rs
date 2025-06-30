use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use move_types::TypeTag;
use sui_graphql_client::{Client, Direction, PaginationFilter};
use sui_sdk_types::Address;

use crate::move_binding::sui;
use crate::move_binding::account_actions as aa;

pub struct DynamicFields {
    pub sui_client: Arc<Client>,
    pub caps: Vec<Cap>,
    pub currencies: HashMap<String, Currency>,
    pub kiosks: HashMap<String, Kiosk>,
    pub packages: HashMap<String, Package>,
    pub vaults: HashMap<String, Vault>,
}

#[derive(Debug)]
pub struct Cap {
    pub type_: String,
}

#[derive(Debug)]
pub struct Currency {
    pub current_supply: u64,
    // rules
    pub max_supply: Option<u64>,
    pub total_minted: u64,
    pub total_burned: u64,
    pub can_mint: bool,
    pub can_burn: bool,
    pub can_update_symbol: bool,
    pub can_update_name: bool,
    pub can_update_description: bool,
    pub can_update_icon: bool,
}

#[derive(Debug)]
pub struct Kiosk {
    pub id: Address,
    pub cap: Address,
    // more data when sui-rust-sdk supports kiosks
}

#[derive(Debug)]
pub struct Package {
    pub package_id: Address,
    pub cap_id: Address,
    pub policy: u8,
    pub delay_ms: u64,
}

#[derive(Debug)]
pub struct Vault {
    pub coins: HashMap<String, u64>,
}

impl DynamicFields {
    pub async fn from_multisig_id(sui_client: Arc<Client>, multisig_id: Address) -> Result<Self> {
        let mut dynamic_fields = Self {
            sui_client,
            caps: Vec::new(),
            currencies: HashMap::new(),
            kiosks: HashMap::new(),
            packages: HashMap::new(),
            vaults: HashMap::new(),
        };
        dynamic_fields.refresh(multisig_id).await?;
        Ok(dynamic_fields)
    }

    pub async fn refresh(&mut self, multisig_id: Address) -> Result<()> {
        let mut cursor = None;
        let mut has_next_page = true;

        while has_next_page {
            let filter = PaginationFilter {
                direction: Direction::Forward,
                cursor: cursor.clone(),
                limit: Some(50),
            };

            let resp = self.sui_client.dynamic_fields(multisig_id, filter).await?;
            for df_output in resp.data() {
                if let TypeTag::Struct(struct_tag) = &df_output.name.type_ {
                    let type_name = format!("{}::{}::{}", struct_tag.address, struct_tag.module, struct_tag.name);
                    let generic = struct_tag
                        .type_params
                        .first()
                        .and_then(|type_tag| match type_tag {
                            TypeTag::Struct(struct_tag) => {
                                Some(format!("{}::{}::{}", struct_tag.address, struct_tag.module, struct_tag.name))
                            }
                            _ => None,
                        })
                        .unwrap_or_default();
                    let key_bcs = df_output.name.bcs.as_ref();
                    let value_bcs = df_output.value.as_ref().ok_or(anyhow!("Couldn't get dynamic field bcs"))?.1.as_ref();

                    match type_name.as_str() {
                        "0x460632ef4e9e708658788229531b99f1f3285de06e1e50e98a22633c7e494867::access_control::CapKey" => {
                            self.caps.push(Cap { type_: generic });
                        },
                        "0x460632ef4e9e708658788229531b99f1f3285de06e1e50e98a22633c7e494867::currency::TreasuryCapKey" => {
                            let treasury_cap: sui::coin::TreasuryCap<()> = bcs::from_bytes(value_bcs)?;
                            self
                                .currencies
                                .entry(generic)
                                .and_modify(|currency| currency.current_supply = treasury_cap.total_supply.value)
                                .or_insert_with(|| Currency {
                                    current_supply: treasury_cap.total_supply.value,
                                    max_supply: None,
                                    total_minted: 0,
                                    total_burned: 0,
                                    can_mint: false,
                                    can_burn: false,
                                    can_update_symbol: false,
                                    can_update_name: false,
                                    can_update_description: false,
                                    can_update_icon: false,
                                });
                        },
                        "0x460632ef4e9e708658788229531b99f1f3285de06e1e50e98a22633c7e494867::currency::CurrencyRulesKey" => {
                            let currency_rules: aa::currency::CurrencyRules<()> = bcs::from_bytes(value_bcs)?;
                            self
                                .currencies
                                .entry(generic) 
                                .and_modify(|currency| {
                                    currency.max_supply = currency_rules.max_supply;
                                    currency.total_minted = currency_rules.total_minted;
                                    currency.total_burned = currency_rules.total_burned;
                                    currency.can_mint = currency_rules.can_mint;
                                    currency.can_burn = currency_rules.can_burn;
                                    currency.can_update_symbol = currency_rules.can_update_symbol;
                                    currency.can_update_name = currency_rules.can_update_name;
                                    currency.can_update_description = currency_rules.can_update_description;
                                    currency.can_update_icon = currency_rules.can_update_icon;
                                })
                                .or_insert_with(|| Currency {
                                    current_supply: 0,
                                    max_supply: currency_rules.max_supply,
                                    total_minted: currency_rules.total_minted,
                                    total_burned: currency_rules.total_burned,
                                    can_mint: currency_rules.can_mint,
                                    can_burn: currency_rules.can_burn,
                                    can_update_symbol: currency_rules.can_update_symbol,
                                    can_update_name: currency_rules.can_update_name,
                                    can_update_description: currency_rules.can_update_description,
                                    can_update_icon: currency_rules.can_update_icon,
                                });
                        },
                        "0x460632ef4e9e708658788229531b99f1f3285de06e1e50e98a22633c7e494867::kiosk::KioskOwnerKey" => {
                            let kiosk_owner_key: aa::kiosk::KioskOwnerKey = bcs::from_bytes(key_bcs)?;
                            let kiosk_owner_cap: sui::kiosk::KioskOwnerCap = bcs::from_bytes(value_bcs)?;

                            self.kiosks.insert(kiosk_owner_key.pos0, Kiosk {
                                id: kiosk_owner_cap.id.into(),
                                cap: kiosk_owner_cap.for_.into(),
                            });
                        },
                        "0x460632ef4e9e708658788229531b99f1f3285de06e1e50e98a22633c7e494867::package_upgrade::UpgradeCapKey" => {
                            let upgrade_cap_key: aa::package_upgrade::UpgradeCapKey = bcs::from_bytes(key_bcs)?;
                            let upgrade_cap: sui::package::UpgradeCap = bcs::from_bytes(value_bcs)?;
                            
                            self
                                .packages
                                .entry(upgrade_cap_key.pos0)
                                .and_modify(|package| {
                                    package.package_id = upgrade_cap.package.into();
                                    package.cap_id = upgrade_cap.id.into();
                                    package.policy = upgrade_cap.policy;
                                })
                                .or_insert_with(|| Package {
                                    package_id: upgrade_cap.package.into(),
                                    cap_id: upgrade_cap.id.into(),
                                    policy: upgrade_cap.policy,
                                    delay_ms: 0,
                                });
                        },
                        "0x460632ef4e9e708658788229531b99f1f3285de06e1e50e98a22633c7e494867::package_upgrade::UpgradeRulesKey" => {
                            let upgrade_rules_key: aa::package_upgrade::UpgradeRulesKey = bcs::from_bytes(key_bcs)?;
                            let upgrade_rules: aa::package_upgrade::UpgradeRules = bcs::from_bytes(value_bcs)?;
                            
                            self
                                .packages
                                .entry(upgrade_rules_key.pos0)
                                .and_modify(|package| package.delay_ms = upgrade_rules.delay_ms)
                                .or_insert_with(|| Package {
                                    package_id: Address::ZERO,
                                    cap_id: Address::ZERO,
                                    policy: 0,
                                    delay_ms: upgrade_rules.delay_ms,
                                });
                        },
                        "0x460632ef4e9e708658788229531b99f1f3285de06e1e50e98a22633c7e494867::vault::VaultKey" => {
                            let vault_key: aa::vault::VaultKey = bcs::from_bytes(key_bcs)?;
                            let vault_bag: sui::bag::Bag = bcs::from_bytes(value_bcs)?;

                            let mut coins_for_vault = HashMap::new();

                            let mut cursor = None;
                            let mut has_next_page = true;
                            while has_next_page {
                                let filter = PaginationFilter {
                                    direction: Direction::Forward,
                                    cursor: cursor.clone(),
                                    limit: Some(50),
                                };
                                let resp = self.sui_client.dynamic_fields(vault_bag.id.into(), filter).await?;
                                for df_output in resp.data() {
                                    if let Some((TypeTag::Struct(struct_tag), value_bcs)) = &df_output.value {
                                        let coin_type = format!("{}::{}::{}", struct_tag.address, struct_tag.module, struct_tag.name);
                                        let coin_amount: u64 = bcs::from_bytes::<sui::coin::Coin<()>>(value_bcs)?.balance.value;
                                        coins_for_vault.insert(coin_type, coin_amount);
                                    };
                                }
                                cursor = resp.page_info().end_cursor.clone();
                                has_next_page = resp.page_info().has_next_page;
                            }
                            
                            self.vaults.insert(vault_key.pos0, Vault { coins: coins_for_vault });
                        },
                        _ => (),
                    }
                }
            }
            cursor = resp.page_info().end_cursor.clone();
            has_next_page = resp.page_info().has_next_page;
        }
        Ok(())
    }
}

impl fmt::Debug for DynamicFields {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DynamicFields")
            .field("caps", &self.caps)
            .field("currencies", &self.currencies)
            .field("kiosks", &self.kiosks)
            .field("packages", &self.packages)
            .field("vaults", &self.vaults)
            .finish()
    }
}
