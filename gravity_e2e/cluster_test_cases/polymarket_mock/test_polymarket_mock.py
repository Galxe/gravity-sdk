"""Polymarket settlement mirror mock e2e test."""

import asyncio
import json
import logging
import time
from pathlib import Path

import pytest
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils.mock_polymarket_polygon import DRAW_MARKET_ID

LOG = logging.getLogger(__name__)

NATIVE_ORACLE_ADDRESS = "0x0000000000000000000000000001625F4000"
DATA_RECORDED_TOPIC0 = Web3.keccak(
    text="DataRecorded(uint32,uint256,uint128,uint256)"
).hex()


def _topic(value: int) -> str:
    return "0x" + value.to_bytes(32, "big").hex()


async def _poll_data_recorded(w3: Web3, timeout: int = 120):
    deadline = time.time() + timeout
    filter_params = {
        "fromBlock": 0,
        "toBlock": "latest",
        "address": NATIVE_ORACLE_ADDRESS,
        "topics": [DATA_RECORDED_TOPIC0, _topic(6), _topic(DRAW_MARKET_ID)],
    }
    while time.time() < deadline:
        logs = await asyncio.to_thread(w3.eth.get_logs, filter_params)
        if logs:
            return logs
        await asyncio.sleep(2)
    return []


@pytest.mark.asyncio
async def test_polymarket_settlement_mirror_mock(cluster: Cluster):
    metadata_path = Path(__file__).with_name("mock_polymarket_metadata.json")
    metadata = json.loads(metadata_path.read_text())

    LOG.info("Verifying gravity node is live")
    assert await cluster.set_full_live(timeout=120), "Gravity node failed to become live"
    assert await cluster.check_block_increasing(timeout=60), "Gravity chain is not producing blocks"

    node = cluster.get_node("node1")
    assert node is not None, "node1 not found in cluster"
    gravity_w3 = node.w3

    LOG.info("Waiting for NativeOracle.DataRecorded(sourceType=6, sourceId=%s)", DRAW_MARKET_ID)
    logs = await _poll_data_recorded(gravity_w3, timeout=180)

    assert logs, "No Polymarket sourceType=6 DataRecorded event observed"
    first = logs[0]
    assert first["topics"][1].hex() == _topic(6)
    assert first["topics"][2].hex() == _topic(DRAW_MARKET_ID)

    LOG.info(
        "Observed Polymarket DataRecorded: market=%s condition=%s sourceBlock=%s sourceLogIndex=%s",
        metadata["market_id"],
        metadata["condition_id"],
        metadata["block"],
        metadata["log_index"],
    )
