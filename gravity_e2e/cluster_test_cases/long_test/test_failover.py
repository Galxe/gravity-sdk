"""
Failover Long-Running Stability Test

4 validators + 1 VFN 集群持续 failover 注入 + bench 压测：
- 随机 kill 一个 validator，保持 >= 3 个 validator 在线（BFT 安全）
- 通过 VFN 节点检查出块
- 后台运行 gravity_bench 持续打压
- 支持无限跑直到 Ctrl-C 或指定时长

用法:
    # 无限跑直到 Ctrl-C
    pytest test_failover.py -v -s

    # 指定时长 (秒)
    FAILOVER_DURATION=3600 pytest test_failover.py -v -s
"""

import asyncio
import logging
import os
import random
import re
import signal
import subprocess
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import List, Optional

import pytest
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.cluster.node import Node, NodeRole, NodeState

LOG = logging.getLogger(__name__)

# ── Configuration ────────────────────────────────────────────────────
# Default: 0 = run forever until SIGINT/SIGTERM
FAILOVER_DURATION = int(os.environ.get("FAILOVER_DURATION", "0"))

# Min validators that must stay alive (BFT requires > 2/3, so 3 out of 4)
MIN_ALIVE_VALIDATORS = 3

# Block height gap threshold
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

# Bench target TPS (0 = unlimited)
BENCH_TARGET_TPS = int(os.environ.get("BENCH_TARGET_TPS", "1000"))


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

    Cluster topology: 4 genesis validators + 1 VFN.
    - Failover targets: only genesis validator nodes
    - Health checks: via VFN node (always alive) + validator heights
    - Bench: runs as a subprocess against VFN RPC
    """

    def __init__(self, cluster: Cluster, duration: int = 0):
        self.cluster = cluster
        self.duration = duration
        self.stats = FailoverStats()
        self._signal_received = False
        self._error: Optional[Exception] = None

        # Separate validators and VFN nodes
        self.validator_nodes: List[Node] = []
        self.vfn_nodes: List[Node] = []
        for node in cluster.nodes.values():
            if node.role in (NodeRole.GENESIS, NodeRole.VALIDATOR):
                self.validator_nodes.append(node)
            elif node.role == NodeRole.VFN:
                self.vfn_nodes.append(node)

        # Bench process handle
        self._bench_proc: Optional[subprocess.Popen] = None

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
        if self._error is None:
            self._error = e

    # ── Bench Management ─────────────────────────────────────────────

    def _setup_bench_log(self) -> Path:
        """
        Create a log file path inside the cluster artifacts dir.
        Returns the absolute path to the bench log file.
        """
        artifacts_dir = os.environ.get("GRAVITY_ARTIFACTS_DIR")
        if artifacts_dir:
            log_dir = Path(artifacts_dir)
        else:
            # Fallback: next to cluster.toml
            log_dir = self.cluster.config_path.parent / "bench_logs"
        log_dir.mkdir(parents=True, exist_ok=True)

        ts = time.strftime("%Y%m%d_%H%M%S")
        log_file = log_dir / f"gravity_bench_{ts}.log"
        return log_file

    def start_bench(self):
        """
        Start gravity_bench as a background subprocess.
        Bench stdout/stderr is redirected to a log file for analysis.
        """
        # bench_config.toml lives alongside cluster.toml
        config_path = self.cluster.config_path.parent / "bench_config.toml"
        if not config_path.exists():
            LOG.warning(f"⚠️  Bench config not found at {config_path}, skipping bench")
            return

        # Find gravity_bench directory
        project_root = self.cluster.config_path.parent.parent.parent.parent
        bench_dir = project_root / "external" / "gravity_bench"
        if not bench_dir.exists():
            LOG.warning(f"⚠️  gravity_bench not found at {bench_dir}, skipping bench")
            return

        self._bench_dir = bench_dir

        # Set up stdout log file (captures println! output)
        self._bench_log_path = self._setup_bench_log()
        self._bench_log_file = open(self._bench_log_path, "w")

        LOG.info(f"🏋️ Starting gravity_bench...")
        LOG.info(f"   Config: {config_path}")
        LOG.info(f"   Bench dir: {bench_dir}")
        LOG.info(f"   Stdout log: {self._bench_log_path}")

        env = os.environ.copy()
        env["RUST_LOG"] = env.get("RUST_LOG", "info")

        try:
            self._bench_proc = subprocess.Popen(
                [
                    "cargo", "run", "--release", "--quiet", "--",
                    "--config", str(config_path),
                ],
                cwd=str(bench_dir),
                env=env,
                stdout=self._bench_log_file,
                stderr=subprocess.STDOUT,
            )
            LOG.info(f"🏋️ Bench started (PID={self._bench_proc.pid})")
        except Exception as e:
            LOG.error(f"❌ Failed to start bench: {e}")
            self._bench_log_file.close()

    def stop_bench(self):
        """Stop the bench subprocess if running."""
        if self._bench_proc is None:
            return

        LOG.info("🏋️ Stopping bench...")
        try:
            self._bench_proc.terminate()
            try:
                self._bench_proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self._bench_proc.kill()
                self._bench_proc.wait(timeout=5)
            LOG.info("🏋️ Bench stopped")
        except Exception as e:
            LOG.warning(f"⚠️  Error stopping bench: {e}")
        finally:
            self._bench_proc = None
            if hasattr(self, "_bench_log_file") and self._bench_log_file:
                self._bench_log_file.close()
                self._bench_log_file = None

    def check_bench_alive(self) -> bool:
        """Check if the bench subprocess is still running."""
        if self._bench_proc is None:
            return False
        return self._bench_proc.poll() is None

    def _find_bench_log(self) -> Optional[Path]:
        """Find the latest bench tracing log file (log.*.log) in _bench_dir."""
        if not hasattr(self, "_bench_dir") or not self._bench_dir:
            return None
        logs = sorted(self._bench_dir.glob("log.*.log"), key=lambda p: p.stat().st_mtime)
        return logs[-1] if logs else None

    def check_bench_progress(self) -> Optional[str]:
        """
        Parse bench's own tracing log file to extract txn progress.
        Tails the latest log.*.log in the bench dir and extracts:
        Progress, TPS, Pending, Send/Exec Failures from the txn_tracker table.
        """
        try:
            log_file = self._find_bench_log()
            if not log_file:
                return None

            file_size = log_file.stat().st_size
            if file_size == 0:
                return "log empty"

            # Read last 8KB for the most recent table
            read_bytes = min(file_size, 8192)
            with open(log_file, "r", errors="replace") as f:
                if file_size > read_bytes:
                    f.seek(file_size - read_bytes)
                tail = f.read()

            # Determine phase
            phase = "starting"
            if "Plan execution failed" in tail:
                phase = "ERROR"
            elif "bench erc20 transfer" in tail or "bench uniswap" in tail:
                phase = "TX"
            elif "faucet distribution" in tail:
                levels = re.findall(r"faucet distribution for LEVEL (\d+)", tail)
                phase = f"faucet-L{levels[-1]}" if levels else "faucet"
            elif "Starting in" in tail:
                phase = "init"

            # Extract metrics from the LAST txn_tracker table
            # Table rows: │ Progress        ┆ 10/10 ┆ TPS           ┆ 0.6   │
            progress = tps = pending = send_fail = exec_fail = None

            for m in re.finditer(r'│ Progress\s+┆\s+(\S+)', tail):
                progress = m.group(1)
            for m in re.finditer(r'TPS\s+┆\s+(\S+)', tail):
                tps = m.group(1)
            for m in re.finditer(r'Pending Txns\s+┆\s+(\S+)', tail):
                pending = m.group(1)
            for m in re.finditer(r'Send Failures\s+┆\s+(\S+)', tail):
                send_fail = m.group(1)
            for m in re.finditer(r'Exec Failures\s+┆\s+(\S+)', tail):
                exec_fail = m.group(1)

            if progress is not None:
                parts = [f"phase={phase}", f"progress={progress}"]
                if tps is not None:
                    parts.append(f"tps={tps}")
                if pending and pending != "0":
                    parts.append(f"pending={pending}")
                if send_fail and send_fail != "0":
                    parts.append(f"send_fail={send_fail}")
                if exec_fail and exec_fail != "0":
                    parts.append(f"exec_fail={exec_fail}")
                return ", ".join(parts)
            else:
                return f"{phase} (no txn table yet)"
        except Exception as e:
            return f"read error: {e}"

    # ── Failover Injection Loop ──────────────────────────────────────

    async def failover_loop(self):
        """
        Main failover injection loop — only kills validator nodes, never VFN.
        """
        try:
            while not self.should_stop:
                self.stats.rounds += 1
                LOG.info(f"\n{'='*60}")
                LOG.info(f"🔄 Failover Round {self.stats.rounds}")
                LOG.info(f"{'='*60}")

                # Get currently running validators
                live_validators = []
                for node in self.validator_nodes:
                    state, _ = await node.get_state()
                    if state == NodeState.RUNNING:
                        live_validators.append(node)

                LOG.info(
                    f"Live validators: {[n.id for n in live_validators]} "
                    f"({len(live_validators)}/{len(self.validator_nodes)})"
                )

                if len(live_validators) <= MIN_ALIVE_VALIDATORS:
                    LOG.warning(
                        f"⚠️  Only {len(live_validators)} validators alive "
                        f"(min={MIN_ALIVE_VALIDATORS}), recovering first..."
                    )
                    await self._recover_validators()
                    await asyncio.sleep(5)
                    continue

                # Pick a random validator to kill
                victim = random.choice(live_validators)
                LOG.info(f"🎯 Selected victim: {victim.id} (role={victim.role.value})")

                # Kill it
                LOG.info(f"💀 Stopping {victim.id}...")
                self.stats.total_kills += 1
                stop_ok = await victim.stop()
                if not stop_ok:
                    LOG.warning(
                        f"⚠️  Stop returned False for {victim.id}, "
                        f"retrying with force..."
                    )
                    # Force kill: send SIGKILL directly via PID
                    await self._force_kill_node(victim)

                # Verify the node is truly dead
                await self._verify_node_stopped(victim)

                # Log remaining
                remaining = []
                for node in self.validator_nodes:
                    state, _ = await node.get_state()
                    if state == NodeState.RUNNING:
                        remaining.append(node.id)
                LOG.info(f"Remaining validators: {remaining}")

                # Check bench status
                if self._bench_proc:
                    bench_ok = self.check_bench_alive()
                    LOG.info(f"🏋️ Bench alive: {bench_ok}")
                    if not bench_ok:
                        LOG.warning("⚠️  Bench process died, restarting...")
                        self.start_bench()

                # Wait random downtime
                down_time = random.uniform(DOWN_TIME_MIN, DOWN_TIME_MAX)
                LOG.info(f"⏳ {victim.id} will be down for {down_time:.1f}s...")

                waited = 0.0
                while waited < down_time and not self.should_stop:
                    await asyncio.sleep(min(2.0, down_time - waited))
                    waited += 2.0

                if self.should_stop:
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
                    await asyncio.sleep(3)
                    start_ok = await victim.start()
                    if not start_ok:
                        self._set_error(
                            RuntimeError(f"Failed to restart {victim.id} after retry")
                        )
                        return

                # Wait for catch-up
                LOG.info(
                    f"⏳ Waiting for {victim.id} to catch up "
                    f"(timeout={CATCHUP_TIMEOUT}s)..."
                )
                caught_up = await self._wait_for_catchup(victim)
                if not caught_up:
                    LOG.error(
                        f"❌ {victim.id} failed to catch up within {CATCHUP_TIMEOUT}s"
                    )
                    self.stats.catchup_failures += 1
                else:
                    LOG.info(f"✅ {victim.id} caught up successfully")

                await self._log_all_heights()

                # Interval before next round
                interval = random.uniform(FAILOVER_INTERVAL_MIN, FAILOVER_INTERVAL_MAX)
                LOG.info(f"💤 Sleeping {interval:.1f}s before next round...")

                waited = 0.0
                while waited < interval and not self.should_stop:
                    await asyncio.sleep(min(2.0, interval - waited))
                    waited += 2.0

        except Exception as e:
            LOG.error(f"❌ Failover loop error: {e}")
            self._set_error(e)
            raise

    async def _wait_for_catchup(self, node: Node) -> bool:
        """Wait for a node to catch up to within acceptable gap."""
        start = time.monotonic()
        while time.monotonic() - start < CATCHUP_TIMEOUT:
            if self.should_stop:
                return True

            try:
                node_height = node.get_block_number()
            except Exception:
                await asyncio.sleep(2)
                continue

            # Get max height from other running nodes (validators + VFN)
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
                LOG.info(
                    f"  {node.id} height={node_height}, max={max_height}, gap={gap} ✓"
                )
                return True

            LOG.info(
                f"  {node.id} catching up: height={node_height}, "
                f"max={max_height}, gap={gap}"
            )
            await asyncio.sleep(3)

        return False

    async def _recover_validators(self):
        """Bring all stopped validators back online."""
        for node in self.validator_nodes:
            state, _ = await node.get_state()
            if state != NodeState.RUNNING:
                LOG.info(f"🔄 Recovering {node.id} (state={state.name})...")
                await node.start()

    async def _force_kill_node(self, node: Node):
        """
        Force kill a node by sending SIGKILL to its PID.
        Used when graceful stop fails.
        """
        if not node.pid_file.exists():
            LOG.warning(f"  No PID file for {node.id}, cannot force kill")
            return
        try:
            pid = int(node.pid_file.read_text().strip())
            LOG.warning(f"  Sending SIGKILL to {node.id} (PID={pid})...")
            os.kill(pid, signal.SIGKILL)
            await asyncio.sleep(1)
            node.pid_file.unlink(missing_ok=True)
        except (ValueError, ProcessLookupError, OSError) as e:
            LOG.info(f"  {node.id} PID already gone: {e}")

    async def _verify_node_stopped(
        self, node: Node, timeout: float = 10.0
    ) -> bool:
        """
        Verify that a node's process is truly dead.
        Polls PID existence and RPC unresponsiveness.
        Returns True if confirmed stopped, False if still alive.
        """
        start = time.monotonic()
        while time.monotonic() - start < timeout:
            state, _ = await node.get_state()
            if state == NodeState.STOPPED:
                LOG.info(f"  ✓ {node.id} verified STOPPED")
                return True
            LOG.debug(f"  {node.id} still {state.name}, waiting...")
            await asyncio.sleep(0.5)

        # Final check
        state, _ = await node.get_state()
        if state != NodeState.STOPPED:
            LOG.error(
                f"  ✗ {node.id} still {state.name} after {timeout}s! "
                f"Force killing..."
            )
            await self._force_kill_node(node)
            await asyncio.sleep(1)
            state, _ = await node.get_state()
            if state != NodeState.STOPPED:
                LOG.error(f"  ✗ {node.id} STILL not stopped after SIGKILL!")
                return False
        return True

    async def _log_all_heights(self):
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
        1. VFN is syncing (block height increasing)
        2. Validators are producing blocks
        3. Block height gap between nodes is within threshold
        4. Bench process is alive
        """
        try:
            prev_heights = {}
            stall_count = 0

            while not self.should_stop:
                await asyncio.sleep(HEALTH_CHECK_INTERVAL)
                self.stats.health_checks += 1

                # Gather heights from all nodes
                current_heights = {}
                for nid, node in self.cluster.nodes.items():
                    try:
                        current_heights[nid] = node.get_block_number()
                    except Exception:
                        current_heights[nid] = -1

                running_heights = {
                    nid: h for nid, h in current_heights.items() if h >= 0
                }

                if len(running_heights) == 0:
                    LOG.error("❌ No nodes are responding!")
                    self._set_error(RuntimeError("All nodes are down"))
                    return

                # Check VFN specifically — it should always be alive and syncing
                vfn_heights = {}
                for vfn in self.vfn_nodes:
                    h = current_heights.get(vfn.id, -1)
                    if h >= 0:
                        vfn_heights[vfn.id] = h

                if self.vfn_nodes and not vfn_heights:
                    LOG.error("❌ VFN node is not responding!")
                    self._set_error(RuntimeError("VFN node is down"))
                    return

                # Check block progression
                if prev_heights:
                    any_progress = False
                    for nid, h in running_heights.items():
                        prev = prev_heights.get(nid, -1)
                        if prev >= 0 and h > prev:
                            any_progress = True

                    # Count live validators
                    live_validator_count = sum(
                        1
                        for n in self.validator_nodes
                        if current_heights.get(n.id, -1) >= 0
                    )

                    if not any_progress and live_validator_count >= MIN_ALIVE_VALIDATORS:
                        stall_count += 1
                        LOG.warning(
                            f"⚠️  No block progress (stall_count={stall_count}). "
                            f"Heights: {running_heights}"
                        )
                        if stall_count >= 6:  # 60s of no progress
                            self._set_error(
                                RuntimeError(
                                    f"Chain stalled for "
                                    f"{stall_count * HEALTH_CHECK_INTERVAL}s! "
                                    f"Heights: {running_heights}"
                                )
                            )
                            return
                    else:
                        if stall_count > 0 and any_progress:
                            LOG.info(
                                f"✅ Chain resumed after {stall_count} stall checks"
                            )
                        stall_count = 0

                # Check gap between running nodes
                if len(running_heights) >= 2:
                    max_h = max(running_heights.values())
                    min_h = min(running_heights.values())
                    gap = max_h - min_h
                    self.stats.max_observed_gap = max(
                        self.stats.max_observed_gap, gap
                    )

                    if gap > MAX_BLOCK_GAP:
                        LOG.error(
                            f"❌ Block gap too large: {gap} (max={MAX_BLOCK_GAP}). "
                            f"Heights: {running_heights}"
                        )
                        self._set_error(
                            RuntimeError(
                                f"Block gap {gap} exceeds {MAX_BLOCK_GAP}. "
                                f"Heights: {running_heights}"
                            )
                        )
                        return

                prev_heights = current_heights

                # Periodic stats log (every ~60s)
                if self.stats.health_checks % 6 == 0:
                    bench_alive = self.check_bench_alive()
                    bench_progress = self.check_bench_progress()
                    bench_status = (
                        f"alive ({bench_progress})"
                        if bench_alive
                        else f"dead/not started (last: {bench_progress})"
                    )
                    LOG.info(self.stats.summary())
                    LOG.info(f"📏 Heights: {running_heights}")
                    LOG.info(f"🏋️ Bench: {bench_status}")

        except Exception as e:
            LOG.error(f"❌ Health check error: {e}")
            self._set_error(e)
            raise


