"""Manual four-validator soak against live Binance index-price data."""

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

try:
    import tomllib
except ImportError:
    import tomli as tomllib

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
DEFAULT_STALL_SECONDS = 15 * 60
DEFAULT_RESTART_RPC_TIMEOUT_SECONDS = 3 * 60
EPOCH_TIMEOUT_SECONDS = 180
ORACLE_TIMEOUT_SECONDS = 360
MIN_PROPOSAL_WINDOW_SECONDS = 40
QUORUM_VALIDATORS = 3
BOOTSTRAP_EPOCH_INTERVAL_MICROS = 60 * 1_000_000
SOAK_EPOCH_INTERVAL_MICROS = 2 * 60 * 60 * 1_000_000
RESTART_EPOCH_GUARD_SECONDS = 5 * 60
SNAPSHOT_CONFIRMATION_BLOCKS = 16
SNAPSHOT_READ_RETRIES = 20
SNAPSHOT_CONVERGENCE_RETRIES = 20
SNAPSHOT_CONVERGENCE_DELAY_SECONDS = 0.25
MAX_GET_LOGS_BLOCKS = 100_000


class SnapshotNotConverged(AssertionError):
    """A canonical snapshot is not yet readable consistently by every RPC."""

_HEARTBEAT_FILE = "oracle_live_soak_heartbeat.jsonl"
_SUMMARY_FILE = "oracle_live_soak_summary.json"


