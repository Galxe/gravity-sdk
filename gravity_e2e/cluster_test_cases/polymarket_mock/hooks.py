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
        MATCH_BLOCK,
        MATCH_CONDITION_ID,
        MATCH_LOG_INDEX,
        MATCH_MARKET_ID,
        MATCH_QUESTION_ID,
        MATCH_TX_HASH,
        MockPolymarketPolygon,
    )

    LOG.info("[hook] Starting MockPolymarketPolygon on port 8546 with hidden settlement")
    _mock = MockPolymarketPolygon(port=8546)
    _mock.start()

    metadata = {
        "port": 8546,
        "rpc_url": _mock.rpc_url,
        "ctf": CTF_ADDRESS,
        "market_id": MATCH_MARKET_ID,
        "condition_id": MATCH_CONDITION_ID,
        "question_id": MATCH_QUESTION_ID,
        "tx_hash": MATCH_TX_HASH,
        "block": MATCH_BLOCK,
        "log_index": MATCH_LOG_INDEX,
        "winning_slot": None,
        "payout_numerators": None,
        "source_log": None,
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
