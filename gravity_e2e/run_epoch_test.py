#!/usr/bin/env python3
"""
独立运行 epoch_consistency 测试的脚本
不依赖 node_connector，直接运行测试用例
"""
import asyncio
import logging
import sys
from pathlib import Path

# 添加项目路径
sys.path.insert(0, str(Path(__file__).parent))

from gravity_e2e.tests.test_cases.test_epoch_consistency import test_epoch_consistency
from gravity_e2e.helpers.test_helpers import RunHelper, TestResult
from gravity_e2e.core.client.gravity_client import GravityClient

# 设置日志
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)

LOG = logging.getLogger(__name__)


async def main():
    """运行测试"""
    LOG.info("=" * 70)
    LOG.info("Running Epoch Consistency Test (Standalone)")
    LOG.info("=" * 70)
    
    # 创建一个简单的 RunHelper（测试用例实际上不需要它，但装饰器需要）
    # 测试用例会自己管理节点，不依赖 RunHelper 的 client
    dummy_client = GravityClient("http://127.0.0.1:8545", "dummy_node")
    run_helper = RunHelper(
        client=dummy_client,
        working_dir=str(Path(__file__).parent),
        faucet_account=None
    )
    
    test_result = None
    try:
        # 运行测试（test_case 装饰器会自动创建 TestResult）
        test_result = await test_epoch_consistency(run_helper=run_helper)
        
        duration = test_result.details.get("duration", 0)
        
        LOG.info("=" * 70)
        LOG.info("TEST COMPLETED")
        LOG.info("=" * 70)
        LOG.info(f"Success: {test_result.success}")
        if duration > 0:
            LOG.info(f"Duration: {duration:.2f} seconds")
        
        if test_result.success:
            LOG.info("✅ Test PASSED")
            if test_result.details:
                LOG.info(f"Details: {test_result.details}")
            sys.exit(0)
        else:
            LOG.error(f"❌ Test FAILED: {test_result.error}")
            sys.exit(1)
            
    except Exception as e:
        LOG.error(f"❌ Test failed with exception: {e}", exc_info=True)
        if test_result:
            test_result.mark_failure(str(e))
        sys.exit(1)


if __name__ == "__main__":
    asyncio.run(main())

