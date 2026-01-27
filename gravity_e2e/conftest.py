"""
Pytest configuration and fixtures for Gravity E2E tests.

This module provides shared fixtures for all pytest-based tests,
including node connections, account management, and test helpers.

Usage:
    # In test files, fixtures are automatically injected:
    
    @test_case
    async def test_something(run_helper, test_result):
        account = await run_helper.create_test_account("test", fund_wei=10**18)
        # ... test code
        test_result.mark_success(account=account['address'])
"""

import asyncio
import logging
import sys
import os
from pathlib import Path
from typing import AsyncGenerator, Optional

# Add the parent package to path for imports
# This allows pytest to find the gravity_e2e package
_current_dir = Path(__file__).resolve().parent
# We are in the root of the repo (gravity_e2e/), so we add this directory to path
# to allow 'import gravity_e2e.cluster' (which resolves to gravity_e2e/gravity_e2e/cluster)
if str(_current_dir) not in sys.path:
    sys.path.insert(0, str(_current_dir))

import pytest
import pytest_asyncio

# Use absolute imports now that path is set
from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.cluster.client.gravity_client import GravityClient
from gravity_e2e.helpers.account_manager import TestAccountManager
from gravity_e2e.helpers.test_helpers import RunHelper, TestResult

LOG = logging.getLogger(__name__)


# Configuration defaults
DEFAULT_NODES_CONFIG = "configs/nodes.json" # Legacy default, but we prefer cluster.toml logic
DEFAULT_CLUSTER_CONFIG = "../cluster/cluster.toml" # Relative to this file if not specified
DEFAULT_ACCOUNTS_CONFIG = "configs/test_accounts.json"
DEFAULT_OUTPUT_DIR = "output"


def pytest_addoption(parser):
    """Add custom command line options for pytest."""
    parser.addoption(
        "--nodes-config",
        action="store",
        default=DEFAULT_NODES_CONFIG,
        help="Path to nodes configuration file (Legacy, prefer --cluster-config)"
    )
    parser.addoption(
        "--cluster-config",
        action="store",
        default=None,
        help="Path to cluster.toml configuration file"
    )
    parser.addoption(
        "--accounts-config",
        action="store",
        default=DEFAULT_ACCOUNTS_CONFIG,
        help="Path to accounts configuration file"
    )
    parser.addoption(
        "--output-dir",
        action="store",
        default=DEFAULT_OUTPUT_DIR,
        help="Output directory for test results"
    )
    parser.addoption(
        "--node-id",
        action="store",
        default=None,
        help="Specific node ID to test against"
    )
    parser.addoption(
        "--cluster",
        action="store",
        default=None,
        help="Cluster name to test"
    )


@pytest.fixture(scope="session")
def cluster_config_path(request) -> Path:
    """Get cluster configuration path."""
    val = request.config.getoption("--cluster-config")
    if val:
        return Path(val).resolve()
    
    # Check env var set by runner
    env_val = os.environ.get("GRAVITY_CLUSTER_CONFIG")
    if env_val:
        return Path(env_val).resolve()
    
    # Try default locations
    # 1. ../cluster/cluster.toml relative to tests/
    default_loc = Path(__file__).parent.parent.parent / "cluster" / "cluster.toml"
    if default_loc.exists():
        return default_loc
        
    return Path("cluster.toml").resolve()


@pytest.fixture(scope="session")
def nodes_config_path(request) -> str:
    """Get nodes configuration path from command line or default."""
    return request.config.getoption("--nodes-config")


@pytest.fixture(scope="session")
def accounts_config_path(request) -> str:
    """Get accounts configuration path from command line or default."""
    return request.config.getoption("--accounts-config")


@pytest.fixture(scope="session")
def output_dir(request) -> Path:
    """Get output directory and create if needed."""
    output_path = Path(request.config.getoption("--output-dir"))
    output_path.mkdir(parents=True, exist_ok=True)
    return output_path


@pytest.fixture(scope="session")
def target_node_id(request) -> Optional[str]:
    """Get target node ID from command line."""
    return request.config.getoption("--node-id")


@pytest.fixture(scope="session")
def target_cluster(request) -> Optional[str]:
    """Get target cluster from command line."""
    return request.config.getoption("--cluster")


