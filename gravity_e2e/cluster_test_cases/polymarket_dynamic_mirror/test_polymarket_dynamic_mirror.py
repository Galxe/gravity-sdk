"""Dynamic Polymarket mirror lifecycle E2E."""

import asyncio
import json
import logging
import time
import urllib.request
from pathlib import Path

import pytest
from eth_account import Account
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils import oracle_test_support as support
from gravity_e2e.utils.mock_polymarket_polygon import (
    CTF_ADDRESS,
    DYNAMIC_BINARY_BLOCK,
    DYNAMIC_BINARY_CONDITION_ID,
    DYNAMIC_BINARY_LOG_INDEX,
    DYNAMIC_BINARY_MARKET_ID,
    DYNAMIC_BINARY_QUESTION_ID,
    DYNAMIC_BINARY_TX_HASH,
    FED_BINARY_BLOCK,
    FED_BINARY_CONDITION_ID,
    FED_BINARY_LOG_INDEX,
    FED_BINARY_MARKET_ID,
    FED_BINARY_QUESTION_ID,
    FED_BINARY_TX_HASH,
)

LOG = logging.getLogger(__name__)
SUITE_DIR = Path(__file__).resolve().parent

SOURCE_TYPE_POLYMARKET_SETTLEMENT = 6
POLYGON_CHAIN_ID = 137
TASK_NAME = Web3.keccak(text="polymarket_settlement")
RECONFIGURATION_ADDRESS = Web3.to_checksum_address(
    "0x00000000000000000000000000000001625F2003"
)
SEL_CURRENT_EPOCH = Web3.keccak(text="currentEpoch()")[:4]
SEL_REMAINING_TIME = Web3.keccak(text="getRemainingTimeSeconds()")[:4]

EPOCH_INTERVAL_SECONDS = 60
MIN_PROPOSAL_WINDOW_SECONDS = 40
EPOCH_TIMEOUT_SECONDS = EPOCH_INTERVAL_SECONDS * 2 + 30
POLL_TIMEOUT_SECONDS = 90
USER_STARTING_BALANCE = 1_000 * support.STAKE_UNIT
NO_STAKE = 100 * support.STAKE_UNIT
YES_STAKE = 200 * support.STAKE_UNIT
TOTAL_POOL = NO_STAKE + YES_STAKE


def _task_uri(mirror_id: int, condition_id: str) -> str:
    return (
        f"gravity://6/{mirror_id}/polymarket_settlement?"
        f"ctf={CTF_ADDRESS}&fromBlock=89222200&condition={condition_id}&"
        "maxBlocksPerPoll=20"
    )


def _mock_rpc(rpc_url: str, method: str, params=None):
    payload = json.dumps(
        {"jsonrpc": "2.0", "id": 1, "method": method, "params": params or []}
    ).encode()
    request = urllib.request.Request(
        rpc_url, data=payload, headers={"Content-Type": "application/json"}
    )
    with urllib.request.urlopen(request, timeout=5) as response:
        body = json.loads(response.read())
    if "error" in body:
        raise RuntimeError(f"{method} failed: {body['error']}")
    return body["result"]


def _current_epoch(w3: Web3) -> int:
    raw = w3.eth.call(
        {"to": RECONFIGURATION_ADDRESS, "data": SEL_CURRENT_EPOCH}
    )
    return int.from_bytes(raw, "big")


def _remaining_epoch_seconds(w3: Web3) -> int:
    raw = w3.eth.call(
        {"to": RECONFIGURATION_ADDRESS, "data": SEL_REMAINING_TIME}
    )
    return int.from_bytes(raw, "big")


async def _wait_for_epoch_advance(w3: Web3, start_epoch: int) -> int:
    deadline = time.monotonic() + EPOCH_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        epoch = _current_epoch(w3)
        if epoch > start_epoch:
            return epoch
        await asyncio.sleep(1)
    raise TimeoutError(f"chain did not advance past epoch {start_epoch}")


async def _wait_for_proposal_window(w3: Web3) -> int:
    deadline = time.monotonic() + EPOCH_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        remaining = _remaining_epoch_seconds(w3)
        if remaining >= MIN_PROPOSAL_WINDOW_SECONDS:
            return _current_epoch(w3)
        epoch = _current_epoch(w3)
        LOG.info(
            "Only %ss remain in epoch %s; waiting for a clean proposal window",
            remaining,
            epoch,
        )
        await _wait_for_epoch_advance(w3, epoch)
    raise TimeoutError("no epoch window was long enough for governance proposal")


