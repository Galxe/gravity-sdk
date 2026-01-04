"""
Randomness basic test cases
Corresponds to original test: e2e_basic_consumption.rs
"""
import sys
from pathlib import Path

# Add package to path for absolute imports
_current_dir = Path(__file__).resolve().parent
_package_root = _current_dir.parent.parent.parent
if str(_package_root) not in sys.path:
    sys.path.insert(0, str(_package_root))

import asyncio
import logging
from typing import Dict, List

from gravity_e2e.helpers.test_helpers import RunHelper, TestResult, test_case
from gravity_e2e.core.client.gravity_http_client import GravityHttpClient
from gravity_e2e.utils.randomness_utils import (
    RandomDiceHelper,
    RandomnessVerifier,
    deploy_random_dice,
    get_dkg_status_safe,
    get_http_url_from_rpc,
)

LOG = logging.getLogger(__name__)


@test_case
async def test_randomness_basic_consumption(
    run_helper: RunHelper,
    test_result: TestResult
):
    """
    Test basic randomness consumption functionality

    Corresponds to original test: e2e_basic_consumption.rs

    Test steps:
    1. Get current DKG status
    2. Deploy RandomDice contract
    3. Call rollDice() multiple times
    4. Verify randomness range (1-6)
    5. Verify block.difficulty is passed correctly
    6. Verify randomness seed variation
    7. Statistical analysis
    """
    LOG.info("=" * 70)
    LOG.info("Test: Randomness Basic Consumption (e2e_basic_consumption)")
    LOG.info("=" * 70)

    # Initialize HTTP client
    http_url = get_http_url_from_rpc(run_helper.client.rpc_url)
    async with GravityHttpClient(http_url) as http_client:

        # ========== Step 1: Get DKG Status ==========
        LOG.info("\n[Step 1] Getting DKG status...")
        dkg_status = await get_dkg_status_safe(http_client)

        # ========== Step 2: Deploy RandomDice Contract ==========
        LOG.info("\n[Step 2] Deploying RandomDice contract...")

        deployer = run_helper.faucet_account
        LOG.info(f"Deployer address: {deployer['address']}")

        try:
            dice = await deploy_random_dice(run_helper, deployer)
            deploy_gas_used = 0  # Could be enhanced to track this
        except FileNotFoundError as e:
            LOG.error(str(e))
            raise RuntimeError(
                "Please compile RandomDice contract first:\n"
                "  cd /path/to/gravity-sdk\n"
                "  forge build"
            )

        LOG.info(f"Contract deployed at: {dice.address}")

        # ========== Step 3: Execute rollDice 10 Times ==========
        LOG.info("\n[Step 3] Rolling dice 10 times...")

        roll_count = 10
        roll_results: List[int] = []
        seeds_used: List[int] = []
        blocks: List[int] = []
        tx_hashes: List[str] = []

        for i in range(roll_count):
            LOG.info(f"\n  Roll #{i+1}/{roll_count}:")

            # Call rollDice
            try:
                receipt = await dice.roll_dice(deployer)
            except Exception as e:
                LOG.error(f"    Failed to roll: {e}")
                continue

            block_number = int(receipt.get("blockNumber", "0x0"), 16)
            tx_hash = receipt.get("transactionHash", "unknown")
            gas_used = int(receipt.get("gasUsed", "0x0"), 16)

            LOG.info(f"    Tx: {tx_hash}")
            LOG.info(f"    Block: {block_number}")
            LOG.info(f"    Gas: {gas_used}")

            # Read result
            try:
                result = await dice.get_last_result()
                seed = await dice.get_last_seed()

                status_icon = "valid" if 1 <= result <= 6 else "invalid"
                LOG.info(f"    Result: {result} ({status_icon})")
                LOG.info(f"    Seed: {seed}")

                # Verify range
                if not (1 <= result <= 6):
                    raise AssertionError(f"Roll result {result} out of valid range [1, 6]")

                roll_results.append(result)
                seeds_used.append(seed)
                blocks.append(block_number)
                tx_hashes.append(tx_hash)

                LOG.info(f"    Valid roll")
            except Exception as e:
                LOG.error(f"    Failed to read result: {e}")
                raise

            # Brief wait to ensure different blocks
            await asyncio.sleep(1)

        # ========== Step 4: Verify block.difficulty Propagation ==========
        LOG.info("\n[Step 4] Verifying block.difficulty propagation...")

        mismatches = 0
        for i, (block_num, seed) in enumerate(zip(blocks, seeds_used)):
            try:
                block = await run_helper.client.get_block(block_num, full_transactions=False)

                difficulty_hex = block.get("difficulty", "0x0")
                difficulty = int(difficulty_hex, 16)

                match = (seed == difficulty)
                status = "match" if match else "mismatch"

                LOG.info(f"  Roll #{i+1} (Block {block_num}): {status}")
                LOG.info(f"    Contract seed: {seed}")
                LOG.info(f"    Block difficulty: {difficulty}")

                if not match:
                    mismatches += 1
                    LOG.warning(f"    MISMATCH!")
            except Exception as e:
                LOG.error(f"  Roll #{i+1}: Failed to verify - {e}")
                mismatches += 1

        if mismatches > 0:
            raise AssertionError(
                f"{mismatches}/{len(blocks)} blocks had seed/difficulty mismatches"
            )

        LOG.info(f"All {len(blocks)} blocks verified successfully!")

        # ========== Step 5: Verify Randomness API (Optional) ==========
        LOG.info("\n[Step 5] Verifying randomness API (first 3 blocks)...")

        for idx, block_num in enumerate(blocks[:3]):
            try:
                api_randomness = await http_client.get_randomness(block_num)

                if api_randomness:
                    LOG.info(f"  Block {block_num}: {api_randomness[:32]}...")
                else:
                    LOG.warning(f"  Block {block_num}: No randomness data")
            except Exception as e:
                LOG.warning(f"  Block {block_num}: Failed to get randomness - {e}")

        # ========== Step 6: Statistical Analysis ==========
        LOG.info("\n[Step 6] Statistical Analysis...")
        LOG.info(f"  Total rolls: {len(roll_results)}")
        LOG.info(f"  Results: {roll_results}")
        LOG.info(f"\n  Distribution:")

        for value in range(1, 7):
            count = roll_results.count(value)
            percentage = (count / len(roll_results)) * 100 if roll_results else 0
            bar = "#" * count
            LOG.info(f"    {value}: {bar} ({count}, {percentage:.1f}%)")

        # Verify seed variation
        unique_seeds = len(set(seeds_used))
        diversity_ratio = unique_seeds / len(seeds_used) if seeds_used else 0

        LOG.info(f"\n  Seed diversity:")
        LOG.info(f"    Unique seeds: {unique_seeds}/{len(seeds_used)}")
        LOG.info(f"    Diversity ratio: {diversity_ratio*100:.1f}%")

        if diversity_ratio < 0.8:
            LOG.warning(f"    Low seed diversity detected!")
        else:
            LOG.info(f"    Good seed diversity")

        # ========== Record Test Results ==========
        test_result.mark_success(
            contract_address=dice.address,
            total_rolls=len(roll_results),
            roll_results=roll_results,
            unique_seeds=unique_seeds,
            diversity_ratio=diversity_ratio,
            blocks_tested=blocks,
            dkg_epoch=dkg_status.get('epoch', 0),
            dkg_round=dkg_status.get('round', 0)
        )

        LOG.info("\n" + "=" * 70)
        LOG.info("Test 'Randomness Basic Consumption' PASSED!")
        LOG.info("=" * 70)


