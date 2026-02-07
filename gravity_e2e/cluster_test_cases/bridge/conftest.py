"""
Pytest configuration and fixtures for Bridge E2E tests.

Provides bridge-specific fixtures only. Shared fixtures (cluster, cluster_config_path,
output_dir, target_node_id, target_cluster) are inherited from the parent conftest.py.

IMPORTANT lifecycle note:
  The runner (runner.py) starts gravity_node BEFORE pytest runs. But the
  relayer reads relayer_config.json ONCE at startup. So we must:
    1. Stop nodes (already started by runner with stale relayer_config)
    2. Start Anvil + deploy bridge contracts
    3. Write relayer_config.json with Anvil URI mapping
    4. Restart nodes (so relayer picks up the new config)
"""

import json
import logging
import subprocess
import sys
import os
import time
from pathlib import Path

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

from gravity_e2e.utils.anvil_manager import AnvilManager, BridgeContracts
from gravity_e2e.utils.bridge_utils import BridgeHelper

LOG = logging.getLogger(__name__)

# Gravity chain core contracts repo
CONTRACTS_DIR_ENV = "GRAVITY_CONTRACTS_DIR"
DEFAULT_CONTRACTS_DIR = Path.home() / "projects" / "gravity_chain_core_contracts"

# Relayer config content for Anvil bridge
ANVIL_RELAYER_CONFIG = {
    "uri_mappings": {
        "gravity://0/31337/events?contract=0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512&eventSignature=0x5646e682c7d994bf11f5a2c8addb60d03c83cda3b65025a826346589df43406e&fromBlock=0": "http://localhost:8546"
    }
}


def pytest_addoption(parser):
    """Add bridge-specific command line options (shared options come from parent conftest)."""
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
def anvil_bridge(cluster, contracts_dir: Path) -> BridgeContracts:
    """
    Start Anvil, deploy bridge contracts, and update relayer_config for each node.

    Lifecycle (handles the relayer_config timing constraint):
    1. Stop gravity_node (runner already started it with stale relayer_config)
    2. Start Anvil on port 8546
    3. Deploy MockGToken, GravityPortal, GBridgeSender via forge
    4. Write relayer_config.json with Anvil URI mapping to each node's config dir
    5. Restart gravity_node (relayer now reads the correct config)
    6. Yield BridgeContracts
    7. Teardown: stop Anvil
    """
    mgr = AnvilManager()

    # Resolve the cluster scripts directory (gravity-sdk/cluster/)
    sdk_root = _gravity_e2e_parent.parent  # gravity-sdk/
    cluster_scripts_dir = sdk_root / "cluster"
    stop_script = cluster_scripts_dir / "stop.sh"
    start_script = cluster_scripts_dir / "start.sh"
    config_path_str = str(cluster.config_path)

    env = os.environ.copy()
    artifacts_dir = _current_dir / "artifacts"
    env["GRAVITY_ARTIFACTS_DIR"] = str(artifacts_dir)

    try:
        # Step 1: Stop nodes so we can update config before restart
        # (runner.py already started them with stale/template relayer_config)
        LOG.info("Stopping gravity nodes to update relayer_config...")
        subprocess.run(
            ["bash", str(stop_script), "--config", config_path_str],
            cwd=str(cluster_scripts_dir),
            env=env,
            check=True,
        )
        time.sleep(2)

        # Step 2-3: Start Anvil and deploy contracts
        mgr.start(port=8546, block_time=1)
        contracts = mgr.deploy_bridge_contracts(contracts_dir)

        # Step 4: Write relayer_config.json for each node
        for node_id, node in cluster.nodes.items():
            relayer_path = node._infra_path / "config" / "relayer_config.json"
            if relayer_path.parent.exists():
                LOG.info(f"Writing relayer_config.json for {node_id} at {relayer_path}")
                with open(relayer_path, "w") as f:
                    json.dump(ANVIL_RELAYER_CONFIG, f, indent=2)
            else:
                LOG.warning(
                    f"Config dir not found for {node_id}: {relayer_path.parent}. "
                    f"Relayer config not written."
                )

        # Step 5: Restart nodes with updated config
        LOG.info("Restarting gravity nodes with updated relayer_config...")
        subprocess.run(
            ["bash", str(start_script), "--config", config_path_str],
            cwd=str(cluster_scripts_dir),
            env=env,
            check=True,
        )
        time.sleep(5)  # warmup

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
    """Configure bridge-specific markers."""
    config.addinivalue_line("markers", "bridge: mark test as bridge-related")