@dataclass(frozen=True)
class SoakSettings:
    duration_seconds: int
    poll_seconds: int
    stall_seconds: int
    minimum_advances: int
    restart_schedule_seconds: tuple[int, ...]
    restart_node: str
    restart_rpc_timeout_seconds: int


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

    configured_schedule = os.environ.get(
        "ORACLE_SOAK_RESTART_SCHEDULE_SECONDS"
    )
    configured_restart = os.environ.get("ORACLE_SOAK_RESTART_AFTER_SECONDS")
    if configured_schedule is not None and configured_restart is not None:
        raise ValueError(
            "configure only one of ORACLE_SOAK_RESTART_SCHEDULE_SECONDS "
            "and ORACLE_SOAK_RESTART_AFTER_SECONDS"
        )
    if configured_schedule is not None:
        restart_schedule = tuple(
            int(value.strip())
            for value in configured_schedule.split(",")
            if value.strip()
        )
    elif configured_restart is not None:
        restart_after = int(configured_restart or 0)
        restart_schedule = (restart_after,) if restart_after else ()
    elif duration >= 60 * 60:
        restart_schedule = (duration // 2,)
    else:
        restart_schedule = ()
    if any(
        restart_after <= 0 or restart_after >= duration
        for restart_after in restart_schedule
    ):
        raise ValueError(
            "restart schedule entries must be positive and less than duration"
        )
    if tuple(sorted(set(restart_schedule))) != restart_schedule:
        raise ValueError(
            "restart schedule entries must be unique and strictly increasing"
        )
    return SoakSettings(
        duration_seconds=duration,
        poll_seconds=poll_seconds,
        stall_seconds=stall_seconds,
        minimum_advances=minimum_advances,
        restart_schedule_seconds=restart_schedule,
        restart_node=os.environ.get("ORACLE_SOAK_RESTART_NODE", "node4"),
        restart_rpc_timeout_seconds=_env_int(
            "ORACLE_SOAK_RESTART_RPC_TIMEOUT_SECONDS",
            DEFAULT_RESTART_RPC_TIMEOUT_SECONDS,
            minimum=30,
        ),
    )


def test_soak_settings_defaults_to_one_midpoint_restart(monkeypatch):
    monkeypatch.setenv("ORACLE_SOAK_DURATION_SECONDS", "7200")
    monkeypatch.delenv(
        "ORACLE_SOAK_RESTART_SCHEDULE_SECONDS", raising=False
    )
    monkeypatch.delenv("ORACLE_SOAK_RESTART_AFTER_SECONDS", raising=False)

    assert _soak_settings().restart_schedule_seconds == (3600,)


def test_soak_settings_accepts_multiple_restarts(monkeypatch):
    monkeypatch.setenv("ORACLE_SOAK_DURATION_SECONDS", "1200")
    monkeypatch.setenv(
        "ORACLE_SOAK_RESTART_SCHEDULE_SECONDS", "360, 720,960"
    )
    monkeypatch.delenv("ORACLE_SOAK_RESTART_AFTER_SECONDS", raising=False)

    assert _soak_settings().restart_schedule_seconds == (360, 720, 960)


def test_soak_settings_legacy_zero_disables_restart(monkeypatch):
    monkeypatch.setenv("ORACLE_SOAK_DURATION_SECONDS", "7200")
    monkeypatch.delenv(
        "ORACLE_SOAK_RESTART_SCHEDULE_SECONDS", raising=False
    )
    monkeypatch.setenv("ORACLE_SOAK_RESTART_AFTER_SECONDS", "0")

    assert _soak_settings().restart_schedule_seconds == ()


@pytest.mark.parametrize("schedule", ["720,360", "360,360", "0", "1200"])
def test_soak_settings_rejects_invalid_restart_schedule(
    monkeypatch, schedule
):
    monkeypatch.setenv("ORACLE_SOAK_DURATION_SECONDS", "1200")
    monkeypatch.setenv("ORACLE_SOAK_RESTART_SCHEDULE_SECONDS", schedule)
    monkeypatch.delenv("ORACLE_SOAK_RESTART_AFTER_SECONDS", raising=False)

    with pytest.raises(ValueError):
        _soak_settings()


def test_soak_settings_rejects_two_restart_controls(monkeypatch):
    monkeypatch.setenv("ORACLE_SOAK_DURATION_SECONDS", "1200")
    monkeypatch.setenv("ORACLE_SOAK_RESTART_SCHEDULE_SECONDS", "360")
    monkeypatch.setenv("ORACLE_SOAK_RESTART_AFTER_SECONDS", "720")

    with pytest.raises(ValueError, match="configure only one"):
        _soak_settings()


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
    if "error" in response:
        raise SnapshotNotConverged(
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
            and latest[0] > 0
            and progress_after[1] == latest[1]
        ):
            return progress_after, latest
        time.sleep(0.05)
    raise SnapshotNotConverged(
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
    final_block = (
        int(w3.eth.block_number) if to_block == "latest" else int(to_block)
    )
    if final_block < from_block:
        return []

    logs = []
    for chunk_start in range(
        from_block, final_block + 1, MAX_GET_LOGS_BLOCKS
    ):
        chunk_end = min(
            chunk_start + MAX_GET_LOGS_BLOCKS - 1, final_block
        )
        logs.extend(
            w3.eth.get_logs(
                {
                    "fromBlock": chunk_start,
                    "toBlock": chunk_end,
                    "address": support.NATIVE_ORACLE_ADDRESS,
                    "topics": [
                        CALLBACK_SUCCESS_TOPIC0,
                        _topic(source_type),
                        _topic(source_id),
                    ],
                }
            )
        )
    return logs


def test_callback_success_logs_chunks_large_ranges():
    class FakeEth:
        block_number = 400_000

        def __init__(self):
            self.requests = []

        def get_logs(self, request):
            self.requests.append(request)
            return [(request["fromBlock"], request["toBlock"])]

    class FakeWeb3:
        def __init__(self):
            self.eth = FakeEth()

    w3 = FakeWeb3()
    logs = _callback_success_logs(w3, 3, 1001, 68, 343_799)

    assert logs == [
        (68, 100_067),
        (100_068, 200_067),
        (200_068, 300_067),
        (300_068, 343_799),
    ]
    assert all(
        request["address"] == support.NATIVE_ORACLE_ADDRESS
        and request["topics"]
        == [CALLBACK_SUCCESS_TOPIC0, _topic(3), _topic(1001)]
        for request in w3.eth.requests
    )


def test_callback_success_logs_snapshots_latest_once():
    class FakeEth:
        block_number = 200_001

        def __init__(self):
            self.requests = []

        def get_logs(self, request):
            self.requests.append(request)
            return []

    class FakeWeb3:
        def __init__(self):
            self.eth = FakeEth()

    w3 = FakeWeb3()
    assert _callback_success_logs(w3, 3, 42, 0) == []
    assert [
        (request["fromBlock"], request["toBlock"])
        for request in w3.eth.requests
    ] == [(0, 99_999), (100_000, 199_999), (200_000, 200_001)]


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


async def _replicated_snapshot_once(
    cluster: Cluster,
    native_artifact: dict,
    price_artifact: dict,
    price_resolver_address: str,
    binance_feeds: list[dict],
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
        if block_hash != snapshot_hash:
            raise SnapshotNotConverged(
                f"{node_id} block hash diverged at Gravity block "
                f"{snapshot_block}: {block_hash} != {snapshot_hash}"
            )

    node1 = cluster.get_node("node1")

    native_oracle = node1.w3.eth.contract(
        address=support.NATIVE_ORACLE_ADDRESS, abi=native_artifact["abi"]
    )
    price_resolver = node1.w3.eth.contract(
        address=price_resolver_address, abi=price_artifact["abi"]
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
        assert latest[0] == round_start // INTERVAL_MS
        assert latest[1] == round_start + INTERVAL_MS - 1
        assert progress[1] == latest[1]
        assert latest[2] > 0
        price_feeds[pair] = {
            "feedId": feed_id,
            "progress": progress,
            "latestPrice": latest,
        }

    for node_id, node in cluster.nodes.items():
        replica_native = node.w3.eth.contract(
            address=support.NATIVE_ORACLE_ADDRESS, abi=native_artifact["abi"]
        )
        replica_price = node.w3.eth.contract(
            address=price_resolver_address, abi=price_artifact["abi"]
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
            if replica_progress != price_feeds[pair]["progress"]:
                raise SnapshotNotConverged(
                    f"{node_id} {pair} progress diverged: "
                    f"{replica_progress} != "
                    f"{price_feeds[pair]['progress']}"
                )
            if replica_latest != price_feeds[pair]["latestPrice"]:
                raise SnapshotNotConverged(
                    f"{node_id} {pair} latest price diverged: "
                    f"{replica_latest} != "
                    f"{price_feeds[pair]['latestPrice']}"
                )

    return {
        "block": snapshot_block,
        "blockHash": snapshot_hash,
        "heights": heights,
        "priceFeeds": price_feeds,
    }


async def _replicated_snapshot(
    cluster: Cluster,
    native_artifact: dict,
    price_artifact: dict,
    price_resolver_address: str,
    binance_feeds: list[dict],
) -> dict:
    first_error = None
    for attempt in range(1, SNAPSHOT_CONVERGENCE_RETRIES + 1):
        try:
            snapshot = await _replicated_snapshot_once(
                cluster,
                native_artifact,
                price_artifact,
                price_resolver_address,
                binance_feeds,
            )
            if attempt > 1:
                LOG.info(
                    "canonical replica snapshot converged after %d attempts",
                    attempt,
                )
            return snapshot
        except SnapshotNotConverged as exc:
            if first_error is None:
                first_error = str(exc)
                LOG.warning(
                    "canonical replica snapshot not yet converged: %s", exc
                )
            if attempt == SNAPSHOT_CONVERGENCE_RETRIES:
                raise AssertionError(
                    "canonical replica snapshot did not converge after "
                    f"{attempt} attempts; first={first_error}; last={exc}"
                ) from exc
            await asyncio.sleep(SNAPSHOT_CONVERGENCE_DELAY_SECONDS)

    raise AssertionError("unreachable replica snapshot retry state")


def _append_json_line(path: Path, payload: dict) -> None:
    with path.open("a") as output:
        output.write(json.dumps(payload, sort_keys=True) + "\n")


async def _restart_validator(
    cluster: Cluster,
    node_id: str,
    target_block: int,
    price_targets: list[tuple[str, int]],
    rpc_timeout: int,
) -> float:
    node = cluster.get_node(node_id)
    if node is None:
        raise AssertionError(f"unknown restart node {node_id}")
    started = time.monotonic()
    assert await node.restart(rpc_timeout=rpc_timeout), (
        f"failed to restart {node_id} within {rpc_timeout}s RPC timeout"
    )
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
    price_resolver_address: str,
    binance_feeds: list[dict],
) -> dict:
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
        price_resolver_address,
        binance_feeds,
    )
    initial_nonces = {
        pair: int(state["progress"][0])
        for pair, state in initial["priceFeeds"].items()
    }
    last_nonces = dict(initial_nonces)
    last_prices = {
        pair: int(state["latestPrice"][2])
        for pair, state in initial["priceFeeds"].items()
    }
    observed_price_changes = {pair: 0 for pair in initial_nonces}
    started = time.monotonic()
    deadline = started + settings.duration_seconds
    last_price_advances = {pair: started for pair in initial_nonces}
    max_price_gaps = {pair: 0.0 for pair in initial_nonces}
    last_height_change = {node_id: started for node_id in cluster.nodes}
    previous_heights = dict(initial["heights"])
    restart_index = 0
    restart_recoveries = []
    restart_epoch_guard_deferrals = 0
    samples = 0
    final = initial

    while time.monotonic() < deadline:
        now = time.monotonic()
        if (
            restart_index < len(settings.restart_schedule_seconds)
            and now - started
            >= settings.restart_schedule_seconds[restart_index]
        ):
            epoch_rpc = next(iter(cluster.nodes.values())).w3
            if _restart_window_is_safe(epoch_rpc):
                scheduled_after = settings.restart_schedule_seconds[
                    restart_index
                ]
                recovery_seconds = await _restart_validator(
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
                    settings.restart_rpc_timeout_seconds,
                )
                restart_recoveries.append(
                    {
                        "sequence": restart_index + 1,
                        "scheduledAfterSeconds": scheduled_after,
                        "actualAfterSeconds": round(
                            time.monotonic() - started, 3
                        ),
                        "recoverySeconds": round(recovery_seconds, 3),
                    }
                )
                restart_index += 1
            else:
                restart_epoch_guard_deferrals += 1
                if restart_epoch_guard_deferrals == 1 or (
                    restart_epoch_guard_deferrals - 1
                ) % 20 == 0:
                    LOG.info(
                        "deferring validator restart %d/%d outside the "
                        "epoch transition guard",
                        restart_index + 1,
                        len(settings.restart_schedule_seconds),
                    )

        final = await _replicated_snapshot(
            cluster,
            native_artifact,
            price_artifact,
            price_resolver_address,
            binance_feeds,
        )
        now = time.monotonic()
        price_heartbeats = {}
        for feed in binance_feeds:
            pair = feed["pair"]
            state = final["priceFeeds"][pair]
            nonce = int(state["progress"][0])
            price = int(state["latestPrice"][2])
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
                "round": int(state["latestPrice"][0]),
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

        samples += 1
        heartbeat = {
            "elapsedSeconds": round(now - started, 3),
            "gravityBlock": int(final["block"]),
            "gravityBlockHash": final["blockHash"],
            "nodeHeights": final["heights"],
            "priceFeeds": price_heartbeats,
            "restartCompleted": bool(settings.restart_schedule_seconds)
            and restart_index == len(settings.restart_schedule_seconds),
            "restartCount": restart_index,
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
        price_resolver_address,
        binance_feeds,
    )
    if settings.restart_schedule_seconds:
        assert restart_index == len(settings.restart_schedule_seconds), (
            f"only {restart_index}/{len(settings.restart_schedule_seconds)} "
            "configured validator restarts ran"
        )

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
        if settings.restart_schedule_seconds:
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
        assert int(state["latestPrice"][2]) == expected_price
        price_summaries[pair] = {
            "feedId": int(feed["feedId"]),
            "initialNonce": initial_nonces[pair],
            "finalNonce": final_nonce,
            "advances": advances,
            "minimumAdvances": settings.minimum_advances,
            "finalPrice": str(state["latestPrice"][2]),
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
            settings.restart_node
            if settings.restart_schedule_seconds
            else None
        ),
        "restartScheduleSeconds": list(settings.restart_schedule_seconds),
        "restartRpcTimeoutSeconds": settings.restart_rpc_timeout_seconds,
        "restartRecoverySeconds": (
            restart_recoveries[0]["recoverySeconds"]
            if len(restart_recoveries) == 1
            else None
        ),
        "restartRecoveries": restart_recoveries,
        "restartEpochGuardDeferrals": restart_epoch_guard_deferrals,
    }


def _write_summary(payload: dict) -> None:
    path = SUITE_DIR / "artifacts" / _SUMMARY_FILE
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")


@pytest.mark.asyncio
async def test_governance_activated_price_feeds_soak_for_configured_duration(
    cluster: Cluster,
):
    settings = _soak_settings()
    metadata = _metadata()
    assert set(metadata) == {"binanceFeeds"}
    binance_feeds = metadata["binanceFeeds"]
    assert [feed["pair"] for feed in binance_feeds] == [
        "NVDAUSDT",
        "BTCUSDT",
        "ETHUSDT",
    ]
    assert len({int(feed["feedId"]) for feed in binance_feeds}) == len(
        binance_feeds
    )
    relayer_config = json.loads(
        (SUITE_DIR / "artifacts" / "relayer_config.live.json").read_text()
    )
    expected_uris = {feed["taskUri"] for feed in binance_feeds}
    assert set(relayer_config["uri_mappings"]) == expected_uris
    assert all(uri.startswith("gravity://3/") for uri in expected_uris)
    with (SUITE_DIR / "genesis.toml").open("rb") as genesis_file:
        oracle_config = tomllib.load(genesis_file)["genesis"]["oracle_config"]
    assert oracle_config["source_types"] == [1, 3]
    assert len(oracle_config["callbacks"]) == 2

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
        ("NativeOracle.sol", "NativeOracle"),
        ("OracleTaskConfig.sol", "OracleTaskConfig"),
        ("EpochConfig.sol", "EpochConfig"),
    ]
    contracts_out = support.ensure_contract_artifacts(SUITE_DIR, required)
    price_artifact = support.load_artifact(
        contracts_out, "PriceFeedResolver.sol", "PriceFeedResolver"
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
    receipt = await support.execute_governance_proposal(
        w3,
        support.faucet_voting_pool(w3),
        targets,
        datas,
        "activate-live-binance-price-soak",
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
        f"gravity://3/{int(feed['feedId'])}" for feed in binance_feeds
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

    for feed in binance_feeds:
        price_progress, initial_price = initial_prices[feed["pair"]]
        first_bucket = _round_start_ms(
            int(feed["bucketStartMs"]), int(price_progress[0])
        )
        expected_initial_price = await _fetch_binance_price_with_retry(
            feed["baseUrl"], feed["pair"], first_bucket
        )
        assert int(initial_price[2]) == expected_initial_price
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
            price_resolver.address,
            binance_feeds,
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
                "restartNode": (
                    settings.restart_node
                    if settings.restart_schedule_seconds
                    else None
                ),
                "restartScheduleSeconds": list(
                    settings.restart_schedule_seconds
                ),
                "binancePairs": [
                    feed["pair"] for feed in binance_feeds
                ],
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
    summary.update(
        {
            "activationEpoch": activation_epoch,
            "governanceBlock": receipt["blockNumber"],
            "priceCallbackEvents": price_delivery_counts,
            "soakEpochIntervalSeconds": (
                SOAK_EPOCH_INTERVAL_MICROS // 1_000_000
            ),
        }
    )
    _write_summary(summary)
    LOG.info("Oracle live soak passed: %s", summary)
