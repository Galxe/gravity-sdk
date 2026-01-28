# Adding Test Cases Guide

This guide explains how to add new test cases using the **Gravity E2E Framework**.

> [!NOTE]
> This framework uses `pytest-asyncio` and a shared `cluster` fixture to manage local test networks.

## Directory Structure

All test suites reside in `gravity_e2e/cluster_test_cases/`.

```
gravity_e2e/cluster_test_cases/
└── <suite_name>/              # e.g., single_node
    ├── cluster.toml           # Network configuration
    └── test_*.py              # Test files
```

## Step-by-Step Guide

### 1. Create a Suite Directory
Create a new folder for your test suite:
```bash
mkdir -p gravity_e2e/cluster_test_cases/my_new_suite
```

### 2. Configure the Cluster
Create `gravity_e2e/cluster_test_cases/my_new_suite/cluster.toml`:

```toml
[cluster]
base_dir = "data"

[[nodes]]
id = "node1"
rpc_port = 8545
mode = "validator"
stake = 100
```

### 3. Write the Test Case
Create `gravity_e2e/cluster_test_cases/my_new_suite/test_example.py`. 
Use the `cluster` fixture to control the environment.

```python
import pytest
import logging
from gravity_e2e.cluster.manager import Cluster

LOG = logging.getLogger(__name__)

@pytest.mark.asyncio
async def test_basic_operation(cluster: Cluster):
    """
    Example test: Start cluster and verify block production.
    """
    # 1. Start the cluster (idempotent)
    # This automatically provisions nodes based on cluster.toml
    await cluster.set_full_live()
    
    # 2. Verify all nodes are up
    # 'cluster.nodes' is a dictionary of {node_id: Node}
    assert len(cluster.nodes) > 0
    
    # 3. Interact with nodes
    # Each 'node' object has helper methods and an 'rpc' accessor
    node = cluster.get_node("node1")
    height = await node.get_block_number()
    LOG.info(f"Node1 is at height: {height}")
    assert height >= 0

    # 4. Verify network liveness
    # Helper to ensure blocks are being produced
    assert await cluster.check_block_increasing(timeout=30)
```

## Running the Test

Use the `run_test.sh` script, passing the **suite directory name** as the argument.

```bash
# Run the entire suite
./gravity_e2e/run_test.sh my_new_suite

# Run a specific test case within the suite
./gravity_e2e/run_test.sh my_new_suite -k test_basic_operation
```
