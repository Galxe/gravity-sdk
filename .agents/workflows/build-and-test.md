---
description: How to build gravity-sdk binaries and run E2E cluster tests
---

# Gravity SDK Build & E2E Test Workflow

## Prerequisites

- **Rust** toolchain (with `cargo`)
- **Foundry** (`forge`, `anvil`) — for genesis generation and bridge tests
- **Python 3** with `tomli`, `web3`, `pytest`, `pytest-asyncio` (see `gravity_e2e/requirements.txt`)
- **envsubst** — from `gettext` package
- **gravity_chain_core_contracts** repo — auto-cloned by `genesis.sh` to `external/gravity_chain_core_contracts`, or set `GRAVITY_CONTRACTS_DIR` env var to a local checkout. Source: `https://github.com/Galxe/gravity_chain_core_contracts.git`

---

## 1. Building Binaries

Working directory: `gravity-sdk`

### Quick-Release Build (faster, used by E2E tests)
// turbo
```bash
RUSTFLAGS="--cfg tokio_unstable" cargo build -p gravity_node --profile quick-release
```
Binary output: `target/quick-release/gravity_node`

### Full Release Build
```bash
RUSTFLAGS="--cfg tokio_unstable" cargo build -p gravity_node --release
```
Binary output: `target/release/gravity_node`

### Build with Makefile
```bash
make gravity_node MODE=quick-release   # or MODE=release
```

### Build gravity_cli (needed for cluster operations)
// turbo
```bash
RUSTFLAGS="--cfg tokio_unstable" cargo build -p gravity_cli --profile quick-release
```

> **Note**: The E2E runner expects both `gravity_node` and `gravity_cli` in `target/quick-release/`.

---

## 2. Local Cluster Management

Working directory: `gravity-sdk/cluster`

### Full lifecycle (new network)
```bash
make init           # Generate node identity keys
make genesis        # Generate genesis.json + waypoint.txt (requires forge)
make deploy         # Prepare runtime environment at base_dir (/tmp/gravity-cluster)
make start          # Launch nodes in background
make status         # Check cluster status
make stop           # Gracefully stop
```

### Convenience targets
```bash
make deploy_start   # deploy + start
make restart        # stop + deploy + start
make clean          # Remove all generated artifacts
make faucet         # Fund testing accounts
```

### Configuration files
- `genesis.toml` — Network-wide genesis parameters (validators, staking, oracle, bridge)
- `cluster.toml` — Node deployment config (ports, binary paths, base_dir)

---

## 3. Running E2E Tests

Working directory: `gravity-sdk`

### Run all test suites
// turbo
```bash
python3 gravity_e2e/runner.py
```
Excludes `long_test` suites by default.

### Run a specific test suite (e.g., bridge)
// turbo
```bash
python3 gravity_e2e/runner.py bridge --bridge-count 10
```

### Runner options
```bash
python3 gravity_e2e/runner.py [SUITE_NAME] [OPTIONS] [-- PYTEST_ARGS]

Options:
  --force-init        Force regeneration of genesis artifacts (ignore cache)
  --no-cleanup        Leave cluster running if tests fail (for debugging)
  --resume            Skip setup, only re-run pytest (cluster already running)
  --exclude SUITE     Exclude specific test suites

Examples:
  python3 gravity_e2e/runner.py bridge --force-init --bridge-count 200
  python3 gravity_e2e/runner.py bridge --resume    # re-run tests, reuse cluster
  python3 gravity_e2e/runner.py --exclude bridge   # run all except bridge
```

### What the runner does (fresh mode)

1. **Cleanup** — kills any stale `gravity_node` processes, removes old cluster data
2. **Init** — generates node identity keys (`init.sh`)
3. **Genesis** — compiles Solidity contracts, generates `genesis.json` (`genesis.sh`). This step clones/pulls `gravity_chain_core_contracts` and runs `forge build`. Takes ~2 minutes first time
4. **Deploy** — copies binaries, genesis, configs to `base_dir` (`deploy.sh`)
5. **Pre-start hooks** — e.g., bridge test starts MockAnvil before node
6. **Start** — launches `gravity_node` processes (`start.sh`)
7. **Pytest** — runs the test suite
8. **Stop** — stops nodes and runs post-stop hooks

### Caching

Genesis artifacts are cached in `cluster_test_cases/<suite>/artifacts/`. Use `--force-init` to regenerate when:
- `genesis.toml` changed
- Contract source (`gravity_chain_core_contracts`) updated
- New contract ref in `dependencies.genesis_contracts.ref`

---

## 4. Bridge E2E Test Details

Suite directory: `gravity_e2e/cluster_test_cases/bridge/`

### Configuration files
- `genesis.toml` — Single-node cluster with oracle + bridge config
- `cluster.toml` — Node deployment (port 8545 RPC, port 6182 P2P)
- `relayer_config.json` — Maps oracle task URI to MockAnvil (port 8546)
- `hooks.py` — Starts MockAnvil before node, stops it after

### Test modes
| Mode | Flag | Description |
|------|------|-------------|
| MockAnvil (default) | `--use-mock-anvil` | Lightweight Python HTTP server, pre-generates `MessageSent` events in memory. No EVM needed |
| Real Anvil | no flag | Starts Foundry Anvil, deploys contracts via `forge script`, sends real bridge txns |

### Key parameters
```
--bridge-count N          Number of bridge events (default: 200 in conftest, 10 in hooks)
--bridge-verify-timeout N Seconds to wait for NativeMinted events (default: 300)
```

### Troubleshooting
- **Port 8546 in use**: Kill stale MockAnvil/Anvil: `kill $(lsof -i :8546 -t)` or `ss -tlnp | grep 8546` to find PID
- **Port 8545 in use**: Kill stale gravity_node: `pkill -9 gravity_node`
- **Genesis outdated**: Use `--force-init` to regenerate
- **`consensus_pop` error**: Must be 96 bytes (192 hex chars) of zeros: `0x` + 192 `0`s
- **`trusted_source_id` missing**: Add `trusted_source_id = <chain_id>` to `[genesis.oracle_config.bridge_config]`

---

## 5. Running Contract Unit Tests

Working directory: `gravity_chain_core_contracts`

// turbo
```bash
forge test -vvv
```

Run specific test file:
```bash
forge test --match-path test/unit/integration/ConsensusEngineFlow.t.sol -vvv
```

Run specific test function:
```bash
forge test --match-test testBridgeFlow -vvv
```