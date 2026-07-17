"""
Progress-based catch-up waiting for deep-syncing nodes.

Why not an absolute deadline: consensus fast-forward-sync replays the
chain **one epoch per round** (sync_manager.rs fast_forward_sync_by_epoch),
so a syncing node's RPC height advances in EPOCH STAIRCASE steps — flat
for a full round, then a ~500-block jump. Measured live
(storage_v2_fresh_sync attempt5, see the tc9-catchup-freeze-investigation
report): full-load rounds take ~128-137s, replay throughput is ~5.5-7.6
blocks/s while the chain keeps producing at ~3.8 blocks/s, leaving a NET
convergence of only ~1.4-1.9 blocks/s. A fixed 900s deadline read the
staircase's flat phase as a freeze and failed a perfectly healthy sync
that physically needed 40-70 minutes.

This module waits on PROGRESS instead:

- fail on a real stall — no height growth across ``stall_window_s``
  (default ~3x the measured staircase period, so flat phases between
  epoch jumps never count as stalls);
- keep a generous HARD budget as the backstop, derived from the live gap:
  ``budget = gap / net_rate_floor * safety`` — with the rate floor taken
  just under the measured net convergence, blowing it means the sync is
  pathologically slower than anything observed, not merely busy.
"""

from __future__ import annotations

import asyncio
import logging
import time
from typing import Optional

LOG = logging.getLogger(__name__)

# Measured full-load fast-forward-sync round duration (attempt5 live:
# epochs 7-11 at 128-137s per round). The stall window must comfortably
# exceed one round or every flat staircase phase looks like a stall.
FULL_LOAD_EPOCH_ROUND_S = 137
# ~3x the staircase period: two consecutive silent rounds plus slack.
DEFAULT_STALL_WINDOW_S = 3 * FULL_LOAD_EPOCH_ROUND_S  # 411s
# Conservative floor of the net convergence on a QUIET chain — TC9's
# from-0 catch-up windows pause the background load before syncing:
# attempt6 measured parallel UNDER-LOAD convergence at only ~0.54 blk/s
# (two nodes sharing one host's replay throughput), invalidating any
# under-load floor, while the no-load resurrection experiment converged
# at minutes scale (replay 5.5-7.6 blk/s vs ~3.8 idle production) but
# only had an 8-minute observation window. This value is therefore a
# deliberate conservative guess — PENDING LIVE CALIBRATION from the
# per-minute (height, tip, net_rate) diagnostics wait_for_catchup logs.
NET_CATCHUP_RATE_FLOOR_BPS = 2.0
# Conservative REPLAY-rate floor against a FROZEN tip (chain halted, so
# net convergence == replay throughput): attempt7 measured the per-block
# recovery ceremony locking replay at 4.3-4.5 blk/s on this host even
# with the tx load paused; 4.0 sits just under that band.
FROZEN_TIP_REPLAY_FLOOR_BPS = 4.0
# Safety factor on the budget derived from the live gap.
BUDGET_SAFETY_FACTOR = 1.5
# Budgets never shrink below this even for tiny gaps (staircase rounds
# are coarse; a node one epoch behind still needs a round or two).
MIN_BUDGET_S = 900.0


def catchup_budget_s(
    gap_blocks: int,
    net_rate_bps: float = NET_CATCHUP_RATE_FLOOR_BPS,
    safety: float = BUDGET_SAFETY_FACTOR,
    floor_s: float = MIN_BUDGET_S,
) -> float:
    """Hard-backstop budget for closing ``gap_blocks`` against a chain
    that keeps producing: gap / conservative-net-rate * safety, floored."""
    if gap_blocks < 0:
        raise ValueError(f"gap_blocks must be >= 0, got {gap_blocks}")
    if net_rate_bps <= 0 or safety <= 0:
        raise ValueError("net_rate_bps and safety must be positive")
    return max(floor_s, gap_blocks / net_rate_bps * safety)


class StallDetector:
    """Pure stall bookkeeping over (timestamp, height) samples.

    ``observe`` returns True once no growth has been seen for
    ``window_s`` — flat phases shorter than the window (the epoch
    staircase) never trigger.
    """

    def __init__(self, window_s: float):
        if window_s <= 0:
            raise ValueError(f"window_s must be positive, got {window_s}")
        self.window_s = window_s
        self.last_height: Optional[int] = None
        self.last_progress_t: Optional[float] = None

    def observe(self, t: float, height: int) -> bool:
        if self.last_height is None or height > self.last_height:
            self.last_height = height
            self.last_progress_t = t
            return False
        return (t - self.last_progress_t) >= self.window_s

    def stalled_for(self, t: float) -> float:
        """Seconds since the last observed progress (0 before the first
        sample)."""
        if self.last_progress_t is None:
            return 0.0
        return max(0.0, t - self.last_progress_t)


