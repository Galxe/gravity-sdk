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

### 5. Docker Runner (CI)
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
