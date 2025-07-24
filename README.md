# Account.tech Multisig Rust SDK

A Rust SDK for integrating with the [account.tech](https://account-tech.gitbook.io/docs/) smart-contract-based multisig account implementation on Sui. This SDK enables developers to build secure, programmable, and flexible multisig workflows for on-chain protocols, but leveraging the modular and intent-based architecture of the account.tech Move Framework.

---

## Design

Note: a [CLI](/cli/) is available to create and manage multisigs.

### Overview

Account.tech introduces a new standard for Smart Accounts on Sui, providing:
- **Account Abstraction**: Programmable accounts with customizable rules and features.
- **Intent-Based Architecture**: High-level intents composed of low-level actions, enabling secure and flexible transaction flows.
- **Enterprise-Grade Multisig**: Next-generation multisig accounts with threshold approvals, role-based access, and upgradeable policies.
- **Modular Design**: Extensible via Move packages, supporting custom actions, intents, and integrations.

This SDK exposes Rust bindings and utilities to interact with these multisig smart accounts, manage on-chain operations, and compose complex workflows.

### Features

- **Create and Manage Multisig Accounts**: Instantiate new multisig accounts, share them, and manage their configuration.
- **Intent Lifecycle**: Request, approve, execute, and delete intents for multisig actions (e.g., transfers, upgrades, policy changes).
- **Role-Based Access Control**: Assign roles, weights, and thresholds to multisig members.
- **Asset Management**: Open vaults, deposit, withdraw, vest, and transfer Sui assets and coins via multisig.
- **Currencies**: Manage treasury caps, mint, burn, update metadata and set more permissions.
- **Package Upgrades**: Securely upgrade Move packages with time-locks and policy restrictions.
- **Admin Caps**: Deposit and borrow different types of caps via a multisig account.
- **Extensible Actions**: Compose custom actions and intents for advanced workflows.

### Architecture

The SDK is built around the following core concepts:

- **Multisig Smart Account**: Programmable shared objects with modular extensions and intent-based execution reprenting an account with multisig-mechanism as ownership rules.
- **User**: An owned object tracking the Multisig Accounts the user is a member of.
- **Intents & Actions**: Intents are usually called proposals in the case of a Multisig. These goes through a lifecycle of: request -> approve -> execute (or delete).
- **Commands**: Unlike proposals, these operations are instantly executable by multisig members.
- **Owned Objects**: account.tech smart accounts replicate the behavior of standard Sui accounts. They can transfer, own and receive objects.
- **Dynamic Fields**: for standardized Sui objects such as `UpgradeCap`, we propose custom flows and permissions using dynamic fields.

For more on the underlying Move architecture, see the [account.tech Move Framework](https://github.com/account-tech/move-framework/) and the [Docs](https://account-tech.gitbook.io/docs).

---

## Development

### Installation

Add the SDK to your Rust project by including it in your `Cargo.toml`:

```toml
[dependencies]
multisig-rust-sdk = { git = "https://github.com/account-tech/multisig-rust-sdk" }
```

### Usage Example

Below is a basic example of creating and sharing a new multisig account:

```rust
use multisig_rust_sdk::MultisigClient;
use sui_sdk_types::TransactionBuilder;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize the client (testnet/mainnet/custom URL)
    let mut client = MultisigClient::new_testnet();
    let mut builder = TransactionBuilder::new();

    // Create a new multisig account
    let multisig = client.create_multisig(&mut builder).await?;
    client.share_multisig(&mut builder, multisig);
    // Alternatively, use MultisigBuilder for more control

    // ... sign and execute the transaction ...
    Ok(())
}
```

For more advanced flows (intents, approvals, asset management, upgrades), see the [examples](./examples/) directory and look at the [MultisigClient](./src/lib.rs).

Alternatively, you might want to use our [CLI](./cli/) for interacting directly with our multisig smart contracts. 

### Documentation & Resources

- [account.tech Documentation](https://account-tech.gitbook.io/docs/)
- [Move Framework GitHub](https://github.com/account-tech/move-framework/)
- [Multisig Implementation GitHub](https://github.com/account-tech/move-framework/)
- [account.tech Website](https://account.tech/)
- [Multisig App](https://multisig.account.tech/)

### Contributing

Contributions are welcome! Please open issues or pull requests for bug reports, feature requests, or improvements.

---

## License

This project is licensed under the Apache-2.0 License. See [LICENSE](./LICENSE) for details.