def _mock_scanned_condition(requests: list[dict], condition_id: str) -> bool:
    expected = condition_id.lower()
    for request in requests:
        if request.get("method") != "eth_getLogs":
            continue
        params = request.get("params") or []
        filter_obj = params[0] if params else {}
        topics = filter_obj.get("topics") or []
        if any(isinstance(topic, str) and topic.lower() == expected for topic in topics):
            return True
    return False


async def _wait_for_mock_scan(rpc_url: str, condition_id: str) -> list[dict]:
    deadline = time.monotonic() + POLL_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        requests = await asyncio.to_thread(_mock_rpc, rpc_url, "mock_getRequests")
        if _mock_scanned_condition(requests, condition_id):
            return requests
        await asyncio.sleep(1)
    raise TimeoutError(f"observer never scanned condition {condition_id}")


def _data_recorded_logs(w3: Web3, mirror_id: int, from_block: int) -> list:
    return w3.eth.get_logs(
        {
            "fromBlock": from_block,
            "toBlock": "latest",
            "address": support.NATIVE_ORACLE_ADDRESS,
            "topics": [
                support.DATA_RECORDED_TOPIC0,
                support.topic(SOURCE_TYPE_POLYMARKET_SETTLEMENT),
                support.topic(mirror_id),
            ],
        }
    )


async def _wait_for_data_recorded(
    w3: Web3, mirror_id: int, from_block: int
) -> list:
    deadline = time.monotonic() + POLL_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        logs = await asyncio.to_thread(
            _data_recorded_logs, w3, mirror_id, from_block
        )
        if logs:
            return logs
        await asyncio.sleep(1)
    raise TimeoutError(f"no DataRecorded event for mirror {mirror_id}")


async def _wait_for_settlement(
    resolver, mirror_id: int, condition_id: bytes
) -> tuple:
    deadline = time.monotonic() + POLL_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        settlement = resolver.functions.getSettlement(
            mirror_id, condition_id
        ).call()
        if settlement[0]:
            return settlement
        await asyncio.sleep(1)
    raise TimeoutError(f"resolver did not store settlement for mirror {mirror_id}")


def _add_condition(
    rpc_url: str,
    *,
    condition_id: str,
    question_id: str,
    block_number: int,
    log_index: int,
    tx_hash: str,
    payouts: list[int],
    replace_existing: bool = False,
):
    return _mock_rpc(
        rpc_url,
        "mock_addCondition",
        [
            {
                "conditionId": condition_id,
                "questionId": question_id,
                "blockNumber": hex(block_number),
                "logIndex": hex(log_index),
                "txHash": tx_hash,
                "payoutNumerators": payouts,
                "replaceExisting": replace_existing,
            }
        ],
    )


def _market_params(
    w3: Web3,
    *,
    mirror_id: int,
    condition_id: bytes,
    resolver: str,
    token: str,
    spec: str,
    lifetime_seconds: int,
) -> tuple:
    now_ts = w3.eth.get_block("latest")["timestamp"]
    settlement_ref = (
        SOURCE_TYPE_POLYMARKET_SETTLEMENT,
        mirror_id,
        condition_id,
        resolver,
        Web3.to_checksum_address(CTF_ADDRESS),
        POLYGON_CHAIN_ID,
        2,
        [0, 1],
        0,
    )
    return (
        Web3.keccak(text=spec),
        now_ts,
        now_ts + lifetime_seconds,
        token,
        settlement_ref,
    )


async def _create_dynamic_mirror(
    w3: Web3,
    *,
    pool_addr: str,
    task_config,
    native_oracle,
    resolver,
    market,
    token,
    mirror_id: int,
    condition_id: bytes,
    condition_hex: str,
    description: str,
    lifetime_seconds: int,
) -> tuple[dict, int, str]:
    uri = _task_uri(mirror_id, condition_hex)
    params = _market_params(
        w3,
        mirror_id=mirror_id,
        condition_id=condition_id,
        resolver=resolver.address,
        token=token.address,
        spec=description,
        lifetime_seconds=lifetime_seconds,
    )
    datas = [
        support.function_calldata(
            task_config.functions.setTask(
                SOURCE_TYPE_POLYMARKET_SETTLEMENT,
                mirror_id,
                TASK_NAME,
                uri.encode(),
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
                Web3.to_checksum_address(CTF_ADDRESS),
                condition_id,
                2,
            )
        ),
        support.function_calldata(market.functions.createMarket(params)),
    ]
    receipt = await support.execute_governance_proposal(
        w3,
        pool_addr,
        [
            support.ORACLE_TASK_CONFIG_ADDRESS,
            support.NATIVE_ORACLE_ADDRESS,
            resolver.address,
            market.address,
        ],
        datas,
        description,
        gas=8_000_000,
    )
    return receipt, support.market_id_from_receipt(receipt), uri


