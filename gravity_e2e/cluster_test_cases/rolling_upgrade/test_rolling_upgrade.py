"""
Rolling Upgrade Test

Verifies that a cluster initialized with an old binary can be rolling-upgraded
to the current code binary without downtime.

The initial binary, hardfork schedule, and genesis contracts are configured via
test_params.toml (see test_params.toml.example). Run render_config.py to
generate cluster.toml and genesis.toml before launching the test.

Test flow:
1. Bootstrap cluster with old binary (configured in test_params.toml)
2. Rolling-upgrade each node (vfn1 first) with continuous tx load (vfn1 → node1 failover during vfn1 upgrade)
3. Wait for all nodes to pass max hardfork block
4. Post-upgrade/hardfork health monitoring
"""

import asyncio
import logging
import math
import os
import statistics
import time

try:
    import tomllib
except ImportError:
    import tomli as tomllib

import pytest
from pathlib import Path
from web3 import Web3
from eth_account import Account

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.cluster.node import Node, NodeState

LOG = logging.getLogger(__name__)

TEST_DIR = Path(__file__).resolve().parent
PROJECT_ROOT = TEST_DIR.parent.parent.parent
GENESIS_TOML_PATH = TEST_DIR / "genesis.toml"

# New binary to upgrade to (default: current build)
NEW_BINARY_PATH = Path(
    os.environ.get(
        "GRAVITY_NEW_BINARY",
        str(PROJECT_ROOT / "target" / "quick-release" / "gravity_node"),
    )
)

# Maximum allowed block height gap between nodes
MAX_HEIGHT_GAP = 50

# Time to wait between upgrading individual nodes (seconds)
INTER_NODE_WAIT = 180  # 3 minutes

# Post-upgrade stabilization wait before monitoring (seconds)
POST_UPGRADE_STABILIZE_WAIT = 180  # 3 minutes

# Post-hardfork monitoring duration (seconds)
POST_HARDFORK_MONITOR_DURATION = 600  # 10 minutes

# Estimated block production rate (blocks/sec) for timeout calculation
BLOCK_RATE = 5

# Block height check retry parameters
HEIGHT_CHECK_MAX_RETRIES = 3
HEIGHT_CHECK_RETRY_INTERVAL = 20  # seconds

# Transaction sender parameters
TX_INTERVAL = 0.2  # 200ms between transactions
TX_RECEIPT_TIMEOUT = 30.0  # seconds to wait for receipt


