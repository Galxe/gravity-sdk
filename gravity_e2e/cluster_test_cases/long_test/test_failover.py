"""
Failover Long-Running Stability Test

持续对 4 节点集群进行 failover 注入，验证：
- 节点被 kill 后集群仍能出块（BFT 安全）
- 节点重启后能追赶同步
- 链在反复 failover 下保持可用

用法:
    # 默认无限跑直到 Ctrl-C
    pytest test_failover.py -v -s

    # 指定时长 (秒)
    FAILOVER_DURATION=3600 pytest test_failover.py -v -s
"""

import asyncio
import logging
import os
import random
import signal
import time
from dataclasses import dataclass, field
from typing import List, Optional

import pytest
from web3 import Web3
from eth_account import Account

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.cluster.node import Node, NodeState

LOG = logging.getLogger(__name__)

# ── Configuration ────────────────────────────────────────────────────
# Default: 0 = run forever until SIGINT/SIGTERM
FAILOVER_DURATION = int(os.environ.get("FAILOVER_DURATION", "0"))

# Min nodes that must stay alive (BFT requires > 2/3, so 3 out of 4)
MIN_ALIVE_NODES = 3

# Block height gap threshold — if any node falls behind by more than this, fail
MAX_BLOCK_GAP = 200

# Time to wait for a restarted node to catch up (seconds)
CATCHUP_TIMEOUT = 120

# Interval between health checks (seconds)
HEALTH_CHECK_INTERVAL = 10

# Interval between failover rounds (seconds)
FAILOVER_INTERVAL_MIN = 10
FAILOVER_INTERVAL_MAX = 30

# Down time for killed node before restart (seconds)
DOWN_TIME_MIN = 10
DOWN_TIME_MAX = 30


@dataclass
class FailoverStats:
    """Track failover test statistics."""
    rounds: int = 0
    total_kills: int = 0
    total_restarts: int = 0
    restart_failures: int = 0
    catchup_failures: int = 0
    health_checks: int = 0
    max_observed_gap: int = 0
    start_time: float = field(default_factory=time.monotonic)

    @property
    def elapsed(self) -> float:
        return time.monotonic() - self.start_time

    def summary(self) -> str:
        elapsed_min = self.elapsed / 60
        return (
            f"📊 Failover Stats after {elapsed_min:.1f} min:\n"
            f"   Rounds: {self.rounds}\n"
            f"   Kills: {self.total_kills}, Restarts: {self.total_restarts}\n"
            f"   Restart failures: {self.restart_failures}\n"
            f"   Catchup failures: {self.catchup_failures}\n"
            f"   Health checks: {self.health_checks}\n"
            f"   Max observed block gap: {self.max_observed_gap}"
        )


