"""
Hardfork E2E Test Framework

Provides a reusable 6-phase test runner for validating any Gravity hardfork.
Each phase is independently callable, but the main entry point
`run_hardfork_lifecycle_test()` runs all 6 phases in sequence.

Phases:
  1. Pre-hardfork validation (cluster liveness)
  2. Pre-hardfork snapshot + light pressure
  3. Hardfork transition (wait for block + verify chain alive)
  4. Post-hardfork bytecode verification
  5. Epoch change verification
  6. Node restart & replay verification

Usage:
    from hardfork_framework import HardforkTestConfig, run_hardfork_lifecycle_test

    config = HardforkTestConfig(
        name="gamma",
        display_name="Gamma Hardfork",
        hardfork_block=500,
        contracts={"StakingConfig": "0x...", ...},
    )
    await run_hardfork_lifecycle_test(cluster, config)
"""

import asyncio
import logging
from dataclasses import dataclass, field
from typing import Dict, Optional

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.cluster.node import NodeState

from hardfork_utils import (
    compare_snapshots,
    get_contract_code_hashes,
    send_eth_transfers,
    wait_for_block,
    wait_for_blocks_after,
)

LOG = logging.getLogger(__name__)


@dataclass
class HardforkTestConfig:
    """Configuration for a hardfork E2E test."""

    # Hardfork identifier (e.g. "gamma", "delta")
    name: str

    # Human-readable name for logging
    display_name: str

    # Block number at which the hardfork activates
    hardfork_block: int

    # System contracts to verify: {name: address}
    contracts: Dict[str, str]

    # How many blocks to wait after hardfork to confirm stability
    post_hardfork_blocks: int = 30

    # Number of light ETH transfers per batch
    light_pressure_txn_count: int = 5

    # Timeout for waiting for blocks to increase (seconds)
    block_increase_timeout: int = 120

    # How many blocks to wait after restart to confirm replay + liveness
    post_restart_blocks: int = 20

    # Minimum number of contracts that must change (sanity check)
    min_changed_contracts: int = 4

    # Number of blocks to wait for epoch observation
    epoch_wait_blocks: int = 60

    # Timeout for epoch wait (seconds)
    epoch_timeout: int = 180

    # Node name in the cluster
    node_name: str = "node1"


def _snapshot(w3, contracts: Dict[str, str]) -> Dict[str, Optional[str]]:
    """Take a snapshot of code hashes for a set of contracts."""
    return get_contract_code_hashes(w3, contracts)


async def phase1_pre_hardfork_validation(cluster: Cluster, config: HardforkTestConfig):
    """Phase 1: Verify cluster is live and producing blocks."""
    LOG.info("\n[Phase 1] Pre-hardfork validation")
    LOG.info("Bringing cluster online...")
    assert await cluster.set_full_live(timeout=60), "Cluster failed to become live"

    node = cluster.get_node(config.node_name)
    assert node is not None, f"{config.node_name} not found in cluster"

    initial_height = node.get_block_number()
    LOG.info(f"Initial block height: {initial_height}")
    assert initial_height >= 0

    LOG.info("Verifying block production...")
    assert await node.wait_for_block_increase(timeout=30, delta=3), \
        "Pre-hardfork: blocks not being produced"
    LOG.info("✅ Phase 1 PASSED: chain is producing blocks")


async def phase2_pre_hardfork_snapshot(cluster: Cluster, config: HardforkTestConfig):
    """
    Phase 2: Take pre-hardfork snapshot and send light pressure.
    Returns the pre-hardfork snapshot.
    """
    LOG.info("\n[Phase 2] Pre-hardfork snapshot & light pressure")
    node = cluster.get_node(config.node_name)
    w3 = node.w3

    LOG.info("Taking pre-hardfork contract snapshot...")
    pre_snapshot = _snapshot(w3, config.contracts)
    pre_existing = {k: v for k, v in pre_snapshot.items() if v is not None}
    LOG.info(f"  Found {len(pre_existing)} system contracts with code pre-hardfork:")
    for name, code_hash in pre_existing.items():
        LOG.info(f"    {name}: {code_hash[:18]}...")

    pre_missing = [k for k, v in pre_snapshot.items() if v is None]
    if pre_missing:
        LOG.info(f"  Contracts not in old genesis (expected): {pre_missing}")

    LOG.info("Sending light transaction pressure (pre-hardfork)...")
    sender = cluster.faucet
    if sender:
        ok, fail = await send_eth_transfers(
            w3, sender, num_txns=config.light_pressure_txn_count
        )
        assert ok > 0, "Pre-hardfork: no transactions succeeded"
        LOG.info(f"  Pre-hardfork txns: {ok} succeeded, {fail} failed")
    else:
        LOG.warning("  No faucet available, skipping pre-hardfork pressure")

    LOG.info("✅ Phase 2 PASSED: snapshot taken, transactions working")
    return pre_snapshot


