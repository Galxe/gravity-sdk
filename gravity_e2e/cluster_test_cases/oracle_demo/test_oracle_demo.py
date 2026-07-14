"""Combined Binance price-feed and Polymarket mirror dashboard e2e."""

import asyncio
import json
import logging
import os
import time
import urllib.request
from pathlib import Path

import pytest
from eth_abi import encode
from eth_account import Account
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils import oracle_test_support as support
from gravity_e2e.utils.mock_polymarket_polygon import (
    CTF_ADDRESS,
    FED_BINARY_BLOCK,
    FED_BINARY_CONDITION_ID,
    FED_BINARY_LOG_INDEX,
    FED_BINARY_MARKET_ID,
    FED_BINARY_QUESTION_ID,
    FED_BINARY_TX_HASH,
    POLYGON_CHAIN_ID,
    UMA_ORACLE,
)

LOG = logging.getLogger(__name__)

SUITE_DIR = Path(__file__).resolve().parent

SOURCE_TYPE_POLYMARKET_SETTLEMENT = 6
SOURCE_TYPE_PRICE_FEED = 3
SETTLEMENT_KIND_CTF_CONDITION_RESOLUTION = 1
CALLBACK_GAS_LIMIT = 2_000_000
STAKE_UNIT = 10**18
USER_STARTING_BALANCE = 1_000 * STAKE_UNIT
NO_STAKE = 100 * STAKE_UNIT
YES_STAKE = 200 * STAKE_UNIT
TOTAL_BINARY_POOL = NO_STAKE + YES_STAKE
SPEC_HASH = Web3.keccak(text="Fed July binary Polymarket mirror demo")

NATIVE_ORACLE_ADDRESS = support.NATIVE_ORACLE_ADDRESS
ORACLE_TASK_CONFIG_ADDRESS = support.ORACLE_TASK_CONFIG_ADDRESS
FAUCET_KEY = support.FAUCET_KEY


def _ensure_contracts_out() -> Path:
    required = [
        ("PriceFeedResolver.sol", "PriceFeedResolver"),
        ("OracleTaskConfig.sol", "OracleTaskConfig"),
        ("NativeOracle.sol", "NativeOracle"),
        ("PolymarketSettlementResolver.sol", "PolymarketSettlementResolver"),
        ("PolymarketBinaryMarket.sol", "PolymarketBinaryMarket"),
        ("MockGToken.sol", "MockGToken"),
    ]
    return support.ensure_contracts_out(SUITE_DIR, required, "Oracle demo")


def _mock_set_binary_winning_slot(rpc_url: str, winning_slot: int) -> dict:
    payload = json.dumps(
        {
            "jsonrpc": "2.0",
            "method": "mock_setBinaryWinningSlot",
            "params": [winning_slot],
            "id": 1,
        }
    ).encode()
    req = urllib.request.Request(
        rpc_url,
        data=payload,
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=5) as resp:
        body = json.loads(resp.read())
    if "error" in body:
        raise RuntimeError(f"mock_setBinaryWinningSlot failed: {body['error']}")
    return body["result"]


async def _poll_polymarket_data_recorded(w3: Web3, timeout: int = 180):
    deadline = time.monotonic() + timeout
    filter_params = {
        "fromBlock": 0,
        "toBlock": "latest",
        "address": NATIVE_ORACLE_ADDRESS,
        "topics": [
            support.DATA_RECORDED_TOPIC0,
            support.topic(SOURCE_TYPE_POLYMARKET_SETTLEMENT),
            support.topic(FED_BINARY_MARKET_ID),
        ],
    }
    while time.monotonic() < deadline:
        logs = await asyncio.to_thread(w3.eth.get_logs, filter_params)
        if logs:
            return logs
        await asyncio.sleep(2)
    return []


async def _wait_for_resolver_settlement(resolver, timeout: int = 120):
    condition_id = bytes.fromhex(FED_BINARY_CONDITION_ID[2:])
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        settlement = resolver.functions.getSettlement(FED_BINARY_MARKET_ID, condition_id).call()
        if settlement[0]:
            return settlement
        await asyncio.sleep(2)
    raise TimeoutError("binary Polymarket resolver settlement was not stored")


def _send_contract_tx(w3: Web3, contract, fn, sender_key: str, gas: int = 1_000_000) -> dict:
    return support.send_contract_tx(w3, contract, fn, sender_key, gas=gas)


def _format_price(value: int) -> str:
    return support.format_price(value)


