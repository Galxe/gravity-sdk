"""Prepare live Binance and Polygon inputs before deploying the soak cluster."""

import ipaddress
import json
import logging
import os
from pathlib import Path
import time
from urllib.parse import urlencode, urlparse
from urllib.request import Request, urlopen

try:
    import tomllib
except ImportError:
    import tomli as tomllib


LOG = logging.getLogger(__name__)

INTERVAL_MS = 60_000
DEFAULT_GRACE_MS = 120_000
DEFAULT_BINANCE_BASE_URL = "https://testnet.binancefuture.com"
DEFAULT_GAMMA_URL = "https://gamma-api.polymarket.com"
BINANCE_FEEDS = (
    {"feedId": 1001, "pair": "NVDAUSDT"},
    {"feedId": 1002, "pair": "BTCUSDT"},
    {"feedId": 1003, "pair": "ETHUSDT"},
)

POLYGON_CHAIN_ID = 137
CTF_ADDRESS = "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045"
CONDITION_RESOLUTION_TOPIC0 = (
    "0xb44d84d3289691f71497564b85d4233648d9dbae8cbdbb4329f301c3a0185894"
)
SCAN_CHUNK = 2_000
MAX_SCAN_BLOCKS = 100_000

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


def _read_json(request: Request) -> object:
    with urlopen(request, timeout=30) as response:
        return json.loads(response.read())


def _rpc(rpc_url: str, method: str, params: list):
    payload = json.dumps(
        {"jsonrpc": "2.0", "id": 1, "method": method, "params": params}
    ).encode()
    body = _read_json(
        Request(
            rpc_url,
            data=payload,
            headers={"Content-Type": "application/json"},
        )
    )
    if not isinstance(body, dict) or "result" not in body:
        detail = body.get("error") if isinstance(body, dict) else body
        raise RuntimeError(f"Polygon {method} failed: {detail}")
    return body["result"]


def _gamma_markets(base_url: str) -> list[dict]:
    query = urlencode(
        {
            "closed": "true",
            "limit": 100,
            "order": "closedTime",
            "ascending": "false",
        }
    )
    markets = _read_json(
        Request(
            f"{base_url}/markets?{query}",
            headers={
                "Accept": "application/json",
                "User-Agent": "gravity-oracle-e2e/1.0",
            },
        )
    )
    if not isinstance(markets, list):
        raise RuntimeError("Polymarket Gamma response is not a market list")
    return markets


def _parse_list(value) -> list:
    if isinstance(value, str):
        value = json.loads(value)
    return list(value or [])


def _binary_markets(base_url: str) -> dict[str, dict]:
    result = {}
    for market in _gamma_markets(base_url):
        condition_id = market.get("conditionId")
        outcomes = _parse_list(market.get("outcomes"))
        if (
            isinstance(condition_id, str)
            and len(condition_id) == 66
            and condition_id.startswith("0x")
            and len(outcomes) == 2
            and market.get("id") is not None
        ):
            result[condition_id.lower()] = {**market, "outcomes": outcomes}
    if not result:
        raise RuntimeError("Gamma returned no recently closed binary markets")
    return result


def _decode_resolution(log: dict) -> tuple[int, list[int]]:
    data = bytes.fromhex(log["data"].removeprefix("0x"))
    if len(data) < 96:
        raise ValueError("ConditionResolution data is too short")
    outcome_count = int.from_bytes(data[0:32], "big")
    offset = int.from_bytes(data[32:64], "big")
    if offset + 32 > len(data):
        raise ValueError("ConditionResolution payout offset is invalid")
    array_len = int.from_bytes(data[offset : offset + 32], "big")
    end = offset + 32 + array_len * 32
    if end > len(data):
        raise ValueError("ConditionResolution payouts exceed encoded data")
    payouts = [
        int.from_bytes(
            data[offset + 32 + index * 32 : offset + 64 + index * 32],
            "big",
        )
        for index in range(array_len)
    ]
    if outcome_count != array_len:
        raise ValueError("ConditionResolution outcome count does not match payouts")
    return outcome_count, payouts


