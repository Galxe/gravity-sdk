# Gravity E2E Framework User Manual

## Overview
The Gravity E2E Framework is a Python-based test runner that orchestrates local cluster environments for integration testing. It leverages the existing `cluster/` scripts to provision nodes and uses `pytest` for test execution.

## Quick Start
```bash
# 1. Build Binaries (Host)
cargo build --bin gravity_node --profile quick-release
cargo build --bin gravity_cli --release

# 2. Run Tests
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
