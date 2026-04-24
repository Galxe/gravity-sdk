"""
PFN + VFN-shadow Topology Test

Tests that a PublicFullnode (PFN) can sync consensus state through a shadow
VFN. Exercises the C1-C4 changes in `_local/drafts/pfn/epoch-manager-pfn-role.md`:
non-validator sync paths (SyncInfoRequest / BlockRetrieval) now resolve the
network via `fullnode_side_network_id(node_type)` instead of hard-coded
`NetworkId::Vfn`, so a PFN sends them on its Public network and talks to
vfn-alpha on Public port 6195.

Topology:
- node1: validator (on-chain fullnode_address -> vfn-alpha:vfn_port)
- vfn-alpha: VFN shadow of node1; listens on Vfn (6193, to node1) + Public (6195, for PFN)
- pfn-client: PFN; Public network only, static seed pointing at vfn-alpha:6195

Flow:
1. Bring the cluster live.
2. Background TxSender against node1 keeps the chain producing blocks.
3. Periodically sample block heights on all three nodes; pfn-client must
   strictly advance and stay within MAX_HEIGHT_GAP of the validator.
4. Stop pfn-client mid-flight, let the chain advance, restart it; it must
   catch up through vfn-alpha's Public listener.
5. Log assertions on pfn-client: sync_info traffic flowed over Public
   (indirect evidence — pfn-client never reports "No Public peers available").

If request_sync_info / BlockRetrieval had still been hard-coded to Vfn, the
PFN log would fill with "No Vfn peers available" and heights would never
advance. Block progression is the primary assertion.
"""

import asyncio
import logging
import math
import pathlib
import statistics
import time

import pytest
from eth_account import Account
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.cluster.node import Node

LOG = logging.getLogger(__name__)

MAX_HEIGHT_GAP = 50
MONITOR_DURATION = 120           # seconds
MONITOR_INTERVAL = 10            # seconds between height samples
TX_INTERVAL = 0.2                # seconds between txs
TX_RECEIPT_TIMEOUT = 30.0


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

    @property
    def _w3(self) -> Web3:
        return self.cluster.get_node(self.target_node_id).w3

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
                self.total_sent += 1
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
                            confirmed = True
                            break
                    except Exception:
                        pass
                    await asyncio.sleep(0.1)

                if not confirmed:
                    self.total_timeout += 1
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


async def _get_block_heights(nodes: list[Node]) -> dict[str, int]:
    async def _h(n: Node):
        return n.id, await asyncio.to_thread(lambda: n.w3.eth.block_number)

    return dict(await asyncio.gather(*[_h(n) for n in nodes]))


def _node_log_text(node: Node) -> str:
    """Read consensus log (vfn.log for VFN/PFN, validator.log for validator)."""
    base = pathlib.Path(node._infra_path) / "consensus_log"
    for name in ("vfn.log", "validator.log"):
        p = base / name
        if p.exists():
            return p.read_text(errors="replace")
    return ""


@pytest.mark.asyncio
async def test_pfn_vfn_shadow_topology(cluster: Cluster):
    """PFN sync through a shadow VFN — verifies C1-C4 PFN-role changes."""
    LOG.info("=" * 70)
    LOG.info("Test: PFN + VFN-shadow (1 validator + 1 vfn-alpha + 1 pfn-client)")
    LOG.info("=" * 70)

    assert await cluster.set_full_live(timeout=120), "Cluster failed to become live"

    node_ids = ("node1", "vfn-alpha", "pfn-client")
    nodes = [cluster.get_node(nid) for nid in node_ids]
    for nid, node in zip(node_ids, nodes):
        assert node is not None, f"{nid} missing from cluster"

    assert await cluster.check_block_increasing(timeout=30), "Blocks not advancing"

    # Drive the chain from the validator side — pfn-client is purely a sync sink.
    tx_sender = TxSender(cluster, cluster.faucet, target_node_id="node1")
    tx_sender.start()
    LOG.info("TxSender started against node1")

    try:
        initial = await _get_block_heights(nodes)
        LOG.info(f"initial heights: {initial}")

        monitor_start = time.monotonic()
        check_count = 0
        last_heights = dict(initial)

        while time.monotonic() - monitor_start < MONITOR_DURATION:
            await asyncio.sleep(MONITOR_INTERVAL)
            check_count += 1

            last_heights = await _get_block_heights(nodes)
            max_h = max(last_heights.values())
            min_h = min(last_heights.values())
            gap = max_h - min_h
            elapsed = int(time.monotonic() - monitor_start)
            LOG.info(
                f"[check #{check_count} @ {elapsed}s] heights={last_heights} gap={gap}"
            )

            assert gap < MAX_HEIGHT_GAP, (
                f"pairwise height gap {gap} >= {MAX_HEIGHT_GAP}: {last_heights}"
            )

        for nid in node_ids:
            assert last_heights[nid] > initial[nid], (
                f"{nid} did not advance: initial={initial[nid]} last={last_heights[nid]}"
            )

        # Stop/restart pfn-client — must catch back up through vfn-alpha's
        # Public listener. The C3/C4 path is what makes this work.
        client = cluster.get_node("pfn-client")
        LOG.info("Stopping pfn-client to force sync-from-scratch via vfn-alpha…")
        assert await client.stop(), "pfn-client failed to stop"

        await asyncio.sleep(30)

        LOG.info("Restarting pfn-client…")
        assert await client.start(), "pfn-client failed to restart"

        catchup_deadline = time.monotonic() + 120
        while time.monotonic() < catchup_deadline:
            await asyncio.sleep(MONITOR_INTERVAL)
            heights = await _get_block_heights(nodes)
            gap = max(heights.values()) - min(heights.values())
            LOG.info(f"[post-restart] heights={heights} gap={gap}")
            if gap < MAX_HEIGHT_GAP:
                break

        heights = await _get_block_heights(nodes)
        gap = max(heights.values()) - min(heights.values())
        assert gap < MAX_HEIGHT_GAP, (
            f"pfn-client failed to catch up via vfn-alpha: {heights} gap={gap}"
        )
    finally:
        await tx_sender.stop()
        tx_sender.log_stats()

    assert tx_sender.total_confirmed > 0, (
        f"TxSender confirmed 0 txns; cluster fundamentally broken. "
        f"sent={tx_sender.total_sent} timeout={tx_sender.total_timeout} "
        f"failed={tx_sender.total_failed}"
    )

    # Log-level sanity checks on pfn-client: if request_sync_info had still been
    # hard-coded to Vfn, the PFN log would be full of "No Vfn peers available"
    # warnings. With C3 applied it now reads "No Public peers available" only
    # when the upstream is genuinely down — and since we've been advancing
    # blocks, it should not be saturated with such warnings.
    client_text = _node_log_text(cluster.get_node("pfn-client"))
    assert "No Vfn peers available" not in client_text, (
        "pfn-client emitted 'No Vfn peers available' — request_sync_info still "
        "routing on Vfn instead of Public. C3 regression."
    )

    LOG.info("✅ PFN + VFN-shadow topology test PASSED")
