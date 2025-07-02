use move_types::functions::Arg;
use sui_sdk_types::Address;
use sui_transaction_builder::TransactionBuilder;

use crate::utils::{pure_as_argument, object_mut_as_argument, object_ref_as_argument, object_val_as_argument};

pub struct ParamsArgs {
    pub key: Arg<String>,
    pub description: Arg<String>,
    pub execution_times: Arg<Vec<u64>>,
    pub expiration_time: Arg<u64>,
}

impl ParamsArgs {
    pub fn new(
        builder: &mut TransactionBuilder, 
        key: String, 
        description: String, 
        execution_times: Vec<u64>, 
        expiration_time: u64
    ) -> Self {
        Self {
            key: pure_as_argument(builder, &key).into(),
            description: pure_as_argument(builder, &description).into(),
            execution_times: pure_as_argument(builder, &execution_times).into(),
            expiration_time: pure_as_argument(builder, &expiration_time).into(),
        }
    }
}

pub struct ConfigMultisigArgs {
    pub addresses: Arg<Vec<Address>>,
    pub weights: Arg<Vec<u64>>,
    pub roles: Arg<Vec<Vec<String>>>,
    pub global: Arg<u64>,
    pub role_names: Arg<Vec<String>>,
    pub role_thresholds: Arg<Vec<u64>>,
}

impl ConfigMultisigArgs {
    pub fn new(
        builder: &mut TransactionBuilder, 
        addresses: Vec<Address>, 
        weights: Vec<u64>, 
        roles: Vec<Vec<String>>, 
        global: u64, 
        role_names: Vec<String>, 
        role_thresholds: Vec<u64>
    ) -> Self {
        Self {
            addresses: pure_as_argument(builder, &addresses).into(),
            weights: pure_as_argument(builder, &weights).into(),
            roles: pure_as_argument(builder, &roles).into(),
            global: pure_as_argument(builder, &global).into(),
            role_names: pure_as_argument(builder, &role_names).into(),
            role_thresholds: pure_as_argument(builder, &role_thresholds).into(),
        }
    }
}

pub struct ConfigDepsArgs {
    pub names: Arg<Vec<String>>,
    pub addresses: Arg<Vec<Address>>,
    pub versions: Arg<Vec<u64>>,
}

impl ConfigDepsArgs {
    pub fn new(
        builder: &mut TransactionBuilder,
        names: Vec<String>,
        addresses: Vec<Address>,
        versions: Vec<u64>,
    ) -> Self {
        Self {
            names: pure_as_argument(builder, &names).into(),
            addresses: pure_as_argument(builder, &addresses).into(),
            versions: pure_as_argument(builder, &versions).into(),
        }
    }
}

pub struct BorrowCapArgs {
    pub cap_type: Arg<String>,
}

impl BorrowCapArgs {
    pub fn new(
        builder: &mut TransactionBuilder,
        cap_type: String,
    ) -> Self {
        Self {
            cap_type: pure_as_argument(builder, &cap_type).into(),
        }
    }
}

pub struct DisableRulesArgs {
    pub coin_type: Arg<String>,
    pub mint: Arg<bool>,
    pub burn: Arg<bool>,
    pub update_symbol: Arg<bool>,
    pub update_name: Arg<bool>,
    pub update_description: Arg<bool>,
    pub update_icon: Arg<bool>,
}

impl DisableRulesArgs {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        builder: &mut TransactionBuilder,
        coin_type: String,
        mint: bool,
        burn: bool,
        update_symbol: bool,
        update_name: bool,
        update_description: bool,
        update_icon: bool,
    ) -> Self {
        Self {
            coin_type: pure_as_argument(builder, &coin_type).into(),
            mint: pure_as_argument(builder, &mint).into(),
            burn: pure_as_argument(builder, &burn).into(),
            update_symbol: pure_as_argument(builder, &update_symbol).into(),
            update_name: pure_as_argument(builder, &update_name).into(),
            update_description: pure_as_argument(builder, &update_description).into(),
            update_icon: pure_as_argument(builder, &update_icon).into(),
        }
    }
}

pub struct UpdateMetadataArgs {
    pub coin_type: Arg<String>,
    pub symbol: Arg<String>,
    pub name: Arg<String>,
    pub description: Arg<String>,
    pub icon_url: Arg<String>,
}

impl UpdateMetadataArgs {
    pub fn new(
        builder: &mut TransactionBuilder,
        coin_type: String,
        symbol: String,
        name: String,
        description: String,
        icon_url: String,
    ) -> Self {
        Self {
            coin_type: pure_as_argument(builder, &coin_type).into(),
            symbol: pure_as_argument(builder, &symbol).into(),
            name: pure_as_argument(builder, &name).into(),
            description: pure_as_argument(builder, &description).into(),
            icon_url: pure_as_argument(builder, &icon_url).into(),
        }
    }
}