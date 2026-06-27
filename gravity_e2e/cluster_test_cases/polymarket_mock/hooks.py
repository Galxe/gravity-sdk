"""Hooks for the Polymarket mock oracle PoC suite."""

import json
import logging
import sys
from pathlib import Path

LOG = logging.getLogger(__name__)

_mock = None
_METADATA_FILE = "mock_polymarket_metadata.json"


def pre_start(test_dir: Path, env: dict, pytest_args: list = None):
    global _mock

    e2e_root = str(Path(__file__).resolve().parent.parent.parent)
    if e2e_root not in sys.path:
        sys.path.insert(0, e2e_root)

    from gravity_e2e.utils.mock_polymarket_polygon import (
        CTF_ADDRESS,
        DRAW_BLOCK,
        DRAW_CONDITION_ID,
        DRAW_LOG_INDEX,
        DRAW_MARKET_ID,
        DRAW_QUESTION_ID,
        DRAW_TX_HASH,
        MockPolymarketPolygon,
    )

    LOG.info("[hook] Starting MockPolymarketPolygon on port 8546")
    _mock = MockPolymarketPolygon(port=8546)
    log = _mock.preload_draw_resolution()
    _mock.start()

    metadata = {
        "port": 8546,
        "rpc_url": _mock.rpc_url,
        "ctf": CTF_ADDRESS,
        "market_id": DRAW_MARKET_ID,
        "condition_id": DRAW_CONDITION_ID,
        "question_id": DRAW_QUESTION_ID,
        "tx_hash": DRAW_TX_HASH,
        "block": DRAW_BLOCK,
        "log_index": DRAW_LOG_INDEX,
        "source_log": log,
    }
    metadata_path = test_dir / _METADATA_FILE
    metadata_path.write_text(json.dumps(metadata, indent=2))
    LOG.info("[hook] Wrote Polymarket mock metadata to %s", metadata_path)


def post_stop(test_dir: Path, env: dict):
    global _mock

    if _mock is not None:
        LOG.info("[hook] Stopping MockPolymarketPolygon")
        _mock.stop()
        _mock = None

    metadata_path = test_dir / _METADATA_FILE
    if metadata_path.exists():
        metadata_path.unlink()
