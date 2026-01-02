"""
Epoch consistency extended test
Tests data consistency after epoch switching for 10 epochs
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
from typing import Dict, Optional

from gravity_e2e.helpers.test_helpers import RunHelper, TestResult, test_case
from gravity_e2e.core.client.gravity_http_client import GravityHttpClient
from gravity_e2e.core.node_manager import NodeManager

LOG = logging.getLogger(__name__)


@test_case
async def test_epoch_consistency_extended(
    run_helper: RunHelper,
    test_result: TestResult
):
    """
    Test epoch switching data consistency for 10 epochs
    
    Test steps:
    1. Deploy node1
    2. Start node1
    3. Wait for 10 epochs (epoch 1-10), checking every 10 seconds
    4. Record data for each epoch
    5. Validate for N = [1, 2, ..., 9]:
       - Epoch N ledger_info.block_number == Epoch N+1 round 1 block.block_number - 1
       - Epoch N+1 round 1 QC commit_info_block_id != Epoch N ledger_info.block_hash
    """
    LOG.info("=" * 70)
    LOG.info("Test: Epoch Consistency Extended Test (10 epochs)")
    LOG.info("=" * 70)
    
    node_manager = NodeManager()
    node_name = "node1"
    install_dir = "/tmp"
    http_url = "http://127.0.0.1:1024"
    
    try:
        # Step 1: Deploy node1
        LOG.info("\n[Step 1] Deploying node1...")
        deploy_success = node_manager.deploy_node(
            node_name=node_name,
            mode="single",
            install_dir=install_dir,
            bin_version="quick-release",  # 使用 quick-release 版本
            recover=False
        )
        
        if not deploy_success:
            raise RuntimeError(f"Failed to deploy {node_name}")
        
        deploy_path = node_manager.get_node_deploy_path(node_name, install_dir)
        LOG.info(f"✅ Node {node_name} deployed to {deploy_path}")
        
        # Step 2: Start node1
        LOG.info("\n[Step 2] Starting node1...")
        start_success = node_manager.start_node(deploy_path)
        
        if not start_success:
            raise RuntimeError(f"Failed to start {node_name}")
        
        LOG.info(f"✅ Node {node_name} started")
        
        # Wait a bit for node to be ready
        LOG.info("Waiting 10 seconds for node to be ready...")
        await asyncio.sleep(10)
        
        # Step 3: Monitor epochs and collect data
        LOG.info("\n[Step 3] Monitoring epochs (checking every 10 seconds)...")
        
        async with GravityHttpClient(base_url=http_url) as http_client:
            epoch_data: Dict[int, Dict] = {}
            # 检查 N = [1, 2, ..., 9]，即检查 epoch 1->2, 2->3, ..., 9->10 的连续性
            target_epochs = list(range(1, 11))  # [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
            check_interval = 10  # 10 seconds
            
            for target_epoch in target_epochs:
                LOG.info(f"\n[Epoch {target_epoch}] Waiting for epoch {target_epoch}...")
                
                # Wait for target epoch
                try:
                    current_epoch = await http_client.wait_for_epoch(
                        target_epoch=target_epoch,
                        timeout=600  # 10 minutes timeout per epoch
                    )
                    LOG.info(f"✅ Reached epoch {current_epoch}")
                except TimeoutError as e:
                    LOG.error(f"❌ Timeout waiting for epoch {target_epoch}: {e}")
                    raise
                
                # Get ledger info for this epoch
                LOG.info(f"[Epoch {target_epoch}] Getting ledger info...")
                try:
                    ledger_info = await http_client.get_ledger_info_by_epoch(target_epoch)
                except RuntimeError:
                    # 如果当前 epoch 还没有完成，使用前一个 epoch
                    LOG.info(f"Epoch {target_epoch} not available yet, using epoch {target_epoch - 1}")
                    ledger_info = await http_client.get_ledger_info_by_epoch(target_epoch - 1)
                
                epoch_data[target_epoch] = {
                    "ledger_info": ledger_info
                }
                
                LOG.info(f"  Epoch {target_epoch} ledger info:")
                LOG.info(f"    block_number: {ledger_info['block_number']}")
                LOG.info(f"    round: {ledger_info['round']}")
                LOG.info(f"    block_hash: {ledger_info['block_hash']}")
                
                # 对于 epoch 2-10，获取 round 1 的 block 和 QC
                if target_epoch >= 2:
                    LOG.info(f"[Epoch {target_epoch}] Getting round 1 block and QC...")
                    
                    # Get block for epoch N+1, round 1
                    block_info = await http_client.get_block_by_epoch_round(epoch=target_epoch, round=1)
                    epoch_data[target_epoch]["block_round_1"] = block_info
                    
                    LOG.info(f"  Epoch {target_epoch}, Round 1 block:")
                    LOG.info(f"    block_number: {block_info.get('block_number')}")
                    LOG.info(f"    block_id: {block_info['block_id']}")
                    
                    # Get QC for epoch N+1, round 1
                    qc_info = await http_client.get_qc_by_epoch_round(epoch=target_epoch, round=1)
                    epoch_data[target_epoch]["qc_round_1"] = qc_info
                    
                    LOG.info(f"  Epoch {target_epoch}, Round 1 QC:")
                    LOG.info(f"    certified_block_id: {qc_info['certified_block_id']}")
                    LOG.info(f"    commit_info_block_id: {qc_info['commit_info_block_id']}")
                
                # Wait 10 seconds before checking next epoch (except for the last epoch)
                if target_epoch < target_epochs[-1]:
                    LOG.info(f"Waiting {check_interval} seconds before checking next epoch...")
                    await asyncio.sleep(check_interval)
        
        # Step 4: Validate consistency
        LOG.info("\n[Step 4] Validating data consistency...")
        
        # 检查 N = [1, 2, ..., 9] 九轮
        validation_n_values = list(range(1, 10))  # [1, 2, 3, 4, 5, 6, 7, 8, 9]
        for n in validation_n_values:
            LOG.info(f"\n[Validation for N={n}]")
            
            # 获取 Epoch N 的 ledger_info
            epoch_n_ledger_info = epoch_data[n]["ledger_info"]
            epoch_n_block_number = epoch_n_ledger_info["block_number"]
            epoch_n_block_hash = epoch_n_ledger_info["block_hash"]
            
            # 获取 Epoch N+1 round 1 的 block 和 QC
            epoch_n_plus_1 = n + 1
            epoch_n_plus_1_block = epoch_data[epoch_n_plus_1]["block_round_1"]
            epoch_n_plus_1_qc = epoch_data[epoch_n_plus_1]["qc_round_1"]
            
            epoch_n_plus_1_block_number = epoch_n_plus_1_block.get("block_number")
            if epoch_n_plus_1_block_number is None:
                raise AssertionError(f"Epoch {epoch_n_plus_1} round 1 block has no block_number")
            
            # Validation 1: Epoch N 的 block_number == Epoch N+1 round 1 的 block_number - 1
            expected_block_number = epoch_n_plus_1_block_number - 1
            if epoch_n_block_number != expected_block_number:
                raise AssertionError(
                    f"Block number mismatch for N={n}: "
                    f"Epoch {n} ledger_info.block_number ({epoch_n_block_number}) "
                    f"should equal Epoch {epoch_n_plus_1} round 1 block.block_number - 1 "
                    f"({epoch_n_plus_1_block_number} - 1 = {expected_block_number})"
                )
            
            LOG.info(f"✅ Validation 1 for N={n} passed: "
                    f"Epoch {n} block_number ({epoch_n_block_number}) == "
                    f"Epoch {epoch_n_plus_1} round 1 block_number - 1 ({expected_block_number})")
            
            # Validation 2: Epoch N+1 round 1 的 commit_info_block_id != Epoch N 的 block_hash
            epoch_n_plus_1_commit_info_block_id = epoch_n_plus_1_qc["commit_info_block_id"]
            
            # Convert hex strings to compare (remove 0x prefix if present)
            epoch_n_block_hash_clean = epoch_n_block_hash.replace("0x", "").lower()
            epoch_n_plus_1_commit_info_clean = epoch_n_plus_1_commit_info_block_id.replace("0x", "").lower()
            
            if epoch_n_block_hash_clean == epoch_n_plus_1_commit_info_clean:
                raise AssertionError(
                    f"Block ID conflict for N={n}: "
                    f"Epoch {n} ledger_info.block_hash ({epoch_n_block_hash}) "
                    f"equals Epoch {epoch_n_plus_1} round 1 QC commit_info_block_id "
                    f"({epoch_n_plus_1_commit_info_block_id})"
                )
            
            LOG.info(f"✅ Validation 2 for N={n} passed: "
                    f"Epoch {n} block_hash ({epoch_n_block_hash}) != "
                    f"Epoch {epoch_n_plus_1} round 1 QC commit_info_block_id "
                    f"({epoch_n_plus_1_commit_info_block_id})")
        
        LOG.info("\n✅ All validations passed for N=[1, 2, ..., 9]!")
        LOG.info("=" * 70)
        
        # Prepare success data with all epoch block numbers
        success_data = {
            "validation_rounds": "N=[1,2,3,4,5,6,7,8,9]"
        }
        for epoch_num in range(1, 11):
            success_data[f"epoch{epoch_num}_block_number"] = epoch_data[epoch_num]["ledger_info"]["block_number"]
        
        test_result.mark_success(**success_data)
        
    except Exception as e:
        LOG.error(f"❌ Test failed: {e}")
        test_result.mark_failure(str(e))
        raise
    finally:
        # Cleanup: Stop node1
        LOG.info("\n[Cleanup] Stopping node1...")
        try:
            deploy_path = node_manager.get_node_deploy_path(node_name, install_dir)
            stop_success = node_manager.stop_node(deploy_path)
            if stop_success:
                LOG.info(f"✅ Node {node_name} stopped")
            else:
                LOG.warning(f"⚠️  Failed to stop {node_name}")
        except Exception as e:
            LOG.warning(f"⚠️  Error stopping node: {e}")


# 允许直接运行此测试文件
if __name__ == "__main__":
    import sys
    from pathlib import Path
    
    # 添加项目路径
    sys.path.insert(0, str(Path(__file__).parent.parent.parent.parent))
    
    from gravity_e2e.helpers.test_helpers import RunHelper
    from gravity_e2e.core.client.gravity_client import GravityClient
    
    # 设置日志
    logging.basicConfig(
        level=logging.INFO,
        format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
    )
    
    async def run_direct():
        """直接运行测试"""
        dummy_client = GravityClient("http://127.0.0.1:8545", "dummy_node")
        run_helper = RunHelper(
            client=dummy_client,
            working_dir=str(Path(__file__).parent.parent.parent.parent),
            faucet_account=None
        )
        result = await test_epoch_consistency_extended(run_helper=run_helper)
        sys.exit(0 if result.success else 1)
    
    asyncio.run(run_direct())