def _write_demo_config(
    cluster: Cluster,
    chain_id: int,
    price_resolver: str,
    poly_resolver: str,
    binary_market: str,
    token: str,
    market_id: int,
    market_state: tuple,
    release: dict,
    observed_rounds: dict[int, tuple],
    target_round_id: int,
    target_resolved_at: int,
    claimable_yes: int,
):
    output = os.environ.get("GRAVITY_DEMO_CONFIG_OUT")
    if not output:
        return

    def feed_config(feed_id: int, pair: str, label: str):
        observed = observed_rounds[feed_id]
        observed_price = int(observed[7])
        return {
            "feedId": feed_id,
            "label": label,
            "pair": pair,
            "sourceType": SOURCE_TYPE_PRICE_FEED,
            "decimals": support.DECIMALS,
            "expectedDeliveryNonce": support.TARGET_DELIVERY_NONCE,
            "expectedRoundId": target_round_id,
            "expectedResolvedAt": target_resolved_at,
            "expectedPrice": str(observed_price),
            "expectedDisplayPrice": _format_price(observed_price),
        }

    demo_config = {
        "version": 2,
        "mode": "local-combined-oracle-demo",
        "network": {
            "chainId": chain_id,
            "rpcUrl": cluster.get_node("node1").url,
            "rpcProxyPath": "/rpc",
        },
        "contracts": {
            "priceFeedResolver": price_resolver,
            "multiSourceOracleResolver": price_resolver,
            "polymarketSettlementResolver": poly_resolver,
            "polymarketBinaryMarket": binary_market,
            "collateral": token,
            "nativeOracle": NATIVE_ORACLE_ADDRESS,
            "oracleTaskConfig": ORACLE_TASK_CONFIG_ADDRESS,
        },
        "provider": {
            "kind": "local-mock",
            "name": "Local deterministic Binance index-kline mock",
            "baseUrl": f"http://127.0.0.1:{support.MOCK_BINANCE_PORT}",
            "endpoint": "/fapi/v1/indexPriceKlines",
            "interval": "1m",
            "live": False,
        },
        "feeds": [
            feed_config(support.NVDA_FEED_ID, "NVDAUSDT", "NVDA mock index"),
            feed_config(support.TSLA_FEED_ID, "TSLAUSDT", "TSLA mock index"),
        ],
        "polymarket": {
            "kind": "binary",
            "title": "Will the Fed cut rates in July?",
            "subtitle": "Local Polymarket CTF mirror fixture",
            "sourceType": SOURCE_TYPE_POLYMARKET_SETTLEMENT,
            "mirrorId": FED_BINARY_MARKET_ID,
            "marketId": market_id,
            "conditionId": FED_BINARY_CONDITION_ID,
            "questionId": FED_BINARY_QUESTION_ID,
            "ctf": CTF_ADDRESS,
            "polygonChainId": POLYGON_CHAIN_ID,
            "outcomeLabels": ["NO", "YES"],
            "slotLabels": ["YES", "NO"],
            "slotToOutcome": [1, 0],
            "winningSlot": release["winning_slot"],
            "winningOutcome": int(market_state[6]),
            "payoutNumerators": release["payout_numerators"],
            "settlementTxHash": FED_BINARY_TX_HASH,
            "settlementBlock": FED_BINARY_BLOCK,
            "settlementLogIndex": FED_BINARY_LOG_INDEX,
            "totalPool": str(market_state[7]),
            "yesAccount": {
                "address": release["yesAccount"],
                "claimable": str(claimable_yes),
            },
        },
        "demo": {
            "providerMode": "mock",
            "bucketStartMs": support.BUCKET_START_MS,
            "intervalMs": support.INTERVAL_MS,
            "targetDeliveryNonce": support.TARGET_DELIVERY_NONCE,
            "targetRoundId": target_round_id,
            "targetResolvedAt": target_resolved_at,
        },
    }

    output_path = Path(output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(demo_config, indent=2) + "\n")
    LOG.info("Wrote combined oracle demo config to %s", output_path)


