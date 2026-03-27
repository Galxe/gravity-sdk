"""
Pre-start / post-stop hooks for hardfork + bridge test suite.

Combines two responsibilities:
1. Inject gravityHardforks (gammaBlock, deltaBlock) into genesis.json (both copies)
2. Start MockAnvil on port 8546 with pre-loaded bridge events

The relayer_config.json is also written so the node's relayer connects
to the local MockAnvil instead of a real external chain.
"""

import json
import logging
import os
import sys
from pathlib import Path

LOG = logging.getLogger(__name__)

# Ensure gravity_e2e is importable
_E2E_ROOT = str(Path(__file__).resolve().parent.parent.parent)
if _E2E_ROOT not in sys.path:
    sys.path.insert(0, _E2E_ROOT)

_mock = None
_METADATA_FILE = "mock_anvil_metadata.json"

# Bridge defaults — preload BOTH pre- and post-hardfork batches
_DEFAULT_BRIDGE_COUNT = 20  # 10 pre-hardfork + 10 post-hardfork
_DEFAULT_BRIDGE_AMOUNT = 1_000_000_000_000_000_000  # 1 ether in wei
_DEFAULT_RECIPIENT = "0x6954476eAe13Bd072D9f19406A6B9543514f765C"
_DEFAULT_SENDER = "0x9fE46736679d2D9a65F0992F2272dE9f3c7fa6e0"

# Relayer config for Anvil bridge
ANVIL_RELAYER_CONFIG = {
    "uri_mappings": {
        "gravity://0/31337/events?contract=0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512&eventSignature=0x5646e682c7d994bf11f5a2c8addb60d03c83cda3b65025a826346589df43406e&fromBlock=0": "http://localhost:8547"
    }
}


# ── Genesis injection ────────────────────────────────────────────────

def _inject_hardfork_blocks(genesis_path: Path, gamma_block: int, delta_block: int):
    """Inject hardfork block numbers into a single genesis.json file.

    Note: reth's spec.rs reads these from config.extra_fields (serde flatten),
    so they MUST be top-level keys in config, NOT nested under gravityHardforks.
    """
    if not genesis_path.exists():
        LOG.warning(f"genesis.json not found at {genesis_path}, skipping")
        return False

    with open(genesis_path) as f:
        genesis = json.load(f)

    if "config" not in genesis:
        genesis["config"] = {}

    # Inject at config top-level (reth reads extra_fields, not gravityHardforks)
    genesis["config"]["alphaBlock"] = 0
    genesis["config"]["betaBlock"] = 0
    genesis["config"]["gammaBlock"] = gamma_block
    genesis["config"]["deltaBlock"] = delta_block

    with open(genesis_path, "w") as f:
        json.dump(genesis, f, indent=2)

    LOG.info(f"  ✅ Patched gammaBlock={gamma_block}, deltaBlock={delta_block} in {genesis_path}")
    return True


def _get_cluster_base_dir(test_dir: Path) -> str:
    """Read cluster base_dir from cluster.toml."""
    cluster_config = test_dir / "cluster.toml"
    if not cluster_config.exists():
        return ""
    try:
        import tomli
    except ImportError:
        try:
            import tomllib as tomli
        except ImportError:
            import toml as tomli

    with open(cluster_config, "rb") as f:
        config = tomli.load(f)
    return config.get("cluster", {}).get("base_dir", "")


def _write_relayer_config(base_dir: str):
    """Write relayer_config.json to each node's config dir in the cluster."""
    base = Path(base_dir)
    if not base.exists():
        LOG.warning(f"Cluster base_dir {base_dir} doesn't exist, skipping relayer config")
        return

    for node_dir in base.iterdir():
        if not node_dir.is_dir():
            continue
        config_dir = node_dir / "config"
        if config_dir.exists():
            relayer_path = config_dir / "relayer_config.json"
            with open(relayer_path, "w") as f:
                json.dump(ANVIL_RELAYER_CONFIG, f, indent=2)
            LOG.info(f"  ✅ Wrote relayer_config.json to {relayer_path}")

            # Clean stale relayer state so relayer does a cold start
            data_dir = node_dir / "data" / "reth"
            relayer_state = data_dir / "relayer_state.json"
            if relayer_state.exists():
                relayer_state.unlink()
                LOG.info(f"  🧹 Removed stale {relayer_state}")


