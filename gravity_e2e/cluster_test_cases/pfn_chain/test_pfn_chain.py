"""
PFN Fan-out + Redundant Leaf Topology Test

Topology:
                      +--- pfn1 <----+
                      |              |
   node1 <-Vfn- vfn1--+              +--- pfn3
                      |              |
                      +--- pfn2 <----+

pfn1 / pfn2 are sibling PFNs (each independently dials vfn1 on Public).
pfn3 is the leaf with TWO redundant upstreams (dials both pfn1 and pfn2).
pfn1 / pfn2 each register pfn3 as a Downstream peer (trusted_peers entry,
no active dial — Downstream is excluded from upstream_roles for Public;
see network_id.rs:173-188).

What this test verifies:
1. PFN-as-upstream-for-PFN sync (pfn3's only upstreams are PFNs).
2. Role auto-inference branch `from = "<pfn>"` (deploy.sh:189).
3. Explicit Downstream-role seed entries on pfn1/pfn2.
4. Tx forwarding from PFN's RPC end-to-end: tx submitted via pfn3's
   eth_sendRawTransaction must propagate pfn3 -> (pfn1 or pfn2) -> vfn1
   -> node1 (mempool broadcast on Public network) and land in a block.
5. Redundant-upstream failover under load: Phase 1 stops pfn1 then pfn2
   in sequence (never both at the same time, only when steady), pfn3
   must keep producing receipts via the surviving upstream.

See _local/drafts/pfn/pfn-chain-test-plan.md for the full design rationale,
including §10's analysis of why we expect confirms to drop during stop
windows (gravity's mempool override defeats upstream's multi-peer broadcast).
"""

import asyncio
import logging
import math
import pathlib
import statistics
import time
from dataclasses import dataclass

import pytest
from eth_account import Account
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.cluster.node import Node

LOG = logging.getLogger(__name__)

MAX_HEIGHT_GAP = 50              # runtime tolerance — absorbs transient spikes
STEADY_GAP_THRESHOLD = 10        # tighter — used to gate "ok to stop a node"
STEADY_WINDOW = 3                # consecutive samples meeting threshold
STEADY_POLL_INTERVAL = 5         # seconds between steady-check samples
MONITOR_INTERVAL = 10            # seconds between general height samples
TX_INTERVAL = 0.2                # ~5 tx/s
TX_RECEIPT_TIMEOUT = 30.0
PFN_FORWARD_RECEIPT_TIMEOUT = 60.0   # Phase 0 / post-restart probe budget
PFN_DOWN_DURATION = 30           # seconds each PFN stays stopped
CATCHUP_TIMEOUT = PFN_DOWN_DURATION * 4   # 120s budget per plan §7.4
POST_TAIL_DURATION = 60          # final monitor window after Phase 1b
STEADY_INITIAL_TIMEOUT = 60      # max wait for first steady before any stops
# No absolute confirm-count threshold: TxSender's loop ensures
# `total_sent == total_confirmed + total_timeout` after stop(), so the
# meaningful health check is "did anything actually fail" — see Phase 1
# assertions at the end of the test.
NODE_IDS = ("node1", "vfn1", "pfn1", "pfn2", "pfn3")


@dataclass
class TxSnap:
    sent: int
    confirmed: int
    timeout: int
    failed: int

    def __sub__(self, other: "TxSnap") -> "TxSnap":
        return TxSnap(
            sent=self.sent - other.sent,
            confirmed=self.confirmed - other.confirmed,
            timeout=self.timeout - other.timeout,
            failed=self.failed - other.failed,
        )

    def __str__(self) -> str:
        return (
            f"sent={self.sent} confirmed={self.confirmed} "
            f"timeout={self.timeout} failed={self.failed}"
        )


