"""
Advanced randomness tests — multi-contract isolation and stress.

- test_randomness_smoke: quick health check (block progress + single roll)
- test_randomness_multi_contract: 3 contracts rolled in parallel, verify seed consistency
- test_randomness_stress: 50 rapid rolls, verify success rate and distribution
"""

import asyncio
import logging
import time
from collections import Counter

import sys
from pathlib import Path

import pytest

_test_dir = Path(__file__).resolve().parent
if str(_test_dir) not in sys.path:
    sys.path.insert(0, str(_test_dir))

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils.transaction_builder import run_sync
from random_dice_helpers import deploy, roll, last_result, last_seed, latest_roll, fund_new_account

LOG = logging.getLogger(__name__)


@pytest.mark.asyncio
@pytest.mark.randomness
async def test_randomness_smoke(cluster: Cluster):
    """Quick check: blocks are progressing and a single dice roll works."""
    node = cluster.get_node("node1")
    w3 = node.w3

    start = await run_sync(lambda: w3.eth.block_number)
    for _ in range(3):
        await asyncio.sleep(3)
    end = await run_sync(lambda: w3.eth.block_number)
    assert end - start >= 3, f"Only {end - start} blocks in ~9s"

    contract = await deploy(w3, cluster.faucet)
    _, _, _ = await roll(w3, contract, cluster.faucet)
    result = await last_result(w3, contract)
    seed = await last_seed(w3, contract)

    assert 1 <= result <= 6, f"Invalid result: {result}"
    assert seed > 0, f"Invalid seed: {seed}"
    LOG.info(f"Smoke OK: result={result}, seed={seed}")


@pytest.mark.asyncio
@pytest.mark.randomness
async def test_randomness_multi_contract(cluster: Cluster):
    """3 contracts + 3 players, 5 parallel rounds. Same-block → same seed."""
    node = cluster.get_node("node1")
    w3 = node.w3
    faucet = cluster.faucet

    players = [fund_new_account(w3, faucet, 2.0) for _ in range(3)]
    contracts = [await deploy(w3, faucet) for _ in range(3)]

    for rnd in range(5):
        rolls = await asyncio.gather(*[
            roll(w3, contracts[i], players[i]) for i in range(3)
        ])
        reads = await asyncio.gather(*[
            latest_roll(w3, c) for c in contracts
        ])

        blk_nums = [r[0] for r in rolls]
        dice = [r[1] for r in reads]
        seeds = [r[2] for r in reads]

        LOG.info(f"Round {rnd+1}: blocks={blk_nums}, results={dice}")

        # All results valid
        for i, d in enumerate(dice):
            assert 1 <= d <= 6, f"Contract {i} invalid: {d}"

        # Same block → seeds must match
        if len(set(blk_nums)) == 1:
            assert len(set(seeds)) == 1, f"Same-block but seeds differ: {seeds}"

        await asyncio.sleep(1)


@pytest.mark.asyncio
@pytest.mark.randomness
@pytest.mark.slow
async def test_randomness_stress(cluster: Cluster):
    """50 rapid rolls across 5 players. Verify >=90% success rate."""
    node = cluster.get_node("node1")
    w3 = node.w3
    faucet = cluster.faucet

    players = [fund_new_account(w3, faucet, 5.0) for _ in range(5)]
    contract = await deploy(w3, faucet)

    num_rolls = 50
    t0 = time.time()
    results, seeds = [], []

    for i in range(num_rolls):
        player = players[i % len(players)]
        try:
            await roll(w3, contract, player)
            _, r, s = await latest_roll(w3, contract)
            results.append(r)
            seeds.append(s)
        except Exception as e:
            LOG.warning(f"Roll {i+1} failed: {e}")
        if (i + 1) % 10 == 0:
            LOG.info(f"Progress: {i+1}/{num_rolls}")

    duration = time.time() - t0
    rate = len(results) / num_rolls * 100
    LOG.info(f"Done: {len(results)}/{num_rolls} ({rate:.0f}%) in {duration:.1f}s, "
             f"{len(results)/duration:.1f} tps")
    assert rate >= 90, f"Success rate {rate:.0f}% < 90%"

    dist = Counter(results)
    for v in range(1, 7):
        LOG.info(f"  {v}: {dist.get(v, 0)}")

    diversity = len(set(seeds)) / len(seeds)
    LOG.info(f"Seed diversity: {diversity*100:.0f}%")
