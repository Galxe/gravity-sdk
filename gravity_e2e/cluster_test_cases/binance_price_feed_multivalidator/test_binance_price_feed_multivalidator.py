"""Four-validator E2E against live Binance closed index-price klines."""

import asyncio
from decimal import Decimal
import json
import logging
import os
from pathlib import Path
import time
from urllib.parse import urlencode
from urllib.request import urlopen

import pytest

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils import oracle_test_support as support

try:
    import tomllib
except ImportError:
    import tomli as tomllib


LOG = logging.getLogger(__name__)
SUITE_DIR = Path(__file__).resolve().parent
FEEDS = {
    support.NVDA_FEED_ID: "NVDAUSDT",
    support.TSLA_FEED_ID: "TSLAUSDT",
}
TARGET_NONCE = 1


def _bucket_start_ms() -> int:
    value = os.environ.get("BINANCE_PRICE_FEED_BUCKET_START_MS")
    assert value, "live bucket was not prepared by hooks.py"
    return int(value)


def _genesis_quorum() -> tuple[int, int, int]:
    config = tomllib.loads((SUITE_DIR / "genesis.toml").read_text())
    validators = config["genesis_validators"]
    powers = [int(validator["voting_power"]) for validator in validators]
    assert len(validators) == 4
    assert len(set(powers)) == 1 and powers[0] > 0
    total_power = sum(powers)
    return len(validators), total_power, total_power * 2 // 3 + 1


def _fetch_live_price(base_url: str, pair: str, bucket_start_ms: int) -> int:
    bucket_end_ms = bucket_start_ms + support.INTERVAL_MS - 1
    query = urlencode(
        {
            "pair": pair,
            "interval": "1m",
            "startTime": bucket_start_ms,
            "endTime": bucket_end_ms,
            "limit": 1,
        }
    )
    with urlopen(f"{base_url}/fapi/v1/indexPriceKlines?{query}", timeout=15) as response:
        rows = json.loads(response.read())

    assert len(rows) == 1, f"Binance returned {len(rows)} rows for {pair}"
    row = rows[0]
    assert row[0] == bucket_start_ms
    assert row[6] == bucket_end_ms
    scaled = Decimal(row[4]) * (10**support.DECIMALS)
    assert scaled == scaled.to_integral_value()
    return int(scaled)


def _consensus_log(cluster: Cluster, node_id: str) -> Path:
    return cluster.base_dir / node_id / "consensus_log" / "validator.log"


def _relayer_state(cluster: Cluster, node_id: str) -> Path:
    return cluster.base_dir / node_id / "data" / "reth" / "relayer_state.json"


def _line_matches(content: str, marker: str, issuer_prefix: str) -> bool:
    return any(
        marker in line and issuer_prefix in line
        for line in content.splitlines()
    )


async def _wait_for_consensus_evidence(
    cluster: Cluster,
    feed_ids: list[int],
    timeout: int = 180,
):
    missing_observations = {
        (node_id, feed_id)
        for node_id in cluster.nodes
        for feed_id in feed_ids
    }
    missing_quorums = set(feed_ids)
    deadline = time.monotonic() + timeout

    while time.monotonic() < deadline:
        logs = {}
        for node_id in cluster.nodes:
            path = _consensus_log(cluster, node_id)
            logs[node_id] = path.read_text(errors="replace") if path.exists() else ""

        for node_id, feed_id in list(missing_observations):
            if _line_matches(
                logs[node_id],
                "Start certifying update.",
                f"gravity://3/{feed_id}",
            ):
                missing_observations.remove((node_id, feed_id))

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

        if not missing_observations and not missing_quorums:
            return
        await asyncio.sleep(2)

    raise AssertionError(
        "missing validator oracle evidence: "
        f"observations={sorted(missing_observations)}, "
        f"quorums={sorted(missing_quorums)}"
    )


def _assert_independent_relayer_state(
    cluster: Cluster,
    uris: dict[int, str],
):
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


