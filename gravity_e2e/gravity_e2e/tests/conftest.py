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
from pathlib import Path
from typing import AsyncGenerator, Optional

# Add the parent package to path for imports
# This allows pytest to find the gravity_e2e package
_current_dir = Path(__file__).resolve().parent
_package_root = _current_dir.parent.parent  # gravity_e2e/gravity_e2e -> gravity_e2e
if str(_package_root) not in sys.path:
    sys.path.insert(0, str(_package_root))

import pytest
import pytest_asyncio

# Use absolute imports now that path is set
from gravity_e2e.core.node_connector import NodeConnector
from gravity_e2e.core.client.gravity_client import GravityClient
from gravity_e2e.helpers.account_manager import TestAccountManager
from gravity_e2e.helpers.test_helpers import RunHelper, TestResult

LOG = logging.getLogger(__name__)


# Configuration defaults
DEFAULT_NODES_CONFIG = "configs/nodes.json"
DEFAULT_ACCOUNTS_CONFIG = "configs/test_accounts.json"
DEFAULT_OUTPUT_DIR = "output"


def pytest_addoption(parser):
    """Add custom command line options for pytest."""
    parser.addoption(
        "--nodes-config",
        action="store",
        default=DEFAULT_NODES_CONFIG,
        help="Path to nodes configuration file"
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
async def node_connector(nodes_config_path: str) -> AsyncGenerator[NodeConnector, None]:
    """
    Create and manage node connector for the test session.
    
    Yields:
        Connected NodeConnector instance
    """
    connector = None
    try:
        connector = NodeConnector(nodes_config_path)
        
        # Connect to all nodes
        LOG.info("Connecting to nodes...")
        results = await connector.connect_all()
        
        connected = [n for n, s in results.items() if s]
        failed = [n for n, s in results.items() if not s]
        
        if failed:
            LOG.warning(f"Failed to connect to nodes: {failed}")
        
        if not connected:
            pytest.skip("No nodes available for testing")
        
        LOG.info(f"Connected to nodes: {connected}")
        
        yield connector
        
    finally:
        # Cleanup
        if connector:
            await connector.close_all()


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
    node_connector: NodeConnector,
    target_node_id: Optional[str]
) -> GravityClient:
    """
    Get a Gravity client for tests.
    
    Creates a new client for each test function to avoid aiohttp session
    sharing issues across different async tasks.
    """
    # Get node info
    if target_node_id:
        node = node_connector.get_node(target_node_id)
        if not node:
            pytest.skip(f"Node {target_node_id} not found")
        rpc_url = node.rpc_url
        node_id = target_node_id
    else:
        # Get first connected node
        connected_nodes = list(node_connector.clients.keys())
        if not connected_nodes:
            pytest.skip("No nodes connected")
        node_id = connected_nodes[0]
        node = node_connector.get_node(node_id)
        rpc_url = node.rpc_url
    
    # Create a fresh client for this test function
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
