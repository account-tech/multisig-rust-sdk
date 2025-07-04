use anyhow::Result;
use move_types::{functions::Arg, ObjectId};
use sui_graphql_client::Client;
use sui_sdk_types::{Address, Argument};
use sui_transaction_builder::{TransactionBuilder, Serialized};

use crate::{
    utils::get_object_as_input_owned as get_input,
    EXTENSIONS_OBJECT,
};

macro_rules! define_args_struct {
    (
        $struct_name:ident {
            $($field_name:ident: $field_type:ty),* $(,)?
        }
    ) => {
        pub struct $struct_name {
            $(pub $field_name: Arg<$field_type>,)*
        }

        impl $struct_name {
            #[allow(clippy::too_many_arguments)]
            pub fn new(
                builder: &mut TransactionBuilder,
                $($field_name: $field_type,)*
            ) -> Self {
                Self {
                    $($field_name: builder.input(Serialized(&$field_name)).into(),)*
                }
            }
        }
    };
}

define_args_struct!(ParamsArgs {
    key: String,
    description: String,
    execution_times: Vec<u64>,
    expiration_time: u64,
});

define_args_struct!(ConfigMultisigArgs {
    addresses: Vec<Address>,
    weights: Vec<u64>,
    roles: Vec<Vec<String>>,
    global: u64,
    role_names: Vec<String>,
    role_thresholds: Vec<u64>,
});

pub struct ConfigDepsArgs {
    pub extensions: Argument,
    pub names: Arg<Vec<String>>,
    pub addresses: Arg<Vec<Address>>,
    pub versions: Arg<Vec<u64>>,
}

impl ConfigDepsArgs {
    pub async fn new(
        sui_client: &Client,
        builder: &mut TransactionBuilder,
        names: Vec<String>,
        addresses: Vec<Address>,
        versions: Vec<u64>,
    ) -> Result<Self> {
        let extensions_input = get_input(sui_client, EXTENSIONS_OBJECT.parse().unwrap()).await?;
        let extensions_argument = builder.input(extensions_input.by_ref());

        Ok(Self {
            extensions: extensions_argument,
            names: builder.input(Serialized(&names)).into(),
            addresses: builder.input(Serialized(&addresses)).into(),
            versions: builder.input(Serialized(&versions)).into(),
        })
    }
}

define_args_struct!(DisableRulesArgs {
    mint: bool,
    burn: bool,
    update_symbol: bool,
    update_name: bool,
    update_description: bool,
    update_icon: bool,
});

define_args_struct!(UpdateMetadataArgs {
    symbol: Option<String>,
    name: Option<String>,
    description: Option<String>,
    icon_url: Option<String>,
});

define_args_struct!(MintAndTransferArgs {
    amounts: Vec<u64>,
    recipients: Vec<Address>,
});

define_args_struct!(MintAndVestArgs {
    total_amount: u64,
    start_timestamp: u64,
    end_timestamp: u64,
    recipient: Address,
});

define_args_struct!(WithdrawAndBurnArgs {
    coin_id: ObjectId,
    amount: u64,
});

define_args_struct!(TakeNftsArgs {
    kiosk_name: String,
    nft_ids: Vec<Address>,
    recipient: Address,
});

define_args_struct!(ListNftsArgs {
    kiosk_name: String,
    nft_ids: Vec<Address>,
    prices: Vec<u64>,
});

define_args_struct!(WithdrawAndTransferToVaultArgs {
    coin_id: ObjectId,
    coin_amount: u64,
    vault_name: String,
});

define_args_struct!(WithdrawAndTransferArgs {
    object_ids: Vec<Address>,
    recipients: Vec<Address>,
});

define_args_struct!(WithdrawAndVestArgs {
    coin_id: Address,
    start_timestamp: u64,
    end_timestamp: u64,
    recipient: Address,
});

define_args_struct!(UpgradePackageArgs {
    package_name: String,
    digest: Vec<u8>,
});

define_args_struct!(RestrictPolicyArgs {
    package_name: String,
    policy: u8,
});

define_args_struct!(SpendAndTransfer {
    vault_name: String,
    amounts: Vec<u64>,
    recipients: Vec<Address>,
});

define_args_struct!(SpendAndVest {
    vault_name: String,
    coin_amount: u64,
    start_timestamp: u64,
    end_timestamp: u64,
    recipient: Address,
});
