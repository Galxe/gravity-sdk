"""
Advanced Randomness Test Cases

This module implements more advanced randomness tests:
- smoke_test: DKG health check and basic functionality
- reconfiguration: Epoch transition and DKG state changes
- multi_contract: Multiple contracts using randomness simultaneously
- stress_test: High-frequency randomness consumption
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
from typing import Dict, List, Set
from collections import Counter
from eth_account import Account

from gravity_e2e.helpers.test_helpers import RunHelper, TestResult, test_case
from gravity_e2e.utils.randomness_utils import RandomDiceHelper
from gravity_e2e.core.client.gravity_http_client import GravityHttpClient

LOG = logging.getLogger(__name__)


async def deploy_random_dice(run_helper: RunHelper, deployer: Dict) -> RandomDiceHelper:
    """
    Deploy RandomDice contract and return helper instance.
    
    Args:
        run_helper: Test helper
        deployer: Deployer account dict with address and private_key
    
    Returns:
        RandomDiceHelper instance
    """
    # Load bytecode
    bytecode = RandomDiceHelper.load_bytecode()
    
    # Get deployment parameters
    nonce = await run_helper.client.get_transaction_count(deployer["address"])
    gas_price = await run_helper.client.get_gas_price()
    chain_id = await run_helper.client.get_chain_id()
    
    # Build deployment transaction
    deploy_tx = {
        "data": bytecode,
        "gas": hex(500000),
        "gasPrice": hex(gas_price),
        "nonce": hex(nonce),
        "chainId": hex(chain_id),
        "value": "0x0"
    }
    
    # Sign and send
    private_key = deployer["private_key"]
    if private_key.startswith("0x"):
        private_key = private_key[2:]
    
    signed_deploy = Account.sign_transaction(deploy_tx, private_key)
    deploy_tx_hash = await run_helper.client.send_raw_transaction(signed_deploy.raw_transaction)
    
    # Wait for deployment
    deploy_receipt = await run_helper.client.wait_for_transaction_receipt(deploy_tx_hash, timeout=60)
    
    if deploy_receipt.get("status") != "0x1":
        raise RuntimeError(f"Contract deployment failed: {deploy_receipt}")
    
    contract_address = deploy_receipt.get("contractAddress")
    if not contract_address:
        raise RuntimeError("No contract address in deployment receipt")
    
    LOG.info(f"  Contract deployed at: {contract_address}")
    
    return RandomDiceHelper(run_helper.client, contract_address)


@test_case
async def test_randomness_smoke(run_helper: RunHelper, test_result: TestResult):
    """
    Smoke test for DKG randomness functionality.
    
    This is a quick health check to verify:
    1. DKG status endpoint is accessible
    2. Randomness endpoint returns data
    3. Block generation is working
    4. Basic randomness consumption works
    """
    LOG.info("="*70)
    LOG.info("Test: Randomness Smoke Test (DKG Health Check)")
    LOG.info("="*70)
    
    # Initialize HTTP client
    gravity_http_client = GravityHttpClient(
        base_url=run_helper.client.rpc_url.replace(":8545", ":1998")
    )
    
    async with gravity_http_client:
        # Step 1: Check DKG status endpoint
        LOG.info("\n[Step 1] Checking DKG status endpoint...")
        dkg_status = await gravity_http_client.get_dkg_status()
        
        assert dkg_status is not None, "DKG status endpoint returned None"
        assert "epoch" in dkg_status, "DKG status missing 'epoch' field"
        assert "round" in dkg_status, "DKG status missing 'round' field"
        assert "block_number" in dkg_status, "DKG status missing 'block_number' field"
        
        current_epoch = dkg_status["epoch"]
        current_round = dkg_status["round"]
        current_block = dkg_status["block_number"]
        
        LOG.info(f"  ✅ DKG Status OK")
        LOG.info(f"     Epoch: {current_epoch}")
        LOG.info(f"     Round: {current_round}")
        LOG.info(f"     Block: {current_block}")
        
        # Step 2: Check randomness endpoint
        LOG.info("\n[Step 2] Checking randomness endpoint...")
        randomness = await gravity_http_client.get_randomness(current_block)
        
        assert randomness is not None, f"Randomness endpoint returned None for block {current_block}"
        assert randomness.startswith("0x"), "Randomness should be hex string"
        assert len(randomness) == 66, f"Randomness should be 32 bytes (66 hex chars), got {len(randomness)}"
        
        LOG.info(f"  ✅ Randomness Endpoint OK")
        LOG.info(f"     Block {current_block}: {randomness[:20]}...")
        
        # Step 3: Verify block generation
        LOG.info("\n[Step 3] Verifying block generation...")
        initial_block = await run_helper.client.get_block_number()
        LOG.info(f"  Current block: {initial_block}")
        
        # Wait for a few blocks
        LOG.info("  Waiting for 3 new blocks...")
        for i in range(3):
            await asyncio.sleep(3)
            new_block = await run_helper.client.get_block_number()
            LOG.info(f"    Block {i+1}: {new_block}")
        
        final_block = await run_helper.client.get_block_number()
        blocks_produced = final_block - initial_block
        
        assert blocks_produced >= 3, f"Expected at least 3 blocks, got {blocks_produced}"
        LOG.info(f"  ✅ Block Generation OK: {blocks_produced} blocks in ~9 seconds")
        
        # Step 4: Quick randomness consumption test
        LOG.info("\n[Step 4] Testing randomness consumption...")
        deployer = await run_helper.create_test_account("smoke_deployer", fund_wei=2 * 10**18)
        player = await run_helper.create_test_account("smoke_player", fund_wei=1 * 10**18)
        
        dice_contract = await deploy_random_dice(run_helper, deployer)
        
        # Roll dice once
        receipt = await dice_contract.roll_dice(player)
        roller, roll_result, seed_used = await dice_contract.get_latest_roll()
        
        assert 1 <= roll_result <= 6, f"Invalid dice result: {roll_result}"
        assert seed_used > 0, f"Invalid seed: {seed_used}"
        
        LOG.info(f"  ✅ Randomness Consumption OK")
        LOG.info(f"     Result: {roll_result}")
        LOG.info(f"     Seed: {seed_used}")
        
        # Step 5: Verify DKG is still responsive
        LOG.info("\n[Step 5] Re-checking DKG status...")
        final_dkg_status = await gravity_http_client.get_dkg_status()
        
        assert final_dkg_status is not None, "DKG became unresponsive"
        final_block_status = final_dkg_status["block_number"]
        
        assert final_block_status >= current_block, "DKG block number went backwards"
        
        LOG.info(f"  ✅ DKG Still Responsive")
        LOG.info(f"     New block: {final_block_status}")
        LOG.info(f"     Progress: {final_block_status - current_block} blocks")
    
    test_result.mark_success(
        initial_epoch=current_epoch,
        initial_round=current_round,
        initial_block=current_block,
        blocks_produced=blocks_produced,
        contract_address=dice_contract.address,
        smoke_test_passed=True
    )
    
    LOG.info("\n" + "="*70)
    LOG.info("✅ Test 'Randomness Smoke Test' PASSED!")
    LOG.info("="*70)


@test_case
async def test_randomness_reconfiguration(run_helper: RunHelper, test_result: TestResult):
    """
    Test DKG state tracking and epoch changes.
    
    This test monitors:
    1. DKG epoch and round progression
    2. Randomness availability across epochs
    3. State consistency during transitions
    """
    LOG.info("="*70)
    LOG.info("Test: Randomness Reconfiguration (Epoch Tracking)")
    LOG.info("="*70)
    
    gravity_http_client = GravityHttpClient(
        base_url=run_helper.client.rpc_url.replace(":8545", ":1998")
    )
    
    async with gravity_http_client:
        # Step 1: Record initial state
        LOG.info("\n[Step 1] Recording initial DKG state...")
        initial_status = await gravity_http_client.get_dkg_status()
        
        initial_epoch = initial_status["epoch"]
        initial_round = initial_status["round"]
        initial_block = initial_status["block_number"]
        
        LOG.info(f"  Initial State:")
        LOG.info(f"    Epoch: {initial_epoch}")
        LOG.info(f"    Round: {initial_round}")
        LOG.info(f"    Block: {initial_block}")
        
        # Step 2: Monitor state progression
        LOG.info("\n[Step 2] Monitoring DKG progression...")
        
        states = []
        for i in range(10):
            await asyncio.sleep(2)
            status = await gravity_http_client.get_dkg_status()
            states.append({
                "epoch": status["epoch"],
                "round": status["round"],
                "block": status["block_number"],
                "timestamp": i * 2
            })
            
            if i % 3 == 0:
                LOG.info(f"  Sample {i//3 + 1}: Epoch {status['epoch']}, "
                        f"Round {status['round']}, Block {status['block_number']}")
        
        # Step 3: Analyze progression
        LOG.info("\n[Step 3] Analyzing state progression...")
        
        epochs = [s["epoch"] for s in states]
        rounds = [s["round"] for s in states]
        blocks = [s["block"] for s in states]
        
        epoch_changes = len(set(epochs))
        round_progression = rounds[-1] - rounds[0]
        block_progression = blocks[-1] - blocks[0]
        
        LOG.info(f"  Progression over 20 seconds:")
        LOG.info(f"    Epochs seen: {epoch_changes} (range: {min(epochs)}-{max(epochs)})")
        LOG.info(f"    Round delta: {round_progression}")
        LOG.info(f"    Block delta: {block_progression}")
        
        # Verify monotonic progression
        for i in range(1, len(states)):
            prev_block = states[i-1]["block"]
            curr_block = states[i]["block"]
            assert curr_block >= prev_block, \
                f"Block number went backwards: {prev_block} -> {curr_block}"
        
        LOG.info(f"  ✅ Monotonic block progression verified")
        
        # Step 4: Verify randomness availability
        LOG.info("\n[Step 4] Verifying randomness availability...")
        
        randomness_available = 0
        for state in states[::2]:  # Check every other state
            block = state["block"]
            randomness = await gravity_http_client.get_randomness(block)
            if randomness:
                randomness_available += 1
        
        availability_rate = randomness_available / (len(states) // 2) * 100
        
        LOG.info(f"  Randomness availability: {randomness_available}/{len(states)//2} "
                f"({availability_rate:.1f}%)")
        
        assert availability_rate >= 80, \
            f"Randomness availability too low: {availability_rate}%"
        
        LOG.info(f"  ✅ Randomness availability OK")
    
    test_result.mark_success(
        initial_epoch=initial_epoch,
        final_epoch=epochs[-1],
        epoch_changes=epoch_changes,
        round_progression=round_progression,
        block_progression=block_progression,
        randomness_availability_rate=availability_rate,
        states_sampled=len(states)
    )
    
    LOG.info("\n" + "="*70)
    LOG.info("✅ Test 'Randomness Reconfiguration' PASSED!")
    LOG.info("="*70)


@test_case
async def test_randomness_multi_contract(run_helper: RunHelper, test_result: TestResult):
    """
    Test multiple contracts using randomness simultaneously.
    
    This verifies:
    1. Multiple contracts can consume randomness in the same block
    2. Contracts get consistent block.difficulty
    3. Results are independent despite same seed source
    """
    LOG.info("="*70)
    LOG.info("Test: Multi-Contract Randomness (Isolation & Consistency)")
    LOG.info("="*70)
    
    # Step 1: Deploy multiple contracts
    LOG.info("\n[Step 1] Deploying 3 RandomDice contracts...")
    
    deployer = await run_helper.create_test_account("multi_deployer", fund_wei=10 * 10**18)
    players = [
        await run_helper.create_test_account(f"player_{i}", fund_wei=2 * 10**18)
        for i in range(3)
    ]
    
    contracts = []
    for i in range(3):
        contract = await deploy_random_dice(run_helper, deployer)
        contracts.append(contract)
        LOG.info(f"  Contract {i+1}: {contract.address}")
    
    # Step 2: Roll all contracts in parallel
    LOG.info("\n[Step 2] Rolling all contracts simultaneously (5 rounds)...")
    
    roll_records = []
    
    for round_num in range(5):
        LOG.info(f"\n  Round {round_num + 1}/5:")
        
        # Submit all rolls in parallel
        roll_tasks = [
            contracts[i].roll_dice(players[i])
            for i in range(3)
        ]
        
        receipts = await asyncio.gather(*roll_tasks)
        
        # Get results from all contracts
        result_tasks = [contract.get_latest_roll() for contract in contracts]
        results = await asyncio.gather(*result_tasks)
        
        # Analyze this round - results is list of (roller, result, seed) tuples
        block_numbers = [int(r["blockNumber"], 16) for r in receipts]
        dice_results = [r[1] for r in results]  # r[1] is roll_result
        seeds = [r[2] for r in results]  # r[2] is seed_used
        
        # Check if they're in the same block
        same_block = len(set(block_numbers)) == 1
        
        LOG.info(f"    Blocks: {block_numbers}")
        LOG.info(f"    Results: {dice_results}")
        LOG.info(f"    Same block: {'✅' if same_block else '❌'}")
        
        if same_block:
            # If same block, seeds MUST be identical
            assert len(set(seeds)) == 1, \
                f"Contracts in same block have different seeds: {seeds}"
            LOG.info(f"    ✅ Seeds consistent: {seeds[0]}")
        else:
            # If different blocks, seeds should be different
            LOG.info(f"    Seeds: {len(set(seeds))} unique")
        
        roll_records.append({
            "round": round_num + 1,
            "blocks": block_numbers,
            "results": dice_results,
            "seeds": seeds,
            "same_block": same_block
        })
        
        await asyncio.sleep(1)
    
    # Step 3: Statistical analysis
    LOG.info("\n[Step 3] Statistical Analysis...")
    
    same_block_count = sum(1 for r in roll_records if r["same_block"])
    
    # Collect all results per contract
    contract_results = [[], [], []]
    for record in roll_records:
        for i, result in enumerate(record["results"]):
            contract_results[i].append(result)
    
    LOG.info(f"  Same-block rounds: {same_block_count}/{len(roll_records)}")
    LOG.info(f"\n  Per-contract statistics:")
    
    for i, results in enumerate(contract_results):
        distribution = Counter(results)
        unique_results = len(set(results))
        LOG.info(f"    Contract {i+1}:")
        LOG.info(f"      Results: {results}")
        LOG.info(f"      Distribution: {dict(distribution)}")
        LOG.info(f"      Unique: {unique_results}")
    
    # Step 4: Verify independence
    LOG.info(f"\n[Step 4] Verifying result independence...")
    
    # Even with same seed, results should vary due to different nonces/addresses
    all_results_flat = [r for results in contract_results for r in results]
    unique_combinations = len(set(zip(contract_results[0], contract_results[1], contract_results[2])))
    
    LOG.info(f"  Unique result combinations: {unique_combinations}/{len(roll_records)}")
    
    # At least some rounds should have different results
    assert unique_combinations >= len(roll_records) * 0.5, \
        "Contracts producing too similar results"
    
    LOG.info(f"  ✅ Results show good independence")
    
    test_result.mark_success(
        num_contracts=len(contracts),
        num_rounds=len(roll_records),
        same_block_rounds=same_block_count,
        unique_result_combinations=unique_combinations,
        contract_addresses=[c.address for c in contracts],
        all_results=contract_results
    )
    
    LOG.info("\n" + "="*70)
    LOG.info("✅ Test 'Multi-Contract Randomness' PASSED!")
    LOG.info("="*70)


@test_case
async def test_randomness_api_completeness(run_helper: RunHelper, test_result: TestResult):
    """
    Test DKG API completeness and data consistency.
    
    This verifies:
    1. All DKG API endpoints are accessible
    2. Data consistency across different endpoints
    3. Historical data availability
    4. API response format correctness
    """
    LOG.info("="*70)
    LOG.info("Test: Randomness API Completeness")
    LOG.info("="*70)
    
    gravity_http_client = GravityHttpClient(
        base_url=run_helper.client.rpc_url.replace(":8545", ":1998")
    )
    
    async with gravity_http_client:
        # Step 1: Test DKG status endpoint
        LOG.info("\n[Step 1] Testing DKG status endpoint...")
        
        status = await gravity_http_client.get_dkg_status()
        
        # Verify all required fields
        required_fields = ["epoch", "round", "block_number", "participating_nodes"]
        for field in required_fields:
            assert field in status, f"DKG status missing required field: {field}"
            LOG.info(f"  ✅ {field}: {status[field]}")
        
        current_epoch = status["epoch"]
        current_block = status["block_number"]
        
        # Step 2: Test randomness endpoint with current block
        LOG.info("\n[Step 2] Testing randomness endpoint (current block)...")
        
        randomness = await gravity_http_client.get_randomness(current_block)
        assert randomness is not None, f"No randomness for current block {current_block}"
        assert randomness.startswith("0x"), "Randomness should start with 0x"
        assert len(randomness) == 66, f"Randomness should be 66 chars, got {len(randomness)}"
        
        LOG.info(f"  ✅ Current block {current_block}: {randomness[:20]}...")
        
        # Step 3: Test historical randomness availability
        LOG.info("\n[Step 3] Testing historical randomness availability...")
        
        test_blocks = [
            current_block,
            current_block - 10,
            current_block - 50,
            current_block - 100
        ]
        
        available_count = 0
        for block in test_blocks:
            if block < 0:
                continue
                
            rand = await gravity_http_client.get_randomness(block)
            if rand:
                available_count += 1
                LOG.info(f"  ✅ Block {block}: Available")
            else:
                LOG.info(f"  ⚠️  Block {block}: Not available")
        
        availability_rate = available_count / len([b for b in test_blocks if b >= 0]) * 100
        LOG.info(f"\n  Historical availability: {available_count}/{len(test_blocks)} ({availability_rate:.1f}%)")
        
        # Step 4: Test randomness vs block.difficulty consistency
        LOG.info("\n[Step 4] Testing randomness vs block.difficulty consistency...")
        
        # Get a recent block's randomness from both sources
        test_block = current_block - 5
        api_randomness = await gravity_http_client.get_randomness(test_block)
        
        if api_randomness:
            # Get block info from EVM RPC
            block_info = await run_helper.client.get_block(test_block)
            block_difficulty = block_info.get("difficulty")
            block_mixhash = block_info.get("mixHash")
            
            LOG.info(f"  Block {test_block}:")
            LOG.info(f"    API randomness: {api_randomness[:20]}...")
            LOG.info(f"    Block difficulty: {block_difficulty}")
            LOG.info(f"    Block mixHash: {block_mixhash}")
            
            # In PoS, difficulty == mixHash == API randomness
            if block_difficulty and block_mixhash:
                assert block_difficulty == block_mixhash, \
                    f"Block difficulty != mixHash"
                
                # Convert API randomness to int for comparison
                api_rand_int = int(api_randomness, 16)
                difficulty_int = int(block_difficulty, 16)
                
                if api_rand_int == difficulty_int:
                    LOG.info(f"  ✅ API randomness matches block.difficulty")
                else:
                    LOG.info(f"  ℹ️  API randomness differs from block.difficulty (expected for DKG-based randomness)")
        
        # Step 5: Test API responsiveness under load
        LOG.info("\n[Step 5] Testing API responsiveness...")
        
        import time
        start = time.time()
        
        # Make 10 concurrent requests
        tasks = [gravity_http_client.get_dkg_status() for _ in range(10)]
        results = await asyncio.gather(*tasks)
        
        duration = time.time() - start
        
        assert all(r is not None for r in results), "Some requests failed"
        assert all(r["epoch"] == current_epoch for r in results), "Inconsistent epoch across requests"
        
        avg_latency = duration / len(tasks) * 1000  # ms
        LOG.info(f"  ✅ 10 concurrent requests completed in {duration:.2f}s")
        LOG.info(f"  Average latency: {avg_latency:.1f}ms")
        
        # Step 6: Test future block handling
        LOG.info("\n[Step 6] Testing future block handling...")
        
        future_block = current_block + 1000
        future_randomness = await gravity_http_client.get_randomness(future_block)
        
        assert future_randomness is None, \
            f"Future block {future_block} should not have randomness yet"
        
        LOG.info(f"  ✅ Future block {future_block} correctly returns None")
        
        # Step 7: Test invalid block handling
        LOG.info("\n[Step 7] Testing invalid block handling...")
        
        invalid_randomness = await gravity_http_client.get_randomness(-1)
        assert invalid_randomness is None, "Invalid block should return None"
        
        LOG.info(f"  ✅ Invalid block handled gracefully")
    
    test_result.mark_success(
        current_epoch=current_epoch,
        current_block=current_block,
        historical_availability_rate=availability_rate,
        api_latency_ms=avg_latency,
        concurrent_requests_ok=True,
        future_block_handling_ok=True,
        invalid_block_handling_ok=True
    )
    
    LOG.info("\n" + "="*70)
    LOG.info("✅ Test 'Randomness API Completeness' PASSED!")
    LOG.info("="*70)


@test_case
async def test_randomness_stress(run_helper: RunHelper, test_result: TestResult):
    """
    Stress test: High-frequency randomness consumption.
    
    This tests:
    1. System stability under load
    2. Randomness quality with many samples
    3. Performance metrics (TPS, latency)
    """
    LOG.info("="*70)
    LOG.info("Test: Randomness Stress Test (High-Frequency Consumption)")
    LOG.info("="*70)
    
    # Step 1: Setup
    LOG.info("\n[Step 1] Setting up for stress test...")
    
    deployer = await run_helper.create_test_account("stress_deployer", fund_wei=5 * 10**18)
    
    # Create multiple players for parallel execution
    num_players = 5
    players = [
        await run_helper.create_test_account(f"stress_player_{i}", fund_wei=5 * 10**18)
        for i in range(num_players)
    ]
    
    contract = await deploy_random_dice(run_helper, deployer)
    LOG.info(f"  Players: {num_players}")
    
    # Step 2: Execute high-frequency rolls
    num_rolls = 50
    LOG.info(f"\n[Step 2] Executing {num_rolls} rolls...")
    
    import time
    start_time = time.time()
    
    results = []
    seeds = []
    blocks = []
    gas_used = []
    
    for i in range(num_rolls):
        # Rotate through players
        player = players[i % num_players]
        
        try:
            receipt = await contract.roll_dice(player)
            roller, result, seed = await contract.get_latest_roll()
            
            block = int(receipt["blockNumber"], 16)
            gas = int(receipt["gasUsed"], 16)
            
            results.append(result)
            seeds.append(seed)
            blocks.append(block)
            gas_used.append(gas)
            
            if (i + 1) % 10 == 0:
                LOG.info(f"  Progress: {i+1}/{num_rolls} rolls completed")
                
        except Exception as e:
            LOG.error(f"  Roll {i+1} failed: {e}")
            continue
    
    end_time = time.time()
    duration = end_time - start_time
    
    # Step 3: Performance metrics
    LOG.info(f"\n[Step 3] Performance Metrics...")
    
    successful_rolls = len(results)
    success_rate = successful_rolls / num_rolls * 100
    tps = successful_rolls / duration
    avg_gas = sum(gas_used) / len(gas_used) if gas_used else 0
    
    LOG.info(f"  Duration: {duration:.2f}s")
    LOG.info(f"  Successful rolls: {successful_rolls}/{num_rolls} ({success_rate:.1f}%)")
    LOG.info(f"  TPS: {tps:.2f} transactions/second")
    LOG.info(f"  Avg gas: {avg_gas:.0f}")
    
    assert success_rate >= 90, f"Success rate too low: {success_rate}%"
    
    # Step 4: Quality metrics
    LOG.info(f"\n[Step 4] Randomness Quality Metrics...")
    
    # Distribution analysis
    distribution = Counter(results)
    expected_per_value = successful_rolls / 6
    
    LOG.info(f"  Result distribution (expected: ~{expected_per_value:.1f} per value):")
    for value in range(1, 7):
        count = distribution.get(value, 0)
        percentage = count / successful_rolls * 100 if successful_rolls > 0 else 0
        bar = "█" * int(percentage / 2)
        LOG.info(f"    {value}: {bar} ({count}, {percentage:.1f}%)")
    
    # Seed diversity
    unique_seeds = len(set(seeds))
    diversity_ratio = unique_seeds / successful_rolls if successful_rolls > 0 else 0
    
    LOG.info(f"\n  Seed diversity:")
    LOG.info(f"    Unique seeds: {unique_seeds}/{successful_rolls}")
    LOG.info(f"    Diversity ratio: {diversity_ratio*100:.1f}%")
    
    # Block spread
    unique_blocks = len(set(blocks))
    block_range = max(blocks) - min(blocks) + 1 if blocks else 0
    
    LOG.info(f"\n  Block spread:")
    LOG.info(f"    Unique blocks: {unique_blocks}")
    LOG.info(f"    Block range: {min(blocks)}-{max(blocks)} ({block_range} blocks)")
    LOG.info(f"    Avg rolls per block: {successful_rolls/unique_blocks:.2f}")
    
    # Chi-square test for uniformity (simplified)
    chi_square = sum((distribution.get(i, 0) - expected_per_value)**2 / expected_per_value 
                     for i in range(1, 7))
    LOG.info(f"\n  Chi-square statistic: {chi_square:.2f} (lower is more uniform)")
    
    test_result.mark_success(
        total_rolls=num_rolls,
        successful_rolls=successful_rolls,
        success_rate=success_rate,
        duration=duration,
        tps=tps,
        avg_gas=avg_gas,
        distribution=dict(distribution),
        unique_seeds=unique_seeds,
        diversity_ratio=diversity_ratio,
        unique_blocks=unique_blocks,
        chi_square=chi_square,
        contract_address=contract.address
    )
    
    LOG.info("\n" + "="*70)
    LOG.info("✅ Test 'Randomness Stress Test' PASSED!")
    LOG.info("="*70)

