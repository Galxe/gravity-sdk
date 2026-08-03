"""Four-validator dynamic Polymarket mirror E2E against real Polygon data."""

import asyncio
import json
import logging
import time
from pathlib import Path

import pytest
from eth_account import Account
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils import oracle_test_support as support

LOG = logging.getLogger(__name__)
SUITE_DIR = Path(__file__).resolve().parent

SOURCE_TYPE_POLYMARKET_SETTLEMENT = 6
POLYGON_CHAIN_ID = 137
CTF_ADDRESS = Web3.to_checksum_address(
    "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045"
)
TASK_NAME = Web3.keccak(text="polymarket_settlement")
RECONFIGURATION_ADDRESS = Web3.to_checksum_address(
    "0x00000000000000000000000000000001625F2003"
)
SEL_CURRENT_EPOCH = Web3.keccak(text="currentEpoch()")[:4]
SEL_REMAINING_TIME = Web3.keccak(text="getRemainingTimeSeconds()")[:4]

EPOCH_TIMEOUT_SECONDS = 180
MIN_PROPOSAL_WINDOW_SECONDS = 40
POLL_TIMEOUT_SECONDS = 300
USER_STARTING_BALANCE = 1_000 * support.STAKE_UNIT
STAKE_BY_OUTCOME = [100 * support.STAKE_UNIT, 200 * support.STAKE_UNIT]
TOTAL_POOL = sum(STAKE_BY_OUTCOME)
QUORUM_VALIDATORS = 3


def _current_epoch(w3: Web3) -> int:
    return int.from_bytes(
        w3.eth.call(
            {"to": RECONFIGURATION_ADDRESS, "data": SEL_CURRENT_EPOCH}
        ),
        "big",
    )


def _remaining_epoch_seconds(w3: Web3) -> int:
    return int.from_bytes(
        w3.eth.call(
            {"to": RECONFIGURATION_ADDRESS, "data": SEL_REMAINING_TIME}
        ),
        "big",
    )


async def _wait_for_epoch_advance(
    cluster: Cluster, start_epoch: int
) -> int:
    deadline = time.monotonic() + EPOCH_TIMEOUT_SECONDS
    node1 = cluster.get_node("node1")
    while time.monotonic() < deadline:
        epoch = _current_epoch(node1.w3)
        if epoch > start_epoch:
            for node in cluster.nodes.values():
                while _current_epoch(node.w3) < epoch:
                    if time.monotonic() >= deadline:
                        raise TimeoutError(
                            f"{node.id} did not reach epoch {epoch}"
                        )
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
            "Only %ss remain in epoch %s; waiting for a clean proposal window",
            remaining,
            current,
        )
        await _wait_for_epoch_advance(cluster, current)
    raise TimeoutError("no epoch window was long enough for governance proposal")


def _oracle_delivered_logs(
    w3: Web3, mirror_id: int, from_block: int
) -> list:
    return w3.eth.get_logs(
        {
            "fromBlock": from_block,
            "toBlock": "latest",
            "address": support.NATIVE_ORACLE_ADDRESS,
            "topics": [
                support.ORACLE_DELIVERED_TOPIC0,
                support.topic(SOURCE_TYPE_POLYMARKET_SETTLEMENT),
                support.topic(mirror_id),
            ],
        }
    )


async def _wait_for_oracle_delivered(
    w3: Web3, mirror_id: int, from_block: int
) -> list:
    deadline = time.monotonic() + POLL_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        logs = await asyncio.to_thread(
            _oracle_delivered_logs, w3, mirror_id, from_block
        )
        if logs:
            return logs
        await asyncio.sleep(2)
    raise TimeoutError(f"no OracleDelivered event for mirror {mirror_id}")


async def _wait_for_settlement(
    resolver, mirror_id: int, condition_id: bytes
) -> tuple:
    deadline = time.monotonic() + POLL_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        settlement = resolver.functions.getSettlement(
            mirror_id, condition_id
        ).call()
        if settlement[0]:
            return tuple(settlement)
        await asyncio.sleep(2)
    raise TimeoutError(f"resolver did not store settlement for mirror {mirror_id}")


def _consensus_log(cluster: Cluster, node_id: str) -> Path:
    return cluster.base_dir / node_id / "consensus_log" / "validator.log"