class TxSender:
    """
    Background transaction sender that continuously sends txs to a target node.
    Supports switching targets (e.g., vfn1 → node1 during upgrade).
    """

    def __init__(
        self, cluster: Cluster, faucet, primary_node_id: str, fallback_node_id: str
    ):
        self.cluster = cluster
        self.faucet = faucet
        self.primary_node_id = primary_node_id
        self.fallback_node_id = fallback_node_id
        self.recipient = Account.create().address

        self._use_fallback = False
        self._stop_event = asyncio.Event()
        self._task: asyncio.Task | None = None

        # Stats
        self.total_sent = 0
        self.total_confirmed = 0
        self.total_failed = 0
        self.total_timeout = 0
        self.latencies: list[float] = []

    @property
    def _current_node_id(self) -> str:
        return self.fallback_node_id if self._use_fallback else self.primary_node_id

    @property
    def _current_w3(self) -> Web3:
        return self.cluster.get_node(self._current_node_id).w3

    def set_fallback(self, use_fallback: bool):
        """Switch between primary (vfn1) and fallback (node1) target."""
        old = self._current_node_id
        self._use_fallback = use_fallback
        LOG.info(f"📡 TxSender target: {old} → {self._current_node_id}")

    async def _send_loop(self):
        """Main send loop running in background."""
        chain_id = await asyncio.to_thread(lambda: self._current_w3.eth.chain_id)
        gas_price = Web3.to_wei("100", "gwei")
        nonce = await asyncio.to_thread(
            lambda: self._current_w3.eth.get_transaction_count(
                self.faucet.address, "pending"
            )
        )
        LOG.info(
            f"📡 TxSender started: target={self._current_node_id}, "
            f"nonce={nonce}, recipient={self.recipient}"
        )

        while not self._stop_event.is_set():
            w3 = self._current_w3
            tx = {
                "nonce": nonce,
                "to": self.recipient,
                "value": 0,
                "gas": 21000,
                "gasPrice": gas_price,
                "chainId": chain_id,
            }
            try:
                signed = w3.eth.account.sign_transaction(tx, self.faucet.key)
                send_time = time.monotonic()
                tx_hash = await asyncio.to_thread(
                    lambda: w3.eth.send_raw_transaction(signed.raw_transaction)
                )
                self.total_sent += 1
                nonce += 1

                # Wait for receipt
                confirmed = False
                while time.monotonic() - send_time < TX_RECEIPT_TIMEOUT:
                    try:
                        receipt = await asyncio.to_thread(
                            lambda: w3.eth.get_transaction_receipt(tx_hash)
                        )
                        if receipt:
                            latency = time.monotonic() - send_time
                            self.latencies.append(latency)
                            self.total_confirmed += 1
                            confirmed = True
                            break
                    except Exception:
                        pass
                    await asyncio.sleep(0.1)

                if not confirmed:
                    self.total_timeout += 1
                    LOG.warning(f"TxSender: tx {tx_hash.hex()[:10]}... timed out")
                    # Re-fetch nonce to recover from potential nonce gap
                    try:
                        nonce = await asyncio.to_thread(
                            lambda: self._current_w3.eth.get_transaction_count(
                                self.faucet.address, "pending"
                            )
                        )
                        LOG.info(f"TxSender: nonce re-synced to {nonce}")
                    except Exception:
                        pass

            except Exception as e:
                self.total_failed += 1
                LOG.warning(f"TxSender: send failed ({self._current_node_id}): {e}")
                # On send failure (e.g., node down), wait and retry
                await asyncio.sleep(1)
                # Re-fetch nonce from current target after failure
                try:
                    nonce = await asyncio.to_thread(
                        lambda: self._current_w3.eth.get_transaction_count(
                            self.faucet.address, "pending"
                        )
                    )
                except Exception:
                    pass
                continue

            await asyncio.sleep(TX_INTERVAL)

    def start(self):
        """Start background transaction sending."""
        self._task = asyncio.create_task(self._send_loop())

    async def stop(self):
        """Stop sending and wait for the task to finish."""
        self._stop_event.set()
        if self._task:
            await self._task

    def log_stats(self):
        """Log transaction statistics."""
        LOG.info("\n" + "=" * 60)
        LOG.info("TRANSACTION STATISTICS")
        LOG.info("=" * 60)
        LOG.info(f"Total Sent:      {self.total_sent}")
        LOG.info(f"Confirmed:       {self.total_confirmed}")
        LOG.info(f"Failed (send):   {self.total_failed}")
        LOG.info(f"Timed Out:       {self.total_timeout}")

        if self.total_sent > 0:
            success_rate = self.total_confirmed / self.total_sent * 100
            LOG.info(f"Success Rate:    {success_rate:.1f}%")

        if self.latencies:
            sorted_lat = sorted(self.latencies)

            def percentile(p: float) -> float:
                k = (len(sorted_lat) - 1) * (p / 100.0)
                f = math.floor(k)
                c = math.ceil(k)
                if f == c:
                    return sorted_lat[int(k)]
                return sorted_lat[f] + (sorted_lat[c] - sorted_lat[f]) * (k - f)

            LOG.info("-" * 60)
            LOG.info(f"Min Latency:     {sorted_lat[0]:.4f}s")
            LOG.info(f"Max Latency:     {sorted_lat[-1]:.4f}s")
            LOG.info(f"Avg Latency:     {statistics.mean(sorted_lat):.4f}s")
            LOG.info(f"P50 Latency:     {percentile(50):.4f}s")
            LOG.info(f"P90 Latency:     {percentile(90):.4f}s")
            LOG.info(f"P99 Latency:     {percentile(99):.4f}s")
        LOG.info("=" * 60 + "\n")


async def get_block_heights(nodes: list[Node]) -> dict[str, int]:
    """
    Query block heights for all given nodes concurrently.
    Returns dict of node_id -> block_height. Raises on any failure.
    """

    async def _get_height(node: Node) -> tuple[str, int]:
        height = await asyncio.to_thread(lambda: node.w3.eth.block_number)
        return node.id, height

    results = await asyncio.gather(*[_get_height(n) for n in nodes])
    return dict(results)


async def check_height_gap_ok(cluster: Cluster, max_gap: int = MAX_HEIGHT_GAP) -> bool:
    """
    Check if all running nodes have block heights within max_gap of each other.
    Returns True if the gap is acceptable.
    """
    running_nodes = []
    for node in cluster.nodes.values():
        state, _ = await node.get_state()
        if state == NodeState.RUNNING:
            running_nodes.append(node)

    if len(running_nodes) < 2:
        LOG.info("Less than 2 running nodes, height gap check trivially passes")
        return True

    heights = await get_block_heights(running_nodes)

    max_h = max(heights.values())
    min_h = min(heights.values())
    gap = max_h - min_h

    LOG.info(f"Block heights: {heights} | gap={gap} (max_allowed={max_gap})")

    if gap >= max_gap:
        LOG.warning(f"Block height gap {gap} >= {max_gap}, not safe to upgrade")
        return False

    return True