class TxSender:
    """Continuously send txs to a target node; track submit/confirm stats."""

    def __init__(self, cluster: Cluster, faucet, target_node_id: str):
        self.cluster = cluster
        self.faucet = faucet
        self.target_node_id = target_node_id
        self.recipient = Account.create().address

        self._stop_event = asyncio.Event()
        self._task: asyncio.Task | None = None

        self.total_sent = 0
        self.total_confirmed = 0
        self.total_failed = 0
        self.total_timeout = 0
        self.latencies: list[float] = []
        # Per-tx tracking for post-mortem hash correlation against node logs.
        self.sent_hashes: list[str] = []
        self.confirmed_hashes: set[str] = set()
        self.timeout_hashes: list[str] = []

    @property
    def in_flight_hashes(self) -> list[str]:
        """Hashes that have been sent but not yet confirmed or timed out."""
        return [h for h in self.sent_hashes
                if h not in self.confirmed_hashes
                and h not in self.timeout_hashes]

    @property
    def _w3(self) -> Web3:
        return self.cluster.get_node(self.target_node_id).w3

    def snapshot(self) -> TxSnap:
        return TxSnap(
            sent=self.total_sent,
            confirmed=self.total_confirmed,
            timeout=self.total_timeout,
            failed=self.total_failed,
        )

    async def _send_loop(self):
        w3 = self._w3
        chain_id = await asyncio.to_thread(lambda: w3.eth.chain_id)
        gas_price = Web3.to_wei("2", "gwei")
        nonce = await asyncio.to_thread(
            lambda: w3.eth.get_transaction_count(self.faucet.address, "pending")
        )
        LOG.info(
            f"TxSender started: target={self.target_node_id}, nonce={nonce}, "
            f"recipient={self.recipient}"
        )

        while not self._stop_event.is_set():
            w3 = self._w3
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
                tx_hash_hex = tx_hash.hex()
                self.total_sent += 1
                self.sent_hashes.append(tx_hash_hex)
                nonce += 1

                confirmed = False
                while time.monotonic() - send_time < TX_RECEIPT_TIMEOUT:
                    try:
                        receipt = await asyncio.to_thread(
                            lambda: w3.eth.get_transaction_receipt(tx_hash)
                        )
                        if receipt:
                            self.latencies.append(time.monotonic() - send_time)
                            self.total_confirmed += 1
                            self.confirmed_hashes.add(tx_hash_hex)
                            confirmed = True
                            break
                    except Exception:
                        pass
                    await asyncio.sleep(0.1)

                if not confirmed:
                    self.total_timeout += 1
                    self.timeout_hashes.append(tx_hash_hex)
                    LOG.warning(f"[txsender] TIMEOUT tx_hash=0x{tx_hash_hex}")
                    try:
                        nonce = await asyncio.to_thread(
                            lambda: self._w3.eth.get_transaction_count(
                                self.faucet.address, "pending"
                            )
                        )
                    except Exception:
                        pass

            except Exception as e:
                self.total_failed += 1
                LOG.warning(f"TxSender send failed ({self.target_node_id}): {e}")
                await asyncio.sleep(1)
                try:
                    nonce = await asyncio.to_thread(
                        lambda: self._w3.eth.get_transaction_count(
                            self.faucet.address, "pending"
                        )
                    )
                except Exception:
                    pass
                continue

            await asyncio.sleep(TX_INTERVAL)

    def start(self):
        self._task = asyncio.create_task(self._send_loop())

    async def stop(self):
        self._stop_event.set()
        if self._task:
            await self._task

    def log_stats(self):
        LOG.info("=" * 60)
        LOG.info("TRANSACTION STATISTICS")
        LOG.info("=" * 60)
        LOG.info(f"Total Sent:    {self.total_sent}")
        LOG.info(f"Confirmed:     {self.total_confirmed}")
        LOG.info(f"Failed (send): {self.total_failed}")
        LOG.info(f"Timed Out:     {self.total_timeout}")
        if self.total_sent > 0:
            LOG.info(
                f"Success Rate:  {self.total_confirmed / self.total_sent * 100:.1f}%"
            )
        if self.latencies:
            sl = sorted(self.latencies)

            def pct(p):
                k = (len(sl) - 1) * (p / 100.0)
                f, c = math.floor(k), math.ceil(k)
                return sl[int(k)] if f == c else sl[f] + (sl[c] - sl[f]) * (k - f)

            LOG.info(f"Latency p50/p90/p99: {pct(50):.3f}s / {pct(90):.3f}s / {pct(99):.3f}s")
            LOG.info(f"Latency min/max/avg: {sl[0]:.3f}s / {sl[-1]:.3f}s / {statistics.mean(sl):.3f}s")
        LOG.info("=" * 60)