# ── Test Entry Point ─────────────────────────────────────────────────


@pytest.mark.longrun
@pytest.mark.asyncio
async def test_failover_stability(cluster: Cluster):
    """
    Long-running failover stability test with bench load.

    Topology: 4 genesis validators + 1 VFN
    - Continuously injects random validator failures
    - Runs gravity_bench in background for tx load
    - Monitors block production via VFN + validators
    - Verifies catch-up after restart

    Runs indefinitely by default. Set FAILOVER_DURATION env var (seconds)
    or stop with Ctrl-C / SIGTERM.
    """
    # Classify nodes
    validators = [
        n for n in cluster.nodes.values()
        if n.role in (NodeRole.GENESIS, NodeRole.VALIDATOR)
    ]
    vfns = [n for n in cluster.nodes.values() if n.role == NodeRole.VFN]

    LOG.info("=" * 70)
    LOG.info("🚀 Failover Long-Running Stability Test")
    if FAILOVER_DURATION > 0:
        LOG.info(
            f"   Duration: {FAILOVER_DURATION}s ({FAILOVER_DURATION / 60:.1f} min)"
        )
    else:
        LOG.info("   Duration: indefinite (Ctrl-C to stop)")
    LOG.info(f"   Validators: {len(validators)} ({[n.id for n in validators]})")
    LOG.info(f"   VFNs: {len(vfns)} ({[n.id for n in vfns]})")
    LOG.info(f"   Min alive validators: {MIN_ALIVE_VALIDATORS}")
    LOG.info(f"   Max block gap: {MAX_BLOCK_GAP}")
    LOG.info(f"   Bench target TPS: {BENCH_TARGET_TPS}")
    LOG.info("=" * 70)

    ctx = FailoverTestContext(cluster, duration=FAILOVER_DURATION)

    # Step 1: Bring all nodes online
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

    # Step 3: Start bench
    LOG.info("\n[Step 3] Starting bench load...")
    ctx.start_bench()
    if ctx.check_bench_alive():
        LOG.info("✅ Bench is running")
    else:
        LOG.warning("⚠️  Bench not running (may not be set up yet)")

    # Step 4: Run failover + health check concurrently
    LOG.info("\n[Step 4] Starting failover injection and health monitoring...")
    tasks = [
        asyncio.create_task(ctx.failover_loop(), name="failover"),
        asyncio.create_task(ctx.health_check_loop(), name="health_check"),
    ]

    done, pending = await asyncio.wait(tasks, return_when=asyncio.FIRST_EXCEPTION)

    for t in pending:
        t.cancel()
        try:
            await t
        except asyncio.CancelledError:
            pass

    for t in done:
        if t.exception() is not None:
            LOG.error(f"Task {t.get_name()} failed: {t.exception()}")

    # Step 5: Cleanup
    LOG.info("\n[Step 5] Cleanup...")
    ctx.stop_bench()

    LOG.info("Recovering all nodes...")
    await cluster.set_full_live(timeout=60)

    # Final stats
    LOG.info("\n" + "=" * 70)
    LOG.info(ctx.stats.summary())
    LOG.info("=" * 70)

    if ctx._error is not None and not ctx._signal_received:
        raise ctx._error

    LOG.info("✅ Failover stability test completed!")