async def _wait_for_consensus_evidence(
    cluster: Cluster, mirror_id: int
) -> None:
    issuer = f"gravity://6/{mirror_id}"
    missing_observers = set(cluster.nodes)
    certifiers = set()
    quorum_seen = False
    deadline = time.monotonic() + POLL_TIMEOUT_SECONDS

    while time.monotonic() < deadline:
        for node_id in cluster.nodes:
            path = _consensus_log(cluster, node_id)
            content = path.read_text(errors="replace") if path.exists() else ""
            lines = content.splitlines()
            if any(
                "JWKObserver spawned." in line and issuer in line
                for line in lines
            ):
                missing_observers.discard(node_id)
            if any(
                "Start certifying update." in line and issuer in line
                for line in lines
            ):
                certifiers.add(node_id)

            if any(
                "Peer vote aggregated." in line
                and issuer in line
                and (
                    "threshold_exceeded=true" in line
                    or '"threshold_exceeded":true' in line
                )
                for line in lines
            ):
                quorum_seen = True

        if (
            not missing_observers
            and len(certifiers) >= QUORUM_VALIDATORS
            and quorum_seen
        ):
            return
        await asyncio.sleep(2)

    raise AssertionError(
        "missing live Polymarket consensus evidence: "
        f"observers={sorted(missing_observers)} "
        f"certifiers={sorted(certifiers)} quorum={quorum_seen}"
    )


