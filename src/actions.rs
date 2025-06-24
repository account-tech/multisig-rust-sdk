use anyhow::Ok;
use anyhow::{anyhow, Result};
use move_types::functions::{Arg, MutRef};
use sui_sdk_types::{Address, TypeTag};
use sui_transaction_builder::unresolved::Input;
use sui_transaction_builder::TransactionBuilder;

use crate::move_binding::account_protocol as ap;
use crate::move_binding::account_actions as aa;
use crate::move_binding::account_multisig as am;

#[derive(Debug, Clone)]
pub enum IntentActionsType {
    ConfigMultisig(ConfigMultisigArgs),
    ConfigDeps(ConfigDepsArgs),
    ToggleUnverifiedAllowed(ToggleUnverifiedAllowedArgs),

    BorrowCap(BorrowCapArgs),

    DisableRules(DisableRulesArgs),
    UpdateMetadata(UpdateMetadataArgs),
    MintAndTransfer(MintAndTransferArgs),
    MintAndVest(MintAndVestArgs),
    WithdrawAndBurn(WithdrawAndBurnArgs),
    
    TakeNfts(TakeNftsArgs),
    ListNfts(ListNftsArgs),

    WithdrawAndTransferToVault(WithdrawAndTransferToVaultArgs),
    WithdrawAndTransfer(WithdrawAndTransferArgs),
    WithdrawAndVest(WithdrawAndVestArgs),
    
    SpendAndTransfer(SpendAndTransferArgs),
    SpendAndVest(SpendAndVestArgs),
    
    UpgradePackage(UpgradePackageArgs), 
    RestrictPolicy(RestrictPolicyArgs),
}

#[derive(Debug, Clone)]
pub struct ConfigMultisigArgs {
    pub global: u64,
    pub members: Vec<(Address, u64, Vec<String>)>,
    pub roles: Vec<(String, u64)>,
}

#[derive(Debug, Clone)]
pub struct ConfigDepsArgs {
    pub deps: Vec<(String, Address, u64)>,
}

#[derive(Debug, Clone)]
pub struct ToggleUnverifiedAllowedArgs {}

#[derive(Debug, Clone)]
pub struct BorrowCapArgs {
    pub cap_type: String,
}

#[derive(Debug, Clone)]
pub struct DisableRulesArgs {
    pub coin_type: String,
    pub mint: bool,
    pub burn: bool,
    pub update_symbol: bool,
    pub update_name: bool,
    pub update_description: bool,
    pub update_icon: bool,
}

#[derive(Debug, Clone)]
pub struct UpdateMetadataArgs {
    pub coin_type: String,
    pub new_name: Option<String>,
    pub new_symbol: Option<String>,
    pub new_description: Option<String>,
    pub new_icon_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MintAndTransferArgs {
    pub coin_type: String,
    pub transfers: Vec<(u64, Address)>,
}

#[derive(Debug, Clone)]
pub struct MintAndVestArgs {
    pub coin_type: String,
    pub amount: u64,
    pub start: u64, // ms
    pub end: u64, // ms
    pub recipient: Address,
}

#[derive(Debug, Clone)]
pub struct WithdrawAndBurnArgs {
    pub coin_type: String,
    pub coin_id: Address,
    pub amount: u64,
}

#[derive(Debug, Clone)]
pub struct TakeNftsArgs {
    pub kiosk_name: String,
    pub nft_ids: Vec<Address>,
    pub recipient: Address,
}

#[derive(Debug, Clone)]
pub struct ListNftsArgs {
    pub kiosk_name: String,
    pub listings: Vec<(Address, u64)>,
}

#[derive(Debug, Clone)]
pub struct WithdrawAndTransferToVaultArgs {
    pub coin_type: String,
    pub coin_id: Address,
    pub coin_amount: u64,
    pub vault_name: String,
}

#[derive(Debug, Clone)]
pub struct WithdrawAndTransferArgs {
    pub transfers: Vec<(Address, Address)>, // object to address
}

#[derive(Debug, Clone)]
pub struct WithdrawAndVestArgs {
    pub coin_id: Address,
    pub start: u64, // ms
    pub end: u64, // ms
    pub recipient: Address,
}

#[derive(Debug, Clone)]
pub struct SpendAndTransferArgs {
    pub vault_name: String,
    pub coin_type: String,
    pub transfers: Vec<(u64, Address)>,
}

#[derive(Debug, Clone)]
pub struct SpendAndVestArgs {
    pub vault_name: String,
    pub coin_type: String,
    pub amount: u64,
    pub start: u64, // ms
    pub end: u64, // ms
    pub recipient: Address,
}

#[derive(Debug, Clone)]
pub struct UpgradePackageArgs {
    pub package_name: String,
    pub digest: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct RestrictPolicyArgs {
    pub package_name: String,
    pub policy: Policy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Policy {
    Compatible = 0,
    Additive = 128,
    DepOnly = 192,
    Immutable = 255,
}

impl TryFrom<u8> for Policy {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Policy::Compatible),
            128 => Ok(Policy::Additive),
            192 => Ok(Policy::DepOnly),
            255 => Ok(Policy::Immutable),
            _ => Err(anyhow!("Invalid policy: {}", value)),
        }
    }
}

