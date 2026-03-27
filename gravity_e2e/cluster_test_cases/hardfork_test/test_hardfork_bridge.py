"""
Delta Hardfork + Bridge E2E Test

Verifies that the cross-chain bridge works correctly both before and after
the delta hardfork. All bridge events are pre-loaded by hooks.py.

Test Flow:
  Phase A: Verify batch 1 bridge events (nonces 1-10, pre-loaded before delta)
  Phase B: Verify delta hardfork occurred (Governance.owner set)
  Phase C: Verify batch 2 bridge events (nonces 11-20, post-delta)
"""

import logging
import os
import sys
from pathlib import Path

import pytest
from eth_abi import encode
from web3 import Web3

# Ensure gravity_e2e is importable
_current_dir = Path(__file__).resolve().parent
_e2e_root = _current_dir
while _e2e_root.name != "gravity_e2e" or not (_e2e_root / "gravity_e2e").is_dir():
    _e2e_root = _e2e_root.parent
    if _e2e_root == _e2e_root.parent:
        break
if str(_e2e_root) not in sys.path:
    sys.path.insert(0, str(_e2e_root))

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils.bridge_utils import (
    poll_all_native_minted,
    GBRIDGE_RECEIVER_ADDRESS,
    BRIDGE_RECEIVER_ABI,
)

from hardfork_utils import (
    wait_for_block,
    wait_for_blocks_after,
)

LOG = logging.getLogger(__name__)

# ── Configuration ────────────────────────────────────────────────────
DELTA_BLOCK = int(os.environ.get("DELTA_BLOCK", "50"))
# Events are split into two halves: batch 1 (pre-hardfork) + batch 2 (post-hardfork)
BATCH_SIZE = 10  # Each batch has 10 events

# Governance contract address
GOVERNANCE = Web3.to_checksum_address("0x00000000000000000000000000000001625F3000")
# Expected owner after delta hardfork (faucet / hardhat #0)
FAUCET_ADDR = Web3.to_checksum_address("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")
SEL_OWNER = Web3.keccak(text="owner()")[:4]