def _discover_market(rpc_url: str, gamma_url: str) -> dict:
    chain_id = int(_rpc(rpc_url, "eth_chainId", []), 16)
    if chain_id != POLYGON_CHAIN_ID:
        raise RuntimeError(f"Polygon RPC chainId must be 137, got {chain_id}")
    finalized = _rpc(rpc_url, "eth_getBlockByNumber", ["finalized", False])
    if not finalized:
        raise RuntimeError("Polygon RPC did not return a finalized block")
    finalized_block = int(finalized["number"], 16)
    markets = _binary_markets(gamma_url)

    scanned = 0
    end = finalized_block
    while end >= 0 and scanned < MAX_SCAN_BLOCKS:
        start = max(0, end - SCAN_CHUNK + 1)
        logs = _rpc(
            rpc_url,
            "eth_getLogs",
            [
                {
                    "fromBlock": hex(start),
                    "toBlock": hex(end),
                    "address": CTF_ADDRESS,
                    "topics": [CONDITION_RESOLUTION_TOPIC0],
                }
            ],
        )
        logs.sort(
            key=lambda item: (
                int(item["blockNumber"], 16),
                int(item["logIndex"], 16),
            ),
            reverse=True,
        )
        for log in logs:
            if log.get("removed") or len(log.get("topics", [])) < 4:
                continue
            condition_id = log["topics"][1].lower()
            market = markets.get(condition_id)
            if market is None:
                continue
            outcome_count, payouts = _decode_resolution(log)
            if outcome_count != 2 or sum(value > 0 for value in payouts) != 1:
                continue
            receipt = _rpc(
                rpc_url,
                "eth_getTransactionReceipt",
                [log["transactionHash"]],
            )
            if not receipt or int(receipt["status"], 16) != 1:
                continue

            block_number = int(log["blockNumber"], 16)
            mirror_id = int(market["id"])
            task_uri = (
                f"gravity://6/{mirror_id}/polymarket_settlement?"
                f"ctf={CTF_ADDRESS}&condition={condition_id}&"
                f"fromBlock={max(0, block_number - 1)}&chainId=137&"
                "maxBlocksPerPoll=100"
            )
            return {
                "mirrorId": mirror_id,
                "question": market.get("question"),
                "outcomes": market["outcomes"],
                "conditionId": condition_id,
                "questionId": log["topics"][3],
                "oracle": "0x" + log["topics"][2][-40:],
                "transactionHash": log["transactionHash"],
                "blockNumber": block_number,
                "finalizedBlock": finalized_block,
                "logIndex": int(log["logIndex"], 16),
                "outcomeSlotCount": outcome_count,
                "payoutNumerators": payouts,
                "winningSlot": next(
                    index for index, value in enumerate(payouts) if value > 0
                ),
                "taskUri": task_uri,
            }

        scanned += end - start + 1
        if start == 0:
            break
        end = start - 1

    raise RuntimeError(
        "no recently closed binary Polymarket market had a finalized CTF "
        f"resolution in the last {MAX_SCAN_BLOCKS} Polygon blocks"
    )


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
    gamma_url = _require_public_https_url(
        env.get("POLYMARKET_GAMMA_URL", DEFAULT_GAMMA_URL),
        "POLYMARKET_GAMMA_URL",
    )
    polygon_rpc_url = env.get("POLYGON_RPC_URL") or env.get(
        "POLYGON_QUICKNONE_HTTP_URL"
    )
    if not polygon_rpc_url:
        raise RuntimeError(
            "POLYGON_RPC_URL or POLYGON_QUICKNONE_HTTP_URL is required"
        )
    polygon_rpc_url = _require_public_https_url(
        polygon_rpc_url, "Polygon RPC URL"
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
    market = _discover_market(polygon_rpc_url, gamma_url)

    artifacts = test_dir / "artifacts"
    artifacts.mkdir(parents=True, exist_ok=True)
    for name in (_HEARTBEAT_FILE, _SUMMARY_FILE):
        (artifacts / name).unlink(missing_ok=True)

    relayer_path = artifacts / _RELAYER_FILE
    relayer_path.write_text(
        json.dumps(
            {
                "uri_mappings": {
                    **{
                        feed["taskUri"]: binance_base_url
                        for feed in binance_feeds
                    },
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
                "binanceFeeds": binance_feeds,
                "polymarket": market,
            },
            indent=2,
            ensure_ascii=False,
        )
        + "\n"
    )
    env["RELAYER_CONFIG_TPL"] = str(relayer_path)
    LOG.info(
        "Prepared live Oracle inputs: Binance pairs=%s anchor=%s; Polymarket "
        "mirrorId=%s block=%s finalized=%s",
        [feed["pair"] for feed in binance_feeds],
        bucket_start_ms,
        market["mirrorId"],
        market["blockNumber"],
        market["finalizedBlock"],
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
