#!/usr/bin/env python3
"""
直接运行测试用例的脚本
"""
import asyncio
import logging
import sys
from pathlib import Path

# 添加项目路径
sys.path.insert(0, str(Path(__file__).parent))

from gravity_e2e.tests.test_cases.test_epoch_consistency_extended import test_epoch_consistency_extended
from gravity_e2e.helpers.test_helpers import RunHelper
from gravity_e2e.core.client.gravity_client import GravityClient

# 设置日志
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)

async def main():
    """直接运行测试用例"""
    # 创建必要的参数（测试用例会自己管理节点）
    dummy_client = GravityClient("http://127.0.0.1:8545", "dummy_node")
    run_helper = RunHelper(
        client=dummy_client,
        working_dir=str(Path(__file__).parent),
        faucet_account=None
    )
    
    # 直接调用测试函数（装饰器会自动处理 TestResult）
    result = await test_epoch_consistency_extended(run_helper=run_helper)
    
    # 输出结果
    print(f"\n测试结果: {'成功' if result.success else '失败'}")
    if not result.success:
        print(f"错误: {result.error}")
        sys.exit(1)

if __name__ == "__main__":
    asyncio.run(main())










