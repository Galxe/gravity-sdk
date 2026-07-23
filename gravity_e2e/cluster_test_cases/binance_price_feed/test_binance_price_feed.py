"""Binance closed index-kline price-feed E2E test."""

import json
import logging
import os
from pathlib import Path

import pytest

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils import oracle_test_support as support

LOG = logging.getLogger(__name__)
SUITE_DIR = Path(__file__).resolve().parent


def _bucket_start_ms() -> int:
    configured = os.environ.get("BINANCE_PRICE_FEED_BUCKET_START_MS")
    return int(configured) if configured else support.BUCKET_START_MS


def _target_delivery_nonce() -> int:
    return 1 if support.is_live_mode() else support.TARGET_DELIVERY_NONCE


def _assert_live_round_shape(round_data, round_id: int, resolved_at: int):
    assert round_data[0] is True
    assert round_data[1] == round_id
    assert round_data[2] == resolved_at
    assert round_data[3] == support.DECIMALS
    assert round_data[4] > 0


def _write_demo_config(
    cluster: Cluster,
    resolver_address: str,
    chain_id: int,
    binance_base_url: str,
    observed_rounds: dict[int, tuple],
    bucket_start_ms: int,
    target_delivery_nonce: int,
    target_round_id: int,
    target_resolved_at: int,
):
    output = os.environ.get("GRAVITY_DEMO_CONFIG_OUT")
    if not output:
        return

    live = support.is_live_mode()
    provider = {
        "kind": "live-binance" if live else "local-mock",
        "name": "Binance USD-M indexPriceKlines"
        if live
        else "Local deterministic Binance index-kline mock",
        "baseUrl": binance_base_url,
        "endpoint": "/fapi/v1/indexPriceKlines",
        "interval": "1m",
        "live": live,
    }

    def feed_config(feed_id: int, pair: str):
        observed_price = int(observed_rounds[feed_id][4])
        return {
            "feedId": feed_id,
            "label": f"{pair[:-4]} {'Binance' if live else 'mock'} index",
            "pair": pair,
            "sourceType": support.SOURCE_TYPE_PRICE_FEED,
            "decimals": support.DECIMALS,
            "expectedDeliveryNonce": target_delivery_nonce,
            "expectedRoundId": target_round_id,
            "expectedResolvedAt": target_resolved_at,
            "expectedPrice": str(observed_price),
            "expectedDisplayPrice": support.format_price(observed_price),
        }

    config = {
        "version": 1,
        "mode": "live-binance-index-kline" if live else "local-binance-index-kline",
        "network": {
            "chainId": chain_id,
            "rpcUrl": cluster.get_node("node1").url,
            "rpcProxyPath": "/rpc",
        },
        "contracts": {
            "priceFeedResolver": resolver_address,
            "multiSourceOracleResolver": resolver_address,
            "nativeOracle": support.NATIVE_ORACLE_ADDRESS,
            "oracleTaskConfig": support.ORACLE_TASK_CONFIG_ADDRESS,
        },
        "provider": provider,
        "feeds": [
            feed_config(support.NVDA_FEED_ID, "NVDAUSDT"),
            feed_config(support.TSLA_FEED_ID, "TSLAUSDT"),
        ],
        "demo": {
            "providerMode": support.price_feed_mode(),
            "bucketStartMs": bucket_start_ms,
            "intervalMs": support.INTERVAL_MS,
            "targetDeliveryNonce": target_delivery_nonce,
            "targetRoundId": target_round_id,
            "targetResolvedAt": target_resolved_at,
        },
    }

    output_path = Path(output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(config, indent=2) + "\n")
    LOG.info("Wrote price feed demo config to %s", output_path)


@pytest.mark.asyncio
async def test_binance_price_feed_resolves_nvda_and_tsla(cluster: Cluster):
    assert await cluster.set_full_live(timeout=120), "Gravity node failed to become live"
    assert await cluster.check_block_increasing(timeout=60), "Gravity chain is not producing blocks"

    node = cluster.get_node("node1")
    assert node is not None, "node1 not found in cluster"
    w3 = node.w3
    assert w3 is not None and w3.is_connected(), "node1 web3 not connected"

    required = [
        ("PriceFeedResolver.sol", "PriceFeedResolver"),
        ("NativeOracle.sol", "NativeOracle"),
        ("OracleTaskConfig.sol", "OracleTaskConfig"),
    ]
    contracts_out = support.ensure_contracts_out(SUITE_DIR, required, "Price feed")
    resolver_artifact = support.load_artifact(
        contracts_out, "PriceFeedResolver.sol", "PriceFeedResolver"
    )
    native_artifact = support.load_artifact(contracts_out, "NativeOracle.sol", "NativeOracle")
    task_artifact = support.load_artifact(
        contracts_out, "OracleTaskConfig.sol", "OracleTaskConfig"
    )

    resolver = support.deploy_contract(w3, resolver_artifact, support.FAUCET_KEY)
    native_oracle = w3.eth.contract(
        address=support.NATIVE_ORACLE_ADDRESS, abi=native_artifact["abi"]
    )
    task_config = w3.eth.contract(
        address=support.ORACLE_TASK_CONFIG_ADDRESS, abi=task_artifact["abi"]
    )
    pool_addr = support.pool0_voted_by_faucet(w3)

    bucket_start_ms = _bucket_start_ms()
    target_nonce = _target_delivery_nonce()
    target_round_id = support.round_id(bucket_start_ms, target_nonce)
    target_resolved_at = support.round_end_ms(bucket_start_ms, target_nonce)
    uris = {
        support.NVDA_FEED_ID: support.price_feed_uri(
            support.NVDA_FEED_ID, "NVDAUSDT", bucket_start_ms
        ),
        support.TSLA_FEED_ID: support.price_feed_uri(
            support.TSLA_FEED_ID, "TSLAUSDT", bucket_start_ms
        ),
    }

    with support.server_context() as binance_base_url:
        setup_data = [
            support.function_calldata(
                native_oracle.functions.setDefaultCallback(
                    support.SOURCE_TYPE_PRICE_FEED, resolver.address
                )
            )
        ]
        for feed_id, uri in uris.items():
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
            [support.NATIVE_ORACLE_ADDRESS, support.ORACLE_TASK_CONFIG_ADDRESS, support.ORACLE_TASK_CONFIG_ADDRESS],
            setup_data,
            "binance-price-feed-e2e-setup",
        )

        observed_rounds = {}
        expected = {
            support.NVDA_FEED_ID: support.EXPECTED_NVDA_PRICE,
            support.TSLA_FEED_ID: support.EXPECTED_TSLA_PRICE,
        }
        for feed_id, expected_price in expected.items():
            assert await support.poll_price_recorded(w3, feed_id, timeout=240)
            latest_nonce = await support.wait_for_latest_nonce(
                native_oracle, feed_id, target_nonce, timeout=240
            )
            assert latest_nonce >= target_nonce
            stored = await support.wait_for_price_round(
                resolver, feed_id, target_round_id, timeout=120
            )
            if support.is_live_mode():
                _assert_live_round_shape(stored, target_round_id, target_resolved_at)
            else:
                support.assert_price_round(
                    stored, expected_price, target_round_id, target_resolved_at
                )
            observed_rounds[feed_id] = stored

        _write_demo_config(
            cluster,
            resolver.address,
            w3.eth.chain_id,
            binance_base_url,
            observed_rounds,
            bucket_start_ms,
            target_nonce,
            target_round_id,
            target_resolved_at,
        )