@pytest.mark.asyncio
async def test_four_validators_certify_live_binance_prices(cluster: Cluster):
    assert support.is_live_mode(), "this suite must never use the local mock"
    assert len(cluster.nodes) == 4
    assert await cluster.set_full_live(timeout=120)
    assert await cluster.check_block_increasing(timeout=60)

    validator_count, total_power, quorum_power = _genesis_quorum()
    validator_set = await cluster.validator_list()
    assert {node.id for node in validator_set.active} == set(cluster.nodes)
    LOG.info(
        "Active validators=%s totalVotingPower=%s JWKQuorumPower=%s",
        validator_count,
        total_power,
        quorum_power,
    )

    node1 = cluster.get_node("node1")
    assert node1 is not None and node1.w3.is_connected()
    w3 = node1.w3

    required = [
        ("PriceFeedResolver.sol", "PriceFeedResolver"),
        ("NativeOracle.sol", "NativeOracle"),
        ("OracleTaskConfig.sol", "OracleTaskConfig"),
    ]
    contracts_out = support.ensure_contracts_out(SUITE_DIR, required, "Price feed")
    resolver_artifact = support.load_artifact(
        contracts_out, "PriceFeedResolver.sol", "PriceFeedResolver"
    )
    native_artifact = support.load_artifact(
        contracts_out, "NativeOracle.sol", "NativeOracle"
    )
    task_artifact = support.load_artifact(
        contracts_out, "OracleTaskConfig.sol", "OracleTaskConfig"
    )

    resolver = support.deploy_contract(w3, resolver_artifact, support.FAUCET_KEY)
    native_oracle = w3.eth.contract(
        address=support.NATIVE_ORACLE_ADDRESS,
        abi=native_artifact["abi"],
    )
    task_config = w3.eth.contract(
        address=support.ORACLE_TASK_CONFIG_ADDRESS,
        abi=task_artifact["abi"],
    )
    pool_addr = support.pool0_voted_by_faucet(w3)

    bucket_start_ms = _bucket_start_ms()
    base_url = support.binance_base_url().rstrip("/")
    expected_prices = {
        feed_id: await asyncio.to_thread(
            _fetch_live_price,
            base_url,
            pair,
            bucket_start_ms,
        )
        for feed_id, pair in FEEDS.items()
    }
    uris = {
        feed_id: support.price_feed_uri(feed_id, pair, bucket_start_ms)
        for feed_id, pair in FEEDS.items()
    }

    setup_data = [
        support.function_calldata(
            native_oracle.functions.setDefaultCallback(
                support.SOURCE_TYPE_PRICE_FEED,
                resolver.address,
            )
        )
    ]
    targets = [support.NATIVE_ORACLE_ADDRESS]
    for feed_id, uri in uris.items():
        targets.append(support.ORACLE_TASK_CONFIG_ADDRESS)
        setup_data.append(
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
        pool_addr,
        targets,
        setup_data,
        "live-binance-four-validator-e2e",
    )

    target_round_id = support.round_id(bucket_start_ms, TARGET_NONCE)
    target_resolved_at = support.round_end_ms(bucket_start_ms, TARGET_NONCE)
    node1_rounds = {}
    for feed_id in FEEDS:
        assert await support.poll_price_recorded(w3, feed_id, timeout=300)
        latest_nonce = await support.wait_for_latest_nonce(
            native_oracle,
            feed_id,
            TARGET_NONCE,
            timeout=300,
        )
        assert latest_nonce >= TARGET_NONCE
        stored = await support.wait_for_price_round(
            resolver,
            feed_id,
            target_round_id,
            timeout=180,
        )
        assert stored[0] is True
        assert stored[1] == target_round_id
        assert stored[2] == target_resolved_at
        assert stored[3] == support.DECIMALS
        assert stored[4] == expected_prices[feed_id]
        node1_rounds[feed_id] = tuple(stored)

    await _wait_for_consensus_evidence(cluster, list(FEEDS), timeout=180)
    _assert_independent_relayer_state(cluster, uris)

    for node_id, node in cluster.nodes.items():
        assert node.w3.is_connected(), f"{node_id} RPC disconnected"
        replica = node.w3.eth.contract(
            address=resolver.address,
            abi=resolver_artifact["abi"],
        )
        for feed_id, expected_round in node1_rounds.items():
            stored = await support.wait_for_price_round(
                replica,
                feed_id,
                target_round_id,
                timeout=60,
            )
            assert tuple(stored) == expected_round

    LOG.info(
        "Four-validator live Binance QC stored roundId=%s prices=%s",
        target_round_id,
        expected_prices,
    )
