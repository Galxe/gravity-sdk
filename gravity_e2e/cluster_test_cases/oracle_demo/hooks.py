"""Hooks for the combined oracle dashboard demo suite."""

import json
import logging
import os
import signal
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

LOG = logging.getLogger(__name__)

BINANCE_SUITE = Path(__file__).resolve().parent.parent / "binance_price_feed"
E2E_ROOT = Path(__file__).resolve().parents[2]

if str(E2E_ROOT) not in sys.path:
    sys.path.insert(0, str(E2E_ROOT))

from gravity_e2e.utils.mock_polymarket_polygon import MockPolymarketPolygon
from gravity_e2e.utils.mock_binance_index import BUCKET_START_MS, INTERVAL_MS, MOCK_BINANCE_PORT

_poly_mock = None


def _pid_file(test_dir: Path) -> Path:
    return test_dir / "artifacts" / "mock_binance.pid"


def _log_file(test_dir: Path) -> Path:
    return test_dir / "artifacts" / "mock_binance.log"


def _process_alive(pid: int) -> bool:
    try:
        os.kill(pid, 0)
        return True
    except OSError:
        return False


def _read_pid(path: Path) -> int | None:
    if not path.exists():
        return None
    try:
        return int(path.read_text().strip())
    except ValueError:
        return None


def _wait_for_binance_ready(timeout: int = 10):
    start_time = BUCKET_START_MS
    end_time = start_time + INTERVAL_MS - 1
    url = (
        f"http://127.0.0.1:{MOCK_BINANCE_PORT}/fapi/v1/indexPriceKlines"
        f"?pair=NVDAUSDT&interval=1m&startTime={start_time}&endTime={end_time}&limit=1"
    )
    deadline = time.monotonic() + timeout
    last_error = None
    while time.monotonic() < deadline:
        try:
            with urllib.request.urlopen(url, timeout=2) as resp:
                json.loads(resp.read())
                return
        except Exception as exc:
            last_error = exc
            time.sleep(0.25)
    raise RuntimeError(f"mock Binance server did not become ready: {last_error}")


def _start_binance_mock(test_dir: Path, env: dict):
    pid_path = _pid_file(test_dir)
    pid = _read_pid(pid_path)
    if pid is not None and _process_alive(pid):
        env["BINANCE_PRICE_FEED_EXTERNAL_MOCK"] = "1"
        _wait_for_binance_ready()
        return

    log_path = _log_file(test_dir)
    log_path.parent.mkdir(parents=True, exist_ok=True)
    with open(log_path, "ab") as log:
        subprocess.Popen(
            [
                sys.executable,
                str(BINANCE_SUITE / "mock_binance.py"),
                "--port",
                str(MOCK_BINANCE_PORT),
                "--pid-file",
                str(pid_path),
            ],
            cwd=test_dir,
            env=env,
            stdout=log,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
    env["BINANCE_PRICE_FEED_EXTERNAL_MOCK"] = "1"
    _wait_for_binance_ready()


def _stop_binance_mock(test_dir: Path):
    pid = _read_pid(_pid_file(test_dir))
    if pid is None or not _process_alive(pid):
        return
    os.kill(pid, signal.SIGTERM)
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        if not _process_alive(pid):
            return
        time.sleep(0.25)
    os.kill(pid, signal.SIGKILL)


def pre_start(test_dir: Path, env: dict, pytest_args: list = None):
    global _poly_mock

    LOG.info("[hook] Starting combined oracle demo mocks")
    env["BINANCE_PRICE_FEED_MODE"] = "mock"
    env.pop("BINANCE_PRICE_FEED_BASE_URL", None)
    env.pop("BINANCE_PRICE_FEED_BUCKET_START_MS", None)
    _start_binance_mock(test_dir, env)

    _poly_mock = MockPolymarketPolygon(port=8546)
    _poly_mock.start()
    metadata = {
        "rpc_url": _poly_mock.rpc_url,
        "winning_slot": None,
        "payout_numerators": None,
        "source_log": None,
    }
    (test_dir / "mock_polymarket_metadata.json").write_text(json.dumps(metadata, indent=2))


def post_stop(test_dir: Path, env: dict):
    global _poly_mock

    _stop_binance_mock(test_dir)

    if _poly_mock is not None:
        LOG.info("[hook] Stopping MockPolymarketPolygon")
        _poly_mock.stop()
        _poly_mock = None

    metadata_path = test_dir / "mock_polymarket_metadata.json"
    if metadata_path.exists():
        metadata_path.unlink()