async def _safe_block_number(node: Node) -> int | None:
    """Fetch a node's block number; return None on RPC failure (node down / blip)."""
    try:
        return await asyncio.to_thread(lambda: node.w3.eth.block_number)
    except Exception:
        return None


async def _get_live_heights(
    cluster: Cluster, excluded: set[str]
) -> dict[str, int]:
    """
    Fetch heights from all nodes not in `excluded`. RPC blips on a "live" node
    are silently dropped from this round (logged as WARN), not raised.
    """
    live_ids = [nid for nid in NODE_IDS if nid not in excluded]
    nodes = [cluster.get_node(nid) for nid in live_ids]
    results = await asyncio.gather(*[_safe_block_number(n) for n in nodes])
    out: dict[str, int] = {}
    for nid, h in zip(live_ids, results):
        if h is None:
            LOG.warning(f"[heights] {nid} RPC unreachable this round, skipping")
        else:
            out[nid] = h
    return out


def _node_log_text(node: Node) -> str:
    """Read consensus log (vfn.log for VFN/PFN, validator.log for validator)."""
    base = pathlib.Path(node._infra_path) / "consensus_log"
    for name in ("vfn.log", "validator.log"):
        p = base / name
        if p.exists():
            return p.read_text(errors="replace")
    return ""


async def _wait_steady(
    cluster: Cluster,
    excluded: set[str],
    timeout: float,
    label: str,
) -> bool:
    """
    Wait until live nodes are in steady state: STEADY_WINDOW consecutive
    samples where (a) live gap < STEADY_GAP_THRESHOLD and (b) every live
    node's height advanced from the previous sample.

    Returns True if reached, False if timeout.
    """
    deadline = time.monotonic() + timeout
    consecutive = 0
    last_heights: dict[str, int] = {}

    while time.monotonic() < deadline:
        heights = await _get_live_heights(cluster, excluded)
        if len(heights) < 2:
            LOG.info(f"[{label}] only {len(heights)} live node(s); skipping gap check")
            await asyncio.sleep(STEADY_POLL_INTERVAL)
            continue

        gap = max(heights.values()) - min(heights.values())
        all_advanced = all(
            nid in last_heights and heights[nid] > last_heights[nid]
            for nid in heights
        ) if last_heights else False
        steady = gap < STEADY_GAP_THRESHOLD and all_advanced

        LOG.info(
            f"[{label}] heights={heights} gap={gap} "
            f"advanced={all_advanced} consecutive={consecutive}/{STEADY_WINDOW}"
        )
        consecutive = consecutive + 1 if steady else 0
        last_heights = heights

        if consecutive >= STEADY_WINDOW:
            return True
        await asyncio.sleep(STEADY_POLL_INTERVAL)

    return False


async def _monitor_window(
    cluster: Cluster,
    excluded: set[str],
    duration: float,
    label: str,
):
    """
    Sample heights every MONITOR_INTERVAL seconds for `duration` seconds.
    Asserts gap < MAX_HEIGHT_GAP among live nodes; advancement is logged
    but not asserted (a victim's restart may make catchup look stalled
    briefly even with the loose threshold).
    """
    end = time.monotonic() + duration
    while time.monotonic() < end:
        await asyncio.sleep(MONITOR_INTERVAL)
        heights = await _get_live_heights(cluster, excluded)
        if len(heights) < 2:
            LOG.info(f"[{label}] only {len(heights)} live; skipping gap")
            continue
        gap = max(heights.values()) - min(heights.values())
        LOG.info(f"[{label}] heights={heights} gap={gap}")
        assert gap < MAX_HEIGHT_GAP, (
            f"[{label}] gap {gap} >= {MAX_HEIGHT_GAP}: {heights}"
        )


