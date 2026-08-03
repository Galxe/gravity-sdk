"""Discover a real finalized Polymarket settlement before cluster deployment."""

import json
import logging
import os
import urllib.parse
import urllib.request
from pathlib import Path

LOG = logging.getLogger(__name__)

CTF_ADDRESS = "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045"
CONDITION_RESOLUTION_TOPIC0 = (
    "0xb44d84d3289691f71497564b85d4233648d9dbae8cbdbb4329f301c3a0185894"
)
DEFAULT_GAMMA_URL = "https://gamma-api.polymarket.com"
SCAN_CHUNK = 2_000
MAX_SCAN_BLOCKS = 100_000
_METADATA_FILE = "polymarket_live_metadata.json"
_RELAYER_FILE = "relayer_config.live.json"


def _read_json(request: urllib.request.Request) -> object:
    with urllib.request.urlopen(request, timeout=30) as response:
        return json.loads(response.read())


def _rpc(rpc_url: str, method: str, params: list):
    payload = json.dumps(
        {"jsonrpc": "2.0", "id": 1, "method": method, "params": params}
    ).encode()
    request = urllib.request.Request(
        rpc_url,
        data=payload,
        headers={"Content-Type": "application/json"},
    )
    body = _read_json(request)
    if "error" in body:
        raise RuntimeError(f"Polygon {method} failed: {body['error']}")
    return body["result"]


def _gamma_markets(base_url: str) -> list[dict]:
    query = urllib.parse.urlencode(
        {
            "closed": "true",
            "limit": 100,
            "order": "closedTime",
            "ascending": "false",
        }
    )
    request = urllib.request.Request(
        f"{base_url.rstrip('/')}/markets?{query}",
        headers={
            "Accept": "application/json",
            "User-Agent": "gravity-oracle-e2e/1.0",
        },
    )
    markets = _read_json(request)
    if not isinstance(markets, list):
        raise RuntimeError("Polymarket Gamma response is not a market list")
    return markets


def _parse_list(value) -> list:
    if isinstance(value, str):
        value = json.loads(value)
    return list(value or [])


def _decode_resolution(log: dict) -> tuple[int, list[int]]:
    data = bytes.fromhex(log["data"][2:])
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


def _find_resolution(
    rpc_url: str, finalized_block: int, condition_id: str
) -> dict | None:
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
                    "topics": [CONDITION_RESOLUTION_TOPIC0, condition_id],
                }
            ],
        )
        if len(logs) > 1:
            raise RuntimeError(
                f"condition {condition_id} has multiple finalized resolution logs"
            )
        if logs:
            return logs[0]
        scanned += end - start + 1
        if start == 0:
            break
        end = start - 1
    return None


def _discover_market(rpc_url: str, gamma_url: str) -> dict:
    chain_id = int(_rpc(rpc_url, "eth_chainId", []), 16)
    if chain_id != 137:
        raise RuntimeError(f"Polygon RPC chainId must be 137, got {chain_id}")
    finalized = _rpc(rpc_url, "eth_getBlockByNumber", ["finalized", False])
    finalized_block = int(finalized["number"], 16)

    for market in _gamma_markets(gamma_url):
        condition_id = market.get("conditionId")
        outcomes = _parse_list(market.get("outcomes"))
        if (
            not condition_id
            or not isinstance(condition_id, str)
            or len(condition_id) != 66
            or len(outcomes) != 2
        ):
            continue

        log = _find_resolution(rpc_url, finalized_block, condition_id)
        if log is None or log.get("removed"):
            continue
        outcome_count, payouts = _decode_resolution(log)
        if outcome_count != 2 or sum(value > 0 for value in payouts) != 1:
            continue

        block_number = int(log["blockNumber"], 16)
        receipt = _rpc(
            rpc_url, "eth_getTransactionReceipt", [log["transactionHash"]]
        )
        if int(receipt["status"], 16) != 1:
            continue

        mirror_id = int(market["id"])
        task_uri = (
            f"gravity://6/{mirror_id}/polymarket_settlement?"
            f"ctf={CTF_ADDRESS}&fromBlock={block_number - 1}&"
            f"condition={condition_id}&maxBlocksPerPoll=100"
        )
        return {
            "mirrorId": mirror_id,
            "question": market.get("question"),
            "outcomes": outcomes,
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

    raise RuntimeError(
        "no recently closed binary Polymarket had a finalized CTF resolution "
        f"within the last {MAX_SCAN_BLOCKS} Polygon blocks"
    )


def pre_deploy(test_dir: Path, env: dict, pytest_args: list[str]):
    rpc_url = (
        env.get("POLYGON_RPC_URL")
        or env.get("POLYGON_QUICKNONE_HTTP_URL")
        or os.environ.get("POLYGON_RPC_URL")
        or os.environ.get("POLYGON_QUICKNONE_HTTP_URL")
    )
    if not rpc_url:
        raise RuntimeError("POLYGON_RPC_URL is required for the live Polymarket suite")

    gamma_url = env.get("POLYMARKET_GAMMA_URL", DEFAULT_GAMMA_URL)
    metadata = _discover_market(rpc_url, gamma_url)
    artifacts = test_dir / "artifacts"
    artifacts.mkdir(parents=True, exist_ok=True)

    metadata_path = artifacts / _METADATA_FILE
    metadata_path.write_text(json.dumps(metadata, indent=2, ensure_ascii=False) + "\n")

    relayer_path = artifacts / _RELAYER_FILE
    relayer_path.write_text(
        json.dumps(
            {"uri_mappings": {metadata["taskUri"]: rpc_url}},
            indent=2,
        )
        + "\n"
    )
    env["RELAYER_CONFIG_TPL"] = str(relayer_path)
    LOG.info(
        "[hook] Selected live Polymarket mirrorId=%s block=%s finalized=%s",
        metadata["mirrorId"],
        metadata["blockNumber"],
        metadata["finalizedBlock"],
    )


def post_stop(test_dir: Path, env: dict):
    artifacts = test_dir / "artifacts"
    for name in (_METADATA_FILE, _RELAYER_FILE):
        path = artifacts / name
        if path.exists():
            path.unlink()
