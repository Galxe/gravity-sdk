"""
Pytest configuration and fixtures for Bridge E2E tests.

Pre-Load & Verify pattern:
  1. Stop nodes (runner already started them)
  2. Start Anvil + deploy bridge contracts
  3. Pre-load N bridge transactions on Anvil (gravity_node NOT running)
  4. Write relayer_config + restart gravity_node
  5. Test verifies all N NativeMinted events on gravity chain
"""

import glob
import json
import logging
import subprocess
import sys
import os
import time
from pathlib import Path
from typing import List

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
from web3 import Web3

from gravity_e2e.utils.anvil_manager import AnvilManager, BridgeContracts
from gravity_e2e.utils.bridge_utils import BridgeHelper

LOG = logging.getLogger(__name__)

# Gravity chain core contracts repo
CONTRACTS_DIR_ENV = "GRAVITY_CONTRACTS_DIR"
DEFAULT_CONTRACTS_DIR = Path.home() / "projects" / "gravity_chain_core_contracts"
# In Docker/CI, init.sh clones contracts to external/
_sdk_root = _gravity_e2e_parent.parent  # gravity-sdk/
EXTERNAL_CONTRACTS_DIR = _sdk_root / "external" / "gravity_chain_core_contracts"

# Relayer config content for Anvil bridge
ANVIL_RELAYER_CONFIG = {
    "uri_mappings": {
        "gravity://0/31337/events?contract=0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512&eventSignature=0x5646e682c7d994bf11f5a2c8addb60d03c83cda3b65025a826346589df43406e&fromBlock=0": "http://localhost:8546"
    }
}

# Bridge amount per transaction: 1000 G tokens (in wei)
BRIDGE_AMOUNT = 1000 * 10**18


