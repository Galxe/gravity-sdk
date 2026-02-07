"""
Pytest configuration and fixtures for Bridge E2E tests.

Provides:
- anvil_bridge: Starts Anvil, deploys bridge contracts, configures relayer
- bridge_helper: web3.py-based bridge interaction utility
"""

import json
import logging
import sys
import os
from pathlib import Path
from typing import Optional

_current_dir = Path(__file__).resolve().parent
# Add gravity_e2e parent to path
_gravity_e2e_parent = _current_dir
while _gravity_e2e_parent.name != "gravity_e2e" or not (_gravity_e2e_parent / "gravity_e2e").is_dir():
    _gravity_e2e_parent = _gravity_e2e_parent.parent
    if _gravity_e2e_parent == _gravity_e2e_parent.parent:
        break
if str(_gravity_e2e_parent) not in sys.path:
    sys.path.insert(0, str(_gravity_e2e_parent))

import pytest

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils.anvil_manager import AnvilManager, BridgeContracts
from gravity_e2e.utils.bridge_utils import BridgeHelper

LOG = logging.getLogger(__name__)

# Default paths
DEFAULT_CLUSTER_CONFIG = "../cluster/cluster.toml"
DEFAULT_OUTPUT_DIR = "output"

# Gravity chain core contracts repo (cloned during init.sh)
# The runner sets GRAVITY_ARTIFACTS_DIR; the contracts repo is cloned beside it
CONTRACTS_DIR_ENV = "GRAVITY_CONTRACTS_DIR"
DEFAULT_CONTRACTS_DIR = Path.home() / "projects" / "gravity_chain_core_contracts"

# Relayer config content for Anvil bridge
ANVIL_RELAYER_CONFIG = {
    "uri_mappings": {
        "gravity://0/31337/events?contract=0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512&eventSignature=0x5646e682c7d994bf11f5a2c8addb60d03c83cda3b65025a826346589df43406e&fromBlock=0": "http://localhost:8546"
    }
}


def pytest_addoption(parser):
    """Add custom command line options for pytest."""
    parser.addoption(
        "--cluster-config",
        action="store",
        default=None,
        help="Path to cluster.toml configuration file",
    )
    parser.addoption(
        "--output-dir",
        action="store",
        default=DEFAULT_OUTPUT_DIR,
        help="Output directory for test results",
    )
    parser.addoption(
        "--node-id",
        action="store",
        default=None,
        help="Specific node ID to test against",
    )
    parser.addoption(
        "--cluster",
        action="store",
        default=None,
        help="Cluster name to test",
    )
    parser.addoption(
        "--contracts-dir",
        action="store",
        default=None,
        help="Path to gravity_chain_core_contracts directory",
    )
    parser.addoption(
        "--bridge-duration",
        action="store",
        default="600",
        help="Duration in seconds for bridge continuous test (default: 600 = 10 min)",
    )


@pytest.fixture(scope="session")
def cluster_config_path(request) -> Path:
    """Get cluster configuration path."""
    val = request.config.getoption("--cluster-config")
    if val:
        return Path(val).resolve()
    env_val = os.environ.get("GRAVITY_CLUSTER_CONFIG")
    if env_val:
        return Path(env_val).resolve()
    # Default: cluster.toml in this directory
    local = _current_dir / "cluster.toml"
    if local.exists():
        return local
    return Path("cluster.toml").resolve()


@pytest.fixture(scope="session")
def output_dir(request) -> Path:
    """Get output directory and create if needed."""
    output_path = Path(request.config.getoption("--output-dir"))
    output_path.mkdir(parents=True, exist_ok=True)
    return output_path


@pytest.fixture(scope="session")
def target_node_id(request) -> Optional[str]:
    return request.config.getoption("--node-id")


@pytest.fixture(scope="session")
def target_cluster(request) -> Optional[str]:
    return request.config.getoption("--cluster")


@pytest.fixture(scope="session")
def contracts_dir(request) -> Path:
    """Get gravity_chain_core_contracts directory."""
    val = request.config.getoption("--contracts-dir")
    if val:
        return Path(val).resolve()
    env_val = os.environ.get(CONTRACTS_DIR_ENV)
    if env_val:
        return Path(env_val).resolve()
    if DEFAULT_CONTRACTS_DIR.exists():
        return DEFAULT_CONTRACTS_DIR
    raise RuntimeError(
        f"gravity_chain_core_contracts not found. "
        f"Set --contracts-dir or {CONTRACTS_DIR_ENV} env var."
    )


@pytest.fixture(scope="session")
def bridge_duration(request) -> int:
    """Get bridge test loop duration in seconds."""
    return int(request.config.getoption("--bridge-duration"))


@pytest.fixture(scope="module")
def cluster(cluster_config_path: Path) -> Cluster:
    """Create Cluster for the test module."""
    LOG.info(f"Loading cluster from {cluster_config_path}")
    c = Cluster(cluster_config_path)
    yield c


@pytest.fixture(scope="module")
def anvil_bridge(cluster: Cluster, contracts_dir: Path) -> BridgeContracts:
    """
    Start Anvil, deploy bridge contracts, and update relayer_config for each node.

    Lifecycle:
    1. Start Anvil on port 8546
    2. Deploy MockGToken, GravityPortal, GBridgeSender via forge
    3. Write relayer_config.json to each node's config dir
    4. Yield BridgeContracts
    5. Teardown: stop Anvil
    """
    mgr = AnvilManager()
    mgr.start(port=8546, block_time=1)

    try:
        contracts = mgr.deploy_bridge_contracts(contracts_dir)

        # Update relayer_config.json for each node
        for node_id, node in cluster.nodes.items():
            relayer_path = node.infra_path / "config" / "relayer_config.json"
            if relayer_path.parent.exists():
                LOG.info(f"Writing relayer_config.json for {node_id} at {relayer_path}")
                with open(relayer_path, "w") as f:
                    json.dump(ANVIL_RELAYER_CONFIG, f, indent=2)
            else:
                LOG.warning(
                    f"Config dir not found for {node_id}: {relayer_path.parent}. "
                    f"Relayer config not written."
                )

        yield contracts
    finally:
        mgr.stop()


@pytest.fixture(scope="module")
def bridge_helper(anvil_bridge: BridgeContracts) -> BridgeHelper:
    """Create BridgeHelper for web3.py bridge interactions."""
    return BridgeHelper(
        anvil_rpc_url=anvil_bridge.rpc_url,
        gtoken_address=anvil_bridge.gtoken_address,
        portal_address=anvil_bridge.portal_address,
        sender_address=anvil_bridge.sender_address,
        deployer_private_key=anvil_bridge.deployer_private_key,
        deployer_address=anvil_bridge.deployer_address,
    )


# Markers
def pytest_configure(config):
    """Configure custom pytest markers."""
    config.addinivalue_line("markers", "slow: mark test as slow running")
    config.addinivalue_line("markers", "cross_chain: mark test as requiring cross-chain setup")
    config.addinivalue_line("markers", "bridge: mark test as bridge-related")


def pytest_collection_modifyitems(session, config, items):
    """Filter out test_case decorator from being collected as a test."""
    items[:] = [item for item in items if item.name != "test_case"]