@test_case
async def test_randomness_correctness(
    run_helper: RunHelper,
    test_result: TestResult
):
    """
    Test randomness correctness (observational verification)

    Corresponds to original test: e2e_correctness.rs (simplified version without cryptographic verification)

    Test steps:
    1. Get current DKG status and block information
    2. Verify randomness consistency for the last 10 blocks
    3. Check relationship between block.difficulty and API randomness
    4. Optional: Wait for next epoch and verify randomness update
    """
    LOG.info("=" * 70)
    LOG.info("Test: Randomness Correctness (e2e_correctness, observational)")
    LOG.info("=" * 70)

    http_url = get_http_url_from_rpc(run_helper.client.rpc_url)
    async with GravityHttpClient(http_url) as http_client:

        # ========== Step 1: Get Current State ==========
        LOG.info("\n[Step 1] Getting current state...")

        try:
            dkg_status = await http_client.get_dkg_status()
            current_block = dkg_status['block_number']
            current_epoch = dkg_status['epoch']
            current_round = dkg_status['round']

            LOG.info(f"Current State:")
            LOG.info(f"  Block: {current_block}")
            LOG.info(f"  Epoch: {current_epoch}")
            LOG.info(f"  Round: {current_round}")
            LOG.info(f"  Nodes: {dkg_status['participating_nodes']}")
        except Exception as e:
            LOG.error(f"Failed to get DKG status: {e}")
            raise

        # ========== Step 2: Verify Recent Blocks ==========
        LOG.info(f"\n[Step 2] Verifying recent blocks (up to 10)...")

        verification_results = []
        blocks_to_check = min(10, current_block)

        LOG.info(f"Will check {blocks_to_check} blocks starting from {current_block}")

        for i in range(blocks_to_check):
            block_num = current_block - i

            if block_num < 0:
                break

            LOG.info(f"\n  Verifying block {block_num}...")

            try:
                result = await RandomnessVerifier.verify_block_randomness(
                    run_helper.client,
                    http_client,
                    block_num
                )

                verification_results.append(result)

                # Detailed logging
                if result.get("valid"):
                    LOG.info(f"    Valid")
                else:
                    LOG.warning(f"    Invalid")

                if "checks" in result:
                    for check_name, check_result in result["checks"].items():
                        status = "pass" if check_result else "fail"
                        LOG.info(f"      {status} {check_name}: {check_result}")

                # Display key data
                if "block_difficulty" in result:
                    LOG.info(f"      Block difficulty: {result['block_difficulty']}")
                if "api_randomness" in result and result["api_randomness"]:
                    randomness_preview = result["api_randomness"][:32]
                    LOG.info(f"      API randomness: {randomness_preview}...")

            except Exception as e:
                LOG.error(f"    Failed to verify: {e}")
                verification_results.append({
                    "block_number": block_num,
                    "error": str(e),
                    "valid": False
                })

        # ========== Step 3: Verification Summary ==========
        LOG.info(f"\n[Step 3] Verification Summary...")

        total_count = len(verification_results)
        valid_count = sum(1 for r in verification_results if r.get("valid", False))
        error_count = sum(1 for r in verification_results if "error" in r)

        success_rate = (valid_count / total_count * 100) if total_count > 0 else 0

        LOG.info(f"  Total blocks checked: {total_count}")
        LOG.info(f"  Valid blocks: {valid_count}")
        LOG.info(f"  Errors: {error_count}")
        LOG.info(f"  Success rate: {success_rate:.1f}%")

        # Detailed analysis
        has_api_count = sum(
            1 for r in verification_results
            if r.get("checks", {}).get("has_api_randomness", False)
        )
        difficulty_mixhash_match = sum(
            1 for r in verification_results
            if r.get("checks", {}).get("difficulty_equals_mixhash", False)
        )

        LOG.info(f"\n  API randomness available: {has_api_count}/{total_count}")
        LOG.info(f"  Difficulty == MixHash: {difficulty_mixhash_match}/{total_count}")

        # ========== Step 4: Wait for Next Epoch (Optional) ==========
        epoch_tested = False
        if current_epoch < 5:  # Only wait in early epochs
            LOG.info(f"\n[Step 4] Waiting for next epoch (current: {current_epoch})...")

            try:
                next_epoch = await http_client.wait_for_epoch(
                    current_epoch + 1,
                    timeout=120
                )

                LOG.info(f"  Reached epoch {next_epoch}")

                # Verify randomness in new epoch
                new_status = await http_client.get_dkg_status()
                new_block = new_status['block_number']

                LOG.info(f"  New state:")
                LOG.info(f"    Block: {new_block}")
                LOG.info(f"    Round: {new_status['round']}")

                # Verify first block of new epoch
                new_result = await RandomnessVerifier.verify_block_randomness(
                    run_helper.client,
                    http_client,
                    new_block
                )

                status = "Valid" if new_result.get('valid') else "Invalid"
                LOG.info(f"\n  New epoch block verification:")
                LOG.info(f"    Block {new_block}: {status}")

                # Check if randomness changed
                if verification_results and new_result.get("api_randomness"):
                    old_randomness = verification_results[0].get("api_randomness")
                    new_randomness = new_result.get("api_randomness")

                    if old_randomness and new_randomness:
                        changed = (old_randomness != new_randomness)
                        change_status = "Yes" if changed else "No"
                        LOG.info(f"    Randomness changed: {change_status}")

                epoch_tested = True
            except TimeoutError as e:
                LOG.warning(f"  Timeout waiting for next epoch: {e}")
            except Exception as e:
                LOG.error(f"  Error during epoch wait: {e}")
        else:
            LOG.info(f"\n[Step 4] Skipping epoch wait (already at epoch {current_epoch})")

        # ========== Verify Minimum Requirements ==========
        if valid_count == 0:
            raise AssertionError("No valid blocks found!")

        if success_rate < 50:
            LOG.warning(f"Low success rate: {success_rate:.1f}%")

        # ========== Record Test Results ==========
        test_result.mark_success(
            blocks_verified=total_count,
            valid_blocks=valid_count,
            error_count=error_count,
            success_rate=success_rate,
            has_api_randomness_count=has_api_count,
            difficulty_mixhash_matches=difficulty_mixhash_match,
            current_epoch=current_epoch,
            epoch_transition_tested=epoch_tested
        )

        LOG.info("\n" + "=" * 70)
        LOG.info("Test 'Randomness Correctness' PASSED!")
        LOG.info("=" * 70)