async def _send_tx_via_pfn_and_assert_inclusion(
    cluster: Cluster, sender_node_id: str
) -> tuple[str, int]:
    """
    Submit one self-funded eth tx to `sender_node_id`'s RPC, wait for receipt,
    assert it landed in a block, AND assert validator (node1) sees the same
    tx at the same block. Strongest single-shot proof of end-to-end forwarding.
    """
    pfn = cluster.get_node(sender_node_id)
    validator = cluster.get_node("node1")
    faucet = cluster.faucet

    w3 = pfn.w3
    chain_id = await asyncio.to_thread(lambda: w3.eth.chain_id)
    gas_price = Web3.to_wei("2", "gwei")
    nonce = await asyncio.to_thread(
        lambda: w3.eth.get_transaction_count(faucet.address, "pending")
    )

    recipient = Account.create().address
    tx = {
        "nonce": nonce,
        "to": recipient,
        "value": Web3.to_wei("0.001", "ether"),
        "gas": 21000,
        "gasPrice": gas_price,
        "chainId": chain_id,
    }
    signed = w3.eth.account.sign_transaction(tx, faucet.key)

    LOG.info(f"[probe] submitting tx via {sender_node_id} (nonce={nonce})")
    send_time = time.monotonic()
    tx_hash = await asyncio.to_thread(
        lambda: w3.eth.send_raw_transaction(signed.raw_transaction)
    )
    LOG.info(f"[probe] {sender_node_id} accepted tx_hash={tx_hash.hex()}")

    receipt = None
    while time.monotonic() - send_time < PFN_FORWARD_RECEIPT_TIMEOUT:
        try:
            receipt = await asyncio.to_thread(
                lambda: w3.eth.get_transaction_receipt(tx_hash)
            )
            if receipt:
                break
        except Exception:
            pass
        await asyncio.sleep(0.5)

    assert receipt is not None, (
        f"tx submitted via {sender_node_id} did not get a receipt within "
        f"{PFN_FORWARD_RECEIPT_TIMEOUT}s — broadcast didn't reach validator"
    )
    assert receipt["status"] == 1, f"tx failed on-chain: {receipt}"
    block_number = receipt["blockNumber"]
    assert block_number is not None and block_number > 0, (
        f"tx receipt has no valid blockNumber: {receipt}"
    )
    LOG.info(
        f"[probe] tx included in block #{block_number} after "
        f"{time.monotonic() - send_time:.2f}s via {sender_node_id}"
    )

    validator_receipt = await asyncio.to_thread(
        lambda: validator.w3.eth.get_transaction_receipt(tx_hash)
    )
    assert validator_receipt is not None, (
        f"validator does not see tx {tx_hash.hex()} — propagation broken"
    )
    assert validator_receipt["blockNumber"] == block_number, (
        f"validator block mismatch: pfn={block_number} "
        f"validator={validator_receipt['blockNumber']}"
    )
    LOG.info(f"[probe] validator confirms inclusion at block #{block_number}")

    return tx_hash.hex(), block_number