pub fn deserialize_actions(move_intent_type: &str, actions: &[(Vec<TypeTag>, Vec<u8>)]) -> Result<IntentActionsType> {
    match move_intent_type {
        "460632ef4e9e708658788229531b99f1f3285de06e1e50e98a22633c7e494867::config::ConfigMultisigIntent" => {
            let action: am::config::ConfigMultisigAction = bcs::from_bytes(&actions[0].1)?;
            Ok(IntentActionsType::ConfigMultisig(ConfigMultisigArgs {
                global: action.config.global,
                members: action.config.members.iter().map(|member| (member.addr, member.weight, member.roles.contents.iter().map(|role| role.to_string()).collect())).collect(),
                roles: action.config.roles.iter().map(|role| (role.name.to_string(), role.threshold)).collect(),
            }))
        },
        "10c87c29ea5d5674458652ababa246742a763f9deafed11608b7f0baea296484::config::ConfigDepsIntent" => {
            let action: ap::config::ConfigDepsAction = bcs::from_bytes(&actions[0].1)?;
            Ok(IntentActionsType::ConfigDeps(ConfigDepsArgs {
                deps: action.deps.iter().map(|dep| (dep.name.to_owned(), dep.addr, dep.version)).collect(),
            }))
        },
        "10c87c29ea5d5674458652ababa246742a763f9deafed11608b7f0baea296484::config::ToggleUnverifiedAllowedIntent" => {
            let _action: ap::config::ToggleUnverifiedAllowedAction = bcs::from_bytes(&actions[0].1)?;
            Ok(IntentActionsType::ToggleUnverifiedAllowed(ToggleUnverifiedAllowedArgs {}))
        },
        "f477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::access_control_intents::BorrowCapIntent" => {
            let _action: aa::access_control::BorrowAction<()> = bcs::from_bytes(&actions[0].1)?;
            Ok(IntentActionsType::BorrowCap(BorrowCapArgs {
                cap_type: actions[0].0[0].to_string(),
            }))
        },
        "f477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::currency_intents::DisableRulesIntent" => {
            let action: aa::currency::DisableAction<()> = bcs::from_bytes(&actions[0].1)?;
            Ok(IntentActionsType::DisableRules(DisableRulesArgs {
                coin_type: actions[0].0[0].to_string(),
                mint: action.mint,
                burn: action.burn,
                update_symbol: action.update_symbol,
                update_name: action.update_name,
                update_description: action.update_description,
                update_icon: action.update_icon,
            }))
        },
        "f477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::currency_intents::UpdateMetadataIntent" => {
            let action: aa::currency::UpdateAction<()> = bcs::from_bytes(&actions[0].1)?;
            Ok(IntentActionsType::UpdateMetadata(UpdateMetadataArgs {
                coin_type: actions[0].0[0].to_string(),
                new_name: action.name,
                new_symbol: action.symbol,
                new_description: action.description,
                new_icon_url: action.icon_url,
            }))
        },
        "f477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::currency_intents::MintAndTransferIntent" => {
            let mut transfers = Vec::new();
            for chunk in actions.chunks(2) {
                let mint: aa::currency::MintAction<()> = bcs::from_bytes(&chunk[0].1)?;
                let transfer: aa::transfer::TransferAction = bcs::from_bytes(&chunk[1].1)?;
                transfers.push((mint.amount, transfer.recipient));
            }

            Ok(IntentActionsType::MintAndTransfer(MintAndTransferArgs { 
                coin_type: actions[0].0[0].to_string(), 
                transfers 
            }))
        },
        "f477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::currency_intents::MintAndVestIntent" => {
            let mint: aa::currency::MintAction<()> = bcs::from_bytes(&actions[0].1)?;
            let vest: aa::vesting::VestAction = bcs::from_bytes(&actions[1].1)?;

            Ok(IntentActionsType::MintAndVest(MintAndVestArgs {
                coin_type: actions[0].0[0].to_string(),
                amount: mint.amount,
                start: vest.start_timestamp,
                end: vest.end_timestamp,
                recipient: vest.recipient,
            }))
        },
        "f477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::currency_intents::WithdrawAndBurnIntent" => {
            let withdraw: ap::owned::WithdrawAction = bcs::from_bytes(&actions[0].1)?;
            let burn: aa::currency::BurnAction<()> = bcs::from_bytes(&actions[1].1)?;

            Ok(IntentActionsType::WithdrawAndBurn(WithdrawAndBurnArgs {
                coin_type: actions[1].0[0].to_string(),
                coin_id: withdraw.object_id.into(),
                amount: burn.amount,
            }))
        },
        "f477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::kiosk_intents::TakeNftsIntent" => {
            let (mut kiosk_name, mut recipient) = (String::new(), Address::ZERO);
            let mut nft_ids = Vec::new();
            for action in actions {
                let take: aa::kiosk::TakeAction = bcs::from_bytes(&action.1)?;
                if kiosk_name.is_empty() {
                    kiosk_name = take.name.to_owned();
                }
                if recipient == Address::ZERO {
                    recipient = take.recipient;
                }
                nft_ids.push(take.nft_id.into());
            }
            
            Ok(IntentActionsType::TakeNfts(TakeNftsArgs {
                kiosk_name,
                nft_ids,
                recipient,
            }))
        },
        "f477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::kiosk_intents::ListNftsIntent" => {
            let mut kiosk_name = String::new();
            let mut listings = Vec::new();
            for action in actions {
                let list: aa::kiosk::ListAction = bcs::from_bytes(&action.1)?;
                if kiosk_name.is_empty() {
                    kiosk_name = list.name.to_owned();
                }
                listings.push((list.nft_id.into(), list.price));
            }
            
            Ok(IntentActionsType::ListNfts(ListNftsArgs {
                kiosk_name,
                listings,
            }))
        },
        "f477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::owned_intents::WithdrawAndTransferToVaultIntent" => {
            let withdraw: ap::owned::WithdrawAction = bcs::from_bytes(&actions[0].1)?;
            let deposit: aa::vault::DepositAction<()> = bcs::from_bytes(&actions[1].1)?;

            Ok(IntentActionsType::WithdrawAndTransferToVault(WithdrawAndTransferToVaultArgs { 
                coin_type: actions[0].0[0].to_string(), 
                coin_id: withdraw.object_id.into(),
                coin_amount: deposit.amount,
                vault_name: deposit.name.to_owned(),
            }))
        },
        "f477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::owned_intents::WithdrawAndTransferIntent" => {
            let mut transfers = Vec::new();
            for chunk in actions.chunks(2) {
                let withdraw: ap::owned::WithdrawAction = bcs::from_bytes(&chunk[0].1)?;
                let transfer: aa::transfer::TransferAction = bcs::from_bytes(&chunk[1].1)?;
                transfers.push((withdraw.object_id.into(), transfer.recipient));
            }

            Ok(IntentActionsType::WithdrawAndTransfer(WithdrawAndTransferArgs { 
                transfers
            }))
        },
        "f477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::owned_intents::WithdrawAndVestIntent" => {
            let withdraw: ap::owned::WithdrawAction = bcs::from_bytes(&actions[0].1)?;
            let vest: aa::vesting::VestAction = bcs::from_bytes(&actions[1].1)?;

            Ok(IntentActionsType::WithdrawAndVest(WithdrawAndVestArgs {
                coin_id: withdraw.object_id.into(),
                start: vest.start_timestamp,
                end: vest.end_timestamp,
                recipient: vest.recipient,
            }))
        },
        "f477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::package_upgrade_intents::UpgradePackageIntent" => {
            let upgrade: aa::package_upgrade::UpgradeAction = bcs::from_bytes(&actions[0].1)?;
            Ok(IntentActionsType::UpgradePackage(UpgradePackageArgs {
                package_name: upgrade.name.to_owned(),
                digest: upgrade.digest.to_vec(),
            }))
        },
        "f477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::package_upgrade_intents::RestrictPolicyIntent" => {
            let restrict: aa::package_upgrade::RestrictAction = bcs::from_bytes(&actions[0].1)?;
            Ok(IntentActionsType::RestrictPolicy(RestrictPolicyArgs {
                package_name: restrict.name.to_owned(),
                policy: Policy::try_from(restrict.policy)?,
            }))
        },
        "f477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::vault_intents::SpendAndTransferIntent" => {
            let mut vault_name = String::new();
            let mut transfers = Vec::new();
            for chunk in actions.chunks(2) {
                let spend: aa::vault::SpendAction<()> = bcs::from_bytes(&chunk[0].1)?;
                let transfer: aa::transfer::TransferAction = bcs::from_bytes(&chunk[1].1)?;
                if vault_name.is_empty() {
                    vault_name = spend.name.to_owned();
                }
                transfers.push((spend.amount, transfer.recipient));
            }

            Ok(IntentActionsType::SpendAndTransfer(SpendAndTransferArgs { 
                vault_name,
                coin_type: actions[0].0[0].to_string(),
                transfers,
            }))
        },
        "f477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::vault_intents::SpendAndVestIntent" => {
            let spend: aa::vault::SpendAction<()> = bcs::from_bytes(&actions[0].1)?;
            let vest: aa::vesting::VestAction = bcs::from_bytes(&actions[1].1)?;

            Ok(IntentActionsType::SpendAndVest(SpendAndVestArgs {
                vault_name: spend.name.to_owned(),
                coin_type: actions[0].0[0].to_string(),
                amount: spend.amount,
                start: vest.start_timestamp,
                end: vest.end_timestamp,
                recipient: vest.recipient,
            }))
        },
        _ => Err(anyhow!("Invalid intent type: {}", move_intent_type)),
    }
}