async def wait_for_height_gap_ok(
    cluster: Cluster,
    max_retries: int = HEIGHT_CHECK_MAX_RETRIES,
    retry_interval: int = HEIGHT_CHECK_RETRY_INTERVAL,
) -> bool:
    """
    Wait until block height gap is acceptable, with retries.
    Returns True if gap is OK within retries, False otherwise.
    """
    for attempt in range(1, max_retries + 1):
        if await check_height_gap_ok(cluster):
            return True
        if attempt < max_retries:
            LOG.info(
                f"Height gap too large, retrying in {retry_interval}s "
                f"(attempt {attempt}/{max_retries})..."
            )
            await asyncio.sleep(retry_interval)

    LOG.error(
        f"Block height gap still too large after {max_retries} retries, aborting upgrade"
    )
    return False


async def upgrade_node(node: Node, new_binary: Path):
    """
    Upgrade a single node: stop, replace binary, start.
    """
    node_id = node.id
    bin_path = node._infra_path / "bin" / "gravity_node"

    LOG.info(f"🔄 [{node_id}] Stopping node...")
    stopped = await node.stop()
    assert stopped, f"Failed to stop {node_id}"

    LOG.info(f"🔄 [{node_id}] Replacing binary: {bin_path}")
    # Remove old binary and hardlink new one (only this node gets the new binary)
    if bin_path.exists():
        bin_path.unlink()
    os.link(str(new_binary), str(bin_path))

    LOG.info(f"🔄 [{node_id}] Starting node with new binary...")
    started = await node.start()
    assert started, f"Failed to start {node_id} after upgrade"

    LOG.info(f"✅ [{node_id}] Upgrade complete")


def get_max_hardfork_block() -> int:
    """
    Read genesis.toml and return the maximum hardfork block number.
    Returns 0 if no hardforks are configured.
    """
    if not GENESIS_TOML_PATH.exists():
        LOG.warning(
            f"genesis.toml not found at {GENESIS_TOML_PATH}, skipping hardfork check"
        )
        return 0

    with open(GENESIS_TOML_PATH, "rb") as f:
        config = tomllib.load(f)

    hardforks = config.get("genesis", {}).get("hardforks", {})
    if not hardforks:
        LOG.info("No hardforks configured in genesis.toml")
        return 0

    max_block = max(hardforks.values())
    LOG.info(f"Hardfork config: {hardforks} | max hardfork block: {max_block}")
    return max_block


