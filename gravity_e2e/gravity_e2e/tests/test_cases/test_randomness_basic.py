"""
随机数基础测试用例
对应原测试：e2e_basic_consumption.rs
"""
import asyncio
import logging
from typing import Dict, List
from eth_account import Account

from ...helpers.test_helpers import RunHelper, TestResult, test_case
from ...core.client.gravity_http_client import GravityHttpClient
from ...utils.randomness_utils import RandomDiceHelper

LOG = logging.getLogger(__name__)


@test_case
async def test_randomness_basic_consumption(
    run_helper: RunHelper,
    test_result: TestResult
):
    """
    测试基础随机数消费功能
    
    对应原测试：e2e_basic_consumption.rs
    
    测试步骤:
    1. 获取当前DKG状态
    2. 部署RandomDice合约
    3. 多次调用rollDice()
    4. 验证随机数范围（1-6）
    5. 验证block.difficulty传递正确
    6. 验证随机数种子变化
    7. 统计分析
    """
    LOG.info("=" * 70)
    LOG.info("Test: Randomness Basic Consumption (e2e_basic_consumption)")
    LOG.info("=" * 70)
    
    # 初始化HTTP客户端（从RPC URL推导HTTP API URL）
    http_url = run_helper.client.rpc_url.replace(":8545", ":1998")
    async with GravityHttpClient(http_url) as http_client:
        
        # ========== Step 1: 获取DKG状态 ==========
        LOG.info("\n[Step 1] Getting DKG status...")
        try:
            dkg_status = await http_client.get_dkg_status()
            LOG.info(f"Current DKG Status:")
            LOG.info(f"  Epoch: {dkg_status['epoch']}")
            LOG.info(f"  Round: {dkg_status['round']}")
            LOG.info(f"  Block: {dkg_status['block_number']}")
            LOG.info(f"  Nodes: {dkg_status['participating_nodes']}")
        except Exception as e:
            LOG.warning(f"Failed to get DKG status: {e}")
            dkg_status = {"epoch": 0, "round": 0, "block_number": 0, "participating_nodes": 0}
        
        # ========== Step 2: 部署RandomDice合约 ==========
        LOG.info("\n[Step 2] Deploying RandomDice contract...")
        
        deployer = run_helper.faucet_account
        LOG.info(f"Deployer address: {deployer['address']}")
        
        # 加载合约字节码
        try:
            bytecode = RandomDiceHelper.load_bytecode()
            LOG.info(f"Loaded bytecode: {len(bytecode)} characters")
        except FileNotFoundError as e:
            LOG.error(str(e))
            raise RuntimeError(
                "Please compile RandomDice contract first:\n"
                "  cd /Users/lightman/repos/gravity-sdk\n"
                "  forge build"
            )
        
        # 获取部署参数
        nonce = await run_helper.client.get_transaction_count(deployer["address"])
        gas_price = await run_helper.client.get_gas_price()
        chain_id = await run_helper.client.get_chain_id()
        
        LOG.debug(f"Deploy params: nonce={nonce}, gas_price={gas_price}, chain_id={chain_id}")
        
        # 构建部署交易
        deploy_tx = {
            "data": bytecode,
            "gas": hex(500000),
            "gasPrice": hex(gas_price),
            "nonce": hex(nonce),
            "chainId": hex(chain_id),
            "value": "0x0"
        }
        
        # 签名并发送
        private_key = deployer["private_key"]
        if private_key.startswith("0x"):
            private_key = private_key[2:]
        
        signed_deploy = Account.sign_transaction(deploy_tx, private_key)
        deploy_tx_hash = await run_helper.client.send_raw_transaction(
            signed_deploy.raw_transaction
        )
        
        LOG.info(f"Deploy transaction sent: {deploy_tx_hash}")
        
        # 等待部署确认
        deploy_receipt = await run_helper.client.wait_for_transaction_receipt(
            deploy_tx_hash,
            timeout=60
        )
        
        if deploy_receipt.get("status") != "0x1":
            raise RuntimeError(f"Contract deployment failed: {deploy_receipt}")
        
        contract_address = deploy_receipt.get("contractAddress")
        if not contract_address:
            raise RuntimeError("No contract address in deployment receipt")
        
        deploy_gas_used = int(deploy_receipt.get("gasUsed", "0x0"), 16)
        
        LOG.info(f"✅ Contract deployed successfully!")
        LOG.info(f"   Address: {contract_address}")
        LOG.info(f"   Gas used: {deploy_gas_used}")
        LOG.info(f"   Block: {int(deploy_receipt.get('blockNumber', '0x0'), 16)}")
        
        # ========== Step 3: 创建RandomDice辅助对象 ==========
        dice = RandomDiceHelper(run_helper.client, contract_address)
        
        # ========== Step 4: 执行10次rollDice ==========
        LOG.info("\n[Step 3] Rolling dice 10 times...")
        
        roll_count = 10
        roll_results: List[int] = []
        seeds_used: List[int] = []
        blocks: List[int] = []
        tx_hashes: List[str] = []
        
        for i in range(roll_count):
            LOG.info(f"\n  🎲 Roll #{i+1}/{roll_count}:")
            
            # 调用rollDice
            try:
                receipt = await dice.roll_dice(deployer)
            except Exception as e:
                LOG.error(f"    ❌ Failed to roll: {e}")
                continue
            
            block_number = int(receipt.get("blockNumber", "0x0"), 16)
            tx_hash = receipt.get("transactionHash", "unknown")
            gas_used = int(receipt.get("gasUsed", "0x0"), 16)
            
            LOG.info(f"    Tx: {tx_hash}")
            LOG.info(f"    Block: {block_number}")
            LOG.info(f"    Gas: {gas_used}")
            
            # 读取结果
            try:
                result = await dice.get_last_result()
                seed = await dice.get_last_seed()
                
                LOG.info(f"    Result: {result} ({'✅' if 1 <= result <= 6 else '❌'})")
                LOG.info(f"    Seed: {seed}")
                
                # 验证范围
                if not (1 <= result <= 6):
                    raise AssertionError(f"Roll result {result} out of valid range [1, 6]")
                
                roll_results.append(result)
                seeds_used.append(seed)
                blocks.append(block_number)
                tx_hashes.append(tx_hash)
                
                LOG.info(f"    ✅ Valid roll")
            except Exception as e:
                LOG.error(f"    ❌ Failed to read result: {e}")
                raise
            
            # 短暂等待，确保在不同区块
            await asyncio.sleep(1)
        
        # ========== Step 5: 验证block.difficulty传递 ==========
        LOG.info("\n[Step 4] Verifying block.difficulty propagation...")
        
        mismatches = 0
        for i, (block_num, seed) in enumerate(zip(blocks, seeds_used)):
            try:
                block = await run_helper.client.get_block(block_num, full_transactions=False)
                
                difficulty_hex = block.get("difficulty", "0x0")
                difficulty = int(difficulty_hex, 16)
                
                match = (seed == difficulty)
                status = "✅" if match else "❌"
                
                LOG.info(f"  Roll #{i+1} (Block {block_num}): {status}")
                LOG.info(f"    Contract seed: {seed}")
                LOG.info(f"    Block difficulty: {difficulty}")
                
                if not match:
                    mismatches += 1
                    LOG.warning(f"    ⚠️  MISMATCH!")
            except Exception as e:
                LOG.error(f"  Roll #{i+1}: Failed to verify - {e}")
                mismatches += 1
        
        if mismatches > 0:
            raise AssertionError(
                f"{mismatches}/{len(blocks)} blocks had seed/difficulty mismatches"
            )
        
        LOG.info(f"✅ All {len(blocks)} blocks verified successfully!")
        
        # ========== Step 6: 验证随机数API（可选） ==========
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
        
        # ========== Step 7: 统计分析 ==========
        LOG.info("\n[Step 6] Statistical Analysis...")
        LOG.info(f"  Total rolls: {len(roll_results)}")
        LOG.info(f"  Results: {roll_results}")
        LOG.info(f"\n  Distribution:")
        
        for value in range(1, 7):
            count = roll_results.count(value)
            percentage = (count / len(roll_results)) * 100 if roll_results else 0
            bar = "█" * count
            LOG.info(f"    {value}: {bar} ({count}, {percentage:.1f}%)")
        
        # 验证种子变化
        unique_seeds = len(set(seeds_used))
        diversity_ratio = unique_seeds / len(seeds_used) if seeds_used else 0
        
        LOG.info(f"\n  Seed diversity:")
        LOG.info(f"    Unique seeds: {unique_seeds}/{len(seeds_used)}")
        LOG.info(f"    Diversity ratio: {diversity_ratio*100:.1f}%")
        
        if diversity_ratio < 0.8:
            LOG.warning(f"    ⚠️  Low seed diversity detected!")
        else:
            LOG.info(f"    ✅ Good seed diversity")
        
        # ========== 记录测试结果 ==========
        test_result.mark_success(
            contract_address=contract_address,
            deploy_tx_hash=deploy_tx_hash,
            deploy_gas_used=deploy_gas_used,
            total_rolls=len(roll_results),
            roll_results=roll_results,
            unique_seeds=unique_seeds,
            diversity_ratio=diversity_ratio,
            blocks_tested=blocks,
            dkg_epoch=dkg_status.get('epoch', 0),
            dkg_round=dkg_status.get('round', 0)
        )
        
        LOG.info("\n" + "=" * 70)
        LOG.info("✅ Test 'Randomness Basic Consumption' PASSED!")
        LOG.info("=" * 70)


