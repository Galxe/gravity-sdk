"""Lifecycle hooks for the dynamic Polymarket mirror E2E."""

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

    from gravity_e2e.utils.mock_polymarket_polygon import MockPolymarketPolygon

    LOG.info("[hook] Starting dynamic Polymarket Polygon mock on port 8646")
    _mock = MockPolymarketPolygon(port=8646)
    _mock.start()

    metadata_path = test_dir / _METADATA_FILE
    metadata_path.write_text(
        json.dumps({"port": _mock.port, "rpc_url": _mock.rpc_url}, indent=2)
    )


def post_stop(test_dir: Path, env: dict):
    global _mock

    if _mock is not None:
        LOG.info("[hook] Stopping dynamic Polymarket Polygon mock")
        _mock.stop()
        _mock = None

    metadata_path = test_dir / _METADATA_FILE
    if metadata_path.exists():
        metadata_path.unlink()
