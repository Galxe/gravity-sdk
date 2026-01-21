# Gravity E2E Testing Guide

This guide explains how to write and run end-to-end (E2E) tests for Gravit nodes using the `pytest` framework and our cluster management tools.

## Key Concepts

-   **Cluster**: A set of deployed Gravity nodes described by a `cluster.toml` file.
-   **Fixture**: Pytest helpers that provide pre-configured objects (like connected clients) to your tests.
-   **Lifecycle**: Tests can control node states (Start/Stop) using the `cluster` fixture.

## Writing a New Test

Create a new file in `gravity_e2e/tests/test_cases/`.

### 1. Basic Connectivity Test
If you just need to interact with a running node (e.g., check balance, call contract), use `run_helper`.

```python
from gravity_e2e.helpers.test_helpers import test_case, RunHelper, TestResult

@test_case
async def test_my_feature(run_helper: RunHelper, test_result: TestResult):
    # run_helper.client is a pre-connected GravityClient
    block = await run_helper.client.get_block_number()
    
    # Assertions
    assert block > 0
    
    test_result.mark_success()
```

### 2. Cluster Lifecycle Test
If you need to stop/start nodes to test fault tolerance, use the `cluster` fixture.
**Note**: You must use `gravity_e2e.cluster.node.NodeState` for state checks.

```python
from gravity_e2e.helpers.test_helpers import test_case, TestResult
from gravity_e2e.cluster.manager import Cluster, NodeState

@test_case
async def test_restart_node(cluster: Cluster, test_result: TestResult):
    # 1. Stop node1
    await cluster.set_node("node1", NodeState.STOPPED)
    
    # 2. Verify it's down
    status = await cluster.get_node_status("node1")
    assert status == NodeState.STOPPED
    
    # 3. Start it back up
    await cluster.set_node("node1", NodeState.RUNNING)
    
    test_result.mark_success()
```

## Running Tests

Run tests using the `main.py` entry point:

```bash
# Run all tests in the cluster_ops suite
python3 -m gravity_e2e.main --test-suite cluster_ops --cluster-config ../cluster/cluster.toml

# Run a specific test file
pytest gravity_e2e/tests/test_cases/test_my_new_test.py --cluster-config ../cluster/cluster.toml
```

## Configuration

-   **`cluster.toml`**: Defines the nodes, ports, and paths.
-   **`test_accounts.json`**: Pre-funded accounts provided by `run_helper`.

## Troubleshooting

-   **"Node X started but RPC never came up"**: Check `logs/cluster.log` or the node's local logs in its infra directory.
-   **"Connection Refused"**: Ensure the cluster is actually running. Use `cluster.set_full_live()` in your test setup if needed.
