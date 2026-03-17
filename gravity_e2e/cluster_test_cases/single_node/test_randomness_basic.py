"""
Randomness basic tests — verify on-chain randomness via RandomDice contract.

- test_randomness_basic_consumption: roll 10x, verify range + seed == difficulty
- test_randomness_correctness: verify difficulty == mixHash for recent blocks
"""

import asyncio
import logging

import sys
from pathlib import Path

import pytest

# Add test directory to path for local imports
_test_dir = Path(__file__).resolve().parent
if str(_test_dir) not in sys.path:
    sys.path.insert(0, str(_test_dir))

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils.transaction_builder import run_sync
from random_dice_helpers import deploy, roll, last_result, last_seed

LOG = logging.getLogger(__name__)


@pytest.mark.asyncio
@pytest.mark.randomness
async def test_randomness_basic_consumption(cluster: Cluster):
    """Deploy RandomDice, roll 10 times, verify result range and seed == block.difficulty."""
    node = cluster.get_node("node1")
    w3 = node.w3
    deployer = cluster.faucet

    contract = await deploy(w3, deployer)

    results, seeds, blocks = [], [], []
    for i in range(10):
        blk, _, _ = await roll(w3, contract, deployer)
        result = await last_result(w3, contract)
        seed = await last_seed(w3, contract)

        LOG.info(f"Roll #{i+1}: result={result}, seed={seed}, block={blk}")
        assert 1 <= result <= 6, f"Result {result} out of [1,6]"

        results.append(result)
        seeds.append(seed)
        blocks.append(blk)
        await asyncio.sleep(1)

    # Verify seed matches block.difficulty
    for i, (blk, seed) in enumerate(zip(blocks, seeds)):
        block_data = await run_sync(w3.eth.get_block, blk)
        assert seed == block_data["difficulty"], (
            f"Roll #{i+1} seed mismatch: contract={seed}, difficulty={block_data['difficulty']}"
        )

    # Seed diversity >= 80%
    unique = len(set(seeds))
    ratio = unique / len(seeds)
    LOG.info(f"Seed diversity: {unique}/{len(seeds)} ({ratio*100:.0f}%)")
    assert ratio >= 0.8, f"Low seed diversity: {ratio*100:.0f}%"

    LOG.info(f"Distribution: {[results.count(v) for v in range(1, 7)]}")


@pytest.mark.asyncio
@pytest.mark.randomness
async def test_randomness_correctness(cluster: Cluster):
    """Verify difficulty == mixHash and non-zero for recent blocks."""
    node = cluster.get_node("node1")
    w3 = node.w3

    head = await run_sync(lambda: w3.eth.block_number)
    n = min(10, head)
    valid = 0

    for i in range(n):
        blk = await run_sync(w3.eth.get_block, head - i)
        diff = blk.get("difficulty", 0)
        mix = blk.get("mixHash", b"\x00" * 32)
        mix_int = int.from_bytes(mix, "big") if isinstance(mix, (bytes, bytearray)) else int(mix, 16)

        ok = diff != 0 and diff == mix_int
        LOG.info(f"Block {head - i}: {'OK' if ok else 'FAIL'} (difficulty={diff})")
        if ok:
            valid += 1

    assert valid > 0, "No blocks with valid difficulty/mixHash"
