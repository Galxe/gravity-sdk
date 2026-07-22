# Gravity E2E Framework User Manual

## Overview
The Gravity E2E Framework is a Python-based test runner that orchestrates local cluster environments for integration testing. It leverages the existing `cluster/` scripts to provision nodes and uses `pytest` for test execution.

## Prerequisites

### macOS Users
The cluster scripts use GNU sed syntax. macOS ships with BSD sed which is incompatible. Install GNU sed first:
```bash
brew install gnu-sed
export PATH="/opt/homebrew/opt/gnu-sed/libexec/gnubin:$PATH"
```
Add the export line to your shell profile (`~/.zshrc` or `~/.bashrc`) to make it permanent.

### Python Environment
Python 3.8+ is required. It's recommended to use a virtual environment:
```bash
cd gravity_e2e
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
```

## Quick Start
```bash
# 1. Build Binaries (Host)
# Option A: Use make (recommended, handles RUSTFLAGS automatically)
make BINARY=gravity_node MODE=quick-release
make BINARY=gravity_cli MODE=release

# Option B: Use cargo directly (must set RUSTFLAGS)
RUSTFLAGS="--cfg tokio_unstable" cargo build --bin gravity_node --profile quick-release
RUSTFLAGS="--cfg tokio_unstable" cargo build --bin gravity_cli --release

# 2. Run Tests (ensure venv is activated)
source gravity_e2e/.venv/bin/activate
./gravity_e2e/run_test.sh
```

## Advanced Usage

### 1. Filtering Tests
The runner smartly forwards arguments to Pytest.
```bash
# Run specific suite (directory name matches)
./gravity_e2e/run_test.sh single_node

# Run specific test case (using pytest -k)
./gravity_e2e/run_test.sh -k test_connectivity

# Combine suite and test filter
./gravity_e2e/run_test.sh single_node -k test_connectivity

# Exclude specific suites
./gravity_e2e/run_test.sh --exclude fuzzy_cluster
```

### 2. Artifact Caching
The runner configures the cluster scripts to output artifacts (generated keys, genesis files) directly to the test suite directory.
-   **Artifacts Path**: `gravity_e2e/cluster_test_cases/<suite>/artifacts/`
-   **Behavior**:
    -   If valid artifacts exist in this folder, `init.sh` is skipped.
    -   If you need to regenerate (e.g. after changing `cluster.toml`), use `--force-init`.
    ```bash
    ./gravity_e2e/run_test.sh single_node --force-init
    ```

### 3. Debugging Failures
Use `--no-cleanup` to inspect the cluster after a failure.
```bash
./gravity_e2e/run_test.sh single_node --no-cleanup
```

### 4. Custom Logging
You can control the Python logging level by passing standard pytest flags to the runner.
```bash
# Enable CLI logging at DEBUG level to see internal logs
./gravity_e2e/run_test.sh --log-cli-level=DEBUG
```

### 5. Polymarket Mock Oracle Suite
The `polymarket_mock` suite is a local-only integration test for a Polymarket-like settlement
flow on Gravity. It starts a local Polygon JSON-RPC mock, maps the oracle task
URI to that mock, waits for the relayer and unsupported-JWK/oracle consensus path
to publish `sourceType=6` bytes into `NativeOracle`, and then settles a
match-market contract from the resolver result.

Run it from the repository root after building `gravity_node` and `gravity_cli`:
```bash
PATH="$CONDA_PREFIX/bin:$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh polymarket_mock --force-init
```

If you use a virtualenv instead of conda, activate it first and omit
`$CONDA_PREFIX/bin` from the `PATH` prefix:
```bash
source gravity_e2e/.venv/bin/activate
PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh polymarket_mock --force-init
```

Expected success logs include:
```text
Released mock Polymarket settlement: winning_slot=<slot> payout=<vector>
Polymarket match market resolved and claimed: marketId=1 winningSlot=<slot> totalPool=600000000000000000000
PASSED
Suite polymarket_mock PASSED
All suites passed!
```

The suite README at `gravity_e2e/cluster_test_cases/polymarket_mock/README.md`
describes the local topology, what the test proves, and how this can evolve into
a production Polymarket-like Gravity product.

### 6. Binance Index-Kline Price Feed Suite
The `binance_price_feed` suite is a local-only integration test for continuous stock-like
price feed rounds on Gravity. It registers deterministic
`provider=binance_index_kline_v1` `sourceType=3` tasks for
`NVDAUSDT` and `TSLAUSDT` through governance, starts a local mock Binance
`/fapi/v1/indexPriceKlines` server, waits for the next short test epoch so
`aptos-jwk-consensus` rebuilds relayer-backed observers, then waits for the
unsupported-JWK/oracle consensus path to publish at least three price rounds
into `NativeOracle` and checks `PriceFeedResolver.priceRounds`.

The suite intentionally does not call live Binance. The local mock validates the
same closed-bucket request shape that the real Binance adapter uses:
`pair`, `interval`, `startTime`, `endTime`, and `limit=1`. Live Binance testnet
fetching is covered separately by the ignored `gravity-reth` relayer smoke test.

