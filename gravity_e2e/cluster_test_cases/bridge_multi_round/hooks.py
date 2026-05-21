"""
Bridge multi-round oracle test hooks — called by runner.py around the node lifecycle.

pre_start:  Start MockAnvil on port 8546 AND preload ALL bridge events with the
            finalized block held back to 0, so the relayer sees nothing at
            startup. The test body releases events round-by-round via the
            mock_setFinalized JSON-RPC, after the early genesis-driven epoch
            reconfig settles, so every release lands in the same epoch.
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
_DEFAULT_ROUNDS = 5
_DEFAULT_BRIDGE_AMOUNT = 1_000_000_000_000_000_000  # 1 ether in wei
_DEFAULT_RECIPIENT = "0x6954476eAe13Bd072D9f19406A6B9543514f765C"
_DEFAULT_SENDER = "0x9fE46736679d2D9a65F0992F2272dE9f3c7fa6e0"


def _parse_int_arg(pytest_args, name, default):
    args = pytest_args or []
    for i, arg in enumerate(args):
        if arg == name and i + 1 < len(args):
            return int(args[i + 1])
    return default


def _split_rounds(total: int, rounds: int):
    """Split `total` events into `rounds` ascending block boundaries.

    Returns a list of `rounds` cumulative block numbers (== highest nonce in
    that round, since events_per_block=1). Earlier rounds get the extra event
    when total doesn't divide evenly.
    """
    rounds = max(1, min(rounds, total))
    base, rem = divmod(total, rounds)
    boundaries = []
    cum = 0
    for r in range(rounds):
        size = base + (1 if r < rem else 0)
        cum += size
        boundaries.append(cum)
    return boundaries


def pre_start(test_dir: Path, env: dict, pytest_args: list = None):
    """Start MockAnvil + preload all events but hide them until the test releases each round."""
    global _mock

    e2e_root = str(Path(__file__).resolve().parent.parent.parent)
    if e2e_root not in sys.path:
        sys.path.insert(0, e2e_root)

    from gravity_e2e.utils.mock_anvil import MockAnvil, DEFAULT_PORTAL_ADDRESS

    bridge_count = _parse_int_arg(pytest_args, "--bridge-count", _DEFAULT_BRIDGE_COUNT)
    rounds = _parse_int_arg(pytest_args, "--rounds", _DEFAULT_ROUNDS)
    if rounds > bridge_count:
        LOG.warning(f"[hook] rounds={rounds} > bridge_count={bridge_count}; clamping rounds={bridge_count}")
        rounds = bridge_count

    LOG.info(
        f"[hook] Starting MockAnvil on port 8546, preloading {bridge_count} events "
        f"across {rounds} round(s)..."
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

    # Hide everything at startup so the first round lands AFTER the early
    # genesis-driven epoch reconfig (epoch 1 -> 2) is complete.
    _mock.set_finalized(0)

    round_boundaries = _split_rounds(bridge_count, rounds)
    LOG.info(
        f"[hook] Multi-round mode: {bridge_count} events / {rounds} round(s); "
        f"per-round cumulative nonce boundaries = {round_boundaries}; "
        f"finalized_block={_mock.current_block}"
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
        "rounds": rounds,
        "round_boundaries": round_boundaries,
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