def pytest_addoption(parser):
    """Add bridge-specific command line options."""
    parser.addoption(
        "--contracts-dir",
        action="store",
        default=None,
        help="Path to gravity_chain_core_contracts directory",
    )
    parser.addoption(
        "--bridge-count",
        action="store",
        default="200",
        help="Number of bridge transactions to pre-load (default: 200)",
    )
    parser.addoption(
        "--bridge-verify-timeout",
        action="store",
        default="300",
        help="Timeout in seconds for verifying all NativeMinted events (default: 300)",
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
    if EXTERNAL_CONTRACTS_DIR.exists():
        return EXTERNAL_CONTRACTS_DIR
    raise RuntimeError(
        f"gravity_chain_core_contracts not found. "
        f"Set --contracts-dir or {CONTRACTS_DIR_ENV} env var."
    )


@pytest.fixture(scope="session")
def bridge_count(request) -> int:
    """Number of bridge transactions to pre-load."""
    return int(request.config.getoption("--bridge-count"))


@pytest.fixture(scope="session")
def bridge_verify_timeout(request) -> int:
    """Timeout for verifying all NativeMinted events."""
    return int(request.config.getoption("--bridge-verify-timeout"))


@pytest.fixture(scope="module")
def preloaded_bridge(cluster, contracts_dir: Path, bridge_count: int):
    """
    Full bridge lifecycle fixture with pre-loading.

    Lifecycle:
    1. Stop gravity_node (runner already started it)
    2. Start Anvil + deploy bridge contracts
    3. Pre-load N bridge transactions on Anvil
    4. Write relayer_config + restart gravity_node
    5. Yield (contracts, nonces, bridge_helper)
    6. Teardown: stop Anvil
    """
    mgr = AnvilManager()
    sdk_root = _gravity_e2e_parent.parent
    cluster_scripts_dir = sdk_root / "cluster"
    stop_script = cluster_scripts_dir / "stop.sh"
    start_script = cluster_scripts_dir / "start.sh"
    config_path_str = str(cluster.config_path)

    env = os.environ.copy()
    artifacts_dir = _current_dir / "artifacts"
    env["GRAVITY_ARTIFACTS_DIR"] = str(artifacts_dir)

    try:
        # Phase 1: Stop nodes
        LOG.info("Phase 1: Stopping gravity nodes...")
        subprocess.run(
            ["bash", str(stop_script), "--config", config_path_str],
            cwd=str(cluster_scripts_dir),
            env=env,
            check=True,
        )
        time.sleep(2)

        # Phase 2: Start Anvil with block_time=1 and deploy contracts
        LOG.info("Phase 2: Starting Anvil and deploying contracts...")
        mgr.start(port=8546, block_time=1)
        contracts = mgr.deploy_bridge_contracts(contracts_dir)

        # Create bridge helper
        helper = BridgeHelper(
            anvil_rpc_url=contracts.rpc_url,
            gtoken_address=contracts.gtoken_address,
            portal_address=contracts.portal_address,
            sender_address=contracts.sender_address,
            deployer_private_key=contracts.deployer_private_key,
            deployer_address=contracts.deployer_address,
        )

        # Phase 3: Pre-load bridge transactions (fire-and-forget, Anvil mines at own pace)
        recipient = Web3.to_checksum_address(contracts.deployer_address)
        LOG.info(
            f"Phase 3: Pre-loading {bridge_count} bridge transactions "
            f"(amount={BRIDGE_AMOUNT} wei each)..."
        )
        nonces = helper.batch_mint_and_bridge(
            count=bridge_count,
            amount=BRIDGE_AMOUNT,
            recipient=recipient,
        )

        # Verify all MessageSent events exist on Anvil
        events = helper.query_message_sent_events(from_block=0)
        LOG.info(f"  Anvil MessageSent events: {len(events)} (expected {bridge_count})")
        assert len(events) >= bridge_count, (
            f"Expected {bridge_count} MessageSent events on Anvil, got {len(events)}"
        )

        # Phase 4: Clean stale relayer state + write config + restart gravity_node
        LOG.info("Phase 4: Cleaning relayer state, writing config, restarting nodes...")
        for node_id, node in cluster.nodes.items():
            # Delete stale relayer_state.json so the relayer does a cold start
            # against the fresh Anvil instance
            relayer_state = node._infra_path / "data" / "reth" / "relayer_state.json"
            if relayer_state.exists():
                LOG.info(f"  Removing stale relayer_state.json for {node_id}")
                relayer_state.unlink()

            relayer_path = node._infra_path / "config" / "relayer_config.json"
            if relayer_path.parent.exists():
                LOG.info(f"  Writing relayer_config.json for {node_id}")
                with open(relayer_path, "w") as f:
                    json.dump(ANVIL_RELAYER_CONFIG, f, indent=2)

            # --- Diagnostic: verify config was written correctly ---
            _dump_relayer_diagnostics(node_id, node._infra_path)

        subprocess.run(
            ["bash", str(start_script), "--config", config_path_str],
            cwd=str(cluster_scripts_dir),
            env=env,
            check=True,
        )
        time.sleep(15)  # warmup — node needs time to init relayer

        # --- Diagnostic: dump early node logs for relayer init ---
        for node_id, node in cluster.nodes.items():
            _dump_node_logs(node_id, node._infra_path, tail_lines=30, label="post-warmup")

        yield {
            "contracts": contracts,
            "nonces": nonces,
            "helper": helper,
            "bridge_count": bridge_count,
            "amount": BRIDGE_AMOUNT,
            "recipient": recipient,
        }

    finally:
        mgr.stop()



# ============================================================================
# Diagnostic helpers
# ============================================================================

def _dump_relayer_diagnostics(node_id: str, infra_path: Path) -> None:
    """Verify relayer and reth config are correct after writing."""
    # 1. Read back relayer_config.json
    relayer_path = infra_path / "config" / "relayer_config.json"
    if relayer_path.exists():
        with open(relayer_path) as f:
            content = json.load(f)
        LOG.info(f"  [{node_id}] relayer_config.json verified: {json.dumps(content)}")
    else:
        LOG.warning(f"  [{node_id}] relayer_config.json NOT FOUND at {relayer_path}")

    # 2. Read reth_config.json to verify relayer_config path reference
    reth_config_path = infra_path / "config" / "reth_config.json"
    if reth_config_path.exists():
        with open(reth_config_path) as f:
            reth_cfg = json.load(f)
        rc_ref = reth_cfg.get("reth_args", {}).get("relayer_config", "NOT SET")
        LOG.info(f"  [{node_id}] reth_config.json gravity.relayer-config = {rc_ref}")
        # Check that the referenced path matches our written file
        if rc_ref != str(relayer_path):
            LOG.warning(
                f"  [{node_id}] MISMATCH: reth_config points to {rc_ref} "
                f"but we wrote to {relayer_path}"
            )
    else:
        LOG.warning(f"  [{node_id}] reth_config.json NOT FOUND at {reth_config_path}")

    # 3. Check that relayer_state.json does NOT exist (clean slate)
    state_path = infra_path / "data" / "reth" / "relayer_state.json"
    if state_path.exists():
        with open(state_path) as f:
            state = json.load(f)
        LOG.warning(
            f"  [{node_id}] relayer_state.json STILL EXISTS after cleanup: "
            f"{json.dumps(state)}"
        )
    else:
        LOG.info(f"  [{node_id}] relayer_state.json absent (clean slate) ✓")

    # 4. Verify Anvil is reachable
    try:
        from web3 import Web3
        w3_check = Web3(Web3.HTTPProvider("http://localhost:8546", request_kwargs={"timeout": 5}))
        if w3_check.is_connected():
            bn = w3_check.eth.block_number
            LOG.info(f"  [{node_id}] Anvil connectivity check OK (block={bn})")
        else:
            LOG.warning(f"  [{node_id}] Anvil connectivity FAILED (not connected)")
    except Exception as e:
        LOG.warning(f"  [{node_id}] Anvil connectivity FAILED: {e}")


def _dump_node_logs(node_id: str, infra_path: Path, tail_lines: int = 30, label: str = "") -> None:
    """Dump tail of debug.log and reth.log for diagnostics."""
    prefix = f"  [{node_id}] [{label}]" if label else f"  [{node_id}]"

    # debug.log (stdout/stderr of gravity_node)
    debug_log = infra_path / "logs" / "debug.log"
    if debug_log.exists():
        lines = debug_log.read_text().splitlines()
        tail = lines[-tail_lines:] if len(lines) > tail_lines else lines
        LOG.info(f"{prefix} debug.log (last {len(tail)} lines):")
        for line in tail:
            LOG.info(f"    {line}")
    else:
        LOG.warning(f"{prefix} debug.log NOT FOUND at {debug_log}")

    # reth.log (execution layer file logging)
    exec_log_dir = infra_path / "execution_logs"
    if exec_log_dir.exists():
        # Find all reth.log files in subdirectories
        reth_logs = sorted(exec_log_dir.glob("*/reth.log"), key=lambda p: p.stat().st_mtime, reverse=True)
        if reth_logs:
            reth_log = reth_logs[0]  # most recent
            lines = reth_log.read_text().splitlines()
            # Filter for relayer-related lines
            relayer_lines = [l for l in lines if any(kw in l.lower() for kw in
                            ["relayer", "data source", "blockchain_source", "event",
                             "oracle", "8546", "connection", "error", "warn"])]
            if relayer_lines:
                tail = relayer_lines[-tail_lines:]
                LOG.info(f"{prefix} reth.log relayer lines (last {len(tail)}):")
                for line in tail:
                    LOG.info(f"    {line}")
            else:
                LOG.info(f"{prefix} reth.log: no relayer-related lines found ({len(lines)} total lines)")
        else:
            LOG.info(f"{prefix} execution_logs: no reth.log files found")
    else:
        LOG.warning(f"{prefix} execution_logs dir NOT FOUND at {exec_log_dir}")


# Markers
def pytest_configure(config):
    """Configure bridge-specific markers."""
    config.addinivalue_line("markers", "bridge: mark test as bridge-related")
