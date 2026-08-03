"""Prepare real Binance and Polygon sources for the live Oracle soak."""

import importlib.util
import ipaddress
import json
import logging
import os
from pathlib import Path
import time
from urllib.parse import urlparse


LOG = logging.getLogger(__name__)

INTERVAL_MS = 60_000
DEFAULT_GRACE_MS = 120_000
DEFAULT_BINANCE_BASE_URL = "https://fapi.binance.com"
NVDA_FEED_ID = 1001
NVDA_PAIR = "NVDAUSDT"

_METADATA_FILE = "oracle_live_soak_metadata.json"
_RELAYER_FILE = "relayer_config.live.json"
_HEARTBEAT_FILE = "oracle_live_soak_heartbeat.jsonl"
_SUMMARY_FILE = "oracle_live_soak_summary.json"


def _bucket_start_ms(env: dict) -> int:
    configured = env.get("BINANCE_PRICE_FEED_BUCKET_START_MS")
    if configured:
        bucket_start_ms = int(configured)
    else:
        grace_ms = int(
            env.get("BINANCE_PRICE_FEED_GRACE_MS", str(DEFAULT_GRACE_MS))
        )
        minimum_lag = (grace_ms + INTERVAL_MS - 1) // INTERVAL_MS + 1
        lag_minutes = int(
            env.get("BINANCE_PRICE_FEED_LAG_MINUTES", str(minimum_lag))
        )
        bucket_start_ms = (
            (int(time.time() * 1000) // INTERVAL_MS) - lag_minutes
        ) * INTERVAL_MS

    if bucket_start_ms <= 0 or bucket_start_ms % INTERVAL_MS != 0:
        raise RuntimeError(
            "BINANCE_PRICE_FEED_BUCKET_START_MS must be a positive aligned minute"
        )
    return bucket_start_ms


def _require_public_https_url(value: str, name: str) -> str:
    parsed = urlparse(value)
    if parsed.scheme != "https" or not parsed.hostname:
        raise RuntimeError(f"{name} must be an https URL with a hostname")
    hostname = parsed.hostname.lower()
    if hostname == "localhost" or hostname.endswith(".localhost"):
        raise RuntimeError(f"{name} must not use a loopback host")
    try:
        address = ipaddress.ip_address(hostname)
    except ValueError:
        address = None
    if address is not None and not address.is_global:
        raise RuntimeError(f"{name} must not use a private or loopback address")
    return value.rstrip("/")


def _price_uri(bucket_start_ms: int, grace_ms: int) -> str:
    return (
        f"gravity://3/{NVDA_FEED_ID}/price_feed?"
        f"provider=binance_index_kline_v1&pair={NVDA_PAIR}&interval=1m&"
        f"bucketStartMs={bucket_start_ms}&decimals=8&graceMs={grace_ms}"
    )


def _polymarket_hooks(test_dir: Path):
    hook_path = test_dir.parent / "polymarket_live_dynamic_mirror" / "hooks.py"
    spec = importlib.util.spec_from_file_location(
        "gravity_e2e_soak_polymarket_hooks", hook_path
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load live Polymarket helpers from {hook_path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def pre_deploy(test_dir: Path, env: dict, pytest_args: list[str]):
    requested_mode = env.get("BINANCE_PRICE_FEED_MODE", "live").strip().lower()
    if requested_mode != "live":
        raise RuntimeError("oracle_live_soak only supports live Binance mode")

    binance_base_url = _require_public_https_url(
        env.get("BINANCE_PRICE_FEED_BASE_URL", DEFAULT_BINANCE_BASE_URL),
        "BINANCE_PRICE_FEED_BASE_URL",
    )
    grace_ms = int(
        env.get("BINANCE_PRICE_FEED_GRACE_MS", str(DEFAULT_GRACE_MS))
    )
    if grace_ms < 0:
        raise RuntimeError("BINANCE_PRICE_FEED_GRACE_MS must be non-negative")
    bucket_start_ms = _bucket_start_ms(env)
    price_uri = _price_uri(bucket_start_ms, grace_ms)

    polygon_rpc_url = (
        env.get("POLYGON_RPC_URL")
        or env.get("POLYGON_QUICKNONE_HTTP_URL")
        or os.environ.get("POLYGON_RPC_URL")
        or os.environ.get("POLYGON_QUICKNONE_HTTP_URL")
    )
    if not polygon_rpc_url:
        raise RuntimeError("POLYGON_RPC_URL is required for oracle_live_soak")

    polymarket = _polymarket_hooks(test_dir)
    gamma_url = env.get("POLYMARKET_GAMMA_URL", polymarket.DEFAULT_GAMMA_URL)
    market = polymarket._discover_market(polygon_rpc_url, gamma_url)

    artifacts = test_dir / "artifacts"
    artifacts.mkdir(parents=True, exist_ok=True)
    for name in (_HEARTBEAT_FILE, _SUMMARY_FILE):
        (artifacts / name).unlink(missing_ok=True)
    relayer_path = artifacts / _RELAYER_FILE
    relayer_path.write_text(
        json.dumps(
            {
                "uri_mappings": {
                    price_uri: binance_base_url,
                    market["taskUri"]: polygon_rpc_url,
                }
            },
            indent=2,
        )
        + "\n"
    )

    metadata_path = artifacts / _METADATA_FILE
    metadata_path.write_text(
        json.dumps(
            {
                "binance": {
                    "baseUrl": binance_base_url,
                    "feedId": NVDA_FEED_ID,
                    "pair": NVDA_PAIR,
                    "bucketStartMs": bucket_start_ms,
                    "graceMs": grace_ms,
                    "taskUri": price_uri,
                },
                "polymarket": market,
            },
            indent=2,
            ensure_ascii=False,
        )
        + "\n"
    )

    env["BINANCE_PRICE_FEED_MODE"] = "live"
    env["BINANCE_PRICE_FEED_BASE_URL"] = binance_base_url
    env["BINANCE_PRICE_FEED_BUCKET_START_MS"] = str(bucket_start_ms)
    env["BINANCE_PRICE_FEED_GRACE_MS"] = str(grace_ms)
    env["RELAYER_CONFIG_TPL"] = str(relayer_path)
    LOG.info(
        "[hook] Prepared NVDA anchor=%s and Polymarket mirrorId=%s block=%s",
        bucket_start_ms,
        market["mirrorId"],
        market["blockNumber"],
    )


def post_stop(test_dir: Path, env: dict):
    artifacts = test_dir / "artifacts"
    for name in (_METADATA_FILE, _RELAYER_FILE):
        (artifacts / name).unlink(missing_ok=True)
