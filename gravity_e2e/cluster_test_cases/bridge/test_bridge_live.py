"""
Bridge E2E Test - Live Round-Trip

Exercises the full bridge path with a single live bridgeToGravity tx:

    Anvil (auto-mine then 1s interval)            Gravity node
    ------------------------------------          --------------
    deploy GToken+Portal+Sender
    mine 70 blocks (seed finalized head)
                                                  (node restarts with
                                                   relayer pointed at Anvil)
    tx: mint+approve+bridgeToGravity  --->
         MessageSent(nonce=1)
         ... Anvil finalizes ~64 blocks later
                                         <---    relayer picks up event
                                                  oracle delivers to receiver
                                                  NativeMinted(nonce=1) emitted
                                                  recipient balance += amount

This is the "textbook" round-trip variant: ONE transaction sent after the
chain is already running, waiting for it to round-trip through the real
relayer and the real NATIVE_MINT_PRECOMPILE. Contrast with test_bridge.py
which pre-loads N bridges before the node starts (stress / throughput).
"""

import asyncio
import logging
import time

import pytest

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils.bridge_utils import poll_native_minted

LOG = logging.getLogger(__name__)


@pytest.mark.cross_chain
@pytest.mark.bridge
@pytest.mark.asyncio
async def test_bridge_live_roundtrip(
    cluster: Cluster,
    live_bridge_ready: dict,
    bridge_verify_timeout: int,
):
    helper = live_bridge_ready["helper"]
    recipient = live_bridge_ready["recipient"]
    amount = live_bridge_ready["amount"]

    LOG.info("Verifying gravity nodes are live...")
    assert await cluster.set_full_live(timeout=120), "Gravity nodes failed to become live"
    assert await cluster.check_block_increasing(timeout=60), "Gravity chain not producing blocks"

    node = cluster.get_node("node1")
    assert node is not None, "node1 not found in cluster"
    gravity_w3 = node.w3

    balance_before = gravity_w3.eth.get_balance(recipient)
    LOG.info(f"Balance before bridge: {balance_before} wei")

    LOG.info(f"Sending live bridgeToGravity(amount={amount}, recipient={recipient})...")
    t0 = time.time()
    nonce = helper.mint_and_bridge(amount=amount, recipient=recipient)
    tx_sent_at = time.time() - t0
    LOG.info(f"Anvil confirmed bridge tx (nonce={nonce}) after {tx_sent_at:.1f}s")
    assert nonce == 1, f"expected first bridge to have nonce=1, got {nonce}"

    LOG.info(
        f"Polling for NativeMinted(nonce={nonce}) on Gravity "
        f"(timeout={bridge_verify_timeout}s, includes ~64s Anvil finalization lag)..."
    )
    result = await poll_native_minted(
        gravity_w3=gravity_w3,
        nonce=nonce,
        timeout=bridge_verify_timeout,
        poll_interval=3.0,
    )
    elapsed = time.time() - t0

    assert result is not None, (
        f"NativeMinted(nonce={nonce}) did not appear within {bridge_verify_timeout}s"
    )
    assert result["recipient"].lower() == recipient.lower(), (
        f"recipient mismatch: expected {recipient}, got {result['recipient']}"
    )
    assert result["amount"] == amount, (
        f"amount mismatch: expected {amount}, got {result['amount']}"
    )
    assert result["nonce"] == nonce, (
        f"nonce mismatch: expected {nonce}, got {result['nonce']}"
    )

    balance_after = gravity_w3.eth.get_balance(recipient)
    assert balance_after - balance_before == amount, (
        f"balance delta mismatch: before={balance_before} after={balance_after} "
        f"expected_delta={amount}"
    )

    LOG.info("=" * 60)
    LOG.info(f"  Live bridge round-trip: {elapsed:.1f}s")
    LOG.info(f"  Anvil confirm:          {tx_sent_at:.1f}s")
    LOG.info(f"  Gravity mint observed:  {elapsed - tx_sent_at:.1f}s after bridge tx")
    LOG.info(f"  Recipient:              {recipient}")
    LOG.info(f"  Amount:                 {amount} wei")
    LOG.info(f"  Nonce:                  {nonce}")
    LOG.info("=" * 60)
