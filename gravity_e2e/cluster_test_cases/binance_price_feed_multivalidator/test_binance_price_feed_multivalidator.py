"""Four-validator Binance closed index-kline oracle E2E."""

import asyncio
import json
import logging
from pathlib import Path
import time

import pytest

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils import oracle_test_support as support
from gravity_e2e.utils.mock_binance_index import (
    BUCKET_START_MS,
    DECIMALS,
    INTERVAL_MS,
    MOCK_BINANCE_PORT,
    mock_binance_index_kline_server,
    mock_scaled_price,
)

LOG = logging.getLogger(__name__)
SUITE_DIR = Path(__file__).resolve().parent
FEEDS = {1001: "NVDAUSDT", 1002: "TSLAUSDT"}
TARGET_NONCE = 1
QUORUM_VALIDATORS = 3


def _price_feed_uri(feed_id: int, pair: str) -> str:
    return (
        f"gravity://3/{feed_id}/price_feed?"
        f"provider=binance_index_kline_v1&pair={pair}&interval=1m&"
        f"bucketStartMs={BUCKET_START_MS}&decimals={DECIMALS}&graceMs=0"
    )


def _consensus_log(cluster: Cluster, node_id: str) -> Path:
    return cluster.base_dir / node_id / "consensus_log" / "validator.log"


def _relayer_state(cluster: Cluster, node_id: str) -> Path:
    return cluster.base_dir / node_id / "data" / "reth" / "relayer_state.json"


def _line_matches(content: str, marker: str, issuer: str) -> bool:
    return any(marker in line and issuer in line for line in content.splitlines())


async def _wait_for_consensus_evidence(
    cluster: Cluster,
    feed_ids: list[int],
    *,
    timeout: int = 180,
) -> None:
    missing_observers = {
        (node_id, feed_id)
        for node_id in cluster.nodes
        for feed_id in feed_ids
    }
    certifiers = {feed_id: set() for feed_id in feed_ids}
    missing_quorums = set(feed_ids)
    deadline = time.monotonic() + timeout

    while time.monotonic() < deadline:
        logs = {}
        for node_id in cluster.nodes:
            path = _consensus_log(cluster, node_id)
            logs[node_id] = path.read_text(errors="replace") if path.exists() else ""

        for node_id, feed_id in list(missing_observers):
            issuer = f"gravity://3/{feed_id}"
            if _line_matches(logs[node_id], "JWKObserver spawned.", issuer):
                missing_observers.remove((node_id, feed_id))

        for feed_id in feed_ids:
            issuer = f"gravity://3/{feed_id}"
            for node_id, content in logs.items():
                if _line_matches(content, "Start certifying update.", issuer):
                    certifiers[feed_id].add(node_id)

        for feed_id in list(missing_quorums):
            issuer = f"gravity://3/{feed_id}"
            if any(
                any(
                    "Peer vote aggregated." in line
                    and issuer in line
                    and (
                        "threshold_exceeded=true" in line
                        or '"threshold_exceeded":true' in line
                    )
                    for line in content.splitlines()
                )
                for content in logs.values()
            ):
                missing_quorums.remove(feed_id)

        if (
            not missing_observers
            and all(len(certifiers[feed_id]) >= QUORUM_VALIDATORS for feed_id in feed_ids)
            and not missing_quorums
        ):
            return
        await asyncio.sleep(2)

    raise AssertionError(
        "missing validator oracle evidence: "
        f"observers={sorted(missing_observers)}, "
        f"certifiers={ {feed: sorted(nodes) for feed, nodes in certifiers.items()} }, "
        f"quorums={sorted(missing_quorums)}"
    )


def _assert_relayer_state(cluster: Cluster, uris: dict[int, str]) -> None:
    for node_id in cluster.nodes:
        state_path = _relayer_state(cluster, node_id)
        assert state_path.exists(), f"{node_id} has no relayer state"
        state = json.loads(state_path.read_text())
        for feed_id, uri in uris.items():
            source = state["sources"].get(uri)
            assert source is not None, f"{node_id} has no state for feedId={feed_id}"
            assert source["source_type"] == support.SOURCE_TYPE_PRICE_FEED
            assert source["source_id"] == feed_id
            assert int(source["last_nonce"]) >= TARGET_NONCE


async def _wait_for_block(w3, block_number: int, timeout: int = 60) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if w3.eth.block_number >= block_number:
            return
        await asyncio.sleep(1)
    raise TimeoutError(f"RPC did not reach Gravity block {block_number}")


