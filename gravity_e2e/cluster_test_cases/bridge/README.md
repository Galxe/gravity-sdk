# Bridge E2E Test Suite

Tests the full bridge flow from an external EVM chain (Anvil) to the Gravity chain, verifying cross-chain token minting via oracle consensus.

## End-to-End Data Flow

```mermaid
sequenceDiagram
    participant Test as test_bridge.py
    participant Anvil as Anvil (port 8546)
    participant Node as gravity_node (port 8545)
    
    Note over Test: Setup Phase
    Test->>Anvil: Start Anvil + forge deploy contracts
    Test->>Node: Write relayer_config (Anvil URI mapping)
    
    Note over Test: 10-min Loop
    loop Each iteration
        Test->>Node: Record recipient balance (before)
        Test->>Anvil: web3.py: mint → approve → bridgeToGravity()
        Anvil-->>Anvil: emit MessageSent
        Node->>Anvil: Relayer fetches event via RPC
        Node->>Node: Consensus → GBridgeReceiver → NativeMinted
        Test->>Node: poll_native_minted(nonce) ✓
        Test->>Node: Verify balance_after - balance_before == amount ✓
    end

    Note over Test: Report stats (latency, success rate, nonce continuity)
```

## Prerequisites

- **Anvil** (from foundry) installed and in PATH
- **forge** installed and in PATH  
- **gravity_chain_core_contracts** repo available (default: `~/projects/gravity_chain_core_contracts`)
- **gravity_node** binary built

## How to Run

### Via the runner script

```bash
cd gravity-sdk/gravity_e2e
./run_test.sh bridge
```

### Directly with pytest

```bash
# From gravity_e2e/ directory
pytest cluster_test_cases/bridge/test_bridge.py \
    --cluster-config cluster_test_cases/bridge/cluster.toml \
    --contracts-dir ~/projects/gravity_chain_core_contracts \
    --bridge-duration 600 \
    -v -s
```

### Custom duration (e.g., 1 minute for quick smoke test)

```bash
pytest cluster_test_cases/bridge/test_bridge.py --bridge-duration 60 -v -s
```

## Configuration

### `cluster.toml`

Single-node cluster with oracle + bridge config:

- `oracle_config.bridge_config.deploy = true` — deploys `GBridgeReceiver` at genesis
- `oracle_config.bridge_config.trusted_bridge` — points to Anvil's `GBridgeSender` (deterministic address)
- `oracle_config.tasks` — watches Anvil (chainId 31337) for `MessageSent` events on `GravityPortal`

### Relayer config

Dynamically written by the `anvil_bridge` fixture into each node's config dir:

```json
{
  "uri_mappings": {
    "gravity://0/31337/events?contract=0xe7f1...&eventSignature=0x5646...&fromBlock=0": "http://localhost:8546"
  }
}
```

## Contract Addresses (Anvil Deterministic)

| Contract | Address | Function |
|----------|---------|----------|
| MockGToken | `0x5FbDB2315678afecb367f032d93F642f64180aa3` | ERC20 token on Anvil |
| GravityPortal | `0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512` | Message relay on Anvil |
| GBridgeSender | `0x9fE46736679d2D9a65F0992F2272dE9f3c7fa6e0` | Bridge entry point on Anvil |
| GBridgeReceiver | `0x595475934ed7d9faa7fca28341c2ce583904a44e` | Genesis-deployed on Gravity |

## File Structure

```
gravity_e2e/
├── gravity_e2e/utils/
│   ├── anvil_manager.py      # Anvil process lifecycle + forge deploy
│   └── bridge_utils.py       # BridgeHelper, poll_native_minted, BridgeStats
└── cluster_test_cases/bridge/
    ├── README.md              # This file
    ├── cluster.toml           # Single-node cluster config
    ├── conftest.py            # anvil_bridge + bridge_helper fixtures
    └── test_bridge.py         # 10-min continuous bridge loop test
```
