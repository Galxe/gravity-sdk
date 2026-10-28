"""A stopped node catches up with Oracle QCs from both sides of a validator-set change."""

from __future__ import annotations

import asyncio
from pathlib import Path
import re
import time

import pytest
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils import oracle_test_support as support
from gravity_e2e.utils.mock_binance_index import (
    BUCKET_START_MS,
    DECIMALS,
    MOCK_BINANCE_PORT,
    mock_binance_index_kline_server,
)


SUITE_DIR = Path(__file__).resolve().parent
SOURCE_ID = 1005
RECONFIGURATION = Web3.to_checksum_address("0x00000000000000000000000000000001625F2003")
EPOCH_CONFIG = Web3.to_checksum_address("0x00000000000000000000000000000001625F1005")
VALIDATOR_MANAGER = Web3.to_checksum_address("0x00000000000000000000000000000001625F2001")
CURRENT_EPOCH = Web3.keccak(text="currentEpoch()")[:4]
ORACLE_DELIVERED = Web3.keccak(
    text="OracleDelivered(uint32,uint256,uint128,uint128,bytes32)"
)
JWK_EXECUTION = re.compile(r"jwk txn epoch (\d+), block number (\d+), data len (\d+)")


def _epoch(w3: Web3) -> int:
    result = w3.eth.call({"to": RECONFIGURATION, "data": CURRENT_EPOCH})
    return int.from_bytes(result, "big")


def _feed_uri() -> str:
    return (
        f"gravity://3/{SOURCE_ID}/price_feed?"
        "provider=binance_index_kline_v1&pair=NVDAUSDT&interval=1m&"
        f"bucketStartMs={BUCKET_START_MS}&decimals={DECIMALS}&graceMs=0"
    )


def _delivery_logs(w3: Web3, native, from_block: int):
    topic = lambda value: Web3.to_hex(value.to_bytes(32, "big"))
    return w3.eth.get_logs(
        {
            "fromBlock": from_block,
            "toBlock": "latest",
            "address": native.address,
            "topics": [
                Web3.to_hex(ORACLE_DELIVERED),
                topic(support.SOURCE_TYPE_PRICE_FEED),
                topic(SOURCE_ID),
            ],
        }
    )


async def _wait_for_delivery(w3: Web3, native, from_block: int, timeout: int):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        logs = _delivery_logs(w3, native, from_block)
        if logs:
            event = logs[0]
            receipt = w3.eth.get_transaction_receipt(event["transactionHash"])
            assert receipt["status"] == 1
            return int(event["blockNumber"]), event["transactionHash"]
        await asyncio.sleep(2)
    raise TimeoutError(f"source {SOURCE_ID} had no delivery after block {from_block}")


async def _wait_for_active_set(cluster: Cluster, expected: set[str], timeout: int):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        validators = await cluster.validator_list()
        active = {node.id for node in validators.active}
        if active == expected:
            return _epoch(cluster.get_node("node1").w3)
        await asyncio.sleep(2)
    raise TimeoutError(f"validator set did not become {sorted(expected)}")


async def _wait_for_height(w3: Web3, height: int, timeout: int):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if w3.eth.block_number >= height:
            return
        await asyncio.sleep(2)
    raise TimeoutError(f"node did not reach block {height}")


def _execution_epoch(cluster: Cluster, block_number: int) -> int:
    for node_id in ("node1", "node2"):
        log = cluster.base_dir / node_id / "consensus_log" / "validator.log"
        for match in JWK_EXECUTION.finditer(log.read_text(errors="replace")):
            if int(match.group(2)) == block_number and int(match.group(3)) > 0:
                return int(match.group(1))
    raise AssertionError(f"no verified Oracle execution at block {block_number}")


def _consensus_block_id(w3: Web3, block_number: int) -> str:
    successor = w3.eth.get_block(block_number + 1)
    block_id = bytes(successor["parentBeaconBlockRoot"]).hex()
    assert len(block_id) == 64, (block_number, block_id)
    return block_id


