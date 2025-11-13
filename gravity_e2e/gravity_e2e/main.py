#!/usr/bin/env python3
import argparse
import asyncio
import json
import logging
import sys
import os
from pathlib import Path

from .core.node_connector import NodeConnector
from .helpers.account_manager import TestAccountManager
from .helpers.test_helpers import RunHelper
from .utils.logging import setup_logging
from .tests.test_cases.test_basic_transfer import test_eth_transfer
from .tests.test_cases.test_contract_deploy import test_simple_storage_deploy
from .tests.test_cases.test_erc20 import test_erc20_deploy_and_transfer

LOG = logging.getLogger(__name__)


async def run_test_module(module_name: str, test_helper: RunHelper, test_results: list):
    """运行测试模块"""
    try:
        if module_name == "cases.basic_transfer":
            result = await test_eth_transfer(run_helper=test_helper)
            test_results.append(result)
        elif module_name == "cases.contract":
            result = await test_simple_storage_deploy(run_helper=test_helper)
            test_results.append(result)
        elif module_name == "cases.erc20":
            result = await test_erc20_deploy_and_transfer(run_helper=test_helper)
            test_results.append(result)
        else:
            LOG.warning(f"Unknown test module: {module_name}")
    except Exception as e:
        LOG.error(f"Test module {module_name} failed: {e}", exc_info=True)
        test_results.append({
            "test_name": module_name,
            "success": False,
            "error": str(e)
        })


