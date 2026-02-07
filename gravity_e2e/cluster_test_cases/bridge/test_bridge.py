"""
Bridge E2E Test — Continuous Bridge Loop

Tests the full bridge flow:
    Anvil (source) → MessageSent → gravity_node oracle → GBridgeReceiver → NativeMinted

Runs continuously for a configurable duration (default 10 minutes),
each iteration:
    1. Check recipient balance on gravity chain
    2. Execute bridge transaction on Anvil (web3.py)
    3. Poll for NativeMinted event on gravity chain
    4. Verify recipient balance change

Outputs cumulative statistics at the end.
"""

import asyncio
import logging
import time

import pytest
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils.anvil_manager import BridgeContracts
from gravity_e2e.utils.bridge_utils import (
    BridgeHelper,
    BridgeStats,
    poll_native_minted,
    GBRIDGE_RECEIVER_ADDRESS,
)

LOG = logging.getLogger(__name__)

# Bridge amount per iteration: 1000 G tokens (in wei)
BRIDGE_AMOUNT = 1000 * 10**18

# Maximum time to wait for NativeMinted event per iteration
VERIFY_TIMEOUT = 120.0

# Polling interval for NativeMinted
POLL_INTERVAL = 2.0


@pytest.mark.cross_chain
@pytest.mark.bridge
@pytest.mark.asyncio
async def test_bridge_continuous(
    cluster: Cluster,
    anvil_bridge: BridgeContracts,
    bridge_helper: BridgeHelper,
    bridge_duration: int,
):
    """
    Continuous bridge stress test.

    Loops for `bridge_duration` seconds (default 600 = 10 min):
    - Records recipient balance before each bridge
    - Executes bridgeToGravity on Anvil via web3.py
    - Waits for NativeMinted event on gravity chain
    - Asserts NativeMinted event fields (recipient, amount, nonce)
    - Asserts balance_after - balance_before == bridge_amount

    Reports cumulative stats: success rate, latencies, nonce continuity.
    """
    # Ensure gravity node is live and producing blocks
    LOG.info("Ensuring gravity nodes are live...")
    is_live = await cluster.set_full_live(timeout=60)
    assert is_live, "Gravity nodes failed to become live"

    is_progressing = await cluster.check_block_increasing(timeout=30)
    assert is_progressing, "Gravity chain is not producing blocks"

    node = cluster.get_node("node1")
    assert node is not None, "node1 not found in cluster"
    gravity_w3 = node.w3

    # Recipient = Anvil deployer address (bridge to self for simplicity)
    recipient = Web3.to_checksum_address(anvil_bridge.deployer_address)

    # Record initial state
    initial_balance = gravity_w3.eth.get_balance(recipient)
    LOG.info(
        f"Initial balance of {recipient} on gravity chain: {initial_balance} wei"
    )
    LOG.info(
        f"Starting {bridge_duration}s continuous bridge test "
        f"(amount={BRIDGE_AMOUNT} wei/round)"
    )

    start_time = time.time()
    stats = BridgeStats()

    while time.time() - start_time < bridge_duration:
        round_num = stats.total + 1
        LOG.info(f"\n{'=' * 50}")
        LOG.info(f"=== Round {round_num} ===")
        LOG.info(f"{'=' * 50}")

        try:
            # 1. Record balance before bridge
            balance_before = gravity_w3.eth.get_balance(recipient)
            LOG.info(f"  Balance before: {balance_before} wei")

            # 2. Execute bridge on Anvil
            t0 = time.time()
            nonce = bridge_helper.mint_and_bridge(BRIDGE_AMOUNT, recipient)
            LOG.info(f"  Bridge tx sent. Nonce: {nonce}")

            # 3. Verify MessageSent event on Anvil
            anvil_events = bridge_helper.query_message_sent_events(from_block=0)
            LOG.info(f"  MessageSent events on Anvil: {len(anvil_events)}")

            # 4. Wait for NativeMinted event on gravity chain
            event = await poll_native_minted(
                gravity_w3=gravity_w3,
                nonce=nonce,
                timeout=VERIFY_TIMEOUT,
                poll_interval=POLL_INTERVAL,
            )
            latency = time.time() - t0

            # 5. Verify NativeMinted event
            assert event is not None, (
                f"Round {round_num}: NativeMinted event not found "
                f"for nonce={nonce} within {VERIFY_TIMEOUT}s"
            )
            assert event["recipient"] == recipient, (
                f"Round {round_num}: recipient mismatch: "
                f"expected {recipient}, got {event['recipient']}"
            )
            assert event["amount"] == BRIDGE_AMOUNT, (
                f"Round {round_num}: amount mismatch: "
                f"expected {BRIDGE_AMOUNT}, got {event['amount']}"
            )
            assert event["nonce"] == nonce, (
                f"Round {round_num}: nonce mismatch: "
                f"expected {nonce}, got {event['nonce']}"
            )

            # 6. Verify balance change
            balance_after = gravity_w3.eth.get_balance(recipient)
            balance_delta = balance_after - balance_before
            LOG.info(
                f"  Balance after: {balance_after} wei "
                f"(delta: {balance_delta} wei)"
            )
            assert balance_delta == BRIDGE_AMOUNT, (
                f"Round {round_num}: balance delta mismatch: "
                f"expected {BRIDGE_AMOUNT}, got {balance_delta}"
            )

            # Record success
            stats.record(nonce=nonce, latency=latency, amount=BRIDGE_AMOUNT)
            LOG.info(
                f"  ✓ Round {round_num} PASSED — "
                f"latency={latency:.1f}s, nonce={nonce}"
            )

        except Exception as e:
            stats.record_failure()
            LOG.error(f"  ✗ Round {round_num} FAILED: {e}")
            # Continue to next iteration instead of stopping the entire test
            # This allows us to collect stats across the full duration
            continue

    # Final report
    elapsed = time.time() - start_time
    LOG.info(f"\nTest completed in {elapsed:.1f}s")
    report = stats.report()

    # Final cumulative balance check
    final_balance = gravity_w3.eth.get_balance(recipient)
    total_expected = initial_balance + stats.total_bridged
    LOG.info(
        f"Final balance check: expected={total_expected}, actual={final_balance}"
    )

    # Assertions on overall results
    assert stats.success > 0, "No successful bridge transactions"
    assert stats.failed == 0, f"{stats.failed} bridge iterations failed"
    assert final_balance == total_expected, (
        f"Cumulative balance mismatch: "
        f"expected {total_expected}, got {final_balance}"
    )
