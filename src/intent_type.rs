use anyhow::{anyhow, Result};
use sui_sdk_types::{Address, ObjectData};

use crate::move_binding::account_protocol as ap;
use crate::move_binding::account_actions as aa;
use crate::move_binding::account_multisig as am;

pub enum IntentType {
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

pub struct ConfigMultisigArgs {
    pub global: u64,
    pub members: Vec<(Address, u64, Vec<String>)>,
    pub roles: Vec<(String, u64)>,
}

pub struct ConfigDepsArgs {
    pub deps: Vec<(String, Address, u64)>,
}

pub struct ToggleUnverifiedAllowedArgs {
    pub allowed: bool
}

pub struct BorrowCapArgs {
    pub cap_type: String,
}

pub struct DisableRulesArgs {
    pub coin_type: String,
    pub mint: bool,
    pub burn: bool,
    pub update_symbol: bool,
    pub update_name: bool,
    pub update_description: bool,
    pub update_icon: bool,
}

pub struct UpdateMetadataArgs {
    pub coin_type: String,
    pub new_name: String,
    pub new_symbol: String,
    pub new_description: String,
    pub new_icon_url: String,
}

pub struct MintAndTransferArgs {
    pub coin_type: String,
    pub transfers: Vec<(u64, Address)>,
}

pub struct MintAndVestArgs {
    pub coin_type: String,
    pub amount: u64,
    pub start: u64, // ms
    pub end: u64, // ms
    pub recipient: Address,
}

pub struct WithdrawAndBurnArgs {
    pub coin_type: String,
    pub coin_id: Address,
    pub amount: u64,
}

pub struct TakeNftsArgs {
    pub kiosk_name: String,
    pub nft_ids: Vec<Address>,
    pub recipient: Address,
}

pub struct ListNftsArgs {
    pub kiosk_name: String,
    pub listings: Vec<(Address, u64)>,
}

pub struct WithdrawAndTransferToVaultArgs {
    pub coin_type: String,
    pub coin_id: Address,
    pub coin_amount: u64,
    pub vault_name: String,
}

pub struct WithdrawAndTransferArgs {
    pub transfers: Vec<(Address, Address)>, // object to address
}

pub struct WithdrawAndVestArgs {
    pub coin_id: Address,
    pub start: u64, // ms
    pub end: u64, // ms
    pub recipient: Address,
}

pub struct SpendAndTransferArgs {
    pub vault_name: String,
    pub coin_type: String,
    pub transfers: Vec<(u64, Address)>,
}

pub struct SpendAndVestArgs {
    pub vault_name: String,
    pub coin_type: String,
    pub amount: u64,
    pub start: u64, // ms
    pub end: u64, // ms
    pub recipient: Address,
}

pub struct UpgradePackageArgs {
    pub package_name: String,
    pub digest: Vec<u8>,
}

pub struct RestrictPolicyArgs {
    pub package_name: String,
    pub policy: Policy,
}

pub enum Policy {
    Compatible = 0,
    Additive = 128,
    DepOnly = 192,
    Immutable = 255,
}

pub fn deserialize_action_args(move_intent_type: &str, actions: &[ObjectData]) -> Result<IntentType> {
    match move_intent_type {
        "0x460632ef4e9e708658788229531b99f1f3285de06e1e50e98a22633c7e494867::config::ConfigMultisig" => {
            let action: am::config::ConfigMultisigAction = bcs::from_bytes(object_contents(&actions[0])?)?;
            Ok(IntentType::ConfigMultisig(ConfigMultisigArgs {
                global: action.config.global,
                members: action.config.members.iter().map(|member| (member.addr, member.weight, member.roles.contents.iter().map(|role| role.to_string()).collect())).collect(),
                roles: action.config.roles.iter().map(|role| (role.name.to_string(), role.threshold)).collect(),
            }))
        },
        // "0x10c87c29ea5d5674458652ababa246742a763f9deafed11608b7f0baea296484::config::ConfigDepsIntent" => {

        // },
        // "0x10c87c29ea5d5674458652ababa246742a763f9deafed11608b7f0baea296484::config::ToggleUnverifiedAllowedIntent" => {

        // },
        // "0xf477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::access_control_intents::BorrowCapIntent" => {
            
        // },
        // "0xf477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::currency_intents::DisableRulesIntent" => {
            
        // },
        // "0xf477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::currency_intents::UpdateMetadataIntent" => {
            
        // },
        // "0xf477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::currency_intents::MintAndTransferIntent" => {
            
        // },
        // "0xf477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::currency_intents::MintAndVestIntent" => {
            
        // },
        // "0xf477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::currency_intents::WithdrawAndBurnIntent" => {
            
        // },
        // "0xf477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::kiosk_intents::TakeNftsIntent" => {
            
        // },
        // "0xf477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::kiosk_intents::ListNftsIntent" => {
            
        // },
        // "0xf477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::owned_intents::WithdrawAndTransferToVaultIntent" => {
            
        // },
        // "0xf477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::owned_intents::WithdrawAndTransferIntent" => {
            
        // },
        // "0xf477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::owned_intents::WithdrawAndVestIntent" => {
            
        // },
        // "0xf477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::package_upgrade_intents::UpgradePackageIntent" => {
            
        // },
        // "0xf477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::package_upgrade_intents::RestrictPolicyIntent" => {
            
        // },
        // "0xf477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::vault_intents::SpendAndTransferIntent" => {
            
        // },
        // "0xf477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94::vault_intents::SpendAndVestIntent" => {
            
        // },
        _ => Err(anyhow!("Invalid intent type: {}", move_intent_type)),
    }
}

fn object_contents(action: &ObjectData) -> Result<&[u8]> {
    match action {
        ObjectData::Struct(obj) => Ok(obj.contents()),
        _ => Err(anyhow!("Invalid action type")),
    }
}