@pytest.mark.asyncio
async def test_four_validators_mirror_live_polymarket_settlement(
    cluster: Cluster,
):
    metadata = json.loads(
        (SUITE_DIR / "artifacts" / "polymarket_live_metadata.json").read_text()
    )
    mirror_id = int(metadata["mirrorId"])
    condition_id = bytes.fromhex(metadata["conditionId"][2:])
    winning_slot = int(metadata["winningSlot"])

    assert len(cluster.nodes) == 4
    assert await cluster.set_full_live(timeout=180)
    assert await cluster.check_block_increasing(timeout=60)
    active = await cluster.validator_list()
    assert {node.id for node in active.active} == set(cluster.nodes)

    node1 = cluster.get_node("node1")
    w3 = node1.w3
    required = [
        ("PolymarketSettlementResolver.sol", "PolymarketSettlementResolver"),
        ("PolymarketBinaryMarket.sol", "PolymarketBinaryMarket"),
        ("MockGToken.sol", "MockGToken"),
        ("NativeOracle.sol", "NativeOracle"),
        ("OracleTaskConfig.sol", "OracleTaskConfig"),
    ]
    contracts_out = support.ensure_contracts_out(SUITE_DIR, required, "Polymarket")
    resolver_artifact = support.load_artifact(
        contracts_out,
        "PolymarketSettlementResolver.sol",
        "PolymarketSettlementResolver",
    )
    market_artifact = support.load_artifact(
        contracts_out, "PolymarketBinaryMarket.sol", "PolymarketBinaryMarket"
    )
    token_artifact = support.load_artifact(
        contracts_out, "MockGToken.sol", "MockGToken"
    )
    native_artifact = support.load_artifact(
        contracts_out, "NativeOracle.sol", "NativeOracle"
    )
    task_artifact = support.load_artifact(
        contracts_out, "OracleTaskConfig.sol", "OracleTaskConfig"
    )

    resolver = support.deploy_contract(
        w3, resolver_artifact, support.FAUCET_KEY
    )
    market = support.deploy_contract(w3, market_artifact, support.FAUCET_KEY)
    token = support.deploy_contract(w3, token_artifact, support.FAUCET_KEY)
    native_oracle = w3.eth.contract(
        address=support.NATIVE_ORACLE_ADDRESS,
        abi=native_artifact["abi"],
    )
    task_config = w3.eth.contract(
        address=support.ORACLE_TASK_CONFIG_ADDRESS,
        abi=task_artifact["abi"],
    )
    pool_addr = support.pool0_voted_by_faucet(w3)

    assert not task_config.functions.hasTask(
        SOURCE_TYPE_POLYMARKET_SETTLEMENT, mirror_id, TASK_NAME
    ).call()
    setup_epoch = await _wait_for_proposal_window(cluster)
    now_ts = w3.eth.get_block("latest")["timestamp"]
    closes_at = now_ts + 55
    settlement_ref = (
        SOURCE_TYPE_POLYMARKET_SETTLEMENT,
        mirror_id,
        condition_id,
        resolver.address,
        CTF_ADDRESS,
        POLYGON_CHAIN_ID,
        2,
        [0, 1],
        0,
    )
    create_params = (
        Web3.keccak(text=str(metadata["question"])),
        now_ts,
        closes_at,
        token.address,
        settlement_ref,
    )
    targets = [
        support.ORACLE_TASK_CONFIG_ADDRESS,
        support.NATIVE_ORACLE_ADDRESS,
        resolver.address,
        market.address,
    ]
    datas = [
        support.function_calldata(
            task_config.functions.setTask(
                SOURCE_TYPE_POLYMARKET_SETTLEMENT,
                mirror_id,
                TASK_NAME,
                metadata["taskUri"].encode(),
            )
        ),
        support.function_calldata(
            native_oracle.functions.setCallback(
                SOURCE_TYPE_POLYMARKET_SETTLEMENT,
                mirror_id,
                resolver.address,
            )
        ),
        support.function_calldata(
            resolver.functions.registerMirror(
                mirror_id,
                POLYGON_CHAIN_ID,
                CTF_ADDRESS,
                condition_id,
                2,
            )
        ),
        support.function_calldata(market.functions.createMarket(create_params)),
    ]
    receipt = await support.execute_governance_proposal(
        w3,
        pool_addr,
        targets,
        datas,
        "live-polymarket-dynamic-mirror",
        gas=8_000_000,
    )
    market_id = support.market_id_from_receipt(receipt)

    assert _current_epoch(w3) == setup_epoch
    assert task_config.functions.hasTask(
        SOURCE_TYPE_POLYMARKET_SETTLEMENT, mirror_id, TASK_NAME
    ).call()
    assert (
        native_oracle.functions.getLatestNonce(
            SOURCE_TYPE_POLYMARKET_SETTLEMENT, mirror_id
        ).call()
        == 0
    )

    bettors = [
        Account.create("live-polymarket-outcome-0"),
        Account.create("live-polymarket-outcome-1"),
    ]
    for outcome, account in enumerate(bettors):
        support.send_tx(
            w3,
            account.address,
            b"",
            support.FAUCET_KEY,
            gas=21_000,
            value=support.STAKE_UNIT,
        )
        support.send_contract_tx(
            w3,
            token,
            token.functions.mint(account.address, USER_STARTING_BALANCE),
            support.FAUCET_KEY,
        )
        support.send_contract_tx(
            w3,
            token,
            token.functions.approve(market.address, USER_STARTING_BALANCE),
            account.key,
        )
        support.send_contract_tx(
            w3,
            market,
            market.functions.placeBet(
                market_id, outcome, STAKE_BY_OUTCOME[outcome]
            ),
            account.key,
            gas=1_500_000,
        )
    assert market.functions.getMarket(market_id).call()[6] == TOTAL_POOL

    activation_epoch = await _wait_for_epoch_advance(cluster, setup_epoch)
    assert activation_epoch == setup_epoch + 1
    logs = await _wait_for_oracle_delivered(
        w3, mirror_id, receipt["blockNumber"]
    )
    assert len(logs) == 1
    progress = await support.wait_for_source_progress(
        native_oracle,
        SOURCE_TYPE_POLYMARKET_SETTLEMENT,
        mirror_id,
        1,
        timeout=POLL_TIMEOUT_SECONDS,
    )
    assert progress == (1, int(metadata["blockNumber"]))

    settlement = await _wait_for_settlement(
        resolver, mirror_id, condition_id
    )
    assert settlement[1] == 1
    assert settlement[2] == POLYGON_CHAIN_ID
    assert Web3.to_checksum_address(settlement[3]) == CTF_ADDRESS
    assert Web3.to_checksum_address(settlement[4]) == Web3.to_checksum_address(
        metadata["oracle"]
    )
    assert Web3.to_hex(settlement[5]).lower() == metadata["questionId"].lower()
    assert settlement[6] == 2
    assert (
        Web3.to_hex(settlement[7]).lower()
        == metadata["transactionHash"].lower()
    )
    assert settlement[8] == metadata["logIndex"]
    assert resolver.functions.getPayoutNumerators(
        mirror_id, condition_id
    ).call() == metadata["payoutNumerators"]

    await _wait_for_consensus_evidence(cluster, mirror_id)
    for node_id, node in cluster.nodes.items():
        replica = node.w3.eth.contract(
            address=resolver.address, abi=resolver_artifact["abi"]
        )
        assert tuple(
            replica.functions.getSettlement(mirror_id, condition_id).call()
        ) == settlement, f"{node_id} resolver state diverged"

    await support.wait_for_chain_time(w3, closes_at)
    support.send_contract_tx(
        w3,
        market,
        market.functions.lockMarket(market_id),
        support.FAUCET_KEY,
    )
    support.send_contract_tx(
        w3,
        market,
        market.functions.finalizeMarket(market_id),
        support.FAUCET_KEY,
        gas=2_000_000,
    )
    finalized_market = market.functions.getMarket(market_id).call()
    assert finalized_market[4] == 2
    assert finalized_market[5] == winning_slot

    winner = bettors[winning_slot]
    assert market.functions.claimable(market_id, winner.address).call() == TOTAL_POOL
    balance_before = token.functions.balanceOf(winner.address).call()
    support.send_contract_tx(
        w3,
        market,
        market.functions.claim(market_id),
        winner.key,
        gas=1_500_000,
    )
    assert (
        token.functions.balanceOf(winner.address).call() - balance_before
        == TOTAL_POOL
    )

    LOG.info(
        "Live Polymarket mirror settled: mirrorId=%s sourceBlock=%s "
        "activationEpoch=%s winningSlot=%s",
        mirror_id,
        metadata["blockNumber"],
        activation_epoch,
        winning_slot,
    )
