# Gravity Node Deployment Guide

This document describes how to deploy a new Gravity node and join an existing network. Two deployment modes are supported:

| Mode | cluster.toml `role` | Generated Config | Description |
|------|---------------------|------------------|-------------|
| **Validator** | `validator` | `validator.yaml` | Deploys directly as a validator; no config changes needed after join |
| **VFN** | `vfn` | `validator_full_node.yaml` | Runs as a full node first; manual config changes required after join |

> [!TIP]
> The **Validator mode** is recommended — the deployment process is simpler and requires no additional configuration changes after joining.

---

## Prerequisites

1. **Running Gravity network**: At least 3 Genesis validator nodes must be running

   | Node ID | Server | IP |
   |---------|--------|------|
   | node1 | gravity-testnet-node-oregon-0 | 34.83.28.182 |
   | node2 | gravity-testnet-node-oregon-1 | 34.83.9.159 |
   | node3 | gravity-testnet-node-losangeles | 34.94.164.9 |

2. **New node server**: Dependencies installed and binaries compiled

## Environment Setup

System dependencies (Ubuntu/Debian):

```bash
apt-get install -y --no-install-recommends \
    clang llvm build-essential pkg-config libssl-dev libudev-dev \
    procps git jq curl python3 python3-pip python3-venv \
    nodejs npm protobuf-compiler bc gettext-base
```

Toolchain:
- **Rust**: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **Foundry**: `curl -L https://foundry.paradigm.xyz | bash && foundryup`

---

3. **Build binaries**:
   ```bash
   RUSTFLAGS="--cfg tokio_unstable" cargo build --profile quick-release -p gravity_node -p gravity_cli
   ```
4. **Network connectivity**: The new node must be able to reach existing nodes' P2P ports (default 6180/6190)
5. **Funded account**: An EVM account for staking (needs sufficient balance for gas + stake amount)

---

## Step 1: Obtain Genesis Files

Get the following two files from the `genesis/testnet/` directory of this repository:

- [`genesis.json`](../genesis.json)
- [`waypoint.txt`](../waypoint.txt)

## Step 2: Configure cluster.toml

Create `cluster.toml` in the `gravity-sdk/cluster/` directory on the new node:

```toml
[cluster]
name = "my-validator"                   # ← Change: cluster name
base_dir = "/home/gravity/gravity-testnet"  # ← Change: deployment directory

[build]
binary_path = "../target/quick-release/gravity_node"  # ← Change: binary path

[genesis_source]
genesis_path = "./output/genesis.json"   # ← Change: genesis file path
waypoint_path = "./output/waypoint.txt"  # ← Change: waypoint file path

[[nodes]]
id = "my-node"
role = "validator"                      # or "vfn"
host = "<YOUR_IP>"                      # Node IP (reachable by other validators)
p2p_port = 6180
vfn_port = 6190
rpc_port = 8545
metrics_port = 9001
inspection_port = 10000
https_port = 1024
authrpc_port = 8551
reth_p2p_port = 12024
```

> [!IMPORTANT]
> - **Validator mode**: `role = "validator"` — deploy generates `validator.yaml` config with a `validator_network` section
> - **VFN mode**: `role = "vfn"` — deploy generates `validator_full_node.yaml` config with `base.role` set to `full_node`

## Step 3: Initialize and Deploy

```bash
cd gravity-sdk/cluster
make init      # Generates identity (consensus_public_key + network_public_key)
make deploy    # Generates runtime config and startup scripts
```

Directory structure after deployment:

```
/home/gravity/gravity-testnet/
├── genesis.json
├── gravity_node
├── gravity_cli
└── my-node/
    ├── config/
    │   ├── validator.yaml              # Validator mode
    │   │   (or validator_full_node.yaml)  # VFN mode
    │   ├── identity.yaml
    │   ├── reth_config.json
    │   ├── relayer_config.json
    │   └── waypoint.txt
    ├── script/
    │   ├── start.sh
    │   └── stop.sh
    ├── data/
    ├── logs/
    ├── execution_logs/
    └── consensus_log/
```

## Step 4: Start the Node

```bash
cd /home/gravity/gravity-testnet/my-node/script
./start.sh
```

## Step 5: Verify the Node is Running

```bash
# Check process
ps aux | grep gravity_node

# View logs (you should see blocks being synced)
tail -f /home/gravity/gravity-testnet/my-node/execution_logs/<chain_id>/reth.log
```

After starting, the node will automatically sync data from the network.

- **Validator mode**: The node already has validator config. After completing [Join Validator Set](testnet_join.md), it can participate in consensus
- **VFN mode**: The node runs as a full node providing RPC services but not participating in consensus. After completing [Join Validator Set](testnet_join.md), additional config changes are required

---

## Next Steps

After node deployment is complete, refer to the **[Join Validator Set Guide](testnet_join.md)** to finish on-chain registration and joining.

---

## Troubleshooting

| Issue | Solution |
|-------|----------|
| Node cannot sync | Verify genesis.json and waypoint.txt match the network |
| On-chain shows `YOUR_PUBLIC_IP` | Placeholder was used in network address; currently requires leave → re-register + join |
