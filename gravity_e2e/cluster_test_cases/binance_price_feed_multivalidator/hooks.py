"""Prepare an explicit closed Binance bucket for the live four-validator suite."""

import json
import time
from pathlib import Path

INTERVAL_MS = 60_000
DEFAULT_GRACE_MS = 120_000
DEFAULT_BASE_URL = "https://fapi.binance.com"
FEEDS = ((1001, "NVDAUSDT"), (1002, "TSLAUSDT"))


def _bucket_start_ms(env: dict) -> int:
    configured = env.get("BINANCE_PRICE_FEED_BUCKET_START_MS")
    if configured:
        return int(configured)

    grace_ms = int(env.get("BINANCE_PRICE_FEED_GRACE_MS", str(DEFAULT_GRACE_MS)))
    minimum_lag = (grace_ms + INTERVAL_MS - 1) // INTERVAL_MS + 1
    lag_minutes = int(env.get("BINANCE_PRICE_FEED_LAG_MINUTES", str(minimum_lag)))
    now_ms = int(time.time() * 1000)
    return ((now_ms // INTERVAL_MS) - lag_minutes) * INTERVAL_MS


def _uri(feed_id: int, pair: str, bucket_start_ms: int, grace_ms: int) -> str:
    return (
        f"gravity://3/{feed_id}/price_feed?"
        f"provider=binance_index_kline_v1&pair={pair}&interval=1m&"
        f"bucketStartMs={bucket_start_ms}&decimals=8&graceMs={grace_ms}"
    )


def pre_deploy(test_dir: Path, env: dict, pytest_args: list[str]):
    requested_mode = env.get("BINANCE_PRICE_FEED_MODE", "live").strip().lower()
    if requested_mode != "live":
        raise RuntimeError("binance_price_feed_multivalidator only supports live mode")

    base_url = env.get("BINANCE_PRICE_FEED_BASE_URL", DEFAULT_BASE_URL)
    grace_ms = int(env.get("BINANCE_PRICE_FEED_GRACE_MS", str(DEFAULT_GRACE_MS)))
    bucket_start_ms = _bucket_start_ms(env)
    uris = {
        _uri(feed_id, pair, bucket_start_ms, grace_ms): base_url
        for feed_id, pair in FEEDS
    }

    config_path = test_dir / "artifacts" / "relayer_config.live.json"
    config_path.parent.mkdir(parents=True, exist_ok=True)
    config_path.write_text(json.dumps({"uri_mappings": uris}, indent=2) + "\n")

    env["BINANCE_PRICE_FEED_MODE"] = "live"
    env["BINANCE_PRICE_FEED_BASE_URL"] = base_url
    env["BINANCE_PRICE_FEED_BUCKET_START_MS"] = str(bucket_start_ms)
    env["BINANCE_PRICE_FEED_GRACE_MS"] = str(grace_ms)
    env["RELAYER_CONFIG_TPL"] = str(config_path)
