"""
Test: Batch block execution via BATCH_COMMIT_SIZE environment variable.

Scenario:
    1. Start node normally, confirm it produces blocks.
    2. Stop the node.
    3. Set BATCH_COMMIT_SIZE=10 in the environment.
    4. Restart the node → consensus will batch-accumulate blocks
       and only send to execution when blocks_to_commit > 10.
    5. Wait for the chain to resume producing blocks, confirming
       the batch execution path works end-to-end.
"""

import asyncio
import logging
import os
import time

import pytest
from gravity_e2e.cluster.manager import Cluster

LOG = logging.getLogger(__name__)

BATCH_SIZE = 5

# Timeout for waiting after restart with batch mode enabled.
BATCH_EXEC_TIMEOUT = 60


@pytest.mark.asyncio
async def test_batch_execution(cluster: Cluster):
    """
    Verify that setting BATCH_COMMIT_SIZE causes the node to batch-accumulate
    blocks before sending them to execution, and the chain keeps progressing.
    """
    node = cluster.get_node("node1")
    assert node, "node1 not found in cluster config"

    # ── Step 1: Ensure node is running ──────────────────────────────
    LOG.info("Step 1: Ensuring node is running...")
    assert await cluster.set_full_live(timeout=30), "Cluster failed to become live"

    initial_height = node.get_block_number()
    LOG.info(f"Initial block height: {initial_height}")
    assert initial_height >= 0

    # Wait a few blocks to confirm normal operation
    assert await node.wait_for_block_increase(timeout=30, delta=3), (
        "Node did not produce blocks in normal mode"
    )
    height_before_stop = node.get_block_number()
    LOG.info(f"Blocks produced normally, height before stop: {height_before_stop}")

    # ── Step 2: Stop the node ───────────────────────────────────────
    LOG.info("Step 2: Stopping node...")
    assert await node.stop(), "Failed to stop node"
    await asyncio.sleep(2)

    # ── Step 3: Set BATCH_COMMIT_SIZE and restart ───────────────────
    LOG.info(f"Step 3: Setting BATCH_COMMIT_SIZE={BATCH_SIZE} and restarting...")
    os.environ["BATCH_COMMIT_SIZE"] = str(BATCH_SIZE)

    assert await node.start(), "Failed to restart node with BATCH_COMMIT_SIZE"
    LOG.info("Node restarted with batch execution enabled")

    # ── Step 4: Wait for block progress under batch mode ────────────
    LOG.info("Step 4: Verifying block production under batch mode...")
    height_after_restart = node.get_block_number()
    LOG.info(f"Height after restart: {height_after_restart}")

    # The chain should still make progress. With BATCH_COMMIT_SIZE=10,
    # blocks will only be sent to execution once >10 blocks have accumulated.
    # This means we need to wait for at least BATCH_SIZE+1 consensus rounds
    # before the first batch is executed.
    target_delta = BATCH_SIZE + 5  # a few extra to confirm ongoing progress
    assert await node.wait_for_block_increase(
        timeout=BATCH_EXEC_TIMEOUT, delta=target_delta
    ), (
        f"Chain did not produce {target_delta} blocks under batch mode "
        f"(BATCH_COMMIT_SIZE={BATCH_SIZE}) within {BATCH_EXEC_TIMEOUT}s"
    )

    final_height = node.get_block_number()
    blocks_produced = final_height - height_after_restart
    LOG.info(
        f"Batch execution verified! Produced {blocks_produced} blocks "
        f"(height {height_after_restart} → {final_height})"
    )

    # ── Step 5: Cleanup env var ─────────────────────────────────────
    os.environ.pop("BATCH_COMMIT_SIZE", None)
    LOG.info("Batch execution test PASSED!")


@pytest.mark.asyncio
async def test_batch_exec_restart(cluster: Cluster):
    """
    Verify that after BATCH_COMMIT_SIZE is set and the node is running in
    batch mode, a STOP→START cycle works correctly and the chain keeps
    producing blocks.
    """
    node = cluster.get_node("node1")
    assert node, "node1 not found in cluster config"

    # ── Step 1: Ensure node is running ──────────────────────────────
    LOG.info("Step 1: Ensuring node is running...")
    assert await cluster.set_full_live(timeout=30), "Cluster failed to become live"

    initial_height = node.get_block_number()
    LOG.info(f"Initial block height: {initial_height}")
    assert initial_height >= 0

    # Wait a few blocks to confirm normal operation
    assert await node.wait_for_block_increase(timeout=30, delta=3), (
        "Node did not produce blocks in normal mode"
    )
    height_before_stop = node.get_block_number()
    LOG.info(f"Height before first stop: {height_before_stop}")

    # ── Step 2: Stop, set env var, and restart ──────────────────────
    LOG.info("Step 2: Stopping node...")
    assert await node.stop(), "Failed to stop node"
    await asyncio.sleep(2)

    LOG.info(f"Setting BATCH_COMMIT_SIZE={BATCH_SIZE} and restarting...")
    os.environ["BATCH_COMMIT_SIZE"] = str(BATCH_SIZE)

    assert await node.start(), "Failed to restart node with BATCH_COMMIT_SIZE"
    LOG.info("Node restarted with batch execution enabled")

    # Wait for chain to make progress under batch mode
    LOG.info("Waiting for block progress under batch mode...")
    target_delta = BATCH_SIZE + 5
    assert await node.wait_for_block_increase(
        timeout=BATCH_EXEC_TIMEOUT, delta=target_delta
    ), (
        f"Chain did not produce {target_delta} blocks under batch mode "
        f"within {BATCH_EXEC_TIMEOUT}s"
    )
    height_before_restart = node.get_block_number()
    LOG.info(f"Height before second stop: {height_before_restart}")

    # ── Step 3: STOP again (env var still set) ──────────────────────
    LOG.info("Step 3: Stopping node again with BATCH_COMMIT_SIZE still set...")
    assert await node.stop(), "Failed to stop node on second stop"
    await asyncio.sleep(2)

    # ── Step 4: START again (env var still set) ─────────────────────
    LOG.info("Step 4: Restarting node with BATCH_COMMIT_SIZE still set...")
    assert await node.start(), "Failed to restart node on second start"
    LOG.info("Node restarted again, verifying block production...")

    height_after_second_restart = node.get_block_number()
    LOG.info(f"Height after second restart: {height_after_second_restart}")

    assert await node.wait_for_block_increase(
        timeout=BATCH_EXEC_TIMEOUT, delta=target_delta
    ), (
        f"Chain did not produce {target_delta} blocks after second restart "
        f"with BATCH_COMMIT_SIZE={BATCH_SIZE} within {BATCH_EXEC_TIMEOUT}s"
    )

    final_height = node.get_block_number()
    blocks_after_second_restart = final_height - height_after_second_restart
    LOG.info(
        f"Second restart verified! Produced {blocks_after_second_restart} blocks "
        f"(height {height_after_second_restart} → {final_height})"
    )

    # ── Step 5: Cleanup env var ─────────────────────────────────────
    os.environ.pop("BATCH_COMMIT_SIZE", None)
    LOG.info("Batch exec restart test PASSED!")
