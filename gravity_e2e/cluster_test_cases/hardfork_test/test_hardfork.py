"""
Gamma Hardfork E2E Test

Tests the full lifecycle of the gamma hardfork (system contract bytecode upgrade):

1. Start chain with v1.0.0 (pre-gamma) contract bytecodes
2. Verify blocks are being produced pre-hardfork
3. Send light transaction pressure
4. Wait for the hardfork block (gammaBlock) to be reached
5. Verify chain continues block production post-hardfork
6. Verify system contract bytecodes have changed
7. Wait for epoch change to verify ongoing consensus
8. Stop node, restart, and verify replay through the hardfork block
9. Verify post-restart liveness and contract state

Configuration:
    GAMMA_BLOCK: Block number at which gamma hardfork activates (default: 50)

Usage:
    # Run via E2E runner (recommended):
    ./gravity_e2e/run_test.sh hardfork_test

    # With custom gamma block:
    GAMMA_BLOCK=30 ./gravity_e2e/run_test.sh hardfork_test
"""

import asyncio
import logging
import os
import time

import pytest
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.cluster.node import NodeState

# Import hardfork utilities (from same directory)
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from hardfork_utils import (
    GAMMA_SYSTEM_CONTRACTS,
    compare_snapshots,
    send_eth_transfers,
    snapshot_system_contracts,
    wait_for_block,
    wait_for_blocks_after,
)

LOG = logging.getLogger(__name__)

# ── Test Configuration ────────────────────────────────────────────────
GAMMA_BLOCK = int(os.environ.get("GAMMA_BLOCK", "500"))

# How many blocks to wait after the hardfork to confirm stability
POST_HARDFORK_BLOCKS = 30

# Number of light ETH transfers per batch
LIGHT_PRESSURE_TXN_COUNT = 5

# Timeout for waiting for blocks to increase (seconds)
BLOCK_INCREASE_TIMEOUT = 120

# How many blocks to wait after restart to confirm replay + liveness
POST_RESTART_BLOCKS = 20


