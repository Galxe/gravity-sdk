# Gravity Join Validator Set Guide

This document describes how to register a deployed node and join the Gravity validator set.

> [!NOTE]
> Please complete [Node Deployment](testnet_deploy.md) before proceeding with this guide.

> [!TIP]
> Sending the join transaction **does not require waiting for sync to complete** — you can execute it immediately after the node starts.

---

## Step 1: Get Public Key Information

Retrieve the two public keys from the identity file:

```bash
cat /home/gravity/gravity-testnet/my-node/config/identity.yaml
```

Record the following fields (without the `0x` prefix):
- `consensus_public_key`
- `network_public_key`

---

## Step 2: Create a StakePool

```bash
./gravity_cli stake create \
  --rpc-url http://<EXISTING_NODE_IP>:8545 \
  --private-key <YOUR_PRIVATE_KEY> \
  --stake-amount <AMOUNT_IN_ETH>
```

On success, the output will include `Pool address` — record it for the next step.

**Query existing StakePool**:

```bash
./gravity_cli stake get \
  --rpc-url http://<EXISTING_NODE_IP>:8545 \
  --owner <YOUR_WALLET_ADDRESS>
```

---

## Step 3: Send Validator Join Transaction

```bash
./gravity_cli validator join \
  --rpc-url http://<EXISTING_NODE_IP>:8545 \
  --private-key <YOUR_PRIVATE_KEY> \
  --stake-pool <STAKE_POOL_ADDRESS> \
  --consensus-public-key "<CONSENSUS_PUBLIC_KEY>" \
  --network-public-key "<NETWORK_PUBLIC_KEY>" \
  --validator-network-address "/ip4/<YOUR_IP>/tcp/6180" \
  --fullnode-network-address "/ip4/<YOUR_IP>/tcp/6190" \
  --moniker "<MY_VALIDATOR_NAME>"
```

**Parameter reference**:

| Parameter | Description |
|-----------|-------------|
| `--rpc-url` | RPC address of any running node |
| `--private-key` | EVM account private key with balance (with `0x` prefix) |
| `--stake-pool` | StakePool address created via `stake create` |
| `--consensus-public-key` | From identity.yaml, **without `0x` prefix** |
| `--network-public-key` | From identity.yaml, 64-char hex (32 bytes) |
| `--validator-network-address` | P2P address, format `/ip4/{IP}/tcp/{port}`, CLI auto-appends noise-ik |
| `--fullnode-network-address` | VFN address, format `/ip4/{IP}/tcp/{port}`, CLI auto-appends noise-ik |
| `--moniker` | Validator name (max 31 bytes) |

> [!NOTE]
> The CLI automatically performs two steps: **Register Validator** → **Join Validator Set**. If already registered, the registration step is skipped.

> [!CAUTION]
> - The IP in the network address **must** be reachable by other nodes (use public IP for cross-VPC)
> - `--stake-pool` must be a created StakePool with sufficient stake

---

## Step 4: Additional VFN Mode Configuration (VFN mode only)

> [!IMPORTANT]
> If you deployed in **Validator mode**, skip this step and go directly to [Step 5](#step-5-verify-status).

Nodes deployed in VFN mode need to upgrade their config from `full_node` to `validator` after joining the validator set.

Edit `config/validator_full_node.yaml`:

```diff
 base:
-  role: "full_node"
+  role: "validator"
```

And **add** a `validator_network` section (before `full_node_networks`):

```yaml
validator_network:
  network_id: validator
  listen_address: "/ip4/0.0.0.0/tcp/6180"
  discovery_method:
    onchain
  mutual_authentication: true
  identity:
    type: "from_file"
    path: <CONFIG_DIR>/identity.yaml
```

Then **restart the node**:

```bash
cd /home/gravity/gravity-testnet/my-node/script
./stop.sh && ./start.sh
```

---

## Step 5: Verify Status

```bash
./gravity_cli validator list --rpc-url http://34.83.28.182:8545
```

On success, your node will appear in the `pending_active` list and will automatically become `ACTIVE` after the **next epoch** transition.

Once `ACTIVE`, you can confirm block production via consensus logs:

```bash
# Validator mode
grep "send block to execution" consensus_log/validator.log

# VFN mode
grep "send block to execution" consensus_log/vfn.log
```

---

## Leaving the Validator Set

```bash
./gravity_cli validator leave \
  --rpc-url http://<EXISTING_NODE_IP>:8545 \
  --private-key <YOUR_PRIVATE_KEY> \
  --stake-pool <YOUR_STAKE_POOL_ADDRESS>
```

After leaving, the node transitions to `PENDING_INACTIVE` and becomes `INACTIVE` after the next epoch.

---

## Troubleshooting

| Issue | Solution |
|-------|----------|
| `validator join` hangs at "Registering validator" | Wait for the 60s timeout and retry with `--stake-pool` |
| `insufficient funds` error | `--stake-amount` is in ETH not wei; check if the value is too large |
| Stuck in `pending_active` | `--stake-amount` exceeds `votingPowerIncreaseLimitPct` limit. Formula: `maxIncrease = totalVotingPower × limitPct%`. E.g., with total 3000 and 20% limit, max increase is 600. Must leave and re-join with a smaller amount |
| On-chain shows `YOUR_PUBLIC_IP` | Placeholder was used in network address; currently requires leave → re-register + join |