async def main():
    """主执行流程"""
    parser = argparse.ArgumentParser(description="Gravity Node E2E Test Framework")
    parser.add_argument("--nodes-config", default="configs/nodes.json",
                       help="Path to nodes configuration file")
    parser.add_argument("--accounts-config", default="configs/test_accounts.json",
                       help="Path to accounts configuration file")
    parser.add_argument("--test-suite", default="all",
                       choices=["all", "basic", "contract", "erc20", "block", "network"],
                       help="Test suite to run")
    parser.add_argument("--cluster", default=None,
                       help="Cluster name to test (defined in nodes.json)")
    parser.add_argument("--node-id", default=None,
                       help="Comma-separated node IDs to test")
    parser.add_argument("--node-type", default=None,
                       choices=["validator", "vfn"],
                       help="Test all nodes of specific type")
    parser.add_argument("--log-level", default="INFO",
                       choices=["DEBUG", "INFO", "WARNING", "ERROR"],
                       help="Logging level")
    parser.add_argument("--log-file", default=None,
                       help="Path to log file")
    parser.add_argument("--output-dir", default="output",
                       help="Output directory for test results")
    
    args = parser.parse_args()
    
    # 设置日志
    setup_logging(args.log_level, args.log_file)
    
    # 创建输出目录
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    
    # 1. 初始化节点连接器
    try:
        async with NodeConnector(args.nodes_config) as node_connector:
            # 2. 确定要测试的节点
            test_nodes = []
            
            if args.cluster:
                # 使用预定义的集群
                try:
                    test_nodes = node_connector.get_cluster_nodes(args.cluster)
                    LOG.info(f"Testing cluster '{args.cluster}' with nodes: {test_nodes}")
                except Exception as e:
                    LOG.error(f"Failed to load cluster '{args.cluster}': {e}")
                    sys.exit(1)
            elif args.node_id:
                # 使用指定的节点ID
                test_nodes = [n.strip() for n in args.node_id.split(",")]
                LOG.info(f"Testing specified nodes: {test_nodes}")
            elif args.node_type:
                # 使用指定类型的所有节点
                test_nodes = node_connector.get_nodes_by_type(args.node_type)
                LOG.info(f"Testing all {args.node_type} nodes: {test_nodes}")
            else:
                # 默认测试所有节点
                test_nodes = node_connector.list_nodes()
                LOG.info(f"Testing all nodes: {test_nodes}")
            
            # 3. 连接到指定节点
            LOG.info("Connecting to nodes...")
            connection_results = await node_connector.connect_all(target_nodes=test_nodes)
            
            failed_connections = [node_id for node_id, success in connection_results.items() if not success]
            if failed_connections:
                LOG.error(f"Failed to connect to nodes: {failed_connections}")
                # 继续执行，只测试成功连接的节点
                
            # 4. 初始化账户管理器
            try:
                account_manager = TestAccountManager(args.accounts_config)
            except Exception as e:
                LOG.error(f"Failed to load account configuration: {e}")
                sys.exit(1)
            
            # 5. 执行健康检查
            LOG.info("Performing health check...")
            health_status = await node_connector.health_check()
            LOG.info("Node health status:")
            for node_id, status in health_status.items():
                if status["status"] == "healthy":
                    LOG.info(f"  {node_id}: OK (block {status['block_number']})")
                else:
                    LOG.error(f"  {node_id}: {status['status']} - {status.get('error', 'Unknown error')}")
            
            # 6. 执行测试用例
            test_results = []
            
            # 使用已成功连接的节点
            connected_nodes = list(node_connector.clients.keys())
            
            for node_id in connected_nodes:
                client = node_connector.get_client(node_id)
                
                # 初始化测试辅助器
                test_helper = RunHelper(
                    client=client,
                    working_dir=str(output_dir),
                    faucet_account=account_manager.get_faucet()
                )
                
                # 确定要运行的测试模块
                if args.test_suite == "all":
                    test_modules = [
                        "cases.basic_transfer",
                        "cases.contract",
                        "cases.erc20"
                    ]
                elif args.test_suite == "basic":
                    test_modules = ["cases.basic_transfer"]
                elif args.test_suite == "contract":
                    test_modules = ["cases.contract"]
                elif args.test_suite == "erc20":
                    test_modules = ["cases.erc20"]
                else:
                    test_modules = [f"cases.{args.test_suite}"]
                
                LOG.info(f"Running tests on node {node_id}: {test_modules}")
                
                # 执行测试
                for module in test_modules:
                    await run_test_module(module, test_helper, test_results)
                    
            # 7. 生成测试报告
            total_tests = len(test_results)
            passed_tests = sum(1 for r in test_results if (getattr(r, 'success', False) if isinstance(r, dict) else r.success))
            failed_tests = total_tests - passed_tests
            
            LOG.info("=" * 60)
            LOG.info("TEST RESULTS SUMMARY")
            LOG.info("=" * 60)
            LOG.info(f"Total tests: {total_tests}")
            LOG.info(f"Passed: {passed_tests}")
            LOG.info(f"Failed: {failed_tests}")
            LOG.info("=" * 60)
            
            if failed_tests > 0:
                LOG.error("Failed tests:")
                for result in test_results:
                    if isinstance(result, dict):
                        if not getattr(result, 'success', False):
                            error_msg = getattr(result, 'error', 'Unknown error')
                            LOG.error(f"  {getattr(result, 'test_name', 'Unknown test')}: {error_msg}")
                    else:
                        if not result.success:
                            LOG.error(f"  {result.test_name}: {result.error}")
            
            # 8. 保存测试结果
            results_file = output_dir / "test_results.json"
            try:
                with open(results_file, 'w') as f:
                    json.dump({
                        "timestamp": asyncio.get_event_loop().time(),
                        "total_tests": total_tests,
                        "passed": passed_tests,
                        "failed": failed_tests,
                        "results": test_results
                    }, f, indent=2)
                LOG.info(f"Test results saved to: {results_file}")
            except Exception as e:
                LOG.error(f"Failed to save test results: {e}")
            
            # 9. 保存生成的测试账户
            await account_manager._save_accounts_async()
            
            # 10. 返回退出码
            return 1 if failed_tests > 0 else 0
    
    except Exception as e:
        LOG.error(f"Failed to initialize node connector: {e}")
        sys.exit(1)


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))