@pytest.mark.asyncio
async def test_rolling_upgrade(cluster: Cluster):
    """
    Test rolling upgrade from v1.1.1 to current binary.

    Steps:
    1. Ensure all nodes are running with old binary, start background tx sender
    2. Rolling-upgrade each node, vfn1 first (vfn1 → node1 failover for tx sender)
    3. Wait for all nodes to pass max hardfork block
    4. Post-upgrade/hardfork health monitoring, then print tx stats
    """
    LOG.info("=" * 70)
    LOG.info("Test: Rolling Upgrade (v1.1.1 → current)")
    LOG.info(f"New binary: {NEW_BINARY_PATH}")
    LOG.info("=" * 70)

    # Validate new binary exists
    assert NEW_BINARY_PATH.exists(), (
        f"New binary not found at {NEW_BINARY_PATH}. "
        f"Build it first: cargo build --profile quick-release -p gravity_node"
    )

    tx_sender: TxSender | None = None

    # ── Step 1: Bootstrap cluster with old binary ──
    LOG.info("\n[Step 1] Ensuring all nodes are running with old binary...")
    assert await cluster.set_full_live(
        timeout=120
    ), "Failed to bring all nodes to RUNNING state with old binary"

    live_nodes = await cluster.get_live_nodes()
    LOG.info(f"✅ All {len(live_nodes)} nodes running: {[n.id for n in live_nodes]}")

    # Verify block production
    assert await cluster.check_block_increasing(
        timeout=30
    ), "Block production not working with old binary"
    LOG.info("✅ Block production verified with old binary")

    # Start background transaction sender: vfn1 primary, node1 fallback
    tx_sender = TxSender(
        cluster, cluster.faucet, primary_node_id="vfn1", fallback_node_id="node1"
    )
    tx_sender.start()
    LOG.info("✅ Background transaction sender started (target: vfn1)")

    # ── Step 2: Rolling upgrade ──
    LOG.info("\n[Step 2] Starting rolling upgrade...")

    # Upgrade order: VFN first, then genesis nodes, then validators
    all_node_ids = list(cluster.nodes.keys())
    # Move vfn1 to the front so it upgrades first
    upgrade_order = sorted(all_node_ids, key=lambda nid: (0 if nid == "vfn1" else 1))
    LOG.info(f"Upgrade order: {upgrade_order}")

    for i, node_id in enumerate(upgrade_order):
        node = cluster.get_node(node_id)
        LOG.info(f"\n{'─' * 50}")
        LOG.info(
            f"Upgrading node {i + 1}/{len(upgrade_order)}: {node_id} (role={node.role.value})"
        )
        LOG.info(f"{'─' * 50}")

        # Pre-check: block height gap must be acceptable
        LOG.info(f"[{node_id}] Pre-upgrade block height gap check...")
        gap_ok = await wait_for_height_gap_ok(cluster)
        assert (
            gap_ok
        ), f"Block height gap too large before upgrading {node_id}, aborting test"

        # Switch tx sender to fallback when upgrading vfn1
        if node_id == "vfn1" and tx_sender:
            tx_sender.set_fallback(True)

        # Perform upgrade
        await upgrade_node(node, NEW_BINARY_PATH)

        # Switch tx sender back to vfn1 after it's upgraded
        if node_id == "vfn1" and tx_sender:
            tx_sender.set_fallback(False)

        # Wait before upgrading next node (skip wait after the last node)
        if i < len(upgrade_order) - 1:
            LOG.info(f"[{node_id}] Waiting {INTER_NODE_WAIT}s before next upgrade...")
            await asyncio.sleep(INTER_NODE_WAIT)

    LOG.info("\n✅ All nodes upgraded successfully!")

    # ── Step 3: Wait for max hardfork block ──
    max_hardfork_block = get_max_hardfork_block()
    if max_hardfork_block > 0:
        # Timeout: estimated time to reach max_hardfork_block at BLOCK_RATE, plus 5 min buffer
        timeout_secs = (max_hardfork_block / BLOCK_RATE) + 300
        LOG.info(
            f"\n[Step 3] Waiting for all nodes to pass hardfork block {max_hardfork_block} "
            f"(timeout={int(timeout_secs)}s)..."
        )

        wait_start = time.monotonic()
        check_count_hf = 0
        all_past_hardfork = False

        while time.monotonic() - wait_start < timeout_secs:
            await asyncio.sleep(10)
            check_count_hf += 1

            heights = await get_block_heights(list(cluster.nodes.values()))

            min_h = min(heights.values())
            max_h = max(heights.values())
            gap = max_h - min_h
            elapsed = int(time.monotonic() - wait_start)

            LOG.info(
                f"[Hardfork check #{check_count_hf} @ {elapsed}s] "
                f"min={min_h} max={max_h} gap={gap} target={max_hardfork_block}"
            )

            assert (
                gap < MAX_HEIGHT_GAP
            ), f"Block height gap {gap} >= {MAX_HEIGHT_GAP} while waiting for hardfork block"

            if min_h > max_hardfork_block:
                LOG.info(
                    f"✅ All nodes past hardfork block {max_hardfork_block} "
                    f"(min={min_h}) after {elapsed}s"
                )
                all_past_hardfork = True
                break

        assert all_past_hardfork, (
            f"Timed out waiting for all nodes to pass hardfork block {max_hardfork_block} "
            f"after {int(timeout_secs)}s"
        )
    else:
        LOG.info("\n[Step 3] No hardfork blocks configured, skipping hardfork wait")

    # ── Step 4: Post-upgrade/hardfork health monitoring ──
    LOG.info(
        f"\n[Step 4] Post-upgrade stabilization wait ({POST_UPGRADE_STABILIZE_WAIT}s)..."
    )
    await asyncio.sleep(POST_UPGRADE_STABILIZE_WAIT)

    LOG.info(f"\n[Step 4] Health monitoring ({POST_HARDFORK_MONITOR_DURATION}s)...")
    monitor_start = time.monotonic()
    check_count = 0

    while time.monotonic() - monitor_start < POST_HARDFORK_MONITOR_DURATION:
        await asyncio.sleep(10)
        check_count += 1

        heights = await get_block_heights(list(cluster.nodes.values()))
        max_h = max(heights.values())
        min_h = min(heights.values())
        gap = max_h - min_h
        elapsed = int(time.monotonic() - monitor_start)

        LOG.info(
            f"[Health check #{check_count} @ {elapsed}s] "
            f"min={min_h} max={max_h} gap={gap}"
        )

        assert (
            gap < MAX_HEIGHT_GAP
        ), f"Block height gap {gap} >= {MAX_HEIGHT_GAP} during post-upgrade monitoring"

    LOG.info(
        f"\n✅ Health monitoring completed: {check_count} checks over "
        f"{POST_HARDFORK_MONITOR_DURATION}s, all nodes healthy!"
    )

    # ── Final: Stop tx sender and print stats ──
    if tx_sender:
        await tx_sender.stop()
        tx_sender.log_stats()

    LOG.info("✅ Rolling upgrade test PASSED")
