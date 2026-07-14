"""Polymarket three-way match mirror E2E test."""

import asyncio
import json
import logging
import os
import secrets
import time
import urllib.request
from pathlib import Path

import pytest
from eth_account import Account
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils import oracle_test_support as support
from gravity_e2e.utils.mock_polymarket_polygon import MATCH_MARKET_ID

LOG = logging.getLogger(__name__)
SUITE_DIR = Path(__file__).resolve().parent

SOURCE_TYPE_POLYMARKET_SETTLEMENT = 6
POLYGON_CHAIN_ID = 137
MATCH_OUTCOME_COUNT = 3
SETTLEMENT_KIND_CTF_CONDITION_RESOLUTION = 1
USER_STARTING_BALANCE = 1_000 * support.STAKE_UNIT
BET_AMOUNTS = [100 * support.STAKE_UNIT, 200 * support.STAKE_UNIT, 300 * support.STAKE_UNIT]
TOTAL_POOL = sum(BET_AMOUNTS)


def _mock_set_winning_slot(rpc_url: str, winning_slot: int) -> dict:
    payload = json.dumps(
        {
            "jsonrpc": "2.0",
            "method": "mock_setWinningSlot",
            "params": [winning_slot],
            "id": 1,
        }
    ).encode()
    request = urllib.request.Request(
        rpc_url, data=payload, headers={"Content-Type": "application/json"}
    )
    with urllib.request.urlopen(request, timeout=5) as response:
        body = json.loads(response.read())
    if "error" in body:
        raise RuntimeError(f"mock_setWinningSlot failed: {body['error']}")
    return body["result"]


async def _poll_data_recorded(w3: Web3, timeout: int = 120):
    deadline = time.monotonic() + timeout
    params = {
        "fromBlock": 0,
        "toBlock": "latest",
        "address": support.NATIVE_ORACLE_ADDRESS,
        "topics": [
            support.DATA_RECORDED_TOPIC0,
            support.topic(SOURCE_TYPE_POLYMARKET_SETTLEMENT),
            support.topic(MATCH_MARKET_ID),
        ],
    }
    while time.monotonic() < deadline:
        logs = await asyncio.to_thread(w3.eth.get_logs, params)
        if logs:
            return logs
        await asyncio.sleep(2)
    return []


async def _wait_for_resolver_settlement(
    resolver, mirror_id: int, condition_id: bytes, timeout: int = 120
):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        settlement = resolver.functions.getSettlement(mirror_id, condition_id).call()
        if settlement[0]:
            return settlement
        await asyncio.sleep(2)
    raise TimeoutError("resolver settlement was not stored")


