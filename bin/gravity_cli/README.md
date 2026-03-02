# gravity_cli

Command-line tool for interacting with the Gravity chain. Provides commands for genesis setup, validator management, stake operations, node lifecycle, and DKG queries.

## Build

```bash
RUSTFLAGS="--cfg tokio_unstable" cargo build --bin gravity_cli --profile quick-release
```

## Commands

### `genesis` — Genesis Setup

#### `genesis generate-key`

Generate validator identity keys (consensus, network, and account keys) and write to a YAML file.

```bash
gravity_cli genesis generate-key \
  --output-file <path>         # Output YAML file path (required)
  [--random-seed <hex>]        # 64-char hex seed for deterministic generation (testing only)
```

**Output file format** (`identity.yaml`):
```yaml
account_address: <sha3-256 hash of consensus public key>
account_private_key: <ed25519 private key hex>
consensus_private_key: <bls12-381 private key hex>
network_private_key: <x25519 private key hex>
consensus_public_key: <bls12-381 public key hex, 96 chars>
network_public_key: <x25519 public key hex, 64 chars>
```

#### `genesis generate-waypoint`

Generate a genesis waypoint from a validator set configuration JSON file.

```bash
gravity_cli genesis generate-waypoint \
  --input-file <path>          # Input JSON file with validator set (required)
  --output-file <path>         # Output waypoint file path (required)
```

#### `genesis generate-account`

Generate a new Ethereum-compatible account (private key, public key, address).

```bash
gravity_cli genesis generate-account \
  --output-file <path>         # Output YAML file path (required)
```

---

### `stake` — Stake Pool Operations

#### `stake create`

Create a new StakePool with the specified stake amount.

```bash
gravity_cli stake create \
  --rpc-url <url>              # RPC endpoint (required)
  --private-key <hex>          # Signing key, with or without 0x prefix (required)
  --stake-amount <eth>         # Stake amount in ETH, e.g. "1.0" (required)
  [--gas-limit <num>]          # Gas limit (default: 2000000)
  [--gas-price <wei>]          # Gas price in wei (default: 20)
  [--lockup-duration <secs>]   # Lockup duration in seconds (default: 2592000 = 30 days)
```

**Example:**
```bash
gravity_cli stake create \
  --rpc-url http://127.0.0.1:8551 \
  --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
  --stake-amount 1.0
```

#### `stake get`

Query StakePools owned by a specific address, by scanning `PoolCreated` events.

```bash
gravity_cli stake get \
  --rpc-url <url>              # RPC endpoint (required)
  --owner <address>            # Owner address to query (required)
  [--from-block <block>]       # Starting block: number, "earliest", or "auto" (default: "auto")
  [--to-block <block>]         # Ending block: number or "latest" (default: "latest")
  [--show-voting-power <bool>] # Query voting power for each pool (default: true)
```

> **Note:** `auto` mode queries the latest block and goes back up to 90,000 blocks to stay within reth's max block range limit.

**Example:**
```bash
gravity_cli stake get \
  --rpc-url http://127.0.0.1:8551 \
  --owner 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
```

---

### `validator` — Validator Management

#### `validator join`

Register a validator and join the validator set. This command performs:
1. Validates the StakePool and its voting power
2. Registers the validator with consensus keys and network addresses (if not already registered)
3. Calls `joinValidatorSet` to request activation

```bash
gravity_cli validator join \
  --rpc-url <url>                       # RPC endpoint (required)
  --private-key <hex>                   # Signing key (required)
  --stake-pool <address>                # StakePool address (required)
  --consensus-public-key <hex>          # BLS12-381 public key, 96 hex chars (required)
  --network-public-key <hex>            # x25519 public key, 64 hex chars (required)
  --validator-network-address <addr>    # Format: /ip4/{host}/tcp/{port} (required)
  --fullnode-network-address <addr>     # Format: /ip4/{host}/tcp/{port} (required)
  [--moniker <name>]                    # Display name, max 31 bytes (default: "Gravity1")
  [--consensus-pop <hex>]               # Proof of possession for BLS key
  [--gas-limit <num>]                   # Gas limit (default: 2000000)
  [--gas-price <wei>]                   # Gas price in wei (default: 20)
```

