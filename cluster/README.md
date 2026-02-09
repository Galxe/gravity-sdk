# Gravity Cluster Management Tools

This directory contains utility scripts to initialize, deploy, and manage a local Gravity Devnet cluster.

## Quick Start

Follow these steps to get a 4-node cluster running in minutes.

### 1. Prerequisites
Ensure you have the following installed:
*   **Rust**: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
*   **Foundry** (for Genesis): `curl -L https://foundry.paradigm.xyz | bash` then `foundryup`
*   **Python 3**: For parsing configurations.
*   **envsubst**: Usually part of the `gettext` package.

### 2. Setup Configuration

**For a new network:**
```bash
cp genesis.toml.example genesis.toml    # Genesis/validator config
cp cluster.toml.example cluster.toml    # Node deployment config
```

**For joining an existing network:**
```bash
cp cluster.toml.example cluster.toml
# Edit cluster.toml to point genesis_source to the existing genesis files
```

*The default configuration sets up 4 nodes on localhost.*

### 3. Initialize Node Keys
```bash
make init
```
*This generates `identity.yaml` for each validator node.*

### 4. Generate Genesis (New Network Only)
```bash
make genesis
```
*This generates `genesis.json` and `waypoint.txt` in `./output`.*

> ⚠️ **Important**: The `genesis.sh` script will clone `gravity_chain_core_contracts` to `external/` if it doesn't exist, but will **NOT** automatically update it. To update:
> ```bash
> cd external/gravity_chain_core_contracts && git pull origin main
> ```

### 5. Deploy and Start
```bash
make deploy_start
```

Congratulations! Your cluster is now running.
*   Check status: `make status`
*   Stop cluster: `make stop`

---

## Detailed Workflow

### 1. Genesis Generation (`make genesis`)
Only needed when creating a **new network**. Reads `genesis.toml` and generates:
*   `./output/genesis.json` - The genesis block
*   `./output/waypoint.txt` - Initial waypoint for node sync

### 2. Initialization (`make init`)
Generates node identity keys. Reads `cluster.toml` and creates:
*   `./output/nodeX/config/identity.yaml` for each validator node

### 3. Deployment (`make deploy`)
Prepares the runtime environment (default: `/tmp/gravity-cluster`).
*   **Creates hardlinks** for `gravity_node` and `gravity_cli` binaries
*   **Copies** genesis and node keys from `./output` or configured `genesis_source`
*   **Renders** configuration templates for each node
*   **Generates** control scripts (`start.sh`, `stop.sh`)

### 4. Execution (`make start` / `make stop`)
*   `make start`: Launches all nodes in the background
*   `make stop`: Gracefully stops all nodes
*   `make status`: Shows PID, status, and current block number

### 5. Faucet Initialization (`make faucet`)
Optional step to fund testing accounts after the cluster is started.

---

## Configuration Reference

### `genesis.toml` (New Networks Only)

Contains network-wide genesis parameters:

| Section | Description |
|---------|-------------|
| `[genesis]` | Core genesis parameters (epoch interval, etc.) |
| `[genesis.validator_config]` | Validator bond limits and restrictions |
| `[[genesis_validators]]` | Genesis validator list with addresses and stake |

**Important validator fields:**
*   `address` - Validator's ETH address (required)
*   `stake_amount` - Initial stake in Wei (required)  
*   `voting_power` - Initial voting power (must be >= stake_amount)

### `cluster.toml` (Node Deployment)

Controls node deployment:

| Section | Description |
|---------|-------------|
| `[cluster]` | Cluster name and base directory |
| `[build]` | Path to `gravity_node` binary |
| `[genesis_source]` | Paths to genesis.json and waypoint.txt |
| `[[nodes]]` | Node definitions with ports and roles |
| `[faucet_init]` | Optional faucet configuration |

**Node roles:**
*   `genesis` - Included in genesis validator set
*   `validator` - Validator node (can join via on-chain transaction)
*   `vfn` - Full node using onchain discovery

---

## Use Cases

### Create a New Network
```bash
vim genesis.toml        # Configure validators and network params
vim cluster.toml        # Configure node deployment
make init               # Generate node identity keys
make genesis            # Generate genesis.json + waypoint.txt
make deploy_start       # Deploy and start
```

### Join an Existing Network
```bash
vim cluster.toml
# Set genesis_source paths:
#   genesis_path = "/path/to/genesis.json"
#   waypoint_path = "/path/to/waypoint.txt"
#   nodes[0].role = "vfn"

make init               # Generate node keys
make deploy_start       # Deploy and start
```

---

## Makefile Targets

| Target | Description |
|--------|-------------|
| `make genesis` | Generate genesis.json + waypoint.txt |
| `make init` | Generate node identity keys |
| `make deploy` | Deploy node configurations |
| `make start` | Start all nodes |
| `make stop` | Stop all nodes |
| `make status` | Check cluster status |
| `make faucet` | Initialize faucet accounts |
| `make clean` | Remove generated artifacts |
| `make deploy_start` | Deploy and start (convenience) |
| `make restart` | Stop, deploy, and start |
