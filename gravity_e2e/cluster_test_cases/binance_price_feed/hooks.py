"""Long-running helper hooks for the Binance price-feed demo suite."""

import json
import os
import signal
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

from gravity_e2e.utils.mock_binance_index import BUCKET_START_MS, INTERVAL_MS, MOCK_BINANCE_PORT

DEFAULT_LIVE_BINANCE_BASE_URL = "https://fapi.binance.com"
DEFAULT_BINANCE_GRACE_MS = 120_000
LIVE_MODE = "live"
MOCK_MODE = "mock"
NVDA_FEED_ID = 1001
TSLA_FEED_ID = 1002


def _pid_file(test_dir: Path) -> Path:
    return test_dir / "artifacts" / "mock_binance.pid"


def _log_file(test_dir: Path) -> Path:
    return test_dir / "artifacts" / "mock_binance.log"


def _mode(env: dict) -> str:
    return env.get("BINANCE_PRICE_FEED_MODE", MOCK_MODE).strip().lower()


def _live_bucket_start_ms(env: dict) -> int:
    configured = env.get("BINANCE_PRICE_FEED_BUCKET_START_MS")
    if configured:
        return int(configured)
    grace_ms = int(env.get("BINANCE_PRICE_FEED_GRACE_MS", str(DEFAULT_BINANCE_GRACE_MS)))
    minimum_lag = (grace_ms + INTERVAL_MS - 1) // INTERVAL_MS + 1
    lag_minutes = int(env.get("BINANCE_PRICE_FEED_LAG_MINUTES", str(minimum_lag)))
    now_ms = int(time.time() * 1000)
    return ((now_ms // INTERVAL_MS) - lag_minutes) * INTERVAL_MS


def _price_feed_uri(
    feed_id: int,
    pair: str,
    bucket_start_ms: int,
    *,
    max_staleness_ms: int,
    grace_ms: int,
) -> str:
    return (
        f"gravity://3/{feed_id}/price_feed?"
        f"provider=binance_index_kline_v1&pair={pair}&interval=1m&"
        f"bucketStartMs={bucket_start_ms}&continuous=true&decimals=8&aggregationMode=2&"
        f"minSourceCount=1&minTotalWeight=1&maxStaleness={max_staleness_ms}&graceMs={grace_ms}"
    )


def _write_live_relayer_config(test_dir: Path, env: dict):
    base_url = env.get("BINANCE_PRICE_FEED_BASE_URL", DEFAULT_LIVE_BINANCE_BASE_URL)
    bucket_start_ms = _live_bucket_start_ms(env)
    max_staleness_ms = int(env.get("BINANCE_PRICE_FEED_MAX_STALENESS_MS", "3600000"))
    grace_ms = int(env.get("BINANCE_PRICE_FEED_GRACE_MS", str(DEFAULT_BINANCE_GRACE_MS)))
    env["BINANCE_PRICE_FEED_BUCKET_START_MS"] = str(bucket_start_ms)
    env["BINANCE_PRICE_FEED_BASE_URL"] = base_url
    env["BINANCE_PRICE_FEED_MAX_STALENESS_MS"] = str(max_staleness_ms)
    env["BINANCE_PRICE_FEED_GRACE_MS"] = str(grace_ms)

    uris = [
        _price_feed_uri(
            NVDA_FEED_ID,
            "NVDAUSDT",
            bucket_start_ms,
            max_staleness_ms=max_staleness_ms,
            grace_ms=grace_ms,
        ),
        _price_feed_uri(
            TSLA_FEED_ID,
            "TSLAUSDT",
            bucket_start_ms,
            max_staleness_ms=max_staleness_ms,
            grace_ms=grace_ms,
        ),
    ]
    relayer_config = {"uri_mappings": {uri: base_url for uri in uris}}
    config_path = test_dir / "artifacts" / "relayer_config.live.json"
    config_path.parent.mkdir(parents=True, exist_ok=True)
    config_path.write_text(json.dumps(relayer_config, indent=2) + "\n")
    env["RELAYER_CONFIG_TPL"] = str(config_path)


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


def _wait_until_ready(timeout: int = 10):
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


def pre_deploy(test_dir: Path, env: dict, pytest_args: list[str]):
    if _mode(env) != LIVE_MODE:
        return
    _write_live_relayer_config(test_dir, env)


def pre_start(test_dir: Path, env: dict, pytest_args: list[str]):
    if _mode(env) == LIVE_MODE:
        return

    if env.get("GRAVITY_DEMO_KEEP_RUNNING") != "1":
        return

    pid_path = _pid_file(test_dir)
    pid = _read_pid(pid_path)
    if pid is not None and _process_alive(pid):
        env["BINANCE_PRICE_FEED_EXTERNAL_MOCK"] = "1"
        _wait_until_ready()
        return

    log_path = _log_file(test_dir)
    log_path.parent.mkdir(parents=True, exist_ok=True)
    with open(log_path, "ab") as log:
        subprocess.Popen(
            [
                sys.executable,
                str(test_dir / "mock_binance.py"),
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
    _wait_until_ready()


def post_stop(test_dir: Path, env: dict):
    if _mode(env) == LIVE_MODE:
        return

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