def _assert_recovery_replayed(cluster: Cluster, height: int, block_id: str) -> None:
    log = cluster.base_dir / "node4" / "consensus_log" / "validator.log"
    lines = log.read_text(errors="replace").splitlines()
    if any(
        f"recover block [id: {block_id[:8]}," in line
        and f"block_number: Some({height})" in line
        for line in lines
    ):
        return
    sync_lines = [
        line for line in lines
        if "Prepared forward epoch sync" in line or "Falling back to legacy reverse epoch sync" in line
    ]
    raise AssertionError(
        f"node4 did not replay old-epoch Oracle block {block_id} at height {height} "
        f"through forward BlockStore recovery; epoch sync: {sync_lines[-5:]}"
    )


def _snapshot(node, native_abi, resolver_abi, resolver_address, block_number, tx_hash):
    block = node.w3.eth.get_block(block_number)
    receipt = node.w3.eth.get_transaction_receipt(tx_hash)
    native = node.w3.eth.contract(address=support.NATIVE_ORACLE_ADDRESS, abi=native_abi)
    resolver = node.w3.eth.contract(address=resolver_address, abi=resolver_abi)
    progress = tuple(native.functions.getSourceProgress(
        support.SOURCE_TYPE_PRICE_FEED, SOURCE_ID
    ).call(block_identifier=block_number))
    price = tuple(resolver.functions.latestPrice(SOURCE_ID).call(
        block_identifier=block_number
    ))
    assert progress[0] > 0 and price[0] > 0 and price[1] == progress[1]
    return (
        bytes(block["hash"]),
        bytes(block["stateRoot"]),
        bytes(block["receiptsRoot"]),
        int(receipt["status"]),
        progress,
        price,
    )


