"""
Bridge E2E Test — Pre-Load & Verify

Tests the full bridge flow with pre-loaded transactions:
    1. Pre-load: N bridge txns sent on Anvil (gravity_node stopped)
    2. Start: gravity_node starts, relayer fetches all events in batch
    3. Verify: poll for all N NativeMinted events, check balance + nonces

This avoids the "starvation" problem where gravity_node processes faster
than Anvil can produce new bridge events.
"""

import asyncio
import logging
import time

import pytest
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils.bridge_utils import (
    BridgeStats,
    poll_all_native_minted,
    GBRIDGE_RECEIVER_ADDRESS,
)

LOG = logging.getLogger(__name__)


@pytest.mark.cross_chain
@pytest.mark.bridge
@pytest.mark.asyncio
async def test_bridge_preloaded(
    cluster: Cluster,
    preloaded_bridge: dict,
    bridge_verify_timeout: int,
):
    """
    Verify all pre-loaded bridge transactions were processed by gravity_node.

    Steps:
    1. Ensure gravity_node is live and producing blocks
    2. Record balance before
    3. Poll for all NativeMinted events (nonces 1..N)
    4. Verify: balance delta == N × amount
    5. Verify: all nonces continuous, no missing
    6. Compute per-event latency and report stats
    """
    contracts_info = preloaded_bridge
    bridge_count = contracts_info["bridge_count"]
    amount = contracts_info["amount"]
    recipient = contracts_info["recipient"]
    nonces = contracts_info["nonces"]
    helper = contracts_info["helper"]

    # Ensure gravity node is live
    LOG.info("Verifying gravity nodes are live...")
    is_live = await cluster.set_full_live(timeout=120)
    assert is_live, "Gravity nodes failed to become live"

    is_progressing = await cluster.check_block_increasing(timeout=60)
    assert is_progressing, "Gravity chain is not producing blocks"

    node = cluster.get_node("node1")
    assert node is not None, "node1 not found in cluster"
    gravity_w3 = node.w3

    # Record balance before processing
    balance_before = gravity_w3.eth.get_balance(recipient)
    LOG.info(f"Balance before: {balance_before} wei")
    LOG.info(
        f"Waiting for {bridge_count} NativeMinted events "
        f"(timeout={bridge_verify_timeout}s)..."
    )

    # Poll for all NativeMinted events
    t0 = time.time()
    result = await poll_all_native_minted(
        gravity_w3=gravity_w3,
        max_nonce=max(nonces),
        timeout=bridge_verify_timeout,
        poll_interval=3.0,
    )
    total_time = time.time() - t0

    found = result["found_nonces"]
    missing = result["missing_nonces"]
    events = result["events"]

    LOG.info(f"\n{'=' * 60}")
    LOG.info(f"  Bridge Pre-Load & Verify Report")
    LOG.info(f"{'=' * 60}")
    LOG.info(f"  Pre-loaded:       {bridge_count} bridge txns")
    LOG.info(f"  Events found:     {len(found)}/{bridge_count}")
    LOG.info(f"  Missing nonces:   {len(missing)}")
    LOG.info(f"  Processing time:  {result['processing_time']:.1f}s")
    LOG.info(f"  Total verify time:{total_time:.1f}s")
    if len(found) > 0:
        throughput = len(found) / result["processing_time"]
        LOG.info(f"  Throughput:       {throughput:.2f} bridges/sec")
    LOG.info(f"{'=' * 60}")

    # ---- Per-Event Latency Measurement ----
    if len(found) > 0:
        LOG.info("Computing per-event bridge latency...")
        # Get source-chain (Anvil) timestamps for each nonce
        anvil_timestamps = helper.query_message_sent_timestamps(from_block=0)

        stats = BridgeStats()
        skipped = 0
        for evt in events:
            nonce_val = evt["nonce"]
            gravity_ts = evt.get("block_timestamp")
            anvil_ts = anvil_timestamps.get(nonce_val)

            if gravity_ts is not None and anvil_ts is not None:
                latency = gravity_ts - anvil_ts
                stats.record(nonce=nonce_val, latency=float(latency), amount=amount)
            else:
                skipped += 1
                LOG.debug(
                    f"  Nonce {nonce_val}: missing timestamp "
                    f"(gravity_ts={gravity_ts}, anvil_ts={anvil_ts})"
                )

        if skipped > 0:
            LOG.warning(f"  Skipped {skipped} events due to missing timestamps")

        stats.report()

    # Assertions
    if len(missing) > 0:
        # Dump node logs for CI diagnosis before assertion fails
        LOG.warning(f"  {len(missing)} missing nonces — dumping node logs for diagnosis:")
        for node_id, node_obj in cluster.nodes.items():
            from gravity_e2e.cluster_test_cases.bridge.conftest import (
                _dump_node_logs,
            )
            _dump_node_logs(node_id, node_obj._infra_path, tail_lines=50, label="on-failure")

    assert len(missing) == 0, (
        f"{len(missing)} nonces not found (out of {bridge_count}): "
        f"{sorted(missing)[:20]}{'...' if len(missing) > 20 else ''}"
    )

    # Verify all events have correct recipient and amount
    for evt in events:
        assert evt["recipient"] == recipient, (
            f"Nonce {evt['nonce']}: recipient mismatch: "
            f"expected {recipient}, got {evt['recipient']}"
        )
        assert evt["amount"] == amount, (
            f"Nonce {evt['nonce']}: amount mismatch: "
            f"expected {amount}, got {evt['amount']}"
        )

    # Verify nonce continuity
    nonces_found = sorted(found)
    expected_nonces = list(range(1, bridge_count + 1))
    assert nonces_found == expected_nonces, (
        f"Nonces not continuous: "
        f"expected 1→{bridge_count}, got {nonces_found[0]}→{nonces_found[-1]}"
    )

    # Verify cumulative balance
    balance_after = gravity_w3.eth.get_balance(recipient)
    expected_delta = bridge_count * amount
    actual_delta = balance_after - balance_before
    LOG.info(
        f"Balance check: before={balance_before}, after={balance_after}, "
        f"delta={actual_delta}, expected={expected_delta}"
    )
    assert actual_delta == expected_delta, (
        f"Balance delta mismatch: expected {expected_delta}, got {actual_delta}"
    )

    LOG.info(f"✓ All {bridge_count} bridge transactions verified successfully!")