class FailoverTestContext:
    """
    Context for failover long-running stability test.

    Manages:
    - Graceful signal handling (SIGINT/SIGTERM)
    - Duration-based or signal-based stopping
    - Failover injection loop
    - Health check monitoring loop
    """

    def __init__(self, cluster: Cluster, duration: int = 0):
        self.cluster = cluster
        self.duration = duration
        self.stats = FailoverStats()
        self._signal_received = False
        self._error: Optional[Exception] = None

        signal.signal(signal.SIGTERM, self._signal_handler)
        signal.signal(signal.SIGINT, self._signal_handler)

    def _signal_handler(self, signum, frame):
        self._signal_received = True
        LOG.info(f"🛑 Received {signal.Signals(signum).name}, stopping gracefully...")

    @property
    def should_stop(self) -> bool:
        if self._signal_received:
            return True
        if self._error is not None:
            return True
        if self.duration > 0 and self.stats.elapsed >= self.duration:
            LOG.info(f"⏰ Duration ({self.duration}s) reached, stopping...")
            return True
        return False

    def _set_error(self, e: Exception):
        """Set error to signal all loops to stop."""
        if self._error is None:
            self._error = e

    # ── Failover Injection Loop ──────────────────────────────────────

    async def failover_loop(self):
        """
        Main failover injection loop:
        1. Pick a random running node to kill (keep >= MIN_ALIVE_NODES alive)
        2. Stop it
        3. Wait random downtime
        4. Restart it
        5. Wait for catch-up
        6. Sleep before next round
        """
        try:
            while not self.should_stop:
                self.stats.rounds += 1
                LOG.info(f"\n{'='*60}")
                LOG.info(f"🔄 Failover Round {self.stats.rounds}")
                LOG.info(f"{'='*60}")

                # Get currently running nodes
                live_nodes = await self.cluster.get_live_nodes()
                live_ids = [n.id for n in live_nodes]
                LOG.info(f"Live nodes: {live_ids} ({len(live_ids)}/{len(self.cluster.nodes)})")

                if len(live_ids) <= MIN_ALIVE_NODES:
                    # Too few nodes alive, try to recover first
                    LOG.warning(
                        f"⚠️  Only {len(live_ids)} nodes alive (min={MIN_ALIVE_NODES}), "
                        f"recovering before next kill..."
                    )
                    await self._recover_all_nodes()
                    await asyncio.sleep(5)
                    continue

                # Pick a random node to kill
                victim = random.choice(live_nodes)
                LOG.info(f"🎯 Selected victim: {victim.id}")

                # Kill it
                LOG.info(f"💀 Stopping {victim.id}...")
                self.stats.total_kills += 1
                stop_ok = await victim.stop()
                if not stop_ok:
                    LOG.warning(f"⚠️  Stop command returned False for {victim.id}, continuing anyway")

                # Log remaining live nodes
                remaining = await self.cluster.get_live_nodes()
                LOG.info(
                    f"Remaining live nodes: {[n.id for n in remaining]} "
                    f"({len(remaining)}/{len(self.cluster.nodes)})"
                )

                # Wait random downtime
                down_time = random.uniform(DOWN_TIME_MIN, DOWN_TIME_MAX)
                LOG.info(f"⏳ {victim.id} will be down for {down_time:.1f}s...")

                # Poll should_stop during downtime
                waited = 0.0
                while waited < down_time and not self.should_stop:
                    await asyncio.sleep(min(2.0, down_time - waited))
                    waited += 2.0

                if self.should_stop:
                    # Recover the killed node before exiting
                    LOG.info(f"🔄 Recovering {victim.id} before exit...")
                    await victim.start()
                    break

                # Restart the victim
                LOG.info(f"🚀 Restarting {victim.id}...")
                self.stats.total_restarts += 1
                start_ok = await victim.start()
                if not start_ok:
                    LOG.error(f"❌ Failed to restart {victim.id}")
                    self.stats.restart_failures += 1
                    # Try once more
                    await asyncio.sleep(3)
                    start_ok = await victim.start()
                    if not start_ok:
                        self._set_error(RuntimeError(f"Failed to restart {victim.id} after retry"))
                        return

                # Wait for catch-up
                LOG.info(f"⏳ Waiting for {victim.id} to catch up (timeout={CATCHUP_TIMEOUT}s)...")
                caught_up = await self._wait_for_catchup(victim)
                if not caught_up:
                    LOG.error(f"❌ {victim.id} failed to catch up within {CATCHUP_TIMEOUT}s")
                    self.stats.catchup_failures += 1
                    # Don't fail immediately — let health check decide if gap is fatal
                else:
                    LOG.info(f"✅ {victim.id} caught up successfully")

                # Log current heights
                await self._log_all_heights()

                # Interval before next round
                interval = random.uniform(FAILOVER_INTERVAL_MIN, FAILOVER_INTERVAL_MAX)
                LOG.info(f"💤 Sleeping {interval:.1f}s before next failover round...")

                waited = 0.0
                while waited < interval and not self.should_stop:
                    await asyncio.sleep(min(2.0, interval - waited))
                    waited += 2.0

        except Exception as e:
            LOG.error(f"❌ Failover loop error: {e}")
            self._set_error(e)
            raise

    async def _wait_for_catchup(self, node: Node) -> bool:
        """Wait for a node to catch up to within MAX_BLOCK_GAP of the highest node."""
        start = time.monotonic()
        while time.monotonic() - start < CATCHUP_TIMEOUT:
            if self.should_stop:
                return True  # Don't block exit

            try:
                node_height = node.get_block_number()
            except Exception:
                await asyncio.sleep(2)
                continue

            # Get max height from other nodes
            max_height = 0
            for other in self.cluster.nodes.values():
                if other.id == node.id:
                    continue
                try:
                    h = other.get_block_number()
                    max_height = max(max_height, h)
                except Exception:
                    pass

            gap = max_height - node_height
            if gap <= MAX_BLOCK_GAP // 2:
                LOG.info(f"  {node.id} height={node_height}, max={max_height}, gap={gap} ✓")
                return True

            LOG.info(f"  {node.id} catching up: height={node_height}, max={max_height}, gap={gap}")
            await asyncio.sleep(3)

        return False

    async def _recover_all_nodes(self):
        """Try to bring all stopped nodes back online."""
        for node in self.cluster.nodes.values():
            state, _ = await node.get_state()
            if state != NodeState.RUNNING:
                LOG.info(f"🔄 Recovering {node.id} (state={state.name})...")
                await node.start()

    async def _log_all_heights(self):
        """Log block heights of all nodes."""
        heights = {}
        for nid, node in self.cluster.nodes.items():
            try:
                heights[nid] = node.get_block_number()
            except Exception:
                heights[nid] = -1
        LOG.info(f"📏 Block heights: {heights}")

    # ── Health Check Loop ────────────────────────────────────────────

    async def health_check_loop(self):
        """
        Periodically check:
        1. At least MIN_ALIVE_NODES are running
        2. Running nodes are producing blocks
        3. Block height gap between nodes is within threshold
        """
        try:
            prev_heights = {}
            stall_count = 0

            while not self.should_stop:
                await asyncio.sleep(HEALTH_CHECK_INTERVAL)
                self.stats.health_checks += 1

                # Gather heights
                current_heights = {}
                for nid, node in self.cluster.nodes.items():
                    try:
                        current_heights[nid] = node.get_block_number()
                    except Exception:
                        current_heights[nid] = -1

                # Filter running nodes
                running_heights = {
                    nid: h for nid, h in current_heights.items() if h >= 0
                }

                if len(running_heights) == 0:
                    LOG.error("❌ No nodes are responding!")
                    self._set_error(RuntimeError("All nodes are down"))
                    return

                # Check block progression on running nodes
                if prev_heights:
                    any_progress = False
                    for nid, h in running_heights.items():
                        prev = prev_heights.get(nid, -1)
                        if prev >= 0 and h > prev:
                            any_progress = True

                    if not any_progress and len(running_heights) >= MIN_ALIVE_NODES:
                        stall_count += 1
                        LOG.warning(
                            f"⚠️  No block progress detected (stall_count={stall_count}). "
                            f"Heights: {running_heights}"
                        )
                        if stall_count >= 6:  # 60s of no progress
                            self._set_error(
                                RuntimeError(
                                    f"Chain stalled for {stall_count * HEALTH_CHECK_INTERVAL}s! "
                                    f"Heights: {running_heights}"
                                )
                            )
                            return
                    else:
                        if stall_count > 0 and any_progress:
                            LOG.info(f"✅ Chain resumed after {stall_count} stall checks")
                        stall_count = 0

                # Check gap between running nodes
                if len(running_heights) >= 2:
                    max_h = max(running_heights.values())
                    min_h = min(running_heights.values())
                    gap = max_h - min_h
                    self.stats.max_observed_gap = max(self.stats.max_observed_gap, gap)

                    if gap > MAX_BLOCK_GAP:
                        LOG.error(
                            f"❌ Block height gap too large: {gap} (max={MAX_BLOCK_GAP}). "
                            f"Heights: {running_heights}"
                        )
                        self._set_error(
                            RuntimeError(
                                f"Block gap {gap} exceeds threshold {MAX_BLOCK_GAP}. "
                                f"Heights: {running_heights}"
                            )
                        )
                        return

                prev_heights = current_heights

                # Periodic stats log
                if self.stats.health_checks % 6 == 0:  # Every ~60s
                    LOG.info(self.stats.summary())
                    LOG.info(f"📏 Current heights: {running_heights}")

        except Exception as e:
            LOG.error(f"❌ Health check error: {e}")
            self._set_error(e)
            raise


