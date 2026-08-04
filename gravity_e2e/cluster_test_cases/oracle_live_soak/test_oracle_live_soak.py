"""Manual four-validator soak against live Binance and Polygon data."""

import asyncio
from dataclasses import dataclass
from decimal import Decimal
from eth_utils.abi import get_abi_output_types
from hexbytes import HexBytes
import json
import logging
import os
from pathlib import Path
import time
from urllib.parse import urlencode
from urllib.request import urlopen

import pytest
from web3 import Web3
from web3._utils.abi import map_abi_data
from web3._utils.normalizers import BASE_RETURN_NORMALIZERS

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils import oracle_test_support as support


LOG = logging.getLogger(__name__)
SUITE_DIR = Path(__file__).resolve().parent

INTERVAL_MS = 60_000
DECIMALS = 8
SOURCE_TYPE_POLYMARKET = 6
POLYGON_CHAIN_ID = 137
CTF_ADDRESS = Web3.to_checksum_address(
    "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045"
)
POLYMARKET_TASK = Web3.keccak(text="polymarket_settlement")
RECONFIGURATION_ADDRESS = Web3.to_checksum_address(
    "0x00000000000000000000000000000001625F2003"
)
EPOCH_CONFIG_ADDRESS = Web3.to_checksum_address(
    "0x00000000000000000000000000000001625F1005"
)
SEL_CURRENT_EPOCH = Web3.keccak(text="currentEpoch()")[:4]
SEL_REMAINING_TIME = Web3.keccak(text="getRemainingTimeSeconds()")[:4]
CALLBACK_SUCCESS_TOPIC0 = Web3.keccak(
    text="CallbackSuccess(uint32,uint256,uint128,address)"
).hex()

DEFAULT_SOAK_SECONDS = 24 * 60 * 60
DEFAULT_POLL_SECONDS = 15
DEFAULT_STALL_SECONDS = 6 * 60
EPOCH_TIMEOUT_SECONDS = 180
ORACLE_TIMEOUT_SECONDS = 360
MIN_PROPOSAL_WINDOW_SECONDS = 40
QUORUM_VALIDATORS = 3
BOOTSTRAP_EPOCH_INTERVAL_MICROS = 60 * 1_000_000
SOAK_EPOCH_INTERVAL_MICROS = 2 * 60 * 60 * 1_000_000
RESTART_EPOCH_GUARD_SECONDS = 5 * 60
SNAPSHOT_CONFIRMATION_BLOCKS = 16
SNAPSHOT_READ_RETRIES = 20

_HEARTBEAT_FILE = "oracle_live_soak_heartbeat.jsonl"
_SUMMARY_FILE = "oracle_live_soak_summary.json"


@dataclass(frozen=True)
class SoakSettings:
    duration_seconds: int
    poll_seconds: int
    stall_seconds: int
    minimum_advances: int
    restart_after_seconds: int
    restart_node: str


def _env_int(name: str, default: int, minimum: int = 0) -> int:
    value = int(os.environ.get(name, str(default)))
    if value < minimum:
        raise ValueError(f"{name} must be at least {minimum}")
    return value


