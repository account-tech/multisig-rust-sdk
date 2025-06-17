use move_binding_derive::move_contract;

move_contract! {
    alias = "sui", 
    package = "0x2", 
    base_path = crate::move_binding,
    network = "testnet"
}

move_contract! {
    alias = "kiosk", 
    package = "0xbd8fc1947cf119350184107a3087e2dc27efefa0dd82e25a1f699069fe81a585", 
    base_path = crate::move_binding, 
    network = "testnet"
}

move_contract! {
    alias = "account_extensions", 
    package = "0x87bee60d3ea6dc5b42e1074134373af27733fb3c5ebc3ac8e013901426d85d53", 
    base_path = crate::move_binding,
    network = "testnet"
}

move_contract! {
    alias = "account_protocol", 
    package = "0x10c87c29ea5d5674458652ababa246742a763f9deafed11608b7f0baea296484", 
    base_path = crate::move_binding,
    network = "testnet"
}

move_contract! {
    alias = "account_actions", 
    package = "0xf477dbfad6ab1de1fdcb6042c0afeda2aa5bf12eb7ef42d280059fc8d6c36c94", 
    base_path = crate::move_binding,
    network = "testnet"
}

move_contract! {
    alias = "account_multisig", 
    package = "0x460632ef4e9e708658788229531b99f1f3285de06e1e50e98a22633c7e494867", 
    base_path = crate::move_binding,
    network = "testnet"
}