@pytest.mark.asyncio
async def test_four_validators_resolve_deterministic_binance_prices(cluster: Cluster):
    assert len(cluster.nodes) == 4
    assert await cluster.set_full_live(timeout=180)
    assert await cluster.check_block_increasing(timeout=60)

    validator_set = await cluster.validator_list()
    assert {validator.id for validator in validator_set.active} == set(cluster.nodes)

    node1 = cluster.get_node("node1")
    assert node1 is not None and node1.w3.is_connected()
    w3 = node1.w3

    required = [
        ("PriceFeedResolver.sol", "PriceFeedResolver"),
        ("NativeOracle.sol", "NativeOracle"),
        ("OracleTaskConfig.sol", "OracleTaskConfig"),
    ]
    contracts_out = support.ensure_contract_artifacts(SUITE_DIR, required)
    resolver_artifact = support.load_artifact(
        contracts_out, "PriceFeedResolver.sol", "PriceFeedResolver"
    )
    native_artifact = support.load_artifact(
        contracts_out, "NativeOracle.sol", "NativeOracle"
    )
    task_artifact = support.load_artifact(
        contracts_out, "OracleTaskConfig.sol", "OracleTaskConfig"
    )

    resolver = support.deploy_contract(w3, resolver_artifact)
    native_oracle = w3.eth.contract(
        address=support.NATIVE_ORACLE_ADDRESS,
        abi=native_artifact["abi"],
    )
    task_config = w3.eth.contract(
        address=support.ORACLE_TASK_CONFIG_ADDRESS,
        abi=task_artifact["abi"],
    )
    uris = {feed_id: _price_feed_uri(feed_id, pair) for feed_id, pair in FEEDS.items()}

    with mock_binance_index_kline_server(MOCK_BINANCE_PORT) as mock:
        assert mock.base_url == f"http://127.0.0.1:{MOCK_BINANCE_PORT}"
        targets = [support.NATIVE_ORACLE_ADDRESS]
        calls = [
            support.function_calldata(
                native_oracle.functions.setDefaultCallback(
                    support.SOURCE_TYPE_PRICE_FEED,
                    resolver.address,
                )
            )
        ]
        for feed_id, uri in uris.items():
            targets.append(support.ORACLE_TASK_CONFIG_ADDRESS)
            calls.append(
                support.function_calldata(
                    task_config.functions.setTask(
                        support.SOURCE_TYPE_PRICE_FEED,
                        feed_id,
                        support.TASK_PRICE_FEED,
                        uri.encode(),
                    )
                )
            )

        await support.execute_governance_proposal(
            w3,
            support.faucet_voting_pool(w3),
            targets,
            calls,
            "deterministic-binance-multivalidator-e2e",
        )

        for feed_id in FEEDS:
            await support.wait_for_latest_price(
                native_oracle,
                resolver,
                feed_id,
                TARGET_NONCE,
            )

        await _wait_for_consensus_evidence(cluster, list(FEEDS))
        _assert_relayer_state(cluster, uris)
        for pair in FEEDS.values():
            assert mock.request_count(pair) >= QUORUM_VALIDATORS

        snapshot_block = w3.eth.block_number
        expected_progress = {}
        expected_rounds = {}
        for feed_id, pair in FEEDS.items():
            progress = tuple(
                native_oracle.functions.getSourceProgress(
                    support.SOURCE_TYPE_PRICE_FEED,
                    feed_id,
                ).call(block_identifier=snapshot_block)
            )
            latest = tuple(
                resolver.functions.latestPrice(feed_id).call(
                    block_identifier=snapshot_block
                )
            )
            delivery_nonce = int(progress[0])
            bucket_start = BUCKET_START_MS + (delivery_nonce - 1) * INTERVAL_MS
            assert latest == (
                True,
                bucket_start // INTERVAL_MS,
                bucket_start + INTERVAL_MS - 1,
                DECIMALS,
                mock_scaled_price(pair, delivery_nonce - 1),
            )
            assert progress[1] == latest[2]
            expected_progress[feed_id] = progress
            expected_rounds[feed_id] = latest

        for node_id, node in cluster.nodes.items():
            await _wait_for_block(node.w3, snapshot_block)
            replica_native = node.w3.eth.contract(
                address=support.NATIVE_ORACLE_ADDRESS,
                abi=native_artifact["abi"],
            )
            replica_resolver = node.w3.eth.contract(
                address=resolver.address,
                abi=resolver_artifact["abi"],
            )
            for feed_id in FEEDS:
                progress = tuple(
                    replica_native.functions.getSourceProgress(
                        support.SOURCE_TYPE_PRICE_FEED,
                        feed_id,
                    ).call(block_identifier=snapshot_block)
                )
                latest = tuple(
                    replica_resolver.functions.latestPrice(feed_id).call(
                        block_identifier=snapshot_block
                    )
                )
                assert progress == expected_progress[feed_id], node_id
                assert latest == expected_rounds[feed_id], node_id

        LOG.info(
            "Four-validator Binance QC stored rounds: %s",
            {feed_id: round_data[1] for feed_id, round_data in expected_rounds.items()},
        )