async def phase3_hardfork_transition(cluster: Cluster, config: HardforkTestConfig):
    """Phase 3: Wait for hardfork block and verify chain continues."""
    LOG.info(f"\n[Phase 3] Waiting for hardfork transition at block {config.hardfork_block}")
    node = cluster.get_node(config.node_name)
    w3 = node.w3

    current_block = node.get_block_number()
    if current_block < config.hardfork_block:
        LOG.info(f"  Current block: {current_block}, waiting for {config.name}Block={config.hardfork_block}...")
        reached = await wait_for_block(w3, config.hardfork_block, timeout=300)
        assert reached, f"Failed to reach {config.name}Block {config.hardfork_block}"
    else:
        LOG.info(f"  Already past {config.name}Block (current={current_block})")

    LOG.info(f"Verifying {config.post_hardfork_blocks} blocks after hardfork...")
    hardfork_height = node.get_block_number()
    continued = await wait_for_blocks_after(
        w3, hardfork_height, config.post_hardfork_blocks,
        timeout=config.block_increase_timeout,
    )
    assert continued, f"Chain stalled after hardfork! Last seen: {node.get_block_number()}"

    LOG.info("Sending transactions post-hardfork...")
    sender = cluster.faucet
    if sender:
        ok, fail = await send_eth_transfers(
            w3, sender, num_txns=config.light_pressure_txn_count
        )
        assert ok > 0, "Post-hardfork: no transactions succeeded"
        LOG.info(f"  Post-hardfork txns: {ok} succeeded, {fail} failed")

    LOG.info("✅ Phase 3 PASSED: hardfork transition successful, chain alive")


async def phase4_bytecode_verification(
    cluster: Cluster, config: HardforkTestConfig,
    pre_snapshot: Dict[str, Optional[str]],
):
    """
    Phase 4: Verify contract bytecodes changed after hardfork.
    Returns (changed, post_snapshot).
    """
    LOG.info("\n[Phase 4] Post-hardfork contract bytecode verification")
    node = cluster.get_node(config.node_name)
    w3 = node.w3

    post_snapshot = _snapshot(w3, config.contracts)
    changed, unchanged, missing = compare_snapshots(pre_snapshot, post_snapshot)

    LOG.info(f"  Changed: {len(changed)} contracts")
    for name in changed:
        pre_hash = pre_snapshot[name][:18] if pre_snapshot[name] else "None"
        post_hash = post_snapshot[name][:18] if post_snapshot[name] else "None"
        LOG.info(f"    ✅ {name}: {pre_hash}... → {post_hash}...")

    if unchanged:
        LOG.info(f"  Unchanged: {len(unchanged)} contracts")
        for name in unchanged:
            hash_str = pre_snapshot[name][:18] if pre_snapshot[name] else "None"
            LOG.info(f"    ⚠️  {name}: {hash_str}...")

    if missing:
        LOG.info(f"  Missing (no code before & after): {missing}")

    assert len(changed) >= config.min_changed_contracts, \
        f"Expected at least {config.min_changed_contracts} contracts to change, " \
        f"but only {len(changed)} changed: {changed}. Unchanged: {unchanged}"

    LOG.info("✅ Phase 4 PASSED: system contracts upgraded")
    return changed, post_snapshot


async def phase5_epoch_verification(
    cluster: Cluster, config: HardforkTestConfig,
    changed: list, post_snapshot: Dict[str, Optional[str]],
):
    """Phase 5: Verify contracts remain upgraded after epoch transitions."""
    LOG.info("\n[Phase 5] Epoch change verification")
    node = cluster.get_node(config.node_name)
    w3 = node.w3

    epoch_start_block = node.get_block_number()
    LOG.info(f"  Current block: {epoch_start_block}, waiting {config.epoch_wait_blocks} more blocks (~2 epochs)...")

    epoch_reached = await wait_for_blocks_after(
        w3, epoch_start_block, config.epoch_wait_blocks,
        timeout=config.epoch_timeout,
    )
    assert epoch_reached, \
        f"Chain stalled during epoch change wait! Last: {node.get_block_number()}"

    LOG.info("Re-verifying contract bytecodes after epoch changes...")
    epoch_snapshot = _snapshot(w3, config.contracts)
    for name in changed:
        assert epoch_snapshot[name] == post_snapshot[name], \
            f"Contract {name} bytecode changed unexpectedly after epoch!"
    LOG.info("  Contract bytecodes stable across epoch boundaries")
    LOG.info("✅ Phase 5 PASSED: epoch change successful")