@pytest.mark.asyncio
async def test_polymarket_match_market_mock_resolves_random_score(cluster: Cluster):
    metadata_path = SUITE_DIR / "mock_polymarket_metadata.json"
    metadata = json.loads(metadata_path.read_text())

    assert await cluster.set_full_live(timeout=120), "Gravity node failed to become live"
    assert await cluster.check_block_increasing(timeout=60), "Gravity chain is not producing blocks"

    node = cluster.get_node("node1")
    assert node is not None, "node1 not found in cluster"
    w3 = node.w3
    assert w3 is not None and w3.is_connected(), "node1 web3 not connected"

    required = [
        ("PolymarketSettlementResolver.sol", "PolymarketSettlementResolver"),
        ("PolymarketMatchMarket.sol", "PolymarketMatchMarket"),
        ("MockGToken.sol", "MockGToken"),
        ("NativeOracle.sol", "NativeOracle"),
    ]
    contracts_out = support.ensure_contracts_out(SUITE_DIR, required, "Polymarket")
    resolver_artifact = support.load_artifact(
        contracts_out, "PolymarketSettlementResolver.sol", "PolymarketSettlementResolver"
    )
    market_artifact = support.load_artifact(
        contracts_out, "PolymarketMatchMarket.sol", "PolymarketMatchMarket"
    )
    token_artifact = support.load_artifact(contracts_out, "MockGToken.sol", "MockGToken")
    native_artifact = support.load_artifact(contracts_out, "NativeOracle.sol", "NativeOracle")

    resolver = support.deploy_contract(w3, resolver_artifact, support.FAUCET_KEY)
    market = support.deploy_contract(w3, market_artifact, support.FAUCET_KEY)
    token = support.deploy_contract(w3, token_artifact, support.FAUCET_KEY)
    native_oracle = w3.eth.contract(
        address=support.NATIVE_ORACLE_ADDRESS, abi=native_artifact["abi"]
    )

    condition_id = bytes.fromhex(metadata["condition_id"][2:])
    ctf = Web3.to_checksum_address(metadata["ctf"])
    pool_addr = support.pool0_voted_by_faucet(w3)

    now_ts = w3.eth.get_block("latest")["timestamp"]
    closes_at = now_ts + 120
    settlement_ref = (
        SOURCE_TYPE_POLYMARKET_SETTLEMENT,
        MATCH_MARKET_ID,
        condition_id,
        resolver.address,
        ctf,
        POLYGON_CHAIN_ID,
        MATCH_OUTCOME_COUNT,
        [0, 1, 2],
        0,
    )
    create_params = (
        Web3.keccak(text="Portugal vs Colombia random e2e match market"),
        now_ts,
        closes_at,
        now_ts + 300,
        token.address,
        settlement_ref,
    )

    setup_data = [
        support.function_calldata(
            native_oracle.functions.setCallback(
                SOURCE_TYPE_POLYMARKET_SETTLEMENT, MATCH_MARKET_ID, resolver.address
            )
        ),
        support.function_calldata(
            resolver.functions.registerMirror(
                MATCH_MARKET_ID,
                POLYGON_CHAIN_ID,
                ctf,
                condition_id,
                MATCH_OUTCOME_COUNT,
            )
        ),
        support.function_calldata(market.functions.createMarket(create_params)),
    ]
    receipt = await support.execute_governance_proposal(
        w3,
        pool_addr,
        [support.NATIVE_ORACLE_ADDRESS, resolver.address, market.address],
        setup_data,
        "polymarket-match-market-e2e-setup",
    )
    market_id = support.market_id_from_receipt(receipt)

    bettors = [Account.create(f"polymarket-bettor-{i}") for i in range(MATCH_OUTCOME_COUNT)]
    for account in bettors:
        support.send_tx(
            w3, account.address, b"", support.FAUCET_KEY, gas=21_000, value=support.STAKE_UNIT
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

    for outcome, (account, amount) in enumerate(zip(bettors, BET_AMOUNTS)):
        support.send_contract_tx(
            w3,
            market,
            market.functions.placeBet(market_id, outcome, amount),
            account.key,
            gas=1_500_000,
        )
    assert market.functions.getMarket(market_id).call()[8] == TOTAL_POOL

    await support.wait_for_chain_time(w3, closes_at)
    support.send_contract_tx(
        w3, market, market.functions.lockMarket(market_id), support.FAUCET_KEY
    )

    forced_slot = os.environ.get("POLYMARKET_MOCK_WINNING_SLOT")
    winning_slot = int(forced_slot) if forced_slot is not None else secrets.randbelow(MATCH_OUTCOME_COUNT)
    assert 0 <= winning_slot < MATCH_OUTCOME_COUNT
    release = _mock_set_winning_slot(metadata["rpc_url"], winning_slot)
    metadata.update(release)
    metadata_path.write_text(json.dumps(metadata, indent=2))

    logs = await _poll_data_recorded(w3, timeout=180)
    assert logs, "No Polymarket sourceType=6 DataRecorded event observed"
    assert support.topic_hex(logs[0]["topics"][1]) == support.topic(
        SOURCE_TYPE_POLYMARKET_SETTLEMENT
    )
    assert support.topic_hex(logs[0]["topics"][2]) == support.topic(MATCH_MARKET_ID)

    settlement = await _wait_for_resolver_settlement(
        resolver, MATCH_MARKET_ID, condition_id, timeout=60
    )
    assert settlement[0] is True
    assert settlement[2] == POLYGON_CHAIN_ID
    assert Web3.to_checksum_address(settlement[3]) == ctf
    assert settlement[6] == MATCH_OUTCOME_COUNT
    assert settlement[9] == SETTLEMENT_KIND_CTF_CONDITION_RESOLUTION
    assert resolver.functions.getPayoutNumerators(MATCH_MARKET_ID, condition_id).call() == release[
        "payout_numerators"
    ]

    support.send_contract_tx(
        w3, market, market.functions.settleMarket(market_id), support.FAUCET_KEY, gas=2_000_000
    )
    final_market = market.functions.getMarket(market_id).call()
    assert final_market[5] == 2
    assert final_market[7] == winning_slot

    winner = bettors[winning_slot]
    assert market.functions.claimable(market_id, winner.address).call() == TOTAL_POOL
    balance_before = token.functions.balanceOf(winner.address).call()
    support.send_contract_tx(
        w3, market, market.functions.claim(market_id), winner.key, gas=1_500_000
    )
    assert token.functions.balanceOf(winner.address).call() - balance_before == TOTAL_POOL
    assert market.functions.claimable(market_id, winner.address).call() == 0

    for outcome, account in enumerate(bettors):
        if outcome != winning_slot:
            assert market.functions.claimable(market_id, account.address).call() == 0

    LOG.info(
        "Polymarket match market resolved: marketId=%s winningSlot=%s totalPool=%s",
        market_id,
        winning_slot,
        TOTAL_POOL,
    )
