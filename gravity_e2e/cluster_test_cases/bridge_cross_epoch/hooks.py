"""
Bridge cross-epoch test hooks — called by runner.py around the node lifecycle.

pre_start:  Start MockAnvil on port 8546 AND preload ALL bridge events with
            finalized_block=0 (events hidden from the relayer until the test
            releases each batch). The test releases the first batch in one
            epoch, waits for an epoch transition (driven by the short
            epoch_interval_micros in genesis.toml), and then releases the
            second batch in the new epoch.
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


def _parse_int_arg(pytest_args, name, default):
    args = pytest_args or []
    for i, arg in enumerate(args):
        if arg == name and i + 1 < len(args):
            return int(args[i + 1])
    return default


def _parse_float_arg(pytest_args, name, default):
    args = pytest_args or []
    for i, arg in enumerate(args):
        if arg == name and i + 1 < len(args):
            return float(args[i + 1])
    return default


def pre_start(test_dir: Path, env: dict, pytest_args: list = None):
    """Start MockAnvil + preload all events but hide them until the test releases each batch."""
    global _mock

    e2e_root = str(Path(__file__).resolve().parent.parent.parent)
    if e2e_root not in sys.path:
        sys.path.insert(0, e2e_root)

    from gravity_e2e.utils.mock_anvil import MockAnvil, DEFAULT_PORTAL_ADDRESS

    bridge_count = _parse_int_arg(pytest_args, "--bridge-count", _DEFAULT_BRIDGE_COUNT)
    first_fraction = _parse_float_arg(pytest_args, "--first-batch-fraction", 0.5)

    LOG.info(
        f"[hook] Starting MockAnvil on port 8546, preloading {bridge_count} events "
        f"(cross-epoch mode, first_fraction={first_fraction})..."
    )
    _mock = MockAnvil(port=8546)
    _mock.start()

    nonces = _mock.preload_events(
        count=bridge_count,
        amount=_DEFAULT_BRIDGE_AMOUNT,
        recipient=_DEFAULT_RECIPIENT,
        sender_address=_DEFAULT_SENDER,
        events_per_block=1,
    )

    first_batch_count = max(1, min(bridge_count - 1, int(round(bridge_count * first_fraction))))
    _mock.set_finalized(0)

    LOG.info(
        f"[hook] Cross-epoch mode: total {bridge_count} events; first batch "
        f"= {first_batch_count} events (release in epoch N), second batch "
        f"= {bridge_count - first_batch_count} events (release in epoch N+1)."
    )

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
        "first_batch_count": first_batch_count,
        "second_batch_block": bridge_count,
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
