"""
Pytest configuration and fixtures for Gravity E2E tests.

This module provides shared fixtures for all pytest-based tests,
including cluster management and node connections.

Usage:
    # In test files, fixtures are automatically injected:
    
    @pytest.mark.asyncio
    async def test_something(cluster: Cluster):
        node = cluster.get_node("node1")
        balance = node.w3.eth.get_balance(address)
        # ... test code
"""

import logging
import sys
import os
from pathlib import Path
from typing import Optional

# Add the parent package to path for imports
# This allows pytest to find the gravity_e2e package
_current_dir = Path(__file__).resolve().parent
if str(_current_dir) not in sys.path:
    sys.path.insert(0, str(_current_dir))

import pytest

from gravity_e2e.cluster.manager import Cluster

LOG = logging.getLogger(__name__)


# Configuration defaults
DEFAULT_CLUSTER_CONFIG = "../cluster/cluster.toml"
DEFAULT_OUTPUT_DIR = "output"


def pytest_addoption(parser):
    """Add custom command line options for pytest."""
    parser.addoption(
        "--cluster-config",
        action="store",
        default=None,
        help="Path to cluster.toml configuration file"
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
    default_loc = Path(__file__).parent.parent.parent / "cluster" / "cluster.toml"
    if default_loc.exists():
        return default_loc
        
    return Path("cluster.toml").resolve()


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


@pytest.fixture(scope="module")
def cluster(cluster_config_path: Path) -> Cluster:
    """
    Create Cluster for the test module.
    
    Uses module scope so all tests in a file share the same cluster instance.
    The cluster lifecycle (start/stop) is managed externally by runner.py.
    """
    LOG.info(f"Loading cluster from {cluster_config_path}")
    c = Cluster(cluster_config_path)
    
    yield c
    
    # No cleanup needed - Web3 HTTPProvider doesn't hold connections


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
    """
    items[:] = [item for item in items if item.name != 'test_case']
