"""
Background transaction sender shared by the upgrade/storage cluster cases.

Third occurrence of the rolling_upgrade TxSender (copied into
storage_v2_upgrade, needed again by storage_v2_fresh_sync) — promoted to a
helper. Orchestration-only differences between the cases are covered by
two knobs:

- fallback target (rolling upgrade: retarget while the primary node is
  itself being swapped);
- pause/resume (storage_v2_fresh_sync: a 2-validator cluster freezes
  during every validator's swap window and during the L3 necessity probe,
  so the sender idles instead of burning failures/timeouts against a
  frozen chain).

The sender deliberately keeps its loose-load semantics: floors on
confirmed count / success rate are the CALLER's assertions.
"""

from __future__ import annotations

import asyncio
import logging
import math
import statistics
import time
from typing import List, Optional, Sequence

from eth_account import Account
from web3 import Web3

LOG = logging.getLogger(__name__)

DEFAULT_TX_INTERVAL_S = 0.2
DEFAULT_RECEIPT_TIMEOUT_S = 30.0
# Poll cadence while paused (no sends, no error counting).
PAUSE_IDLE_S = 0.2


def percentile(sorted_values: Sequence[float], p: float) -> float:
    """Linear-interpolated percentile over an already-sorted sequence."""
    if not sorted_values:
        raise ValueError("percentile of an empty sequence")
    k = (len(sorted_values) - 1) * (p / 100.0)
    f, c = math.floor(k), math.ceil(k)
    if f == c:
        return sorted_values[int(k)]
    return sorted_values[f] + (sorted_values[c] - sorted_values[f]) * (k - f)


class TxSender:
    """Continuously sends txs to a target node.

    Optional fallback target for the window where the primary is itself
    being restarted/upgraded, and pause()/resume() for windows where the
    whole chain is expected to freeze (sends would only pollute the
    stats there).
    """

    def __init__(
        self,
        cluster,
        faucet,
        primary_node_id: str,
        fallback_node_id: Optional[str] = None,
        tx_interval: float = DEFAULT_TX_INTERVAL_S,
        receipt_timeout: float = DEFAULT_RECEIPT_TIMEOUT_S,
    ):
        self.cluster = cluster
        self.faucet = faucet
        self.primary_node_id = primary_node_id
        self.fallback_node_id = fallback_node_id
        self.tx_interval = tx_interval
        self.receipt_timeout = receipt_timeout
        self.recipient = Account.create().address

        self._use_fallback = False
        self._paused = False
        self._stop_event = asyncio.Event()
        self._task: Optional[asyncio.Task] = None

        self.total_sent = 0
        self.total_confirmed = 0
        self.total_failed = 0
        self.total_timeout = 0
        self.latencies: List[float] = []

    @property
    def _current_node_id(self) -> str:
        if self._use_fallback and self.fallback_node_id is not None:
            return self.fallback_node_id
        return self.primary_node_id

    @property
    def _current_w3(self) -> Web3:
        return self.cluster.get_node(self._current_node_id).w3

    def set_fallback(self, use_fallback: bool):
        if use_fallback and self.fallback_node_id is None:
            raise ValueError(
                "set_fallback(True) on a sender with no fallback_node_id — "
                "use pause() for windows without an alternate target"
            )
        old = self._current_node_id
        self._use_fallback = use_fallback
        LOG.info("TxSender target: %s -> %s", old, self._current_node_id)

    def pause(self):
        """Stop sending (idle loop) without stopping the task or counting
        the frozen window as failures."""
        if not self._paused:
            self._paused = True
            LOG.info("TxSender paused")

    def resume(self):
        if self._paused:
            self._paused = False
            LOG.info("TxSender resumed")

    @property
    def paused(self) -> bool:
        return self._paused

    async def _resync_nonce(self) -> Optional[int]:
        try:
            return await asyncio.to_thread(
                lambda: self._current_w3.eth.get_transaction_count(
                    self.faucet.address, "pending"
                )
            )
        except Exception:
            return None

    async def _send_loop(self):
        chain_id = await asyncio.to_thread(lambda: self._current_w3.eth.chain_id)
        gas_price = Web3.to_wei("100", "gwei")
        nonce = await asyncio.to_thread(
            lambda: self._current_w3.eth.get_transaction_count(
                self.faucet.address, "pending"
            )
        )
        LOG.info(
            "TxSender started: target=%s nonce=%d recipient=%s",
            self._current_node_id,
            nonce,
            self.recipient,
        )

        resync_after_pause = False
        while not self._stop_event.is_set():
            if self._paused:
                resync_after_pause = True
                await asyncio.sleep(PAUSE_IDLE_S)
                continue
            if resync_after_pause:
                # The chain may have processed queued txs (or dropped
                # them) across the frozen window; re-anchor the nonce
                # before resuming sends.
                resync_after_pause = False
                resynced = await self._resync_nonce()
                if resynced is not None:
                    nonce = resynced
                    LOG.info("TxSender: nonce re-synced to %d after pause", nonce)

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

                confirmed = False
                while time.monotonic() - send_time < self.receipt_timeout:
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
                    LOG.warning("TxSender: tx %s... timed out", tx_hash.hex()[:10])
                    resynced = await self._resync_nonce()
                    if resynced is not None:
                        nonce = resynced
                        LOG.info("TxSender: nonce re-synced to %d", nonce)

            except Exception as e:
                self.total_failed += 1
                LOG.warning(
                    "TxSender: send failed (%s): %s", self._current_node_id, e
                )
                await asyncio.sleep(1)
                resynced = await self._resync_nonce()
                if resynced is not None:
                    nonce = resynced
                continue

            await asyncio.sleep(self.tx_interval)

    def start(self):
        self._task = asyncio.create_task(self._send_loop())

    async def stop(self):
        self._stop_event.set()
        if self._task:
            await self._task
            self._task = None

    @property
    def success_rate(self) -> float:
        return self.total_confirmed / self.total_sent if self.total_sent else 0.0

    def log_stats(self):
        LOG.info("=" * 60)
        LOG.info("TRANSACTION STATISTICS")
        LOG.info("=" * 60)
        LOG.info("Total Sent:      %d", self.total_sent)
        LOG.info("Confirmed:       %d", self.total_confirmed)
        LOG.info("Failed (send):   %d", self.total_failed)
        LOG.info("Timed Out:       %d", self.total_timeout)
        if self.total_sent:
            LOG.info("Success Rate:    %.1f%%", self.success_rate * 100)
        if self.latencies:
            sorted_lat = sorted(self.latencies)
            LOG.info(
                "Min/Avg/Max:     %.4fs / %.4fs / %.4fs",
                sorted_lat[0],
                statistics.mean(sorted_lat),
                sorted_lat[-1],
            )
            LOG.info(
                "P50/P90/P99:     %.4fs / %.4fs / %.4fs",
                percentile(sorted_lat, 50),
                percentile(sorted_lat, 90),
                percentile(sorted_lat, 99),
            )
        LOG.info("=" * 60)