# ── Hooks ────────────────────────────────────────────────────────────

def pre_start(test_dir, env, pytest_args=None):
    """
    Called by runner.py after deploy but before node start.

    1. Start MockAnvil + preload batch 1 bridge events
    2. Inject gammaBlock into both genesis.json copies
    3. Write relayer_config.json to each node
    """
    global _mock
    test_dir = Path(test_dir)

    from gravity_e2e.utils.mock_anvil import MockAnvil, DEFAULT_PORTAL_ADDRESS

    # ── 1. Start MockAnvil ──
    bridge_count = int(os.environ.get("BRIDGE_COUNT", str(_DEFAULT_BRIDGE_COUNT)))
    LOG.info(f"🌉 Starting MockAnvil on port 8547, preloading {bridge_count} events...")
    _mock = MockAnvil(port=8547)
    _mock.start()

    nonces = _mock.preload_events(
        count=bridge_count,
        amount=_DEFAULT_BRIDGE_AMOUNT,
        recipient=_DEFAULT_RECIPIENT,
        sender_address=_DEFAULT_SENDER,
        events_per_block=1,
    )

    LOG.info(
        f"  MockAnvil ready: {bridge_count} events, "
        f"finalized_block={_mock.current_block}"
    )

    # Write metadata for conftest/test to read
    metadata = {
        "port": 8546,
        "rpc_url": _mock.rpc_url,
        "bridge_count": bridge_count,
        "amount": _DEFAULT_BRIDGE_AMOUNT,
        "recipient": _DEFAULT_RECIPIENT,
        "sender_address": _DEFAULT_SENDER,
        "portal_address": DEFAULT_PORTAL_ADDRESS,
        "nonces": nonces,
        "finalized_block": _mock.current_block,
    }
    metadata_path = test_dir / _METADATA_FILE
    metadata_path.write_text(json.dumps(metadata, indent=2))
    LOG.info(f"  Wrote metadata to {metadata_path}")

    # ── 2. Inject gammaBlock + deltaBlock ──
    gamma_block = int(os.environ.get("GAMMA_BLOCK", "0"))
    delta_block = int(os.environ.get("DELTA_BLOCK", "50"))
    LOG.info(f"🔧 Injecting gravityHardforks (gammaBlock={gamma_block}, deltaBlock={delta_block})")

    artifacts_dir = env.get("GRAVITY_ARTIFACTS_DIR", str(test_dir / "artifacts"))
    _inject_hardfork_blocks(Path(artifacts_dir) / "genesis.json", gamma_block, delta_block)

    base_dir = _get_cluster_base_dir(test_dir)
    if base_dir:
        _inject_hardfork_blocks(Path(base_dir) / "genesis.json", gamma_block, delta_block)

    os.environ["GAMMA_BLOCK"] = str(gamma_block)
    os.environ["DELTA_BLOCK"] = str(delta_block)

    # ── 3. Write relayer_config ──
    if base_dir:
        LOG.info("📡 Writing relayer_config.json for MockAnvil...")
        _write_relayer_config(base_dir)

    LOG.info("✅ Pre-start hook complete")


def post_stop(test_dir, env):
    """Stop MockAnvil after cluster stops."""
    global _mock

    if _mock is not None:
        LOG.info("🌉 Stopping MockAnvil...")
        _mock.stop()
        _mock = None

    metadata_path = Path(test_dir) / _METADATA_FILE
    if metadata_path.exists():
        metadata_path.unlink()
