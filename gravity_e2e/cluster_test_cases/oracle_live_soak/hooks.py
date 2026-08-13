"""Prepare live Binance inputs before deploying the soak cluster."""

import ipaddress
import json
import logging
import os
from pathlib import Path
import time
from urllib.parse import urlparse

try:
    import tomllib
except ImportError:
    import tomli as tomllib


LOG = logging.getLogger(__name__)

INTERVAL_MS = 60_000
DEFAULT_GRACE_MS = 120_000
DEFAULT_BINANCE_BASE_URL = "https://testnet.binancefuture.com"
BINANCE_FEEDS = (
    {"feedId": 1001, "pair": "NVDAUSDT"},
    {"feedId": 1002, "pair": "BTCUSDT"},
    {"feedId": 1003, "pair": "ETHUSDT"},
)

_METADATA_FILE = "oracle_live_soak_metadata.json"
_RELAYER_FILE = "relayer_config.live.json"
_HEARTBEAT_FILE = "oracle_live_soak_heartbeat.jsonl"
_SUMMARY_FILE = "oracle_live_soak_summary.json"


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


def _bucket_start_ms(env: dict) -> int:
    configured = env.get("BINANCE_PRICE_FEED_BUCKET_START_MS")
    if configured:
        bucket_start_ms = int(configured)
    else:
        grace_ms = int(env.get("BINANCE_PRICE_FEED_GRACE_MS", DEFAULT_GRACE_MS))
        minimum_lag = (grace_ms + INTERVAL_MS - 1) // INTERVAL_MS + 1
        lag_minutes = int(
            env.get("BINANCE_PRICE_FEED_LAG_MINUTES", minimum_lag)
        )
        bucket_start_ms = (
            int(time.time() * 1000) // INTERVAL_MS - lag_minutes
        ) * INTERVAL_MS
    if bucket_start_ms <= 0 or bucket_start_ms % INTERVAL_MS != 0:
        raise RuntimeError(
            "BINANCE_PRICE_FEED_BUCKET_START_MS must be a positive aligned minute"
        )
    return bucket_start_ms


def _price_uri(
    feed_id: int, pair: str, bucket_start_ms: int, grace_ms: int
) -> str:
    return (
        f"gravity://3/{feed_id}/price_feed?"
        f"provider=binance_index_kline_v1&pair={pair}&interval=1m&"
        f"bucketStartMs={bucket_start_ms}&decimals=8&graceMs={grace_ms}"
    )


def pre_deploy(test_dir: Path, env: dict, pytest_args: list[str]):
    binance_base_url = _require_public_https_url(
        env.get("BINANCE_PRICE_FEED_BASE_URL", DEFAULT_BINANCE_BASE_URL),
        "BINANCE_PRICE_FEED_BASE_URL",
    )
    grace_ms = int(env.get("BINANCE_PRICE_FEED_GRACE_MS", DEFAULT_GRACE_MS))
    if grace_ms < 0:
        raise RuntimeError("BINANCE_PRICE_FEED_GRACE_MS must be non-negative")
    bucket_start_ms = _bucket_start_ms(env)
    binance_feeds = [
        {
            "baseUrl": binance_base_url,
            **feed,
            "bucketStartMs": bucket_start_ms,
            "graceMs": grace_ms,
            "taskUri": _price_uri(
                feed["feedId"], feed["pair"], bucket_start_ms, grace_ms
            ),
        }
        for feed in BINANCE_FEEDS
    ]
    artifacts = test_dir / "artifacts"
    artifacts.mkdir(parents=True, exist_ok=True)
    for name in (_HEARTBEAT_FILE, _SUMMARY_FILE):
        (artifacts / name).unlink(missing_ok=True)

    relayer_path = artifacts / _RELAYER_FILE
    relayer_path.write_text(
        json.dumps(
            {
                "uri_mappings": {
                    feed["taskUri"]: binance_base_url
                    for feed in binance_feeds
                }
            },
            indent=2,
        )
        + "\n"
    )
    metadata_path = artifacts / _METADATA_FILE
    metadata_path.write_text(
        json.dumps(
            {"binanceFeeds": binance_feeds},
            indent=2,
            ensure_ascii=False,
        )
        + "\n"
    )
    env["RELAYER_CONFIG_TPL"] = str(relayer_path)
    LOG.info(
        "Prepared live Oracle inputs: Binance pairs=%s anchor=%s",
        [feed["pair"] for feed in binance_feeds],
        bucket_start_ms,
    )


def post_stop(test_dir: Path, env: dict):
    artifacts = test_dir / "artifacts"
    for name in (_METADATA_FILE, _RELAYER_FILE):
        (artifacts / name).unlink(missing_ok=True)

    cluster_config = Path(
        env.get("GRAVITY_CLUSTER_CONFIG", test_dir / "cluster.toml")
    )
    try:
        with cluster_config.open("rb") as config_file:
            config = tomllib.load(config_file)
        base_dir = Path(config["cluster"]["base_dir"])
        for node in config.get("nodes", []):
            (base_dir / node["id"] / "config" / "relayer_config.json").unlink(
                missing_ok=True
            )
    except (KeyError, OSError, tomllib.TOMLDecodeError) as error:
        LOG.warning("Could not remove deployed live relayer configs: %s", error)
