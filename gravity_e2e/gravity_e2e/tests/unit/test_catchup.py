"""
Unit tests for gravity_e2e.helpers.catchup.

The regression these lock down (storage_v2_fresh_sync attempt5): a deep
sync advances in ~130s epoch-staircase steps, so a fixed-deadline waiter
misreads the flat phases as a freeze. The detector must tolerate the
staircase, fail on a genuine stall, and the hard budget must scale with
the actual gap.
"""

import asyncio
from types import SimpleNamespace

import pytest

from gravity_e2e.helpers.catchup import (
    BUDGET_SAFETY_FACTOR,
    DEFAULT_STALL_WINDOW_S,
    FULL_LOAD_EPOCH_ROUND_S,
    MIN_BUDGET_S,
    NET_CATCHUP_RATE_FLOOR_BPS,
    StallDetector,
    catchup_budget_s,
    wait_for_catchup,
)


class TestStallDetector:
    def test_staircase_progression_is_not_a_stall(self):
        # Epoch staircase: flat for one full-load round (~137s), then a
        # ~500-block jump — the default window (~3x) must tolerate it.
        detector = StallDetector(DEFAULT_STALL_WINDOW_S)
        t, height = 0.0, 1000
        for _step in range(5):
            assert detector.observe(t, height) is False
            for _flat in range(13):  # 13 x 10s ≈ one full-load round, flat
                t += 10
                assert detector.observe(t, height) is False, (
                    f"flat staircase phase misread as stall at t={t}"
                )
            t += 10
            height += 531  # the epoch jump
        assert detector.observe(t, height) is False
        assert detector.last_height == 1000 + 5 * 531

    def test_true_stall_trips_after_the_window(self):
        detector = StallDetector(100.0)
        assert detector.observe(0.0, 500) is False
        assert detector.observe(99.0, 500) is False
        assert detector.observe(100.0, 500) is True
        assert detector.stalled_for(100.0) == 100.0

    def test_regression_never_counts_as_progress(self):
        # A height going BACKWARDS (RPC flap to an older snapshot) must
        # not reset the stall clock.
        detector = StallDetector(50.0)
        detector.observe(0.0, 500)
        detector.observe(30.0, 499)
        assert detector.observe(50.0, 499) is True

    def test_rejects_nonpositive_window(self):
        with pytest.raises(ValueError):
            StallDetector(0)


class TestCatchupBudget:
    def test_scales_with_gap(self):
        # attempt5 numbers: a ~37-min-old chain ≈ 8400-block gap needed
        # 40-70 real minutes; the backstop must sit safely above that.
        budget = catchup_budget_s(8400)
        assert budget == 8400 / NET_CATCHUP_RATE_FLOOR_BPS * BUDGET_SAFETY_FACTOR
        assert budget > 70 * 60

    def test_floor_applies_to_small_gaps(self):
        assert catchup_budget_s(10) == MIN_BUDGET_S
        assert catchup_budget_s(0) == MIN_BUDGET_S

    def test_rejects_bad_inputs(self):
        with pytest.raises(ValueError):
            catchup_budget_s(-1)
        with pytest.raises(ValueError):
            catchup_budget_s(100, net_rate_bps=0)

    def test_window_covers_the_measured_staircase(self):
        assert DEFAULT_STALL_WINDOW_S >= 2 * FULL_LOAD_EPOCH_ROUND_S


def _scripted_node(node_id: str, heights):
    """A Node-shaped stub whose w3.eth.block_number pops scripted values
    (the last one repeats forever)."""
    heights = list(heights)

    class Eth:
        @property
        def block_number(self):
            if len(heights) > 1:
                return heights.pop(0)
            return heights[0]

    return SimpleNamespace(id=node_id, w3=SimpleNamespace(eth=Eth()))


class TestWaitForCatchup:
    @pytest.mark.asyncio
    async def test_converges_on_a_moving_tip(self):
        node = _scripted_node("sync", [0, 100, 300, 700, 995])
        ref = _scripted_node("ref", [900, 920, 940, 960, 1000])
        height = await wait_for_catchup(
            node, ref, max_gap=50, poll_s=0.01, stall_window_s=10.0
        )
        assert height >= 950

    @pytest.mark.asyncio
    async def test_true_stall_raises(self):
        node = _scripted_node("sync", [100])  # frozen forever
        ref = _scripted_node("ref", [5000])
        with pytest.raises(AssertionError, match="stalled"):
            await wait_for_catchup(
                node, ref, max_gap=50, poll_s=0.01, stall_window_s=0.05
            )

    @pytest.mark.asyncio
    async def test_budget_backstop_raises_on_pathological_slowness(self):
        # Progresses by 1 block per poll — never stalls, but blows the
        # (tiny, injected) budget: net rate far below the floor.
        node = _scripted_node("sync", list(range(0, 10000)))
        ref = _scripted_node("ref", [100000])
        with pytest.raises(AssertionError, match="budget"):
            await wait_for_catchup(
                node,
                ref,
                max_gap=50,
                poll_s=0.01,
                stall_window_s=60.0,
                # 100000 gap / 200000 bps * 1.0 => 0.5s budget
                net_rate_bps=200000.0,
                safety=1.0,
                min_budget_s=0.1,
            )