@pytest.mark.asyncio
async def test_polymarket_mirrors_activate_on_successive_epochs(cluster: Cluster):
    metadata = json.loads((SUITE_DIR / "mock_polymarket_metadata.json").read_text())
    rpc_url = metadata["rpc_url"]

    assert await cluster.set_full_live(timeout=180), "Gravity node failed to become live"
    assert await cluster.check_block_increasing(timeout=60), (
        "Gravity chain is not producing blocks"
    )

    node = cluster.get_node("node1")
    assert node is not None, "node1 not found"
    w3 = node.w3
    assert w3 is not None and w3.is_connected(), "node1 web3 not connected"

    required = [
        ("PolymarketSettlementResolver.sol", "PolymarketSettlementResolver"),
        ("PolymarketBinaryMarket.sol", "PolymarketBinaryMarket"),
        ("MockGToken.sol", "MockGToken"),
        ("NativeOracle.sol", "NativeOracle"),
        ("OracleTaskConfig.sol", "OracleTaskConfig"),
    ]
    contracts_out = support.ensure_contracts_out(SUITE_DIR, required, "Polymarket")
    resolver = support.deploy_contract(
        w3,
        support.load_artifact(
            contracts_out,
            "PolymarketSettlementResolver.sol",
            "PolymarketSettlementResolver",
        ),
        support.FAUCET_KEY,
    )
    market = support.deploy_contract(
        w3,
        support.load_artifact(
            contracts_out, "PolymarketBinaryMarket.sol", "PolymarketBinaryMarket"
        ),
        support.FAUCET_KEY,
    )
    token = support.deploy_contract(
        w3,
        support.load_artifact(contracts_out, "MockGToken.sol", "MockGToken"),
        support.FAUCET_KEY,
    )
    native_oracle = w3.eth.contract(
        address=support.NATIVE_ORACLE_ADDRESS,
        abi=support.load_artifact(
            contracts_out, "NativeOracle.sol", "NativeOracle"
        )["abi"],
    )
    task_config = w3.eth.contract(
        address=support.ORACLE_TASK_CONFIG_ADDRESS,
        abi=support.load_artifact(
            contracts_out, "OracleTaskConfig.sol", "OracleTaskConfig"
        )["abi"],
    )
    pool_addr = support.pool0_voted_by_faucet(w3)

    first_condition = bytes.fromhex(FED_BINARY_CONDITION_ID[2:])
    second_condition = bytes.fromhex(DYNAMIC_BINARY_CONDITION_ID[2:])
    assert not task_config.functions.hasTask(
        SOURCE_TYPE_POLYMARKET_SETTLEMENT,
        FED_BINARY_MARKET_ID,
        TASK_NAME,
    ).call()
    assert not task_config.functions.hasTask(
        SOURCE_TYPE_POLYMARKET_SETTLEMENT,
        DYNAMIC_BINARY_MARKET_ID,
        TASK_NAME,
    ).call()

    _mock_rpc(rpc_url, "mock_clearRequests")
    first_epoch = await _wait_for_proposal_window(w3)
    first_receipt, first_market_id, first_uri = await _create_dynamic_mirror(
        w3,
        pool_addr=pool_addr,
        task_config=task_config,
        native_oracle=native_oracle,
        resolver=resolver,
        market=market,
        token=token,
        mirror_id=FED_BINARY_MARKET_ID,
        condition_id=first_condition,
        condition_hex=FED_BINARY_CONDITION_ID,
        description="dynamic-polymarket-mirror-one",
        lifetime_seconds=55,
    )

    assert _current_epoch(w3) == first_epoch, "governance execution crossed epoch"
    assert task_config.functions.hasTask(
        SOURCE_TYPE_POLYMARKET_SETTLEMENT,
        FED_BINARY_MARKET_ID,
        TASK_NAME,
    ).call()
    first_task = task_config.functions.getTask(
        SOURCE_TYPE_POLYMARKET_SETTLEMENT,
        FED_BINARY_MARKET_ID,
        TASK_NAME,
    ).call()
    assert bytes(first_task[0]).decode() == first_uri
    assert (
        native_oracle.functions.getLatestNonce(
            SOURCE_TYPE_POLYMARKET_SETTLEMENT, FED_BINARY_MARKET_ID
        ).call()
        == 0
    )
    assert not resolver.functions.getSettlement(
        FED_BINARY_MARKET_ID, first_condition
    ).call()[0]
    requests = _mock_rpc(rpc_url, "mock_getRequests")
    assert not _mock_scanned_condition(requests, FED_BINARY_CONDITION_ID), (
        "new task was polled before the next epoch"
    )

    no_account = Account.create("dynamic-polymarket-no")
    yes_account = Account.create("dynamic-polymarket-yes")
    for account in (no_account, yes_account):
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
        market.functions.placeBet(first_market_id, 0, NO_STAKE),
        no_account.key,
        gas=1_500_000,
    )
    support.send_contract_tx(
        w3,
        market,
        market.functions.placeBet(first_market_id, 1, YES_STAKE),
        yes_account.key,
        gas=1_500_000,
    )
    assert market.functions.getMarket(first_market_id).call()[6] == TOTAL_POOL

    activated_epoch = await _wait_for_epoch_advance(w3, first_epoch)
    assert activated_epoch == first_epoch + 1
    await _wait_for_mock_scan(rpc_url, FED_BINARY_CONDITION_ID)

    _add_condition(
        rpc_url,
        condition_id=FED_BINARY_CONDITION_ID,
        question_id=FED_BINARY_QUESTION_ID,
        block_number=FED_BINARY_BLOCK,
        log_index=FED_BINARY_LOG_INDEX,
        tx_hash=FED_BINARY_TX_HASH,
        payouts=[0, 1],
    )
    _mock_rpc(
        rpc_url,
        "mock_setHeads",
        [hex(FED_BINARY_BLOCK), hex(FED_BINARY_BLOCK - 1)],
    )
    await asyncio.sleep(12)
    assert (
        native_oracle.functions.getLatestNonce(
            SOURCE_TYPE_POLYMARKET_SETTLEMENT, FED_BINARY_MARKET_ID
        ).call()
        == 0
    ), "latest-visible but unfinalized settlement was submitted"

    _mock_rpc(
        rpc_url,
        "mock_setHeads",
        [hex(FED_BINARY_BLOCK), hex(FED_BINARY_BLOCK)],
    )
    first_logs = await _wait_for_data_recorded(
        w3, FED_BINARY_MARKET_ID, first_receipt["blockNumber"]
    )
    assert len(first_logs) == 1
    first_settlement = await _wait_for_settlement(
        resolver, FED_BINARY_MARKET_ID, first_condition
    )
    assert first_settlement[1] == 1
    assert first_settlement[2] == POLYGON_CHAIN_ID
    assert Web3.to_hex(first_settlement[7]).lower() == FED_BINARY_TX_HASH.lower()
    assert first_settlement[8] == FED_BINARY_LOG_INDEX
    assert resolver.functions.getPayoutNumerators(
        FED_BINARY_MARKET_ID, first_condition
    ).call() == [0, 1]

    _add_condition(
        rpc_url,
        condition_id=FED_BINARY_CONDITION_ID,
        question_id=FED_BINARY_QUESTION_ID,
        block_number=FED_BINARY_BLOCK,
        log_index=FED_BINARY_LOG_INDEX,
        tx_hash=FED_BINARY_TX_HASH,
        payouts=[0, 1],
    )
    await asyncio.sleep(12)
    assert (
        native_oracle.functions.getLatestNonce(
            SOURCE_TYPE_POLYMARKET_SETTLEMENT, FED_BINARY_MARKET_ID
        ).call()
        == 1
    )
    assert len(
        _data_recorded_logs(
            w3, FED_BINARY_MARKET_ID, first_receipt["blockNumber"]
        )
    ) == 1

    first_market = market.functions.getMarket(first_market_id).call()
    await support.wait_for_chain_time(w3, first_market[2])
    support.send_contract_tx(
        w3,
        market,
        market.functions.lockMarket(first_market_id),
        support.FAUCET_KEY,
    )
    support.send_contract_tx(
        w3,
        market,
        market.functions.finalizeMarket(first_market_id),
        support.FAUCET_KEY,
        gas=2_000_000,
    )
    finalized_market = market.functions.getMarket(first_market_id).call()
    assert finalized_market[4] == 2
    assert finalized_market[5] == 1
    assert market.functions.claimable(
        first_market_id, yes_account.address
    ).call() == TOTAL_POOL
    balance_before = token.functions.balanceOf(yes_account.address).call()
    support.send_contract_tx(
        w3,
        market,
        market.functions.claim(first_market_id),
        yes_account.key,
        gas=1_500_000,
    )
    assert (
        token.functions.balanceOf(yes_account.address).call() - balance_before
        == TOTAL_POOL
    )

    _mock_rpc(rpc_url, "mock_clearRequests")
    second_epoch = await _wait_for_proposal_window(w3)
    second_receipt, second_market_id, second_uri = await _create_dynamic_mirror(
        w3,
        pool_addr=pool_addr,
        task_config=task_config,
        native_oracle=native_oracle,
        resolver=resolver,
        market=market,
        token=token,
        mirror_id=DYNAMIC_BINARY_MARKET_ID,
        condition_id=second_condition,
        condition_hex=DYNAMIC_BINARY_CONDITION_ID,
        description="dynamic-polymarket-mirror-two",
        lifetime_seconds=180,
    )
    assert second_market_id == first_market_id + 1
    assert _current_epoch(w3) == second_epoch, "second proposal crossed epoch"
    second_task = task_config.functions.getTask(
        SOURCE_TYPE_POLYMARKET_SETTLEMENT,
        DYNAMIC_BINARY_MARKET_ID,
        TASK_NAME,
    ).call()
    assert bytes(second_task[0]).decode() == second_uri
    assert (
        native_oracle.functions.getLatestNonce(
            SOURCE_TYPE_POLYMARKET_SETTLEMENT, DYNAMIC_BINARY_MARKET_ID
        ).call()
        == 0
    )
    assert not _mock_scanned_condition(
        _mock_rpc(rpc_url, "mock_getRequests"), DYNAMIC_BINARY_CONDITION_ID
    ), "second task was polled before the next epoch"

    second_activated_epoch = await _wait_for_epoch_advance(w3, second_epoch)
    assert second_activated_epoch == second_epoch + 1
    await _wait_for_mock_scan(rpc_url, DYNAMIC_BINARY_CONDITION_ID)

    _add_condition(
        rpc_url,
        condition_id=DYNAMIC_BINARY_CONDITION_ID,
        question_id=DYNAMIC_BINARY_QUESTION_ID,
        block_number=DYNAMIC_BINARY_BLOCK,
        log_index=DYNAMIC_BINARY_LOG_INDEX,
        tx_hash=DYNAMIC_BINARY_TX_HASH,
        payouts=[1, 0],
    )
    _mock_rpc(
        rpc_url,
        "mock_setHeads",
        [hex(DYNAMIC_BINARY_BLOCK), hex(DYNAMIC_BINARY_BLOCK)],
    )
    second_logs = await _wait_for_data_recorded(
        w3, DYNAMIC_BINARY_MARKET_ID, second_receipt["blockNumber"]
    )
    assert len(second_logs) == 1
    second_settlement = await _wait_for_settlement(
        resolver, DYNAMIC_BINARY_MARKET_ID, second_condition
    )
    assert second_settlement[1] == 1
    assert Web3.to_hex(second_settlement[7]).lower() == DYNAMIC_BINARY_TX_HASH.lower()
    assert second_settlement[8] == DYNAMIC_BINARY_LOG_INDEX
    assert resolver.functions.getPayoutNumerators(
        DYNAMIC_BINARY_MARKET_ID, second_condition
    ).call() == [1, 0]
    assert (
        native_oracle.functions.getLatestNonce(
            SOURCE_TYPE_POLYMARKET_SETTLEMENT, FED_BINARY_MARKET_ID
        ).call()
        == 1
    ), "second mirror changed the first mirror nonce"

    LOG.info(
        "Dynamic mirrors settled across epochs: first=%s@%s second=%s@%s",
        FED_BINARY_MARKET_ID,
        activated_epoch,
        DYNAMIC_BINARY_MARKET_ID,
        second_activated_epoch,
    )
