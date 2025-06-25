use move_types::functions::Arg;
use sui_sdk_types::Address;
use sui_transaction_builder::{TransactionBuilder, Serialized};

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
        let key_input = builder.input(Serialized(&key));
        let description_input = builder.input(Serialized(&description));
        let execution_times_input = builder.input(Serialized(&execution_times));
        let expiration_time_input = builder.input(Serialized(&expiration_time));

        Self {
            key: key_input.into(),
            description: description_input.into(),
            execution_times: execution_times_input.into(),
            expiration_time: expiration_time_input.into(),
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
        let addresses_input = builder.input(Serialized(&addresses));
        let weights_input = builder.input(Serialized(&weights));
        let roles_input = builder.input(Serialized(&roles));
        let global_input = builder.input(Serialized(&global));
        let role_names_input = builder.input(Serialized(&role_names));
        let role_thresholds_input = builder.input(Serialized(&role_thresholds));

        Self {
            addresses: addresses_input.into(),
            weights: weights_input.into(),
            roles: roles_input.into(),
            global: global_input.into(),
            role_names: role_names_input.into(),
            role_thresholds: role_thresholds_input.into(),
        }
    }
}