# ── Test Entry Point ─────────────────────────────────────────────────


@pytest.mark.longrun
@pytest.mark.asyncio
async def test_failover_stability(cluster: Cluster):
    """
    Long-running failover stability test.

    Continuously injects random node failures and verifies:
    - Cluster maintains liveness (>= 3/4 nodes → consensus holds)
    - Killed nodes can restart and catch up
    - Block production never stalls for extended periods
    - Block height gap between nodes stays within bounds

    Runs indefinitely by default. Set FAILOVER_DURATION env var (seconds)
    or stop with Ctrl-C / SIGTERM.
    """
    LOG.info("=" * 70)
    LOG.info("🚀 Failover Long-Running Stability Test")
    if FAILOVER_DURATION > 0:
        LOG.info(
            f"   Duration: {FAILOVER_DURATION}s ({FAILOVER_DURATION / 60:.1f} min)"
        )
    else:
        LOG.info("   Duration: indefinite (Ctrl-C to stop)")
    LOG.info(f"   Nodes: {len(cluster.nodes)}")
    LOG.info(f"   Min alive: {MIN_ALIVE_NODES}")
    LOG.info(f"   Max block gap: {MAX_BLOCK_GAP}")
    LOG.info("=" * 70)

    ctx = FailoverTestContext(cluster, duration=FAILOVER_DURATION)

    # Step 1: Ensure all nodes are running
    LOG.info("\n[Step 1] Bringing all nodes online...")
    assert await cluster.set_full_live(timeout=120), "Failed to bring all nodes online"

    live = await cluster.get_live_nodes()
    LOG.info(f"✅ {len(live)} nodes are RUNNING: {[n.id for n in live]}")

    # Step 2: Verify initial block production
    LOG.info("\n[Step 2] Verifying initial block production...")
    assert await cluster.check_block_increasing(timeout=30, delta=3), (
        "Initial block production check failed"
    )
    LOG.info("✅ Blocks are being produced")

    # Step 3: Run failover + health check concurrently
    LOG.info("\n[Step 3] Starting failover injection and health monitoring...")
    tasks = [
        asyncio.create_task(ctx.failover_loop(), name="failover"),
        asyncio.create_task(ctx.health_check_loop(), name="health_check"),
    ]

    # Wait for all tasks — they exit on signal, duration, or error
    done, pending = await asyncio.wait(tasks, return_when=asyncio.FIRST_EXCEPTION)

    # Cancel remaining tasks
    for t in pending:
        t.cancel()
        try:
            await t
        except asyncio.CancelledError:
            pass

    # Check for errors
    for t in done:
        if t.exception() is not None:
            LOG.error(f"Task {t.get_name()} failed: {t.exception()}")

    # Step 4: Recovery — bring all nodes back online
    LOG.info("\n[Step 4] Recovering all nodes...")
    await cluster.set_full_live(timeout=60)

    # Final stats
    LOG.info("\n" + "=" * 70)
    LOG.info(ctx.stats.summary())
    LOG.info("=" * 70)

    # Raise if there was an error (not a signal-based stop)
    if ctx._error is not None and not ctx._signal_received:
        raise ctx._error

    LOG.info("✅ Failover stability test completed!")