@pytest.mark.asyncio
async def test_hardfork_bridge(
    cluster: Cluster,
    mock_anvil_metadata: dict,
    bridge_verify_timeout: int,
):
    """
    Verify bridge works before and after delta hardfork.

    All events are pre-loaded by hooks.py in a single batch.
    We verify them in two phases: batch 1 (nonces 1-10) then batch 2 (nonces 11-20).
    """
    node = cluster.get_node("node1")
    assert node is not None, "node1 not found in cluster"
    w3 = node.w3

    total_count = mock_anvil_metadata["bridge_count"]
    amount = mock_anvil_metadata["amount"]
    recipient = mock_anvil_metadata["recipient"]

    # Split into two batches
    batch1_count = min(BATCH_SIZE, total_count)
    batch2_count = total_count - batch1_count

    LOG.info("=" * 70)
    LOG.info("🌉 Delta Hardfork + Bridge E2E Test")
    LOG.info(f"   deltaBlock = {DELTA_BLOCK}")
    LOG.info(f"   Total pre-loaded events = {total_count}")
    LOG.info(f"   Batch 1 (pre-hardfork verify) = nonces 1-{batch1_count}")
    if batch2_count > 0:
        LOG.info(f"   Batch 2 (post-hardfork verify) = nonces {batch1_count+1}-{total_count}")
    LOG.info("=" * 70)

    # ================================================================
    # Phase A: Verify batch 1 bridge events
    # ================================================================
    LOG.info(f"\n[Phase A] Verify batch 1 bridge events (nonces 1-{batch1_count})")

    current = w3.eth.block_number
    LOG.info(f"  Current block: {current}")

    LOG.info(f"  Waiting for {batch1_count} NativeMinted events...")
    result_a = await poll_all_native_minted(
        gravity_w3=w3,
        max_nonce=batch1_count,
        timeout=bridge_verify_timeout,
        poll_interval=3.0,
    )

    found_a = result_a["found_nonces"]
    missing_a = result_a["missing_nonces"]
    LOG.info(
        f"  Batch 1: {len(found_a)}/{batch1_count} events found, "
        f"{len(missing_a)} missing"
    )

    assert len(missing_a) == 0, (
        f"Batch 1: {len(missing_a)} nonces missing: "
        f"{sorted(missing_a)[:20]}"
    )

    # Verify balance for batch 1
    balance_a = w3.eth.get_balance(recipient)
    expected_a = batch1_count * amount
    LOG.info(f"  Balance: {balance_a} wei (expected >= {expected_a})")
    assert balance_a >= expected_a, (
        f"Batch 1 balance too low: {balance_a} < {expected_a}"
    )

    LOG.info("✅ Phase A PASSED: batch 1 bridge events verified")

    # ================================================================
    # Phase B: Verify delta hardfork occurred
    # ================================================================
    LOG.info(f"\n[Phase B] Verify delta hardfork at block {DELTA_BLOCK}")

    current = w3.eth.block_number
    if current < DELTA_BLOCK + 2:
        LOG.info(f"  Waiting for deltaBlock+2={DELTA_BLOCK + 2} (current={current})...")
        reached = await wait_for_block(w3, DELTA_BLOCK + 2, timeout=300)
        assert reached, f"Chain did not reach deltaBlock+2 ({DELTA_BLOCK + 2})"

    # Verify Governance.owner() was set by delta hardfork
    owner_raw = w3.eth.call({"to": GOVERNANCE, "data": SEL_OWNER})
    owner_addr = Web3.to_checksum_address("0x" + owner_raw[-20:].hex())
    LOG.info(f"  Governance.owner() = {owner_addr}")
    assert owner_addr == FAUCET_ADDR, (
        f"Expected Governance.owner={FAUCET_ADDR}, got {owner_addr}"
    )

    LOG.info("✅ Phase B PASSED: delta hardfork verified (Governance.owner set)")

    # ================================================================
    # Phase C: Verify batch 2 bridge events (post-hardfork)
    # ================================================================
    if batch2_count <= 0:
        LOG.info("\n[Phase C] SKIPPED: no batch 2 events")
    else:
        LOG.info(
            f"\n[Phase C] Verify batch 2 bridge events "
            f"(nonces {batch1_count+1}-{total_count})"
        )

        LOG.info(f"  Waiting for all {total_count} NativeMinted events...")
        result_c = await poll_all_native_minted(
            gravity_w3=w3,
            max_nonce=total_count,
            timeout=bridge_verify_timeout,
            poll_interval=3.0,
        )

        found_c = result_c["found_nonces"]
        missing_c = result_c["missing_nonces"]
        LOG.info(
            f"  Total: {len(found_c)}/{total_count} events found, "
            f"{len(missing_c)} missing"
        )

        assert len(missing_c) == 0, (
            f"Post-hardfork: {len(missing_c)} nonces missing: "
            f"{sorted(missing_c)[:20]}"
        )

        # Verify nonce continuity
        nonces_found = sorted(found_c)
        expected_nonces = list(range(1, total_count + 1))
        assert nonces_found == expected_nonces, (
            f"Nonces not continuous: "
            f"expected 1-{total_count}, got {nonces_found[0]}-{nonces_found[-1]}"
        )

        # Verify cumulative balance
        balance_c = w3.eth.get_balance(recipient)
        expected_c = total_count * amount
        LOG.info(f"  Final balance: {balance_c} wei (expected >= {expected_c})")
        assert balance_c >= expected_c, (
            f"Post-hardfork balance too low: {balance_c} < {expected_c}"
        )

        LOG.info("✅ Phase C PASSED: batch 2 bridge events verified post-hardfork")

    # ================================================================
    # Summary
    # ================================================================
    LOG.info("\n" + "=" * 70)
    LOG.info("🎉 ALL PHASES PASSED — Delta Hardfork + Bridge E2E Test")
    LOG.info(f"   Batch 1 (pre-hardfork):  {batch1_count} ✅")
    LOG.info(f"   Batch 2 (post-hardfork): {batch2_count} ✅")
    LOG.info(f"   Total bridges:           {total_count}")
    LOG.info(f"   Final block:             {w3.eth.block_number}")
    LOG.info("=" * 70)