> **Note:** The network addresses are automatically expanded to the full format:
> `/ip4/{host}/tcp/{port}/noise-ik/{network_public_key}/handshake/0`

**Example:**
```bash
gravity_cli validator join \
  --rpc-url http://127.0.0.1:8551 \
  --private-key ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
  --stake-pool 0x2F3Eaf272bf50aCd32fe9C4C4c7C8F3f9CB6bde4 \
  --consensus-public-key 8284ba07212714d28b21c2a437245881496efb6b02b48462cf30485c14840dc7912357a4154e843baaa908f42e74580b \
  --network-public-key 40911cec7b5df3d62f46ea01c9672b909b7b2eb678a8ed0ca2866d708d1f3604 \
  --validator-network-address /ip4/127.0.0.1/tcp/6184 \
  --fullnode-network-address /ip4/127.0.0.1/tcp/6194
```

#### `validator leave`

Request to leave the validator set. The validator transitions to `PENDING_INACTIVE` and becomes `INACTIVE` at the next epoch.

```bash
gravity_cli validator leave \
  --rpc-url <url>              # RPC endpoint (required)
  --private-key <hex>          # Signing key (required)
  --stake-pool <address>       # StakePool address (required)
  [--gas-limit <num>]          # Gas limit (default: 2000000)
  [--gas-price <wei>]          # Gas price in wei (default: 20)
```

**Example:**
```bash
gravity_cli validator leave \
  --rpc-url http://127.0.0.1:8551 \
  --private-key ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
  --stake-pool 0x2F3Eaf272bf50aCd32fe9C4C4c7C8F3f9CB6bde4
```

#### `validator list`

List all validators (active, pending active, pending inactive) and output as JSON.

```bash
gravity_cli validator list \
  --rpc-url <url>              # RPC endpoint (required)
```

**Example:**
```bash
gravity_cli validator list \
  --rpc-url http://127.0.0.1:8551
```

---

### `node` — Node Lifecycle

#### `node start`

Start a Gravity node using the deployment's `script/start.sh`.

```bash
gravity_cli node start \
  --deploy-path <path>         # Deployment directory containing script/start.sh (required)
```

#### `node stop`

Stop a running Gravity node using the deployment's `script/stop.sh`.

```bash
gravity_cli node stop \
  --deploy-path <path>         # Deployment directory containing script/stop.sh (required)
```

---

### `dkg` — Distributed Key Generation

#### `dkg status`

Query the current DKG status from a node's API.

```bash
gravity_cli dkg status \
  --server-url <url>           # Server address (e.g. 127.0.0.1:1024) (required)
```

#### `dkg randomness`

Query the randomness value for a specific block number.

```bash
gravity_cli dkg randomness \
  --server-url <url>           # Server address (e.g. 127.0.0.1:1024) (required)
  --block-number <num>         # Block number to query (required)
```

---

## Validator Lifecycle

The typical validator lifecycle follows these steps:

```
1. genesis generate-key          → Generate identity keys
2. stake create                  → Create a StakePool with initial stake
3. validator join                → Register and join the validator set
   Status: INACTIVE → PENDING_ACTIVE → ACTIVE (next epoch)
4. validator leave               → Request to leave
   Status: ACTIVE → PENDING_INACTIVE → INACTIVE (next epoch)
```

## Input Validation

The CLI performs client-side validation before sending transactions:

| Field | Validation |
|-------|-----------|
| `moniker` | Max 31 bytes |
| `consensus-public-key` | Exactly 96 hex characters (48 bytes BLS key) |
| `network-public-key` | Exactly 64 hex characters (32 bytes) |
| `validator-network-address` | Must match `/ip4/{host}/tcp/{port}` format |
| `fullnode-network-address` | Must match `/ip4/{host}/tcp/{port}` format |