@pytest.mark.asyncio
async def test_combined_oracle_demo_resolves_price_and_polymarket(cluster: Cluster):
    os.environ["BINANCE_PRICE_FEED_MODE"] = "mock"
    os.environ.pop("BINANCE_PRICE_FEED_BASE_URL", None)
    os.environ.pop("BINANCE_PRICE_FEED_BUCKET_START_MS", None)

    metadata_path = SUITE_DIR / "mock_polymarket_metadata.json"
    metadata = json.loads(metadata_path.read_text())

    LOG.info("Verifying gravity node is live")
    assert await cluster.set_full_live(timeout=120), "Gravity node failed to become live"
    assert await cluster.check_block_increasing(timeout=60), "Gravity chain is not producing blocks"

    node = cluster.get_node("node1")
    assert node is not None, "node1 not found in cluster"
    w3 = node.w3
    assert w3 is not None and w3.is_connected(), "node1 web3 not connected"

    contracts_out = _ensure_contracts_out()
    price_resolver_artifact = support.load_artifact(
        contracts_out,
        "PriceFeedResolver.sol",
        "PriceFeedResolver",
    )
    poly_resolver_artifact = support.load_artifact(
        contracts_out,
        "PolymarketSettlementResolver.sol",
        "PolymarketSettlementResolver",
    )
    binary_market_artifact = support.load_artifact(
        contracts_out,
        "PolymarketBinaryMarket.sol",
        "PolymarketBinaryMarket",
    )
    token_artifact = support.load_artifact(contracts_out, "MockGToken.sol", "MockGToken")
    native_artifact = support.load_artifact(contracts_out, "NativeOracle.sol", "NativeOracle")
    task_artifact = support.load_artifact(contracts_out, "OracleTaskConfig.sol", "OracleTaskConfig")

    price_resolver = support.deploy_contract(w3, price_resolver_artifact, FAUCET_KEY)
    poly_resolver = support.deploy_contract(w3, poly_resolver_artifact, FAUCET_KEY)
    binary_market = support.deploy_contract(w3, binary_market_artifact, FAUCET_KEY)
    token = support.deploy_contract(w3, token_artifact, FAUCET_KEY)
    native_oracle = w3.eth.contract(address=NATIVE_ORACLE_ADDRESS, abi=native_artifact["abi"])
    task_config = w3.eth.contract(address=ORACLE_TASK_CONFIG_ADDRESS, abi=task_artifact["abi"])

    pool_addr = support.pool0_voted_by_faucet(w3)
    condition_id = bytes.fromhex(FED_BINARY_CONDITION_ID[2:])
    ctf = Web3.to_checksum_address(CTF_ADDRESS)

    now_ts = w3.eth.get_block("latest")["timestamp"]
    closes_at = now_ts + 20
    oracle_deadline = now_ts + 180
    settlement_ref = (
        SOURCE_TYPE_POLYMARKET_SETTLEMENT,
        FED_BINARY_MARKET_ID,
        condition_id,
        poly_resolver.address,
        ctf,
        POLYGON_CHAIN_ID,
        2,
        [1, 0],
        0,
    )
    create_params = (
        SPEC_HASH,
        now_ts,
        closes_at,
        oracle_deadline,
        token.address,
        settlement_ref,
    )

    nvda_uri = support.price_feed_uri(support.NVDA_FEED_ID, "NVDAUSDT", support.BUCKET_START_MS)
    tsla_uri = support.price_feed_uri(support.TSLA_FEED_ID, "TSLAUSDT", support.BUCKET_START_MS)

    setup_datas = [
        support.function_calldata(
            native_oracle.functions.setDefaultCallback(
                SOURCE_TYPE_PRICE_FEED,
                price_resolver.address,
            )
        ),
        support.function_calldata(
            task_config.functions.setTask(
                SOURCE_TYPE_PRICE_FEED,
                support.NVDA_FEED_ID,
                support.TASK_PRICE_FEED,
                nvda_uri.encode(),
            )
        ),
        support.function_calldata(
            task_config.functions.setTask(
                SOURCE_TYPE_PRICE_FEED,
                support.TSLA_FEED_ID,
                support.TASK_PRICE_FEED,
                tsla_uri.encode(),
            )
        ),
        support.function_calldata(
            native_oracle.functions.setCallback(
                SOURCE_TYPE_POLYMARKET_SETTLEMENT,
                FED_BINARY_MARKET_ID,
                poly_resolver.address,
            )
        ),
        support.function_calldata(
            poly_resolver.functions.registerMirror(
                FED_BINARY_MARKET_ID,
                POLYGON_CHAIN_ID,
                ctf,
                condition_id,
                2,
            )
        ),
        support.function_calldata(binary_market.functions.createMarket(create_params)),
    ]
    receipt = await support.execute_governance_proposal(
        w3,
        pool_addr,
        [
            NATIVE_ORACLE_ADDRESS,
            ORACLE_TASK_CONFIG_ADDRESS,
            ORACLE_TASK_CONFIG_ADDRESS,
            NATIVE_ORACLE_ADDRESS,
            poly_resolver.address,
            binary_market.address,
        ],
        setup_datas,
        "combined-oracle-demo-setup",
        gas=8_000_000,
    )
    market_id = support.market_id_from_receipt(receipt)

    no_account = Account.create("oracle-demo-no")
    yes_account = Account.create("oracle-demo-yes")
    for account in [no_account, yes_account]:
        support.send_tx(w3, account.address, b"", FAUCET_KEY, gas=21_000, value=STAKE_UNIT)
        _send_contract_tx(w3, token, token.functions.mint(account.address, USER_STARTING_BALANCE), FAUCET_KEY)
        _send_contract_tx(w3, token, token.functions.approve(binary_market.address, USER_STARTING_BALANCE), account.key)

    _send_contract_tx(
        w3,
        binary_market,
        binary_market.functions.placeBet(market_id, 0, NO_STAKE),
        no_account.key,
        gas=1_500_000,
    )
    _send_contract_tx(
        w3,
        binary_market,
        binary_market.functions.placeBet(market_id, 1, YES_STAKE),
        yes_account.key,
        gas=1_500_000,
    )
    assert binary_market.functions.getMarket(market_id).call()[7] == TOTAL_BINARY_POOL

    observed_rounds = {}
    target_round_id = support.round_id(support.BUCKET_START_MS, support.TARGET_DELIVERY_NONCE)
    target_resolved_at = support.round_end_ms(support.BUCKET_START_MS, support.TARGET_DELIVERY_NONCE)
    for feed_id, expected_price in [
        (support.NVDA_FEED_ID, support.EXPECTED_NVDA_PRICE),
        (support.TSLA_FEED_ID, support.EXPECTED_TSLA_PRICE),
    ]:
        logs = await support.poll_price_recorded(w3, feed_id, timeout=240)
        assert logs, f"No sourceType=3 DataRecorded event observed for feedId={feed_id}"
        latest_nonce = await support.wait_for_latest_nonce(
            native_oracle,
            feed_id,
            support.TARGET_DELIVERY_NONCE,
            timeout=240,
        )
        assert latest_nonce >= support.TARGET_DELIVERY_NONCE
        stored = await support.wait_for_price_round(price_resolver, feed_id, target_round_id, timeout=120)
        support.assert_price_round(stored, expected_price, target_round_id, target_resolved_at)
        observed_rounds[feed_id] = stored
        LOG.info("Price feed resolved: feedId=%s roundId=%s price=%s", feed_id, target_round_id, int(stored[7]))

    await support.wait_for_chain_time(w3, closes_at)
    _send_contract_tx(w3, binary_market, binary_market.functions.lockMarket(market_id), FAUCET_KEY)

    release = _mock_set_binary_winning_slot(metadata["rpc_url"], 0)
    release["yesAccount"] = yes_account.address
    metadata.update(release)
    metadata_path.write_text(json.dumps(metadata, indent=2))
    LOG.info("Released mock binary Polymarket settlement: payout=%s", release["payout_numerators"])

    logs = await _poll_polymarket_data_recorded(w3, timeout=180)
    assert logs, "No Polymarket sourceType=6 DataRecorded event observed"
    settlement = await _wait_for_resolver_settlement(poly_resolver, timeout=60)
    assert settlement[0] is True
    assert settlement[2] == POLYGON_CHAIN_ID
    assert Web3.to_checksum_address(settlement[3]) == ctf
    assert settlement[6] == 2
    assert settlement[9] == SETTLEMENT_KIND_CTF_CONDITION_RESOLUTION
    assert poly_resolver.functions.getPayoutNumerators(FED_BINARY_MARKET_ID, condition_id).call() == [1, 0]

    _send_contract_tx(w3, binary_market, binary_market.functions.settleMarket(market_id), FAUCET_KEY, gas=2_000_000)
    market_state = binary_market.functions.getMarket(market_id).call()
    assert market_state[5] == 2, f"binary market did not settle: status={market_state[5]}"
    assert market_state[6] == 1, f"winning outcome should be YES, got {market_state[6]}"
    claimable_yes = binary_market.functions.claimable(market_id, yes_account.address).call()
    assert claimable_yes == TOTAL_BINARY_POOL

    _write_demo_config(
        cluster,
        w3.eth.chain_id,
        price_resolver.address,
        poly_resolver.address,
        binary_market.address,
        token.address,
        market_id,
        market_state,
        release,
        observed_rounds,
        target_round_id,
        target_resolved_at,
        claimable_yes,
    )

    LOG.info(
        "Combined oracle demo resolved: NVDA/TSLA roundId=%s, polymarket marketId=%s YES claimable=%s",
        target_round_id,
        market_id,
        claimable_yes,
    )