@pytest.mark.asyncio
async def test_pfn_chain_topology(cluster: Cluster):
    """PFN fan-out + redundant leaf: see module docstring."""
    LOG.info("=" * 70)
    LOG.info("Test: PFN fan-out + redundant leaf (5 nodes)")
    LOG.info("=" * 70)

    assert await cluster.set_full_live(timeout=120), "Cluster failed to become live"

    for nid in NODE_IDS:
        assert cluster.get_node(nid) is not None, f"{nid} missing from cluster"

    assert await cluster.check_block_increasing(timeout=30), "Blocks not advancing"

    initial = await _get_live_heights(cluster, set())
    LOG.info(f"initial heights: {initial}")

    # Phase 0 — single-tx probe via pfn3 with validator cross-check.
    LOG.info("[phase 0] probing pfn3 RPC -> validator inclusion path")
    await _send_tx_via_pfn_and_assert_inclusion(cluster, sender_node_id="pfn3")

    # Phase 1 — steady-state load + fixed-order pfn1/pfn2 stop cycles.
    tx_sender = TxSender(cluster, cluster.faucet, target_node_id="pfn3")
    tx_sender.start()
    LOG.info("TxSender started against pfn3 (drives load through full topology)")

    excluded: set[str] = set()
    window_log: list[tuple[str, TxSnap]] = []  # (label, delta) per window

    try:
        # Phase 1a — wait for cluster to reach steady state before any stops.
        snap_test_start = tx_sender.snapshot()
        LOG.info("[phase 1a] waiting for steady state before first stop")
        ok = await _wait_steady(
            cluster, excluded, timeout=STEADY_INITIAL_TIMEOUT, label="phase1a-steady"
        )
        assert ok, (
            f"Cluster did not reach steady state within {STEADY_INITIAL_TIMEOUT}s "
            f"of starting TxSender — refusing to begin stop cycles"
        )
        snap_after_warmup = tx_sender.snapshot()
        window_log.append(("Phase 1a (warmup)", snap_after_warmup - snap_test_start))

        # Phase 1b — fixed order: stop pfn1, restart, then stop pfn2, restart.
        for victim_id in ("pfn1", "pfn2"):
            victim = cluster.get_node(victim_id)

            pre_stop_heights = await _get_live_heights(cluster, excluded)
            pre_stop_snap = tx_sender.snapshot()
            in_flight_at_stop = list(tx_sender.in_flight_hashes)
            LOG.info(
                f"[phase 1b] STOP {victim_id} — pre_stop_heights={pre_stop_heights} "
                f"in_flight_hashes={in_flight_at_stop}"
            )

            assert await victim.stop(), f"{victim_id} failed to stop"
            excluded.add(victim_id)

            # Down window: monitor remaining 4 nodes for PFN_DOWN_DURATION.
            await _monitor_window(
                cluster,
                excluded,
                duration=PFN_DOWN_DURATION,
                label=f"phase1b-down-{victim_id}",
            )
            during_stop_snap = tx_sender.snapshot()
            window_log.append(
                (f"Phase 1b {victim_id} DOWN ({PFN_DOWN_DURATION}s)",
                 during_stop_snap - pre_stop_snap)
            )

            LOG.info(f"[phase 1b] RESTART {victim_id}")
            assert await victim.start(), f"{victim_id} failed to restart"
            excluded.discard(victim_id)

            # Catchup: wait until victim re-joins steady state.
            ok = await _wait_steady(
                cluster, set(), timeout=CATCHUP_TIMEOUT,
                label=f"phase1b-catchup-{victim_id}",
            )
            assert ok, (
                f"{victim_id} did not re-converge within {CATCHUP_TIMEOUT}s — "
                f"refusing to proceed to next stop"
            )
            after_catchup_snap = tx_sender.snapshot()
            window_log.append(
                (f"Phase 1b {victim_id} catchup",
                 after_catchup_snap - during_stop_snap)
            )

        # Phase 1c — post-tail steady state to confirm everything still healthy.
        LOG.info(f"[phase 1c] post-tail monitoring for {POST_TAIL_DURATION}s")
        post_tail_start_snap = tx_sender.snapshot()
        await _monitor_window(
            cluster, set(), duration=POST_TAIL_DURATION, label="phase1c-tail"
        )
        post_tail_end_snap = tx_sender.snapshot()
        window_log.append(
            ("Phase 1c (post-tail)", post_tail_end_snap - post_tail_start_snap)
        )
    finally:
        await tx_sender.stop()
        tx_sender.log_stats()

    # Per-window TxSender breakdown — primary signal for §10 mempool theory.
    LOG.info("=" * 60)
    LOG.info("PER-WINDOW TX BREAKDOWN")
    LOG.info("=" * 60)
    for label, delta in window_log:
        LOG.info(f"  {label}: {delta}")
    LOG.info("=" * 60)

    # Dump every sent hash + status to a file for post-mortem grep against
    # node-side mempool-trace logs. Shows which peers received which txns.
    import json as _json
    artifacts_dir = pathlib.Path(__file__).parent / "artifacts"
    artifacts_dir.mkdir(exist_ok=True)
    dump_path = artifacts_dir / "tx_trace.json"
    dump_path.write_text(_json.dumps({
        "sent": tx_sender.sent_hashes,
        "confirmed": sorted(tx_sender.confirmed_hashes),
        "timeout": tx_sender.timeout_hashes,
    }, indent=2))
    LOG.info(f"[trace] wrote tx hash dump to {dump_path}")

    # Phase 1 hard assertions.
    final_heights = await _get_live_heights(cluster, set())
    for nid in NODE_IDS:
        assert nid in final_heights, f"{nid} unreachable at Phase 1 end"
        assert final_heights[nid] > initial[nid], (
            f"{nid} did not advance: initial={initial[nid]} "
            f"final={final_heights[nid]}"
        )

    # Strict end-to-end health: every tx pfn3 accepted via RPC must have
    # produced a receipt at pfn3 (= broadcast through PFN failover, included
    # in a validator block, and synced back to pfn3). No timeouts or send
    # failures are tolerated — if any appear, the redundant-upstream
    # mechanism is partially broken even if heights still advance.
    assert tx_sender.total_sent > 0, "TxSender produced no txns at all"
    assert tx_sender.total_failed == 0, (
        f"TxSender saw {tx_sender.total_failed} send failures during Phase 1"
    )
    assert tx_sender.total_timeout == 0, (
        f"TxSender saw {tx_sender.total_timeout} receipt timeouts during "
        f"Phase 1 (sent={tx_sender.total_sent} confirmed={tx_sender.total_confirmed}). "
        f"Each timeout = one tx pfn3's RPC accepted but never saw a receipt for "
        f"within {TX_RECEIPT_TIMEOUT}s — implies broadcast / sync failover broke "
        f"during a stop window. See per-window breakdown above for the affected window."
    )
    assert tx_sender.total_confirmed == tx_sender.total_sent, (
        f"sent={tx_sender.total_sent} confirmed={tx_sender.total_confirmed} mismatch "
        f"(timeout={tx_sender.total_timeout} failed={tx_sender.total_failed})"
    )

    # Phase 2 — pfn3 self-restart with both upstreams healthy.
    LOG.info("[phase 2] stopping pfn3 to verify leaf-node recovery via redundant upstreams")
    pfn3 = cluster.get_node("pfn3")
    assert await pfn3.stop(), "pfn3 failed to stop"
    await asyncio.sleep(30)
    LOG.info("[phase 2] restarting pfn3")
    assert await pfn3.start(), "pfn3 failed to restart"

    ok = await _wait_steady(
        cluster, set(), timeout=CATCHUP_TIMEOUT, label="phase2-pfn3-catchup"
    )
    assert ok, f"pfn3 failed to catch up via pfn1/pfn2 within {CATCHUP_TIMEOUT}s"

    # Final RPC probe: prove pfn3 RPC forwarding works after its restart.
    LOG.info("[phase 2] post-restart pfn3 RPC -> validator probe")
    await _send_tx_via_pfn_and_assert_inclusion(cluster, sender_node_id="pfn3")

    # Log regression guard: pfn3's consensus log must not show
    # "No Vfn peers available" (commit 16ebf363's smoke alarm).
    pfn3_text = _node_log_text(cluster.get_node("pfn3"))
    assert "No Vfn peers available" not in pfn3_text, (
        "pfn3 emitted 'No Vfn peers available' — sync-path still routing on "
        "Vfn instead of Public. Regression of commit 16ebf363."
    )

    LOG.info("PFN fan-out test PASSED")
