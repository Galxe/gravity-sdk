"""
Bridge test hooks — called by runner.py around the node lifecycle.

pre_start:  Start MockAnvil on port 8546 AND preload bridge events
            BEFORE gravity_node starts, so the relayer can connect immediately.
post_stop:  Shut down MockAnvil.
"""

import json
import logging
import sys
from pathlib import Path

LOG = logging.getLogger(__name__)

_mock = None
_METADATA_FILE = "mock_anvil_metadata.json"

# Defaults
_DEFAULT_BRIDGE_COUNT = 10
_DEFAULT_BRIDGE_AMOUNT = 1_000_000_000_000_000_000  # 1 ether in wei
_DEFAULT_RECIPIENT = "0x6954476eAe13Bd072D9f19406A6B9543514f765C"
_DEFAULT_SENDER = "0x9fE46736679d2D9a65F0992F2272dE9f3c7fa6e0"


def _parse_bridge_count(pytest_args: list) -> int:
    """Parse --bridge-count from pytest args."""
    for i, arg in enumerate(pytest_args):
        if arg == "--bridge-count" and i + 1 < len(pytest_args):
            return int(pytest_args[i + 1])
    return _DEFAULT_BRIDGE_COUNT


def pre_start(test_dir: Path, env: dict, pytest_args: list = None):
    """Start MockAnvil + preload events before gravity_node starts."""
    global _mock

    # Ensure gravity_e2e is importable
    e2e_root = str(Path(__file__).resolve().parent.parent.parent)
    if e2e_root not in sys.path:
        sys.path.insert(0, e2e_root)

    from gravity_e2e.utils.mock_anvil import MockAnvil, DEFAULT_PORTAL_ADDRESS

    bridge_count = _parse_bridge_count(pytest_args or [])

    LOG.info(f"[hook] Starting MockAnvil on port 8546, preloading {bridge_count} events...")
    _mock = MockAnvil(port=8546)
    _mock.start()

    nonces = _mock.preload_events(
        count=bridge_count,
        amount=_DEFAULT_BRIDGE_AMOUNT,
        recipient=_DEFAULT_RECIPIENT,
        sender_address=_DEFAULT_SENDER,
        events_per_block=1,
    )

    LOG.info(
        f"[hook] MockAnvil ready: {bridge_count} events, "
        f"finalized_block={_mock.current_block}"
    )

    # Write metadata file for conftest to read (cross-process communication)
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
    LOG.info(f"[hook] Wrote metadata to {metadata_path}")


def post_stop(test_dir: Path, env: dict):
    """Stop MockAnvil after gravity_node stops."""
    global _mock

    if _mock is not None:
        LOG.info("[hook] Stopping MockAnvil...")
        _mock.stop()
        _mock = None

    metadata_path = test_dir / _METADATA_FILE
    if metadata_path.exists():
        metadata_path.unlink()
