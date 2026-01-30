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
Copy the example configuration file:
```bash
cp cluster.toml.example cluster.toml
```
*The default configuration sets up 4 nodes on localhost starting at port 6180.*

### 3. Initialize Artifacts
Generate validator keys and the genesis block. This acts as the "setup" phase.
```bash
make init
```
*Note: The first run will clone and build the genesis contract, which may take a few minutes.*

> ⚠️ **Important**: The `init.sh` script will clone `gravity_chain_core_contracts` to `external/` if it doesn't exist, but will **NOT** automatically update it if it already exists. If the contracts have been updated upstream, you need to manually pull the latest changes:
> ```bash
> cd external/gravity_chain_core_contracts && git pull origin main
> ```

### 4. Deploy and Start
Deploy the configurations to the runtime directory and start the nodes.
```bash
make deploy_start
```

Congratulations! Your cluster is now running.
*   Check status: `make status`
*   Stop cluster: `make stop`

---

## Detailed Workflow

### 1. Initialization (`make init`)
This step generates the static "metadata" for the cluster and stores it in the `./output` directory.
*   **Keys**: Generates `identity.yaml` for each node.
*   **Genesis**: Aggregates validator info and uses `forge` to compile and generate `genesis.json`.
*   **Waypoint**: Generates `waypoint.txt` from the genesis.

**Why separate?** This ensures that your chain ID and validator keys remain consistent even if you redeploy the node configurations.

### 2. Deployment (`make deploy`)
This step prepares the runtime environment (default: `/tmp/gravity-cluster`).
*   **Cleans** the target directory to remove old data.
*   **Copies** the generated artifacts (keys, genesis) from `./output`.
*   **Renders** configuration templates (`validator.yaml`, `reth_config.json`) with the correct ports and paths defined in `cluster.toml`.
*   **Generates** control scripts (`start.sh`, `stop.sh`) for each node.

### 3. Execution (`make start` / `make stop`)
*   `make start`: Launches all nodes in the background. Logs are written to the node's data directory.
*   `make stop`: Gracefully stops all nodes.
*   `make status`: Shows the PID, status, and current block number of each node.

### 4. Faucet Initialization (`make faucet`)
Optional step to distribute initial funds to a large number of testing accounts.
1.  Configure `[faucet_init]` in `cluster.toml`.
2.  Run `make faucet` after the cluster is started.
3.  Generated accounts are saved to `./output/accounts.csv`.


---

## Configuration Reference (`cluster.toml`)

The `cluster.toml` file controls the entire setup.

### `[cluster]`
*   **name**: Name of the cluster (display only).
*   **base_dir**: The runtime directory where nodes are deployed (e.g., `/tmp/gravity-cluster`).

### `[build]`
*   **binary_path**: Path to the compiled `gravity_node` binary.

### `[[nodes]]`
An array of node configurations. You can add as many nodes as you like.
*   **id**: Unique identifier (e.g., "node1"). Used for directory names.
*   **host**: IP address (use `127.0.0.1` for local).
*   **p2p_port**: The primary P2P port. Other ports are derived relative to this if not specified, but explicit configuration is safer.
*   **rpc_port**: Port for JSON-RPC API.
*   **metrics_port**: Port for Prometheus metrics.
*   **data_dir** (Optional): Override the default data directory path for this node.

### `[faucet_init]`
Optional configuration for auto-generating funded accounts.
*   **num_accounts**: Number of accounts to create and fund (set to 0 to disable).
*   **private_key**: Private key of the faucet (must hold initial funds in genesis).
*   **eth_balance**: Amount of Wei to send to each generated account.