@pytest_asyncio.fixture(scope="session")
async def cluster(cluster_config_path: Path) -> AsyncGenerator[Cluster, None]:
    """
    Create and manage Cluster for the test session.
    Yields a Cluster instance ready for use.
    """
    LOG.info(f"Loading cluster from {cluster_config_path}")
    c = Cluster(cluster_config_path)
    
    # Optional: ensure we can talk to at least one node?
    # Or just yield it and let tests decide.
    # Given existing tests expect connection, maybe we should verify.
    
    # For now, just yield. Tests that need connectivity will fail fast if nodes are down.
    yield c
    
    # Cleanup? Cluster object doesn't really have open resources except clients inside nodes
    # We could explicitly close clients if we wanted.
    for node in c.nodes.values():
        if node.client.session:
            await node.client.session.close()


# Alias for backward compatibility if tests request 'node_connector'
# But ideally tests should migrate to 'cluster'
@pytest_asyncio.fixture(scope="session")
async def node_connector(cluster: Cluster) -> Cluster:
    """DEPRECATED: Compatibility shim. Returns cluster object."""
    return cluster


@pytest_asyncio.fixture(scope="session")
async def account_manager(accounts_config_path: str) -> TestAccountManager:
    """
    Create account manager for the test session.
    
    Returns:
        TestAccountManager instance
    """
    return TestAccountManager(accounts_config_path)


@pytest_asyncio.fixture(scope="function")
async def gravity_client(
    cluster: Cluster,
    target_node_id: Optional[str]
) -> GravityClient:
    """
    Get a Gravity client for tests.
    
    Creates a new client for each test function to avoid aiohttp session
    sharing issues across different async tasks.
    """
    node = None
    node_id = None
    
    if target_node_id:
        node = cluster.get_node(target_node_id)
        if not node:
            pytest.skip(f"Node {target_node_id} not found in cluster")
        node_id = target_node_id
    else:
        # Get first connected/running node
        # Prefer RUNNING nodes
        live_nodes = await cluster.get_live_nodes()
        if live_nodes:
            node = live_nodes[0]
            node_id = node.id
        else:
            # Fallback to any node if none are confirmed live (maybe they are starting up)
            # Connectivity test might fail, but we try.
            all_nodes = list(cluster.nodes.values())
            if not all_nodes:
                pytest.skip("No nodes in cluster config")
            node = all_nodes[0]
            node_id = node.id

    rpc_url = node.url
    
    # Create a fresh client for this test function
    # Note: We are using the NEW client class in cluster.client
    client = GravityClient(rpc_url, node_id)
    async with client:
        yield client


@pytest_asyncio.fixture(scope="function")
async def run_helper(
    gravity_client: GravityClient,
    account_manager: TestAccountManager,
    output_dir: Path
) -> RunHelper:
    """
    Create RunHelper for test execution.
    
    Returns:
        Configured RunHelper instance
    """
    return RunHelper(
        client=gravity_client,
        working_dir=str(output_dir),
        faucet_account=account_manager.get_faucet()
    )


@pytest.fixture(scope="function")
def test_result(request) -> TestResult:
    """
    Create TestResult for tracking test outcomes.
    
    Returns:
        TestResult instance named after the test function
    """
    return TestResult(request.node.name)


# Markers
def pytest_configure(config):
    """Configure custom pytest markers."""
    config.addinivalue_line(
        "markers", "slow: mark test as slow running"
    )
    config.addinivalue_line(
        "markers", "self_managed: mark test as managing its own nodes"
    )
    config.addinivalue_line(
        "markers", "cross_chain: mark test as requiring cross-chain setup"
    )
    config.addinivalue_line(
        "markers", "randomness: mark test as randomness-related"
    )
    config.addinivalue_line(
        "markers", "epoch: mark test as epoch consistency test"
    )
    config.addinivalue_line(
        "markers", "validator: mark test as validator management test"
    )


def pytest_collection_modifyitems(session, config, items):
    """
    Filter out test_case decorator from being collected as a test.

    The test_case decorator is used to wrap test functions but should not
    be collected as a test itself.
    """
    # Filter out any item named 'test_case' (the decorator)
    items[:] = [item for item in items if item.name != 'test_case']
