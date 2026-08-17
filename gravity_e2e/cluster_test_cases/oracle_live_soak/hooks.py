"""Prepare live Binance inputs before deploying the soak cluster."""

import ipaddress
import json
import logging
import os
from pathlib import Path
import time
from urllib.parse import urlparse

from web3 import Web3

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

PROJECT_ROOT = Path(__file__).resolve().parents[3]
SIGNED_TESTNET_GENESIS = PROJECT_ROOT / "genesis" / "testnet" / "genesis.json"
ORACLE_V1_CONTRACTS = (
    {
        "name": "NativeOracle",
        "address": "0x00000000000000000000000000000001625f4000",
        "preForkCodeHash": "0x30dd3888ce26735c0d6c5a036b48a1de668dd5506efa7588ce450f976da28255",
        "postForkCodeHash": "0x981087ccdaa0b7843960782e99b078ccdd3820b331f86ce337d9750c5565d984",
    },
    {
        "name": "OracleTaskConfig",
        "address": "0x00000000000000000000000000000001625f1009",
        "preForkCodeHash": "0x74127baf705119810746598b2695ff5fa38f94bd778f0edae46799ffd3606bda",
        "postForkCodeHash": "0xa21bf93e6123b0104b9ea851b8154fb342a5b576c22c71f15f851e266faa9f7f",
    },
)


def _alloc_key(alloc: dict, address: str) -> str:
    normalized = address.removeprefix("0x").lower()
    for key in alloc:
        if key.removeprefix("0x").lower() == normalized:
            return key
    raise RuntimeError(f"genesis alloc is missing system contract {address}")


def _runtime_hash(code: str) -> str:
    if not code.startswith("0x") or len(code) % 2 != 0:
        raise RuntimeError("system-contract runtime is not canonical hex")
    return Web3.to_hex(Web3.keccak(hexstr=code)).lower()


def _install_oracle_v1_pre_fork_runtimes(test_dir: Path) -> dict:
    genesis_path = test_dir / "artifacts" / "genesis.json"
    if not genesis_path.exists():
        raise RuntimeError(
            "OracleV1 soak requires generated artifacts/genesis.json; use --force-init"
        )
    if not SIGNED_TESTNET_GENESIS.exists():
        raise RuntimeError(
            "signed SDK genesis/testnet/genesis.json fixture is missing"
        )

    generated = json.loads(genesis_path.read_text())
    signed_testnet = json.loads(SIGNED_TESTNET_GENESIS.read_text())
    activation_block = generated.get("config", {}).get("oracleV1Block")
    if not isinstance(activation_block, int) or activation_block <= 0:
        raise RuntimeError(
            "generated genesis must configure a positive config.oracleV1Block"
        )

    generated_alloc = generated.get("alloc", {})
    signed_alloc = signed_testnet.get("alloc", {})
    evidence = []
    for contract in ORACLE_V1_CONTRACTS:
        generated_key = _alloc_key(generated_alloc, contract["address"])
        signed_key = _alloc_key(signed_alloc, contract["address"])
        signed_code = signed_alloc[signed_key].get("code", "")
        signed_hash = _runtime_hash(signed_code)
        if signed_hash != contract["preForkCodeHash"]:
            raise RuntimeError(
                f"signed testnet {contract['name']} runtime hash {signed_hash} "
                f"does not match {contract['preForkCodeHash']}"
            )

        generated_code = generated_alloc[generated_key].get("code", "")
        generated_hash = _runtime_hash(generated_code)
        if generated_hash not in {
            contract["preForkCodeHash"],
            contract["postForkCodeHash"],
        }:
            raise RuntimeError(
                f"generated {contract['name']} runtime hash {generated_hash} "
                "matches neither frozen OracleV1 state"
            )
        generated_alloc[generated_key]["code"] = signed_code
        evidence.append(dict(contract))

    temporary_path = genesis_path.with_suffix(".json.tmp")
    temporary_path.write_text(json.dumps(generated, indent=2) + "\n")
    temporary_path.replace(genesis_path)
    return {
        "activationBlock": activation_block,
        "contracts": evidence,
        "fixture": "genesis/testnet/genesis.json",
    }


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
    oracle_v1_hardfork = _install_oracle_v1_pre_fork_runtimes(test_dir)
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
            {
                "binanceFeeds": binance_feeds,
                "oracleV1Hardfork": oracle_v1_hardfork,
            },
            indent=2,
            ensure_ascii=False,
        )
        + "\n"
    )
    env["RELAYER_CONFIG_TPL"] = str(relayer_path)
    LOG.info(
        "Prepared OracleV1 block=%s and live Binance pairs=%s anchor=%s",
        oracle_v1_hardfork["activationBlock"],
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
