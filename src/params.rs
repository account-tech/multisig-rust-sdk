use move_types::functions::Arg;
use sui_sdk_types::Address;
use sui_transaction_builder::TransactionBuilder;

use crate::utils;

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
                    $($field_name: utils::pure_as_argument(builder, &$field_name).into(),)*
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

define_args_struct!(ConfigDepsArgs {
    names: Vec<String>,
    addresses: Vec<Address>,
    versions: Vec<u64>,
});

define_args_struct!(BorrowCapArgs {
    cap_type: String,
});

define_args_struct!(DisableRulesArgs {
    coin_type: String,
    mint: bool,
    burn: bool,
    update_symbol: bool,
    update_name: bool,
    update_description: bool,
    update_icon: bool,
});

define_args_struct!(UpdateMetadataArgs {
    coin_type: String,
    symbol: String,
    name: String,
    description: String,
    icon_url: String,
});

define_args_struct!(MintAndTransferArgs {
    coin_type: String,
    amounts: Vec<u64>,
    recipients: Vec<Address>,
});

define_args_struct!(MintAndVestArgs {
    coin_type: String,
    total_amount: u64,
    start_timestamp: u64,
    end_timestamp: u64,
    recipient: Address,
});

define_args_struct!(WithdrawAndBurnArgs {
    coin_type: String,
    coin_id: Address,
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
    coin_id: Address,
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