@pytest.mark.asyncio
async def test_oracle_recovery_across_changed_validator_set(cluster: Cluster):
    assert len(cluster.nodes) == 4
    assert await cluster.set_full_live(timeout=180)
    assert await cluster.check_block_increasing(timeout=60)
    node1 = cluster.get_node("node1")
    node2 = cluster.get_node("node2")
    node3 = cluster.get_node("node3")
    node4 = cluster.get_node("node4")
    assert node1 is not None and node2 is not None
    assert node3 is not None and node4 is not None
    w3 = node1.w3
    validators = await cluster.validator_list()
    assert {node.id for node in validators.active} == set(cluster.nodes)
    assert node3.stake_pool is not None

    out = support.ensure_contract_artifacts(
        SUITE_DIR,
        [
            ("PriceFeedResolver.sol", "PriceFeedResolver"),
            ("NativeOracle.sol", "NativeOracle"),
            ("OracleTaskConfig.sol", "OracleTaskConfig"),
            ("EpochConfig.sol", "EpochConfig"),
            ("ValidatorManagement.sol", "ValidatorManagement"),
        ],
    )
    resolver_artifact = support.load_artifact(out, "PriceFeedResolver.sol", "PriceFeedResolver")
    native_artifact = support.load_artifact(out, "NativeOracle.sol", "NativeOracle")
    task_artifact = support.load_artifact(out, "OracleTaskConfig.sol", "OracleTaskConfig")
    epoch_artifact = support.load_artifact(out, "EpochConfig.sol", "EpochConfig")
    manager_artifact = support.load_artifact(out, "ValidatorManagement.sol", "ValidatorManagement")
    resolver = support.deploy_contract(w3, resolver_artifact)
    native = w3.eth.contract(address=support.NATIVE_ORACLE_ADDRESS, abi=native_artifact["abi"])
    task_config = w3.eth.contract(
        address=support.ORACLE_TASK_CONFIG_ADDRESS, abi=task_artifact["abi"]
    )
    epoch_config = w3.eth.contract(address=EPOCH_CONFIG, abi=epoch_artifact["abi"])
    validator_manager = w3.eth.contract(address=VALIDATOR_MANAGER, abi=manager_artifact["abi"])
    epoch_micros = int(epoch_config.functions.epochIntervalMicros().call())
    assert epoch_micros == 180_000_000
    epoch_timeout = 3 * ((epoch_micros + 999_999) // 1_000_000) + 60

    with mock_binance_index_kline_server(MOCK_BINANCE_PORT):
        registration = await support.execute_governance_proposal(
            w3,
            support.faucet_voting_pool(w3),
            [support.NATIVE_ORACLE_ADDRESS, support.ORACLE_TASK_CONFIG_ADDRESS],
            [
                support.function_calldata(native.functions.setCallback(
                    support.SOURCE_TYPE_PRICE_FEED, SOURCE_ID, resolver.address
                )),
                support.function_calldata(task_config.functions.setTask(
                    support.SOURCE_TYPE_PRICE_FEED,
                    SOURCE_ID,
                    support.TASK_PRICE_FEED,
                    _feed_uri().encode(),
                )),
            ],
            "oracle-cross-epoch-recovery-feed",
        )
        await _wait_for_delivery(
            w3, native, int(registration["blockNumber"]) + 1, epoch_timeout
        )

        node4_stopped = False
        try:
            node4_stopped = True
            assert await node4.stop(), "failed to stop node4 before Oracle delivery"
            offline_start_height = w3.eth.block_number
            before_height, before_tx = await _wait_for_delivery(
                w3, native, offline_start_height + 1, epoch_timeout
            )
            before_epoch = _execution_epoch(cluster, before_height)

            await support.execute_governance_proposal(
                w3,
                support.faucet_voting_pool(w3),
                [VALIDATOR_MANAGER],
                [support.function_calldata(
                    validator_manager.functions.forceLeaveValidatorSet(
                        Web3.to_checksum_address(node3.stake_pool)
                    )
                )],
                "oracle-cross-epoch-change-validator-set",
            )
            after_epoch = await _wait_for_active_set(
                cluster, {"node1", "node2", "node4"}, epoch_timeout
            )
            assert after_epoch > before_epoch
            assert await cluster.check_block_increasing("node1", timeout=60)
            assert await cluster.check_block_increasing("node2", timeout=60)
            changed_set_height = w3.eth.block_number
            after_height, after_tx = await _wait_for_delivery(
                w3, native, changed_set_height + 1, epoch_timeout
            )
            assert before_height < after_height
            assert _execution_epoch(cluster, after_height) >= after_epoch

            await _wait_for_height(w3, after_height + 1, 60)
            before_block_id = _consensus_block_id(w3, before_height)
            peer_tip = w3.eth.block_number
            assert await node4.start(rpc_timeout=180), "node4 did not restart after epoch change"
            node4_stopped = False
            await _wait_for_height(node4.w3, peer_tip, epoch_timeout)
            _assert_recovery_replayed(cluster, before_height, before_block_id)

            for height, tx_hash in ((before_height, before_tx), (after_height, after_tx)):
                snapshots = {
                    node_id: _snapshot(
                        cluster.nodes[node_id], native_artifact["abi"], resolver_artifact["abi"],
                        resolver.address, height, tx_hash
                    )
                    for node_id in ("node1", "node2", "node4")
                }
                assert len(set(snapshots.values())) == 1, (height, snapshots)
                assert snapshots["node4"][3] == 1
            before_progress = native.functions.getSourceProgress(
                support.SOURCE_TYPE_PRICE_FEED, SOURCE_ID
            ).call(block_identifier=before_height)
            after_progress = native.functions.getSourceProgress(
                support.SOURCE_TYPE_PRICE_FEED, SOURCE_ID
            ).call(block_identifier=after_height)
            assert after_progress[0] > before_progress[0]
            for node_id in ("node1", "node2", "node4"):
                assert await cluster.check_block_increasing(node_id, timeout=90)
        finally:
            if node4_stopped:
                assert await node4.start(rpc_timeout=180), "failed to restore node4"