This suite proves the epoch-config path for long-running feeds. It does not
implement intra-epoch dynamic request discovery; that still needs a deterministic
request watcher if Gravity wants one-off requests such as a future match score.
The Binance provider itself is always continuous and rejects the legacy
`continuous` URI parameter. Keep `bucketStartMs` and `interval` immutable for
each `feedId`; introduce a new `feedId` when either changes.

Run it from the repository root after building `gravity_node` and `gravity_cli`:
```bash
PATH="$CONDA_PREFIX/bin:$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh binance_price_feed --force-init
```

To keep the price-feed backend alive for the web demo, run:
```bash
PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh binance_price_feed \
    --force-init \
    --keep-running \
    --demo-config-out ../gravity_price_feed_demo_web/public/demo-config.json \
    --log-cli-level=INFO
```

The keep-running mode leaves the Gravity node and the local Binance kline mock
running after the suite passes. The web app reads the generated
`public/demo-config.json` to discover the local RPC URL, resolver address, feed
IDs, expected nonce, and expected round.

For a live Binance demo, set `BINANCE_PRICE_FEED_MODE=live`. This mode is for
manual demos and sends outbound requests to Binance public market-data APIs. It
does not use `BINANCE_API_KEY` or `BINANCE_SECRET_KEY`.

```bash
BINANCE_PRICE_FEED_MODE=live \
BINANCE_PRICE_FEED_LAG_MINUTES=3 \
PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh binance_price_feed \
    --force-init \
    --keep-running \
    --demo-config-out ../gravity_price_feed_demo_web/public/demo-config.json \
    --log-cli-level=INFO
```

Live mode computes a 1-minute bucket that is at least two minutes past close,
generates a run-local relayer config, registers matching
`provider=binance_index_kline_v1` tasks, and then keeps the local chain running
so the frontend can display ongoing resolver updates.

Expected success logs include:
```text
Binance price feed resolved: feedId=1001 deliveryNonce=3 roundId=29720877 price=19612645000
Binance price feed resolved: feedId=1002 deliveryNonce=3 roundId=29720877 price=40117545000
PASSED
Suite binance_price_feed PASSED
All suites passed!
```

This proves the contract-facing path a Gravity PerpDex or price-index market
would consume: `sourceType=3` data is recorded by `NativeOracle`, callbacked into
`PriceFeedResolver`, and exposed as `latestPrice(feedId)`. BBO, mid
price, TWAP, risk, and market-specific weighting should be decided by the
downstream product contract or by a separately versioned resolver policy.

### 7. Combined Binance + Polymarket Suite

The `oracle_demo` suite is the review and dashboard gate for both current
oracle products. One local Gravity cluster receives continuous Binance
index-kline rounds and a finalized Polymarket-style CTF settlement through the
same unsupported-JWK consensus path.

Run it without public network traffic:

```bash
PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh oracle_demo --force-init
```

Keep the backend alive and write frontend discovery metadata:

```bash
PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh oracle_demo \
    --force-init \
    --keep-running \
    --demo-config-out ../gravity_price_feed_demo_web/public/demo-config.json \
    --log-cli-level=INFO
```

The suite directly deploys `PriceFeedResolver`,
`PolymarketSettlementResolver`, and `PolymarketBinaryMarket`. Shared deployment,
governance, polling, and deterministic provider helpers live under
`gravity_e2e/gravity_e2e/utils/`; product tests should reuse them instead of
importing another pytest module.

Expected terminal evidence includes two resolved price feeds, one released CTF
payout vector, one settled Gravity market, and `Suite oracle_demo PASSED`.

### 8. Docker Runner (CI)
The `run_docker.sh` script runs the full pipeline inside Docker. It accepts the same arguments as `run_test.sh`:
```bash
# Run all suites in Docker
./gravity_e2e/run_docker.sh

# Run specific suite
./gravity_e2e/run_docker.sh single_node

# Exclude suites
./gravity_e2e/run_docker.sh --exclude fuzzy_cluster
```

## CI/CD Workflows

### PR Workflow (`e2e-docker.yml`)
Triggered on every PR to `main` / `gravity-testnet-v**`. Runs **all suites except `fuzzy_cluster`** to keep PR feedback fast.

Can also be triggered manually via `workflow_dispatch` with an optional `suite` input to run specific suites (including `fuzzy_cluster`).

### Nightly Workflow (`e2e-docker-nightly.yml`)
Runs **only `fuzzy_cluster`** on a daily schedule (00:00 UTC / 08:00 UTC+8). Also supports manual `workflow_dispatch`.

| Trigger | Suites Run |
|---|---|
| PR | All **except** `fuzzy_cluster` |
| Manual dispatch (e2e-docker) | User-specified (or all) |
| Nightly schedule | `fuzzy_cluster` only |

## Writing Tests

### Directory Structure
Tests are located in `gravity_e2e/cluster_test_cases/`.
Each suite directory (e.g., `single_node`) must contain:
1.  `cluster.toml`: Defines the network.
2.  `test_*.py`: Pytest files.

### Shared Fixtures
Common fixtures (like `cluster`) are defined in `gravity_e2e/conftest.py`.

### Using the `cluster` Fixture
```python
import pytest
from gravity_e2e.cluster.manager import Cluster

@pytest.mark.asyncio
async def test_my_feature(cluster: Cluster):
    # Ensure cluster is ready
    await cluster.set_full_live()

    # Get a node and interact
    node = cluster.get_node("node1")
    # ...
```