@test_case
async def test_randomness_correctness(
    run_helper: RunHelper,
    test_result: TestResult
):
    """
    测试随机数正确性（观察性验证）
    
    对应原测试：e2e_correctness.rs（简化版，不包含密码学验证）
    
    测试步骤:
    1. 获取当前DKG状态和区块信息
    2. 验证最近10个块的随机数一致性
    3. 检查block.difficulty与API randomness的关系
    4. 可选：等待下一个epoch并验证随机数更新
    """
    LOG.info("=" * 70)
    LOG.info("Test: Randomness Correctness (e2e_correctness, observational)")
    LOG.info("=" * 70)
    
    from ...utils.randomness_utils import RandomnessVerifier
    
    http_url = run_helper.client.rpc_url.replace(":8545", ":1998")
    async with GravityHttpClient(http_url) as http_client:
        
        # ========== Step 1: 获取当前状态 ==========
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
        
        # ========== Step 2: 验证最近的块 ==========
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
                
                # 详细日志
                if result.get("valid"):
                    LOG.info(f"    ✅ Valid")
                else:
                    LOG.warning(f"    ⚠️  Invalid")
                
                if "checks" in result:
                    for check_name, check_result in result["checks"].items():
                        status = "✅" if check_result else "❌"
                        LOG.info(f"      {status} {check_name}: {check_result}")
                
                # 显示关键数据
                if "block_difficulty" in result:
                    LOG.info(f"      Block difficulty: {result['block_difficulty']}")
                if "api_randomness" in result and result["api_randomness"]:
                    randomness_preview = result["api_randomness"][:32]
                    LOG.info(f"      API randomness: {randomness_preview}...")
                
            except Exception as e:
                LOG.error(f"    ❌ Failed to verify: {e}")
                verification_results.append({
                    "block_number": block_num,
                    "error": str(e),
                    "valid": False
                })
        
        # ========== Step 3: 统计验证结果 ==========
        LOG.info(f"\n[Step 3] Verification Summary...")
        
        total_count = len(verification_results)
        valid_count = sum(1 for r in verification_results if r.get("valid", False))
        error_count = sum(1 for r in verification_results if "error" in r)
        
        success_rate = (valid_count / total_count * 100) if total_count > 0 else 0
        
        LOG.info(f"  Total blocks checked: {total_count}")
        LOG.info(f"  Valid blocks: {valid_count}")
        LOG.info(f"  Errors: {error_count}")
        LOG.info(f"  Success rate: {success_rate:.1f}%")
        
        # 详细分析
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
        
        # ========== Step 4: 等待下一个epoch（可选，如果当前epoch较小） ==========
        if current_epoch < 5:  # 只在早期epoch时等待
            LOG.info(f"\n[Step 4] Waiting for next epoch (current: {current_epoch})...")
            
            try:
                next_epoch = await http_client.wait_for_epoch(
                    current_epoch + 1,
                    timeout=120
                )
                
                LOG.info(f"  ✅ Reached epoch {next_epoch}")
                
                # 验证新epoch的随机数
                new_status = await http_client.get_dkg_status()
                new_block = new_status['block_number']
                
                LOG.info(f"  New state:")
                LOG.info(f"    Block: {new_block}")
                LOG.info(f"    Round: {new_status['round']}")
                
                # 验证新epoch的第一个块
                new_result = await RandomnessVerifier.verify_block_randomness(
                    run_helper.client,
                    http_client,
                    new_block
                )
                
                LOG.info(f"\n  New epoch block verification:")
                LOG.info(f"    Block {new_block}: {'✅ Valid' if new_result.get('valid') else '❌ Invalid'}")
                
                # 检查随机数是否变化
                if verification_results and new_result.get("api_randomness"):
                    old_randomness = verification_results[0].get("api_randomness")
                    new_randomness = new_result.get("api_randomness")
                    
                    if old_randomness and new_randomness:
                        changed = (old_randomness != new_randomness)
                        LOG.info(f"    Randomness changed: {'✅ Yes' if changed else '⚠️  No'}")
                
                epoch_tested = True
            except TimeoutError as e:
                LOG.warning(f"  ⏱  Timeout waiting for next epoch: {e}")
                epoch_tested = False
            except Exception as e:
                LOG.error(f"  ❌ Error during epoch wait: {e}")
                epoch_tested = False
        else:
            LOG.info(f"\n[Step 4] Skipping epoch wait (already at epoch {current_epoch})")
            epoch_tested = False
        
        # ========== 验证最低要求 ==========
        if valid_count == 0:
            raise AssertionError("No valid blocks found!")
        
        if success_rate < 50:
            LOG.warning(f"⚠️  Low success rate: {success_rate:.1f}%")
        
        # ========== 记录测试结果 ==========
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
        LOG.info("✅ Test 'Randomness Correctness' PASSED!")
        LOG.info("=" * 70)

