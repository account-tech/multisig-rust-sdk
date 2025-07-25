# Account.tech Multisig CLI

A command-line interface for managing [account.tech](https://account-tech.gitbook.io/docs/) multisig accounts on Sui. This CLI provides an interactive and programmatic way to create, manage, and interact with multisig smart accounts.

---

## Installation

Install the CLI from source:

```bash
cargo install --git https://github.com/account-tech/multisig-rust-sdk account-multisig-cli
```

---

## Usage

### Basic Usage

```bash
# Start interactive mode
account-multisig <network> [multisig_id]

# Examples:
account-multisig testnet
account-multisig mainnet
account-multisig https://fullnode.testnet.sui.io:443
account-multisig testnet 0x123...abc
```

**Networks:**
- `testnet` - Sui testnet
- `mainnet` - Sui mainnet  
- `<url>` - Custom RPC endpoint

### Interactive Mode

The CLI runs in interactive mode by default. Type `help` to see available commands or `exit` to quit.

---

## Commands Reference

### User Management

Manage your user profile and multisig memberships.

#### `user list-multisigs`
List all multisigs you are a member of.

#### `user join-multisig <multisig_id>`
Add a multisig to your user object.

#### `user leave-multisig <multisig_id>`
Remove a multisig from your user object.

#### `user list-invites`
List all pending invites you have received.

#### `user accept-invite <invite_id>`
Accept a multisig invitation.

#### `user refuse-invite <invite_id>`
Refuse a multisig invitation.

---

### Multisig Creation & Loading

#### `create`
Create a new multisig account.

```bash
create --name "My Multisig" --global-threshold 2 --member 0x123...abc:2:admin,member --member 0x456...def:1:member --role admin:2 --role member:1
```

**Options:**
- `--name` - Multisig name
- `--global-threshold` - Global approval threshold
- `--member` - Member as `address:weight:roles` (can have multiple)
- `--role` - Role as `role_name:role_threshold` (can have multiple)

**Member format:** `address:weight:role1,role2,role3`
**Role format:** `role_name:threshold`

#### `load [id]`
Load a specific multisig or reload the current one.

```bash
load                    # Reload current multisig
load 0x123...abc       # Load specific multisig
```

---

### Proposal Management

Proposals (intents) require approval before execution.

#### `proposals`
List all proposals for the current multisig.

#### `proposals <key>`
Show details of a specific proposal.

#### `proposals <key> approve`
Approve a proposal.

#### `proposals <key> disapprove`
Remove your approval from a proposal.

#### `proposals <key> execute`
Execute an approved proposal.

```bash
# Executing package upgrade necessitates additional arguments 
# Output given by `sui move build --dump-bytecode-as-base64`
proposals <key> execute --package-id 0x123...abc --modules "inbase64" --dependencies "0x456...def,0x789...ghi"
```

#### `proposals <key> delete`
Delete a proposal.

---

### Configuration Management

#### `config`
Display current multisig configuration.

#### `config modify-name <name>`
Update the multisig name.

#### `config propose-config-multisig`
Create a proposal to modify multisig configuration.

```bash
config propose-config-multisig --global-threshold 2 --member 0xyour_addy:2:0x456::role_module,0x7::other --member 0x123:1 --role 0x456:1
```

---

### Dependencies Management

#### `deps`
Display current package dependencies for your Multisig.

#### `deps update-to-latest`
Update verified dependencies to their latest versions.

#### `deps propose-config-deps`
Create a proposal to update dependencies. By default you can only add allowed dependencies from the Extensions object.

```bash
deps propose-config-deps --name "Update Dependencies" --names package1 package2 --addresses 0x123 0x456 --versions 1 2
```

#### `deps propose-toggle-unverified-allowed`
Create a proposal to toggle unverified dependencies, allowing you to add any package as a dependency.

```bash
deps propose-toggle-unverified-allowed --name "Allow Unverified"
```

---

### Cap Management

#### `caps`
Display deposited caps.

#### `caps deposit-cap`
Deposit a capability into the multisig.

```bash
caps deposit-cap --cap-id 0x123...abc --cap-type "0x456::module::CapType"
```

#### `caps propose-borrow-cap`
Create a proposal to borrow a cap.

```bash
caps propose-borrow-cap --name "Borrow Cap" --cap-type "0x456::module::CapType"
```

---

### Currency Management

#### `currencies`
Display managed currencies and their permissions.

#### `currencies deposit-treasury-cap`
Deposit a treasury cap to manage a currency.

```bash
currencies deposit-treasury-cap --max-supply 1000000 --cap-id 0x123...abc --coin-type "0x456::module::COIN"
```

#### `currencies propose-disable-rules`
Create a proposal to disable currency permissions. Only include flags for the permissions you want to disable.

```bash
# Disable only burning
currencies propose-disable-rules --name "Disable Burning" --coin-type "0x456::module::Coin" --burn

# Disable multiple permissions
currencies propose-disable-rules --name "Disable Multiple" --coin-type "0x456::module::Coin" --mint --burn --update-symbol
```

#### `currencies propose-update-metadata`
Create a proposal to update currency metadata.

```bash
currencies propose-update-metadata --name "Update Metadata" --coin-type "0x456::module::Coin" --symbol "NEW" --name-field "New Coin" --description "Updated description"
```

#### `currencies propose-mint-and-transfer`
Create a proposal to mint and transfer coins.

```bash
currencies propose-mint-and-transfer --name "Mint and Transfer" --coin-type "0x456::module::Coin" --amounts 1000 2000 --recipients 0x123...abc 0x456...def
```

#### `currencies propose-mint-and-vest`
Create a proposal to mint and vest coins.

```bash
currencies propose-mint-and-vest --name "Mint and Vest" --coin-type "0x456::module::Coin" --total-amount 10000 --start-timestamp 1640995200000 --end-timestamp 1672531200000 --recipient 0x123...abc
```

#### `currencies propose-withdraw-and-burn`
Create a proposal to withdraw and burn coins.

```bash
currencies propose-withdraw-and-burn --name "Burn Coins" --coin-type "0x456::module::Coin" --coin-id 0x123...abc --amount 1000
```

---

### Owned Objects Management

#### `owned`
Display owned objects and coins.

#### `owned propose-withdraw-and-transfer`
Create a proposal to withdraw and transfer owned objects.

```bash
owned propose-withdraw-and-transfer --name "Transfer Objects" --object-ids 0x123...abc 0x456...def --recipients 0x789...ghi 0xabc...jkl
```

#### `owned propose-withdraw-and-vest`
Create a proposal to withdraw and vest a coin.

```bash
owned propose-withdraw-and-vest --name "Vest Coin" --coin-id 0x123...abc --start-timestamp 1640995200000 --end-timestamp 1672531200000 --recipient 0x456...def
```

---

### Package Management

#### `packages`
Display managed packages and their policies.

#### `packages deposit-upgrade-cap`
Deposit an upgrade cap for a package.

```bash
packages deposit-upgrade-cap --cap-id 0x123...abc --package-name "my-package" --timelock-duration 86400000
```

#### `packages propose-upgrade-package`
Create a proposal to upgrade a package.

```bash
packages propose-upgrade-package --name "Upgrade Package" --package-name "my-package" --digest 0x1234567890abcdef
```

#### `packages propose-restrict-policy`
Create a proposal to restrict package policy.

```bash
packages propose-restrict-policy --name "Restrict Policy" --package-name "my-package" --policy 255
```

**Policy values:**
- `0` - Compatible (default)
- `128` - Additive
- `192` - DepOnly  
- `255` - Immutable

---

### Vault Management

#### `vaults`
Display vaults and their contents.

#### `vaults open-vault`
Open a new vault.

```bash
vaults open-vault --vault-name "treasury"
```

#### `vaults deposit-from-wallet`
Deposit coins from your wallet into a vault.

```bash
vaults deposit-from-wallet --vault-name "treasury" --amount 1000000 --coin-type "0x456::module::Coin"
```

#### `vaults close-vault`
Close a vault.

```bash
vaults close-vault --vault-name "treasury"
```

#### `vaults propose-withdraw-and-transfer-to-vault`
Create a proposal to withdraw and transfer to a vault.

```bash
vaults propose-withdraw-and-transfer-to-vault --name "Move to Vault" --coin-type "0x456::module::Coin" --coin-id 0x123...abc --coin-amount 1000 --vault-name "treasury"
```

#### `vaults propose-spend-and-transfer`
Create a proposal to spend and transfer from a vault.

```bash
vaults propose-spend-and-transfer --name "Spend from Vault" --coin-type "0x456::module::Coin" --vault-name "treasury" --amounts 1000 2000 --recipients 0x123...abc 0x456...def
```

#### `vaults propose-spend-and-vest`
Create a proposal to spend and vest from a vault.

```bash
vaults propose-spend-and-vest --name "Vest from Vault" --coin-type "0x456::module::Coin" --vault-name "treasury" --coin-amount 5000 --start-timestamp 1640995200000 --end-timestamp 1672531200000 --recipient 0x123...abc
```

---

## Examples

### Creating a New Multisig

```bash
# Start CLI
account-multisig testnet

# Create multisig with 3 members, threshold 2
create --name "Team Treasury" --global-threshold 2 --member 0x123...abc:2:admin,member --member 0x456...def:1:member --member 0x789...ghi:1:member --role admin:2 --role member:1
```

### Managing Proposals

```bash
# List all proposals
proposals

# View proposal details
proposals proposal_key_123

# Approve a proposal
proposals proposal_key_123 approve

# Execute approved proposal
proposals proposal_key_123 execute
```

### Currency Operations

```bash
# Deposit treasury cap
currencies deposit-treasury-cap \
    --cap-id 0x123...abc \
    --coin-type "0x456::module::MyCoin"

# Create mint proposal
currencies propose-mint-and-transfer --name "Team Distribution" --coin-type "0x456::module::MyCoin" --amounts 1000 2000 3000 --recipients 0x123...abc 0x456...def 0x789...ghi
```

---

## Configuration

The CLI uses the same configuration file for authentication as the `sui client`. Ensure you have:

1. A valid Sui configuration file (`~/.sui/sui_config/sui-client.yaml`)
2. An active address with sufficient balance
3. The keypair is Ed25519 (currently required)

---

## Troubleshooting

### Common Issues

- **"User not found"** - Ensure you have a user object created
- **"Multisig not loaded"** - Use `load <multisig_id>` to load a multisig
- **"Invalid arguments"** - Check command syntax and required parameters
- **"Transaction failed"** - Verify you have sufficient gas and permissions

### Getting Help

- Use `help` in interactive mode
- Check the [account.tech documentation](https://account-tech.gitbook.io/docs/)
- Review the [Move Framework](https://github.com/account-tech/move-framework/)

---

## Contributing

Contributions are welcome! Please open issues or pull requests for bug reports, feature requests, or improvements.