def _soak_settings() -> SoakSettings:
    duration = _env_int(
        "ORACLE_SOAK_DURATION_SECONDS", DEFAULT_SOAK_SECONDS, minimum=1
    )
    poll_seconds = _env_int(
        "ORACLE_SOAK_POLL_SECONDS", DEFAULT_POLL_SECONDS, minimum=1
    )
    stall_seconds = _env_int(
        "ORACLE_SOAK_STALL_TIMEOUT_SECONDS",
        DEFAULT_STALL_SECONDS,
        minimum=poll_seconds * 2,
    )
    configured_minimum = os.environ.get("ORACLE_SOAK_MIN_ADVANCES")
    minimum_advances = (
        max(1, (duration // 60) * 8 // 10)
        if configured_minimum is None and duration >= 120
        else int(configured_minimum or 0)
    )
    if minimum_advances < 0:
        raise ValueError("ORACLE_SOAK_MIN_ADVANCES must be non-negative")

    configured_restart = os.environ.get("ORACLE_SOAK_RESTART_AFTER_SECONDS")
    restart_after = (
        duration // 2
        if configured_restart is None and duration >= 60 * 60
        else int(configured_restart or 0)
    )
    if restart_after < 0 or restart_after >= duration:
        raise ValueError(
            "ORACLE_SOAK_RESTART_AFTER_SECONDS must be zero or less than duration"
        )
    return SoakSettings(
        duration_seconds=duration,
        poll_seconds=poll_seconds,
        stall_seconds=stall_seconds,
        minimum_advances=minimum_advances,
        restart_after_seconds=restart_after,
        restart_node=os.environ.get("ORACLE_SOAK_RESTART_NODE", "node4"),
    )


def _metadata() -> dict:
    path = SUITE_DIR / "artifacts" / "oracle_live_soak_metadata.json"
    return json.loads(path.read_text())


def _current_epoch(w3: Web3) -> int:
    return int.from_bytes(
        w3.eth.call({"to": RECONFIGURATION_ADDRESS, "data": SEL_CURRENT_EPOCH}),
        "big",
    )


def _remaining_epoch_seconds(w3: Web3) -> int:
    return int.from_bytes(
        w3.eth.call(
            {"to": RECONFIGURATION_ADDRESS, "data": SEL_REMAINING_TIME}
        ),
        "big",
    )


def _restart_window_is_safe(w3: Web3) -> bool:
    remaining = _remaining_epoch_seconds(w3)
    interval = SOAK_EPOCH_INTERVAL_MICROS // 1_000_000
    return (
        RESTART_EPOCH_GUARD_SECONDS < remaining
        < interval - RESTART_EPOCH_GUARD_SECONDS
    )


async def _wait_for_epoch_advance(cluster: Cluster, start_epoch: int) -> int:
    deadline = time.monotonic() + EPOCH_TIMEOUT_SECONDS
    node1 = cluster.get_node("node1")
    while time.monotonic() < deadline:
        epoch = _current_epoch(node1.w3)
        if epoch > start_epoch:
            for node_id, node in cluster.nodes.items():
                while _current_epoch(node.w3) < epoch:
                    if time.monotonic() >= deadline:
                        raise TimeoutError(f"{node_id} did not reach epoch {epoch}")
                    await asyncio.sleep(1)
            return epoch
        await asyncio.sleep(1)
    raise TimeoutError(f"chain did not advance past epoch {start_epoch}")


async def _wait_for_proposal_window(cluster: Cluster) -> int:
    node1 = cluster.get_node("node1")
    deadline = time.monotonic() + EPOCH_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        remaining = _remaining_epoch_seconds(node1.w3)
        if remaining >= MIN_PROPOSAL_WINDOW_SECONDS:
            return _current_epoch(node1.w3)
        current = _current_epoch(node1.w3)
        LOG.info(
            "Only %ss remain in epoch %s; waiting for a proposal window",
            remaining,
            current,
        )
        await _wait_for_epoch_advance(cluster, current)
    raise TimeoutError("no epoch had enough time for governance activation")


def _validator_log(cluster: Cluster, node_id: str) -> Path:
    return cluster.base_dir / node_id / "consensus_log" / "validator.log"


def _log_contains(cluster: Cluster, node_id: str, marker: str, issuer: str) -> bool:
    path = _validator_log(cluster, node_id)
    content = path.read_text(errors="replace") if path.exists() else ""
    return any(marker in line and issuer in line for line in content.splitlines())


async def _wait_for_consensus_evidence(
    cluster: Cluster, issuers: list[str]
) -> None:
    missing_observers = {
        (node_id, issuer)
        for node_id in cluster.nodes
        for issuer in issuers
    }
    certifiers = {issuer: set() for issuer in issuers}
    missing_quorums = set(issuers)
    deadline = time.monotonic() + ORACLE_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        logs = {
            node_id: (
                _validator_log(cluster, node_id).read_text(errors="replace")
                if _validator_log(cluster, node_id).exists()
                else ""
            )
            for node_id in cluster.nodes
        }
        for node_id, issuer in list(missing_observers):
            if any(
                "JWKObserver spawned." in line and issuer in line
                for line in logs[node_id].splitlines()
            ):
                missing_observers.remove((node_id, issuer))
        for issuer in issuers:
            for node_id, content in logs.items():
                lines = content.splitlines()
                if any(
                    "Start certifying update." in line and issuer in line
                    for line in lines
                ):
                    certifiers[issuer].add(node_id)
                if any(
                    "Peer vote aggregated." in line
                    and issuer in line
                    and (
                        "threshold_exceeded=true" in line
                        or '"threshold_exceeded":true' in line
                    )
                    for line in lines
                ):
                    missing_quorums.discard(issuer)
        if (
            not missing_observers
            and not missing_quorums
            and all(
                len(certifiers[issuer]) >= QUORUM_VALIDATORS
                for issuer in issuers
            )
        ):
            return
        await asyncio.sleep(2)
    raise AssertionError(
        "missing consensus evidence: "
        f"observers={sorted(missing_observers)} "
        f"certifiers={ {key: sorted(value) for key, value in certifiers.items()} } "
        f"quorums={sorted(missing_quorums)}"
    )


async def _wait_for_block(
    node_id: str, w3: Web3, block_number: int, timeout: int = 90
):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            if w3.eth.block_number >= block_number:
                return
        except Exception:
            pass
        await asyncio.sleep(1)
    last_height = None
    try:
        last_height = w3.eth.block_number
    except Exception:
        pass
    raise TimeoutError(
        f"{node_id} RPC did not reach Gravity block {block_number}; "
        f"last height was {last_height}"
    )


async def _wait_for_settlement(
    resolver, mirror_id: int, condition_id: bytes
) -> tuple:
    deadline = time.monotonic() + ORACLE_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        settlement = tuple(
            resolver.functions.getSettlement(mirror_id, condition_id).call()
        )
        if settlement[0]:
            return settlement
        await asyncio.sleep(2)
    raise TimeoutError(f"Polymarket mirror {mirror_id} was not settled")


def _call_at_block_hash(function, block_hash: str):
    response = function.w3.provider.make_request(
        "eth_call",
        [
            {
                "to": function.address,
                "data": function._encode_transaction_data(),
            },
            {"blockHash": block_hash, "requireCanonical": True},
        ],
    )
    assert "error" not in response, (
        f"EIP-1898 eth_call failed at {block_hash}: {response['error']}"
    )
    output_types = get_abi_output_types(function.abi)
    decoded = function.w3.codec.decode(
        output_types, HexBytes(response["result"])
    )
    normalized = map_abi_data(
        BASE_RETURN_NORMALIZERS, output_types, decoded
    )
    return normalized[0] if len(normalized) == 1 else normalized


def _price_state_at_block_hash(
    native_oracle,
    price_resolver,
    feed_id: int,
    pair: str,
    block_hash: str,
) -> tuple[tuple, tuple]:
    for _ in range(SNAPSHOT_READ_RETRIES):
        progress_before = tuple(
            _call_at_block_hash(
                native_oracle.functions.getSourceProgress(
                    support.SOURCE_TYPE_PRICE_FEED, feed_id
                ),
                block_hash,
            )
        )
        latest = tuple(
            _call_at_block_hash(
                price_resolver.functions.latestPrice(feed_id), block_hash
            )
        )
        progress_after = tuple(
            _call_at_block_hash(
                native_oracle.functions.getSourceProgress(
                    support.SOURCE_TYPE_PRICE_FEED, feed_id
                ),
                block_hash,
            )
        )
        if (
            progress_before == progress_after
            and latest[0]
            and progress_after[1] == latest[2]
        ):
            return progress_after, latest
        time.sleep(0.05)
    raise AssertionError(
        f"could not obtain an atomic {pair} Oracle snapshot at {block_hash}: "
        f"progress_before={progress_before}, latest={latest}, "
        f"progress_after={progress_after}"
    )


def _topic(value: int) -> str:
    return "0x" + value.to_bytes(32, "big").hex()


def _callback_success_logs(
    w3: Web3,
    source_type: int,
    source_id: int,
    from_block: int,
    to_block: int | str = "latest",
) -> list:
    return w3.eth.get_logs(
        {
            "fromBlock": from_block,
            "toBlock": to_block,
            "address": support.NATIVE_ORACLE_ADDRESS,
            "topics": [
                CALLBACK_SUCCESS_TOPIC0,
                _topic(source_type),
                _topic(source_id),
            ],
        }
    )


def _round_start_ms(first_bucket_start_ms: int, delivery_nonce: int) -> int:
    return first_bucket_start_ms + (delivery_nonce - 1) * INTERVAL_MS


def _fetch_binance_price(
    base_url: str, pair: str, bucket_start_ms: int
) -> int:
    bucket_end_ms = bucket_start_ms + INTERVAL_MS - 1
    query = urlencode(
        {
            "pair": pair,
            "interval": "1m",
            "startTime": bucket_start_ms,
            "endTime": bucket_end_ms,
            "limit": 1,
        }
    )
    with urlopen(
        f"{base_url}/fapi/v1/indexPriceKlines?{query}", timeout=20
    ) as response:
        rows = json.loads(response.read())
    assert len(rows) == 1, f"Binance returned {len(rows)} rows for {pair}"
    assert int(rows[0][0]) == bucket_start_ms
    assert int(rows[0][6]) == bucket_end_ms
    scaled = Decimal(rows[0][4]) * (10**DECIMALS)
    assert scaled == scaled.to_integral_value()
    return int(scaled)


async def _fetch_binance_price_with_retry(
    base_url: str,
    pair: str,
    bucket_start_ms: int,
    timeout: int = 120,
) -> int:
    deadline = time.monotonic() + timeout
    last_error = None
    while time.monotonic() < deadline:
        try:
            return await asyncio.to_thread(
                _fetch_binance_price, base_url, pair, bucket_start_ms
            )
        except (AssertionError, OSError, ValueError) as error:
            last_error = error
            await asyncio.sleep(5)
    raise TimeoutError(
        f"Binance did not return the exact closed {pair} bucket"
    ) from last_error


def _relayer_sources(cluster: Cluster, node_id: str) -> dict:
    path = cluster.base_dir / node_id / "data" / "reth" / "relayer_state.json"
    if not path.exists():
        return {}
    try:
        return json.loads(path.read_text()).get("sources", {})
    except (OSError, json.JSONDecodeError):
        return {}


def _relayer_quorum(cluster: Cluster, uri: str, minimum_nonce: int) -> list[str]:
    reached = []
    for node_id in cluster.nodes:
        raw_nonce = _relayer_sources(cluster, node_id).get(uri, {}).get("last_nonce")
        try:
            nonce = int(raw_nonce or 0)
        except (TypeError, ValueError):
            nonce = 0
        if nonce >= minimum_nonce:
            reached.append(node_id)
    return sorted(reached)


async def _wait_for_relayer_node(
    cluster: Cluster,
    node_id: str,
    uri: str,
    minimum_nonce: int,
    timeout: int = 180,
) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        state = _relayer_sources(cluster, node_id).get(uri, {})
        try:
            nonce = int(state.get("last_nonce") or 0)
        except (TypeError, ValueError):
            nonce = 0
        if nonce >= minimum_nonce:
            return
        await asyncio.sleep(2)
    raise TimeoutError(
        f"{node_id} relayer did not catch up to nonce {minimum_nonce}"
    )


async def _replicated_snapshot(
    cluster: Cluster,
    native_artifact: dict,
    price_artifact: dict,
    polymarket_artifact: dict,
    price_resolver_address: str,
    polymarket_resolver_address: str,
    binance_feeds: list[dict],
    mirror_id: int,
    condition_id: bytes,
) -> dict:
    heights = {}
    for node_id, node in cluster.nodes.items():
        assert node.w3.is_connected(), f"{node_id} RPC disconnected"
        heights[node_id] = node.w3.eth.block_number

    minimum_height = min(heights.values())
    assert minimum_height >= SNAPSHOT_CONFIRMATION_BLOCKS, (
        "Gravity chain is too young for a confirmed Oracle snapshot"
    )
    snapshot_block = minimum_height - SNAPSHOT_CONFIRMATION_BLOCKS
    snapshot_hash = None
    for node_id, node in cluster.nodes.items():
        block = node.w3.eth.get_block(snapshot_block)
        assert int(block["number"]) == snapshot_block
        block_hash = Web3.to_hex(block["hash"]).lower()
        if snapshot_hash is None:
            snapshot_hash = block_hash
        assert block_hash == snapshot_hash, (
            f"{node_id} block hash diverged at Gravity block {snapshot_block}"
        )

    node1 = cluster.get_node("node1")

    native_oracle = node1.w3.eth.contract(
        address=support.NATIVE_ORACLE_ADDRESS, abi=native_artifact["abi"]
    )
    price_resolver = node1.w3.eth.contract(
        address=price_resolver_address, abi=price_artifact["abi"]
    )
    polymarket_resolver = node1.w3.eth.contract(
        address=polymarket_resolver_address, abi=polymarket_artifact["abi"]
    )
    price_feeds = {}
    for feed in binance_feeds:
        feed_id = int(feed["feedId"])
        pair = feed["pair"]
        progress, latest = _price_state_at_block_hash(
            native_oracle,
            price_resolver,
            feed_id,
            pair,
            snapshot_hash,
        )
        assert progress[0] >= 1
        round_start = _round_start_ms(
            int(feed["bucketStartMs"]), progress[0]
        )
        assert latest[0]
        assert latest[1] == round_start // INTERVAL_MS
        assert latest[2] == round_start + INTERVAL_MS - 1
        assert progress[1] == latest[2]
        assert latest[3] == DECIMALS
        assert latest[4] > 0
        price_feeds[pair] = {
            "feedId": feed_id,
            "progress": progress,
            "latestPrice": latest,
        }
    polymarket_progress = tuple(
        _call_at_block_hash(
            native_oracle.functions.getSourceProgress(
                SOURCE_TYPE_POLYMARKET, mirror_id
            ),
            snapshot_hash,
        )
    )
    settlement = tuple(
        _call_at_block_hash(
            polymarket_resolver.functions.getSettlement(
                mirror_id, condition_id
            ),
            snapshot_hash,
        )
    )

    assert polymarket_progress[0] == 1
    assert settlement[0]

    for node_id, node in cluster.nodes.items():
        replica_native = node.w3.eth.contract(
            address=support.NATIVE_ORACLE_ADDRESS, abi=native_artifact["abi"]
        )
        replica_price = node.w3.eth.contract(
            address=price_resolver_address, abi=price_artifact["abi"]
        )
        replica_polymarket = node.w3.eth.contract(
            address=polymarket_resolver_address,
            abi=polymarket_artifact["abi"],
        )
        for feed in binance_feeds:
            feed_id = int(feed["feedId"])
            pair = feed["pair"]
            replica_progress, replica_latest = _price_state_at_block_hash(
                replica_native,
                replica_price,
                feed_id,
                pair,
                snapshot_hash,
            )
            assert replica_progress == price_feeds[pair]["progress"], (
                f"{node_id} {pair} progress diverged"
            )
            assert replica_latest == price_feeds[pair]["latestPrice"], (
                f"{node_id} {pair} latest price diverged"
            )
        assert tuple(
            _call_at_block_hash(
                replica_native.functions.getSourceProgress(
                    SOURCE_TYPE_POLYMARKET, mirror_id
                ),
                snapshot_hash,
            )
        ) == polymarket_progress, f"{node_id} Polymarket progress diverged"
        assert tuple(
            _call_at_block_hash(
                replica_polymarket.functions.getSettlement(
                    mirror_id, condition_id
                ),
                snapshot_hash,
            )
        ) == settlement, f"{node_id} Polymarket settlement diverged"

    return {
        "block": snapshot_block,
        "blockHash": snapshot_hash,
        "heights": heights,
        "priceFeeds": price_feeds,
        "polymarketProgress": polymarket_progress,
        "settlement": settlement,
    }


def _append_json_line(path: Path, payload: dict) -> None:
    with path.open("a") as output:
        output.write(json.dumps(payload, sort_keys=True) + "\n")


async def _restart_validator(
    cluster: Cluster,
    node_id: str,
    target_block: int,
    price_targets: list[tuple[str, int]],
) -> float:
    node = cluster.get_node(node_id)
    if node is None:
        raise AssertionError(f"unknown restart node {node_id}")
    started = time.monotonic()
    assert await node.restart(), f"failed to restart {node_id}"
    await _wait_for_block(node_id, node.w3, target_block, timeout=180)
    assert await cluster.check_block_increasing(timeout=60)
    for uri, target_nonce in price_targets:
        await _wait_for_relayer_node(
            cluster, node_id, uri, target_nonce
        )
    return time.monotonic() - started


async def _run_soak(
    cluster: Cluster,
    settings: SoakSettings,
    native_artifact: dict,
    price_artifact: dict,
    polymarket_artifact: dict,
    price_resolver_address: str,
    polymarket_resolver_address: str,
    binance_feeds: list[dict],
    polymarket: dict,
    expected_settlement: tuple,
) -> dict:
    mirror_id = int(polymarket["mirrorId"])
    condition_id = bytes.fromhex(polymarket["conditionId"].removeprefix("0x"))
    heartbeat_path = SUITE_DIR / "artifacts" / _HEARTBEAT_FILE
    heartbeat_path.unlink(missing_ok=True)

    initial_tip = max(
        node.w3.eth.block_number for node in cluster.nodes.values()
    )
    confirmation_target = (
        initial_tip + SNAPSHOT_CONFIRMATION_BLOCKS + 1
    )
    for node_id, node in cluster.nodes.items():
        await _wait_for_block(
            node_id, node.w3, confirmation_target, timeout=90
        )

    initial = await _replicated_snapshot(
        cluster,
        native_artifact,
        price_artifact,
        polymarket_artifact,
        price_resolver_address,
        polymarket_resolver_address,
        binance_feeds,
        mirror_id,
        condition_id,
    )
    initial_nonces = {
        pair: int(state["progress"][0])
        for pair, state in initial["priceFeeds"].items()
    }
    last_nonces = dict(initial_nonces)
    last_prices = {
        pair: int(state["latestPrice"][4])
        for pair, state in initial["priceFeeds"].items()
    }
    observed_price_changes = {pair: 0 for pair in initial_nonces}
    started = time.monotonic()
    deadline = started + settings.duration_seconds
    last_price_advances = {pair: started for pair in initial_nonces}
    max_price_gaps = {pair: 0.0 for pair in initial_nonces}
    last_height_change = {node_id: started for node_id in cluster.nodes}
    previous_heights = dict(initial["heights"])
    restart_done = False
    restart_recovery_seconds = None
    restart_epoch_guard_deferrals = 0
    samples = 0
    final = initial

    while time.monotonic() < deadline:
        now = time.monotonic()
        if (
            settings.restart_after_seconds
            and not restart_done
            and now - started >= settings.restart_after_seconds
        ):
            epoch_rpc = next(iter(cluster.nodes.values())).w3
            if _restart_window_is_safe(epoch_rpc):
                restart_recovery_seconds = await _restart_validator(
                    cluster,
                    settings.restart_node,
                    max(int(height) for height in final["heights"].values()),
                    [
                        (
                            feed["taskUri"],
                            int(
                                final["priceFeeds"][feed["pair"]][
                                    "progress"
                                ][0]
                            ),
                        )
                        for feed in binance_feeds
                    ],
                )
                restart_done = True
            else:
                restart_epoch_guard_deferrals += 1
                if restart_epoch_guard_deferrals == 1:
                    LOG.info(
                        "deferring validator restart outside the epoch "
                        "transition guard"
                    )

        final = await _replicated_snapshot(
            cluster,
            native_artifact,
            price_artifact,
            polymarket_artifact,
            price_resolver_address,
            polymarket_resolver_address,
            binance_feeds,
            mirror_id,
            condition_id,
        )
        now = time.monotonic()
        price_heartbeats = {}
        for feed in binance_feeds:
            pair = feed["pair"]
            state = final["priceFeeds"][pair]
            nonce = int(state["progress"][0])
            price = int(state["latestPrice"][4])
            assert nonce >= last_nonces[pair], (
                f"{pair} source nonce regressed"
            )
            if nonce > last_nonces[pair]:
                max_price_gaps[pair] = max(
                    max_price_gaps[pair], now - last_price_advances[pair]
                )
                last_price_advances[pair] = now
                last_nonces[pair] = nonce
            if price != last_prices[pair]:
                observed_price_changes[pair] += 1
                last_prices[pair] = price
            assert now - last_price_advances[pair] <= settings.stall_seconds, (
                f"{pair} price feed stalled at nonce {nonce} for "
                f"{now - last_price_advances[pair]:.1f}s"
            )
            relayer_nodes = _relayer_quorum(
                cluster, feed["taskUri"], nonce
            )
            assert len(relayer_nodes) >= QUORUM_VALIDATORS, (
                f"only {relayer_nodes} persisted {pair} nonce {nonce}"
            )
            price_heartbeats[pair] = {
                "feedId": int(feed["feedId"]),
                "nonce": nonce,
                "position": int(state["progress"][1]),
                "round": int(state["latestPrice"][1]),
                "price": str(price),
                "observedPriceChanges": observed_price_changes[pair],
                "relayerQuorumNodes": relayer_nodes,
            }

        for node_id, height in final["heights"].items():
            assert height >= previous_heights[node_id], (
                f"{node_id} block height regressed"
            )
            if height > previous_heights[node_id]:
                last_height_change[node_id] = now
                previous_heights[node_id] = height
            assert now - last_height_change[node_id] <= settings.stall_seconds, (
                f"{node_id} chain height stalled for "
                f"{now - last_height_change[node_id]:.1f}s"
            )

        assert tuple(final["polymarketProgress"]) == (
            1,
            int(polymarket["blockNumber"]),
        )
        assert tuple(final["settlement"]) == expected_settlement

        samples += 1
        heartbeat = {
            "elapsedSeconds": round(now - started, 3),
            "gravityBlock": int(final["block"]),
            "gravityBlockHash": final["blockHash"],
            "nodeHeights": final["heights"],
            "priceFeeds": price_heartbeats,
            "polymarketNonce": int(final["polymarketProgress"][0]),
            "restartCompleted": restart_done,
            "restartEpochGuardDeferrals": restart_epoch_guard_deferrals,
        }
        _append_json_line(heartbeat_path, heartbeat)
        LOG.info("Oracle soak heartbeat: %s", heartbeat)
        await asyncio.sleep(
            min(settings.poll_seconds, max(0.0, deadline - time.monotonic()))
        )

    final = await _replicated_snapshot(
        cluster,
        native_artifact,
        price_artifact,
        polymarket_artifact,
        price_resolver_address,
        polymarket_resolver_address,
        binance_feeds,
        mirror_id,
        condition_id,
    )
    if settings.restart_after_seconds:
        assert restart_done, "configured validator restart did not run"

    price_summaries = {}
    for feed in binance_feeds:
        pair = feed["pair"]
        state = final["priceFeeds"][pair]
        final_nonce = int(state["progress"][0])
        advances = final_nonce - initial_nonces[pair]
        assert advances >= settings.minimum_advances, (
            f"{pair} advanced {advances} rounds; expected at least "
            f"{settings.minimum_advances}"
        )
        if settings.restart_after_seconds:
            await _wait_for_relayer_node(
                cluster,
                settings.restart_node,
                feed["taskUri"],
                final_nonce,
            )
        final_bucket = _round_start_ms(
            int(feed["bucketStartMs"]), final_nonce
        )
        max_price_gaps[pair] = max(
            max_price_gaps[pair],
            time.monotonic() - last_price_advances[pair],
        )
        expected_price = await _fetch_binance_price_with_retry(
            feed["baseUrl"], pair, final_bucket
        )
        assert int(state["latestPrice"][4]) == expected_price
        price_summaries[pair] = {
            "feedId": int(feed["feedId"]),
            "initialNonce": initial_nonces[pair],
            "finalNonce": final_nonce,
            "advances": advances,
            "minimumAdvances": settings.minimum_advances,
            "finalPrice": str(state["latestPrice"][4]),
            "observedPriceChanges": observed_price_changes[pair],
            "maxObservedGapSeconds": round(max_price_gaps[pair], 3),
        }
    return {
        "status": "passed",
        "configuredDurationSeconds": settings.duration_seconds,
        "actualDurationSeconds": round(time.monotonic() - started, 3),
        "samples": samples,
        "priceFeeds": price_summaries,
        "finalGravityBlock": int(final["block"]),
        "finalGravityBlockHash": final["blockHash"],
        "restartNode": (
            settings.restart_node if settings.restart_after_seconds else None
        ),
        "restartRecoverySeconds": restart_recovery_seconds,
        "restartEpochGuardDeferrals": restart_epoch_guard_deferrals,
        "polymarketMirrorId": mirror_id,
        "polymarketNonce": int(final["polymarketProgress"][0]),
    }


def _write_summary(payload: dict) -> None:
    path = SUITE_DIR / "artifacts" / _SUMMARY_FILE
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")


@pytest.mark.asyncio
async def test_governance_activated_oracles_soak_for_configured_duration(
    cluster: Cluster,
):
    settings = _soak_settings()
    metadata = _metadata()
    binance_feeds = metadata["binanceFeeds"]
    polymarket = metadata["polymarket"]
    assert [feed["pair"] for feed in binance_feeds] == [
        "NVDAUSDT",
        "BTCUSDT",
        "ETHUSDT",
    ]
    assert len({int(feed["feedId"]) for feed in binance_feeds}) == len(
        binance_feeds
    )
    mirror_id = int(polymarket["mirrorId"])
    condition_id = bytes.fromhex(polymarket["conditionId"].removeprefix("0x"))

    assert len(cluster.nodes) == 4
    assert await cluster.set_full_live(timeout=180)
    assert await cluster.check_block_increasing(timeout=60)
    active = await cluster.validator_list()
    assert {node.id for node in active.active} == set(cluster.nodes)

    node1 = cluster.get_node("node1")
    assert node1 is not None and node1.w3.is_connected()
    w3 = node1.w3
    required = [
        ("PriceFeedResolver.sol", "PriceFeedResolver"),
        ("PolymarketSettlementResolver.sol", "PolymarketSettlementResolver"),
        ("NativeOracle.sol", "NativeOracle"),
        ("OracleTaskConfig.sol", "OracleTaskConfig"),
        ("EpochConfig.sol", "EpochConfig"),
    ]
    contracts_out = support.ensure_contract_artifacts(SUITE_DIR, required)
    price_artifact = support.load_artifact(
        contracts_out, "PriceFeedResolver.sol", "PriceFeedResolver"
    )
    polymarket_artifact = support.load_artifact(
        contracts_out,
        "PolymarketSettlementResolver.sol",
        "PolymarketSettlementResolver",
    )
    native_artifact = support.load_artifact(
        contracts_out, "NativeOracle.sol", "NativeOracle"
    )
    task_artifact = support.load_artifact(
        contracts_out, "OracleTaskConfig.sol", "OracleTaskConfig"
    )
    epoch_artifact = support.load_artifact(
        contracts_out, "EpochConfig.sol", "EpochConfig"
    )

    price_resolver = support.deploy_contract(w3, price_artifact)
    polymarket_resolver = support.deploy_contract(w3, polymarket_artifact)
    native_oracle = w3.eth.contract(
        address=support.NATIVE_ORACLE_ADDRESS, abi=native_artifact["abi"]
    )
    task_config = w3.eth.contract(
        address=support.ORACLE_TASK_CONFIG_ADDRESS, abi=task_artifact["abi"]
    )
    epoch_config = w3.eth.contract(
        address=EPOCH_CONFIG_ADDRESS, abi=epoch_artifact["abi"]
    )

    assert (
        epoch_config.functions.epochIntervalMicros().call()
        == BOOTSTRAP_EPOCH_INTERVAL_MICROS
    )
    assert tuple(epoch_config.functions.getPendingConfig().call()) == (False, 0)

    for feed in binance_feeds:
        assert not task_config.functions.hasTask(
            support.SOURCE_TYPE_PRICE_FEED,
            int(feed["feedId"]),
            support.TASK_PRICE_FEED,
        ).call()
    assert not task_config.functions.hasTask(
        SOURCE_TYPE_POLYMARKET, mirror_id, POLYMARKET_TASK
    ).call()

    setup_epoch = await _wait_for_proposal_window(cluster)
    targets = [EPOCH_CONFIG_ADDRESS, support.NATIVE_ORACLE_ADDRESS]
    datas = [
        support.function_calldata(
            epoch_config.functions.setForNextEpoch(
                SOAK_EPOCH_INTERVAL_MICROS
            )
        ),
        support.function_calldata(
            native_oracle.functions.setDefaultCallback(
                support.SOURCE_TYPE_PRICE_FEED, price_resolver.address
            )
        ),
    ]
    for feed in binance_feeds:
        targets.append(support.ORACLE_TASK_CONFIG_ADDRESS)
        datas.append(
            support.function_calldata(
                task_config.functions.setTask(
                    support.SOURCE_TYPE_PRICE_FEED,
                    int(feed["feedId"]),
                    support.TASK_PRICE_FEED,
                    feed["taskUri"].encode(),
                )
            )
        )
    targets.extend(
        [
            support.ORACLE_TASK_CONFIG_ADDRESS,
            support.NATIVE_ORACLE_ADDRESS,
            polymarket_resolver.address,
        ]
    )
    datas.extend(
        [
            support.function_calldata(
                task_config.functions.setTask(
                    SOURCE_TYPE_POLYMARKET,
                    mirror_id,
                    POLYMARKET_TASK,
                    polymarket["taskUri"].encode(),
                )
            ),
            support.function_calldata(
                native_oracle.functions.setCallback(
                    SOURCE_TYPE_POLYMARKET,
                    mirror_id,
                    polymarket_resolver.address,
                )
            ),
            support.function_calldata(
                polymarket_resolver.functions.registerMirror(
                    mirror_id,
                    POLYGON_CHAIN_ID,
                    CTF_ADDRESS,
                    condition_id,
                    int(polymarket["outcomeSlotCount"]),
                )
            ),
        ]
    )
    receipt = await support.execute_governance_proposal(
        w3,
        support.faucet_voting_pool(w3),
        targets,
        datas,
        "activate-live-binance-and-polymarket-soak",
        gas=8_000_000,
    )
    assert _current_epoch(w3) == setup_epoch
    assert (
        epoch_config.functions.epochIntervalMicros().call()
        == BOOTSTRAP_EPOCH_INTERVAL_MICROS
    )
    assert tuple(epoch_config.functions.getPendingConfig().call()) == (
        True,
        SOAK_EPOCH_INTERVAL_MICROS,
    )
    configured_tasks = [
        (
            support.SOURCE_TYPE_PRICE_FEED,
            int(feed["feedId"]),
            support.TASK_PRICE_FEED,
            feed["taskUri"],
        )
        for feed in binance_feeds
    ]
    configured_tasks.append(
        (
            SOURCE_TYPE_POLYMARKET,
            mirror_id,
            POLYMARKET_TASK,
            polymarket["taskUri"],
        )
    )
    for source_type, source_id, task_name, expected_uri in configured_tasks:
        assert task_config.functions.hasTask(
            source_type, source_id, task_name
        ).call()
        task = task_config.functions.getTask(
            source_type, source_id, task_name
        ).call()
        assert bytes(task[0]).decode() == expected_uri
        assert native_oracle.functions.getLatestNonce(
            source_type, source_id
        ).call() == 0

    issuers = [
        *(f"gravity://3/{int(feed['feedId'])}" for feed in binance_feeds),
        f"gravity://6/{mirror_id}",
    ]
    for node_id in cluster.nodes:
        for issuer in issuers:
            assert not _log_contains(
                cluster, node_id, "JWKObserver spawned.", issuer
            ), f"{node_id} started {issuer} before the epoch boundary"

    activation_epoch = await _wait_for_epoch_advance(cluster, setup_epoch)
    assert activation_epoch == setup_epoch + 1
    assert (
        epoch_config.functions.epochIntervalMicros().call()
        == SOAK_EPOCH_INTERVAL_MICROS
    )
    assert tuple(epoch_config.functions.getPendingConfig().call()) == (False, 0)
    assert _remaining_epoch_seconds(w3) > (
        SOAK_EPOCH_INTERVAL_MICROS // 1_000_000 - 300
    )
    initial_prices = {}
    for feed in binance_feeds:
        initial_prices[feed["pair"]] = await support.wait_for_latest_price(
            native_oracle,
            price_resolver,
            int(feed["feedId"]),
            1,
            timeout=ORACLE_TIMEOUT_SECONDS,
        )
    deadline = time.monotonic() + ORACLE_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        polymarket_progress = tuple(
            native_oracle.functions.getSourceProgress(
                SOURCE_TYPE_POLYMARKET, mirror_id
            ).call()
        )
        if polymarket_progress[0] >= 1:
            break
        await asyncio.sleep(2)
    else:
        raise TimeoutError("Polymarket source did not reach nonce 1")

    assert polymarket_progress == (1, int(polymarket["blockNumber"]))
    settlement = await _wait_for_settlement(
        polymarket_resolver, mirror_id, condition_id
    )
    assert settlement[1] == 1
    assert settlement[2] == POLYGON_CHAIN_ID
    assert Web3.to_checksum_address(settlement[3]) == CTF_ADDRESS
    assert Web3.to_checksum_address(settlement[4]) == Web3.to_checksum_address(
        polymarket["oracle"]
    )
    assert Web3.to_hex(settlement[5]).lower() == polymarket["questionId"].lower()
    assert settlement[6] == int(polymarket["outcomeSlotCount"])
    assert Web3.to_hex(settlement[7]).lower() == polymarket[
        "transactionHash"
    ].lower()
    assert settlement[8] == int(polymarket["logIndex"])
    observation = tuple(
        polymarket_resolver.functions.getSettlementObservation(
            mirror_id, condition_id
        ).call()
    )
    assert observation[0] == 1
    expected_winning_slot = next(
        index
        for index, payout in enumerate(polymarket["payoutNumerators"])
        if payout > 0
    )
    assert observation[1] == expected_winning_slot
    assert observation[2] == 1
    assert Web3.to_hex(observation[4]).lower() == polymarket[
        "transactionHash"
    ].lower()
    assert observation[5] == int(polymarket["logIndex"])
    resolution_events = (
        polymarket_resolver.events.PolymarketConditionResolved().get_logs(
            from_block=receipt["blockNumber"],
            to_block="latest",
            argument_filters={
                "mirrorId": mirror_id,
                "conditionId": condition_id,
            },
        )
    )
    assert len(resolution_events) == 1
    assert list(resolution_events[0]["args"]["payoutNumerators"]) == polymarket[
        "payoutNumerators"
    ]

    for feed in binance_feeds:
        price_progress, initial_price = initial_prices[feed["pair"]]
        first_bucket = _round_start_ms(
            int(feed["bucketStartMs"]), int(price_progress[0])
        )
        expected_initial_price = await _fetch_binance_price_with_retry(
            feed["baseUrl"], feed["pair"], first_bucket
        )
        assert int(initial_price[4]) == expected_initial_price
    await _wait_for_consensus_evidence(cluster, issuers)
    for feed in binance_feeds:
        price_progress, _ = initial_prices[feed["pair"]]
        assert len(
            _relayer_quorum(
                cluster, feed["taskUri"], int(price_progress[0])
            )
        ) >= QUORUM_VALIDATORS

    try:
        summary = await _run_soak(
            cluster,
            settings,
            native_artifact,
            price_artifact,
            polymarket_artifact,
            price_resolver.address,
            polymarket_resolver.address,
            binance_feeds,
            polymarket,
            settlement,
        )
    except BaseException as error:
        last_heartbeat = None
        heartbeat_path = SUITE_DIR / "artifacts" / _HEARTBEAT_FILE
        if heartbeat_path.exists():
            lines = heartbeat_path.read_text().splitlines()
            if lines:
                last_heartbeat = json.loads(lines[-1])
        _write_summary(
            {
                "status": "failed",
                "errorType": type(error).__name__,
                "error": str(error),
                "configuredDurationSeconds": settings.duration_seconds,
                "binancePairs": [
                    feed["pair"] for feed in binance_feeds
                ],
                "polymarketMirrorId": mirror_id,
                "lastHeartbeat": last_heartbeat,
            }
        )
        raise

    price_delivery_counts = {}
    for feed in binance_feeds:
        pair = feed["pair"]
        delivery_count = len(
            _callback_success_logs(
                w3,
                support.SOURCE_TYPE_PRICE_FEED,
                int(feed["feedId"]),
                receipt["blockNumber"],
                summary["finalGravityBlock"],
            )
        )
        assert delivery_count == summary["priceFeeds"][pair]["finalNonce"], (
            f"{pair} callback count does not match final source nonce"
        )
        price_delivery_counts[pair] = delivery_count
    polymarket_delivery_count = len(
        _callback_success_logs(
            w3,
            SOURCE_TYPE_POLYMARKET,
            mirror_id,
            receipt["blockNumber"],
            summary["finalGravityBlock"],
        )
    )
    assert polymarket_delivery_count == 1, (
        "Polymarket settlement was delivered more than once"
    )
    summary.update(
        {
            "activationEpoch": activation_epoch,
            "governanceBlock": receipt["blockNumber"],
            "priceCallbackEvents": price_delivery_counts,
            "polymarketCallbackEvents": polymarket_delivery_count,
            "soakEpochIntervalSeconds": (
                SOAK_EPOCH_INTERVAL_MICROS // 1_000_000
            ),
        }
    )
    _write_summary(summary)
    LOG.info("Oracle live soak passed: %s", summary)