@pytest.mark.skip(
    reason="Genesis baseline is gravity-testnet-v1.4.0 which already ships the "
    "post-Gamma bytecode set, so the hardfork is a no-op on this cluster — "
    "Phase 4's bytecode-diff assertion has nothing to observe. Coverage for the "
    "Zeta-on-v1.5 transition lives in test_zeta.py + test_full_lifecycle.py."
)
@pytest.mark.asyncio
async def test_gamma_hardfork(cluster: Cluster):
    """
    End-to-end test for the gamma hardfork lifecycle.

    Verifies:
    - Pre-hardfork chain liveness and transaction execution
    - Hardfork transition without stall or panic
    - Post-hardfork contract bytecode upgrades
    - Epoch change after hardfork
    - Node restart and replay through hardfork block
    """
    node = cluster.get_node("node1")
    assert node is not None, "node1 not found in cluster"
    w3 = node.w3

    LOG.info("=" * 70)
    LOG.info("🔱 Gamma Hardfork E2E Test")
    LOG.info(f"   gammaBlock = {GAMMA_BLOCK}")
    LOG.info(f"   Post-hardfork blocks = {POST_HARDFORK_BLOCKS}")
    LOG.info(f"   Post-restart blocks = {POST_RESTART_BLOCKS}")
    LOG.info("=" * 70)

    # ── Phase 1: Pre-hardfork validation ─────────────────────────────
    LOG.info("\n[Phase 1] Pre-hardfork validation")
    LOG.info("Bringing cluster online...")
    assert await cluster.set_full_live(timeout=60), "Cluster failed to become live"

    initial_height = node.get_block_number()
    LOG.info(f"Initial block height: {initial_height}")
    assert initial_height >= 0

    # Verify blocks are being produced
    LOG.info("Verifying block production...")
    assert await node.wait_for_block_increase(timeout=30, delta=3), \
        "Pre-hardfork: blocks not being produced"
    LOG.info("✅ Phase 1 PASSED: chain is producing blocks")

    # ── Phase 2: Pre-hardfork snapshot & light pressure ──────────────
    LOG.info("\n[Phase 2] Pre-hardfork snapshot & light pressure")

    # Take snapshot of system contract bytecodes BEFORE hardfork
    LOG.info("Taking pre-hardfork contract snapshot...")
    pre_snapshot = snapshot_system_contracts(w3)
    pre_existing = {k: v for k, v in pre_snapshot.items() if v is not None}
    LOG.info(f"  Found {len(pre_existing)} system contracts with code pre-hardfork:")
    for name, code_hash in pre_existing.items():
        LOG.info(f"    {name}: {code_hash[:18]}...")

    # Some contracts may not exist in v1.0.0 genesis — that's expected
    pre_missing = [k for k, v in pre_snapshot.items() if v is None]
    if pre_missing:
        LOG.info(f"  Contracts not in v1.0.0 genesis (expected): {pre_missing}")

    # Send some light pressure before hardfork
    LOG.info("Sending light transaction pressure (pre-hardfork)...")
    sender = cluster.faucet
    if sender:
        ok, fail = await send_eth_transfers(w3, sender, num_txns=LIGHT_PRESSURE_TXN_COUNT)
        assert ok > 0, "Pre-hardfork: no transactions succeeded"
        LOG.info(f"  Pre-hardfork txns: {ok} succeeded, {fail} failed")
    else:
        LOG.warning("  No faucet available, skipping pre-hardfork pressure")

    LOG.info("✅ Phase 2 PASSED: snapshot taken, transactions working")

    # ── Phase 3: Hardfork transition ─────────────────────────────────
    LOG.info(f"\n[Phase 3] Waiting for hardfork transition at block {GAMMA_BLOCK}")

    current_block = node.get_block_number()
    if current_block < GAMMA_BLOCK:
        LOG.info(f"  Current block: {current_block}, waiting for gammaBlock={GAMMA_BLOCK}...")
        reached = await wait_for_block(w3, GAMMA_BLOCK, timeout=300)
        assert reached, f"Failed to reach gammaBlock {GAMMA_BLOCK}"
    else:
        LOG.info(f"  Already past gammaBlock (current={current_block})")

    # Verify chain continues producing blocks after the hardfork
    LOG.info(f"Verifying {POST_HARDFORK_BLOCKS} blocks after hardfork...")
    hardfork_height = node.get_block_number()
    continued = await wait_for_blocks_after(
        w3, hardfork_height, POST_HARDFORK_BLOCKS, timeout=BLOCK_INCREASE_TIMEOUT
    )
    assert continued, \
        f"Chain stalled after hardfork! Last seen: {node.get_block_number()}"

    # Send transactions post-hardfork
    LOG.info("Sending transactions post-hardfork...")
    if sender:
        ok, fail = await send_eth_transfers(w3, sender, num_txns=LIGHT_PRESSURE_TXN_COUNT)
        assert ok > 0, "Post-hardfork: no transactions succeeded"
        LOG.info(f"  Post-hardfork txns: {ok} succeeded, {fail} failed")

    LOG.info("✅ Phase 3 PASSED: hardfork transition successful, chain alive")

    # ── Phase 4: Post-hardfork contract verification ─────────────────
    LOG.info("\n[Phase 4] Post-hardfork contract bytecode verification")

    post_snapshot = snapshot_system_contracts(w3)
    changed, unchanged, missing = compare_snapshots(pre_snapshot, post_snapshot)

    LOG.info(f"  Changed: {len(changed)} contracts")
    for name in changed:
        LOG.info(f"    ✅ {name}: {pre_snapshot[name][:18] if pre_snapshot[name] else 'None'}... → {post_snapshot[name][:18] if post_snapshot[name] else 'None'}...")

    if unchanged:
        LOG.info(f"  Unchanged: {len(unchanged)} contracts")
        for name in unchanged:
            LOG.info(f"    ⚠️  {name}: {pre_snapshot[name][:18] if pre_snapshot[name] else 'None'}...")

    if missing:
        LOG.info(f"  Missing (no code before & after): {missing}")

    # At least the core contracts that existed before should have changed
    # Some contracts may not exist in v1.0.0 genesis or may have identical bytecodes
    assert len(changed) >= 4, \
        f"Expected at least 4 system contracts to change, but only {len(changed)} changed: {changed}. Unchanged: {unchanged}"

    LOG.info("✅ Phase 4 PASSED: system contracts upgraded")

    # ── Phase 5: Epoch change verification ───────────────────────────
    LOG.info("\n[Phase 5] Epoch change verification")
    LOG.info("Waiting for more blocks to observe epoch transition...")

    # With 60s epoch interval, we need to wait ~60-120s
    # We'll wait for a significant number of blocks
    epoch_wait_blocks = 60
    epoch_start_block = node.get_block_number()
    LOG.info(f"  Current block: {epoch_start_block}, waiting {epoch_wait_blocks} more blocks (~2 epochs)...")

    epoch_reached = await wait_for_blocks_after(
        w3, epoch_start_block, epoch_wait_blocks, timeout=180
    )
    assert epoch_reached, \
        f"Chain stalled during epoch change wait! Last: {node.get_block_number()}"

    final_post_hardfork_block = node.get_block_number()
    LOG.info(f"  Block height after epoch wait: {final_post_hardfork_block}")

    # Verify contracts still have new bytecode after epoch changes
    LOG.info("Re-verifying contract bytecodes after epoch changes...")
    epoch_snapshot = snapshot_system_contracts(w3)
    for name in changed:
        assert epoch_snapshot[name] == post_snapshot[name], \
            f"Contract {name} bytecode changed unexpectedly after epoch!"
    LOG.info("  Contract bytecodes stable across epoch boundaries")

    LOG.info("✅ Phase 5 PASSED: epoch change successful")

    # ── Phase 6: Node restart & replay ───────────────────────────────
    LOG.info("\n[Phase 6] Node restart & replay verification")

    pre_stop_height = node.get_block_number()
    LOG.info(f"  Pre-stop block height: {pre_stop_height}")

    # Stop node
    LOG.info("  Stopping node...")
    stop_ok = await node.stop()
    assert stop_ok, "Failed to stop node"

    # Verify node is stopped
    state, _ = await node.get_state()
    assert state == NodeState.STOPPED, f"Node not stopped, state={state.name}"
    LOG.info("  ✅ Node stopped and verified")

    # Wait a moment
    await asyncio.sleep(3)

    # Start node
    LOG.info("  Starting node (replay through hardfork)...")
    start_ok = await node.start()
    assert start_ok, "Failed to restart node"
    LOG.info("  ✅ Node restarted")

    # Verify node catches up and continues
    LOG.info(f"  Verifying post-restart liveness ({POST_RESTART_BLOCKS} blocks)...")
    restart_height = node.get_block_number()
    LOG.info(f"  Restart block height: {restart_height}")

    restart_ok = await wait_for_blocks_after(
        w3, restart_height, POST_RESTART_BLOCKS, timeout=BLOCK_INCREASE_TIMEOUT
    )
    assert restart_ok, \
        f"Node failed to produce blocks after restart! Last: {node.get_block_number()}"

    # Final contract verification after restart + replay
    LOG.info("  Verifying contract bytecodes after restart...")
    restart_snapshot = snapshot_system_contracts(w3)
    for name in changed:
        assert restart_snapshot[name] == post_snapshot[name], \
            f"Contract {name} bytecode mismatch after restart replay!"
    LOG.info("  Contract bytecodes match after replay")

    # Send final transactions to confirm full EVM functionality
    if sender:
        LOG.info("  Sending final verification transactions...")
        ok, fail = await send_eth_transfers(w3, sender, num_txns=LIGHT_PRESSURE_TXN_COUNT)
        assert ok > 0, "Post-restart: no transactions succeeded"
        LOG.info(f"  Post-restart txns: {ok} succeeded, {fail} failed")

    final_height = node.get_block_number()
    LOG.info("✅ Phase 6 PASSED: node restart & replay successful")

    # ── Final Summary ────────────────────────────────────────────────
    LOG.info("\n" + "=" * 70)
    LOG.info("🎉 ALL PHASES PASSED - Gamma Hardfork E2E Test")
    LOG.info(f"   gammaBlock: {GAMMA_BLOCK}")
    LOG.info(f"   Contracts upgraded: {len(changed)}")
    LOG.info(f"   Final block height: {final_height}")
    LOG.info(f"   Node restart + replay: ✅")
    LOG.info("=" * 70)