async def phase6_restart_replay(
    cluster: Cluster, config: HardforkTestConfig,
    changed: list, post_snapshot: Dict[str, Optional[str]],
):
    """Phase 6: Stop node, restart, and verify replay through hardfork."""
    LOG.info("\n[Phase 6] Node restart & replay verification")
    node = cluster.get_node(config.node_name)
    w3 = node.w3

    pre_stop_height = node.get_block_number()
    LOG.info(f"  Pre-stop block height: {pre_stop_height}")

    LOG.info("  Stopping node...")
    stop_ok = await node.stop()
    assert stop_ok, "Failed to stop node"

    state, _ = await node.get_state()
    assert state == NodeState.STOPPED, f"Node not stopped, state={state.name}"
    LOG.info("  ✅ Node stopped and verified")

    await asyncio.sleep(3)

    LOG.info("  Starting node (replay through hardfork)...")
    start_ok = await node.start()
    assert start_ok, "Failed to restart node"
    LOG.info("  ✅ Node restarted")

    LOG.info(f"  Verifying post-restart liveness ({config.post_restart_blocks} blocks)...")
    restart_height = node.get_block_number()
    LOG.info(f"  Restart block height: {restart_height}")

    restart_ok = await wait_for_blocks_after(
        w3, restart_height, config.post_restart_blocks,
        timeout=config.block_increase_timeout,
    )
    assert restart_ok, \
        f"Node failed to produce blocks after restart! Last: {node.get_block_number()}"

    LOG.info("  Verifying contract bytecodes after restart...")
    restart_snapshot = _snapshot(w3, config.contracts)
    for name in changed:
        assert restart_snapshot[name] == post_snapshot[name], \
            f"Contract {name} bytecode mismatch after restart replay!"
    LOG.info("  Contract bytecodes match after replay")

    sender = cluster.faucet
    if sender:
        LOG.info("  Sending final verification transactions...")
        ok, fail = await send_eth_transfers(
            w3, sender, num_txns=config.light_pressure_txn_count
        )
        assert ok > 0, "Post-restart: no transactions succeeded"
        LOG.info(f"  Post-restart txns: {ok} succeeded, {fail} failed")

    final_height = node.get_block_number()
    LOG.info("✅ Phase 6 PASSED: node restart & replay successful")
    return final_height


async def run_hardfork_lifecycle_test(
    cluster: Cluster, config: HardforkTestConfig
):
    """
    Run the full 6-phase hardfork lifecycle test.

    This is the main entry point for hardfork E2E tests. Each hardfork
    only needs to create a HardforkTestConfig and call this function.
    """
    LOG.info("=" * 70)
    LOG.info(f"🔱 {config.display_name} E2E Test")
    LOG.info(f"   {config.name}Block = {config.hardfork_block}")
    LOG.info(f"   Post-hardfork blocks = {config.post_hardfork_blocks}")
    LOG.info(f"   Post-restart blocks = {config.post_restart_blocks}")
    LOG.info(f"   Contracts to verify = {len(config.contracts)}")
    LOG.info("=" * 70)

    # Phase 1: Pre-hardfork validation
    await phase1_pre_hardfork_validation(cluster, config)

    # Phase 2: Pre-hardfork snapshot & light pressure
    pre_snapshot = await phase2_pre_hardfork_snapshot(cluster, config)

    # Phase 3: Hardfork transition
    await phase3_hardfork_transition(cluster, config)

    # Phase 4: Post-hardfork bytecode verification
    changed, post_snapshot = await phase4_bytecode_verification(
        cluster, config, pre_snapshot
    )

    # Phase 5: Epoch change verification
    await phase5_epoch_verification(cluster, config, changed, post_snapshot)

    # Phase 6: Node restart & replay
    final_height = await phase6_restart_replay(
        cluster, config, changed, post_snapshot
    )

    # Final Summary
    LOG.info("\n" + "=" * 70)
    LOG.info(f"🎉 ALL PHASES PASSED - {config.display_name} E2E Test")
    LOG.info(f"   {config.name}Block: {config.hardfork_block}")
    LOG.info(f"   Contracts upgraded: {len(changed)}")
    LOG.info(f"   Final block height: {final_height}")
    LOG.info(f"   Node restart + replay: ✅")
    LOG.info("=" * 70)