async def wait_for_catchup(
    node,
    reference_node,
    *,
    max_gap: int = 50,
    stall_window_s: float = DEFAULT_STALL_WINDOW_S,
    net_rate_bps: float = NET_CATCHUP_RATE_FLOOR_BPS,
    safety: float = BUDGET_SAFETY_FACTOR,
    min_budget_s: float = MIN_BUDGET_S,
    poll_s: float = 5.0,
    log_every_s: float = 60.0,
) -> int:
    """Wait until ``node`` closes to within ``max_gap`` of the LIVE
    ``reference_node`` tip. Progress-based: fails on a real stall or on
    blowing the gap-derived hard budget, never on a fixed wall-clock.

    Returns the node's height at convergence. RPC errors on the syncing
    node are tolerated (it may be mid-restart); errors reading the
    reference are tolerated briefly (it may be mid-probe elsewhere).
    """
    start_t = time.monotonic()

    async def _height(n) -> Optional[int]:
        try:
            return await asyncio.to_thread(lambda: n.w3.eth.block_number)
        except Exception:
            return None

    ref0 = await _height(reference_node)
    h0 = await _height(node) or 0
    assert ref0 is not None, (
        f"cannot read reference height from {reference_node.id} at "
        f"catch-up start"
    )
    initial_gap = max(0, ref0 - h0)
    budget_s = catchup_budget_s(initial_gap, net_rate_bps, safety, min_budget_s)
    LOG.info(
        "[catchup/%s] start: height=%d, tip(%s)=%d, gap=%d, "
        "budget=%.0fs (net-rate floor %.1f blk/s x safety %.1f), "
        "stall window=%.0fs",
        node.id, h0, reference_node.id, ref0, initial_gap,
        budget_s, net_rate_bps, safety, stall_window_s,
    )

    detector = StallDetector(stall_window_s)
    detector.observe(start_t, h0)
    last_log_t = start_t

    while True:
        await asyncio.sleep(poll_s)
        now = time.monotonic()
        height = await _height(node)
        ref = await _height(reference_node)

        if height is not None:
            if detector.observe(now, height):
                raise AssertionError(
                    f"{node.id}: sync stalled — no height growth for "
                    f"{detector.stalled_for(now):.0f}s (window "
                    f"{stall_window_s:.0f}s, ~3x the measured "
                    f"{FULL_LOAD_EPOCH_ROUND_S}s epoch staircase round); "
                    f"stuck at {detector.last_height}"
                )
            if ref is not None and ref - height <= max_gap:
                elapsed = now - start_t
                rate = (height - h0) / elapsed if elapsed > 0 else 0.0
                net = (
                    (initial_gap - (ref - height)) / elapsed
                    if elapsed > 0
                    else 0.0
                )
                LOG.info(
                    "[catchup/%s] converged: height=%d, tip=%d, gap=%d "
                    "in %.0fs (replay avg %.1f blk/s, net %.2f blk/s)",
                    node.id, height, ref, ref - height, elapsed, rate, net,
                )
                return height

        if now - start_t > budget_s:
            raise AssertionError(
                f"{node.id}: catch-up blew the hard budget {budget_s:.0f}s "
                f"(initial gap {initial_gap} at a {net_rate_bps} blk/s "
                f"net-rate floor x{safety}) — slower than anything "
                f"measured; height={detector.last_height}, tip={ref}"
            )

        if now - last_log_t >= log_every_s and height is not None:
            # Per-minute calibration diagnostics: net_rate is the gap
            # shrink rate — the number NET_CATCHUP_RATE_FLOOR_BPS is
            # waiting to be calibrated from on future live runs.
            elapsed = now - start_t
            rate = (height - h0) / elapsed if elapsed > 0 else 0.0
            net = (
                (initial_gap - (ref - height)) / elapsed
                if (ref is not None and elapsed > 0)
                else None
            )
            LOG.info(
                "[catchup/%s] height=%d tip=%s gap=%s elapsed=%.0fs "
                "replay=%.1f blk/s net_rate=%s (budget %.0fs)",
                node.id, height, ref,
                (ref - height) if ref is not None else "?",
                elapsed, rate,
                f"{net:.2f} blk/s" if net is not None else "?",
                budget_s,
            )
            last_log_t = now
