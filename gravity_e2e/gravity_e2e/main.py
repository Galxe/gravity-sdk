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
from .tests.test_cases.test_cross_chain_deposit import test_cross_chain_gravity_deposit
from .tests.test_cases.test_randomness_basic import (
    test_randomness_basic_consumption,
    test_randomness_correctness
)
from .tests.test_cases.test_randomness_advanced import (
    test_randomness_smoke,
    test_randomness_reconfiguration,
    test_randomness_multi_contract,
    test_randomness_api_completeness,
    test_randomness_stress
)
from .tests.test_cases.test_epoch_consistency import test_epoch_consistency
from .tests.test_cases.test_validator_add_remove import test_validator_add_remove

LOG = logging.getLogger(__name__)


async def run_test_module(module_name: str, test_helper: RunHelper, test_results: list):
    """Run test module"""
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
        elif module_name == "cases.cross_chain_deposit":
            result = await test_cross_chain_gravity_deposit(run_helper=test_helper)
        elif module_name == "cases.randomness_basic":
            result = await test_randomness_basic_consumption(run_helper=test_helper)
            test_results.append(result)
        elif module_name == "cases.randomness_correctness":
            result = await test_randomness_correctness(run_helper=test_helper)
            test_results.append(result)
        elif module_name == "cases.randomness_smoke":
            result = await test_randomness_smoke(run_helper=test_helper)
            test_results.append(result)
        elif module_name == "cases.randomness_reconfiguration":
            result = await test_randomness_reconfiguration(run_helper=test_helper)
            test_results.append(result)
        elif module_name == "cases.randomness_multi_contract":
            result = await test_randomness_multi_contract(run_helper=test_helper)
            test_results.append(result)
        elif module_name == "cases.randomness_api_completeness":
            result = await test_randomness_api_completeness(run_helper=test_helper)
            test_results.append(result)
        elif module_name == "cases.randomness_stress":
            result = await test_randomness_stress(run_helper=test_helper)
            test_results.append(result)
        elif module_name == "cases.epoch_consistency":
            result = await test_epoch_consistency(run_helper=test_helper)
            test_results.append(result)
        elif module_name == "cases.validator_add_remove":
            result = await test_validator_add_remove(run_helper=test_helper)
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
    """Main execution flow"""
    parser = argparse.ArgumentParser(description="Gravity Node E2E Test Framework")
    parser.add_argument("--nodes-config", default="configs/nodes.json",
                       help="Path to nodes configuration file")
    parser.add_argument("--accounts-config", default="configs/test_accounts.json",
                       help="Path to accounts configuration file")
    parser.add_argument("--test-suite", default="all",
                       choices=["all", "basic", "contract", "erc20", "cross_chain_deposit", "block", "network",
                               "randomness", "randomness_basic", "randomness_correctness",
                               "randomness_smoke", "randomness_reconfiguration",
                               "randomness_multi_contract", "randomness_api_completeness",
                               "randomness_stress", "epoch_consistency", "validator_add_remove"],
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
    
    # Setup logging
    setup_logging(args.log_level, args.log_file)
    
    # Create output directory
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    
    # 1. Initialize node connector
    try:
        async with NodeConnector(args.nodes_config) as node_connector:
            # 2. Determine nodes to test
            test_nodes = []
            
            if args.cluster:
                # Use predefined cluster
                try:
                    test_nodes = node_connector.get_cluster_nodes(args.cluster)
                    LOG.info(f"Testing cluster '{args.cluster}' with nodes: {test_nodes}")
                except Exception as e:
                    LOG.error(f"Failed to load cluster '{args.cluster}': {e}")
                    sys.exit(1)
            elif args.node_id:
                # Use specified node IDs
                test_nodes = [n.strip() for n in args.node_id.split(",")]
                LOG.info(f"Testing specified nodes: {test_nodes}")
            elif args.node_type:
                # Use all nodes of specified type
                test_nodes = node_connector.get_nodes_by_type(args.node_type)
                LOG.info(f"Testing all {args.node_type} nodes: {test_nodes}")
            else:
                # Default: test all nodes
                test_nodes = node_connector.list_nodes()
                LOG.info(f"Testing all nodes: {test_nodes}")
            
            # 3. Connect to specified nodes
            LOG.info("Connecting to nodes...")
            connection_results = await node_connector.connect_all(target_nodes=test_nodes)
            
            failed_connections = [node_id for node_id, success in connection_results.items() if not success]
            if failed_connections:
                LOG.error(f"Failed to connect to nodes: {failed_connections}")
                # Continue with successfully connected nodes
                
            # 4. Initialize account manager
            try:
                account_manager = TestAccountManager(args.accounts_config)
            except Exception as e:
                LOG.error(f"Failed to load account configuration: {e}")
                sys.exit(1)
            
            # 5. Perform health check
            LOG.info("Performing health check...")
            health_status = await node_connector.health_check()
            LOG.info("Node health status:")
            for node_id, status in health_status.items():
                if status["status"] == "healthy":
                    LOG.info(f"  {node_id}: OK (block {status['block_number']})")
                else:
                    LOG.error(f"  {node_id}: {status['status']} - {status.get('error', 'Unknown error')}")
            
            # 6. Execute test cases
            test_results = []
            
            # Check if test suite manages its own nodes (doesn't need pre-existing connections)
            self_managed_tests = ["epoch_consistency", "validator_add_remove"]
            is_self_managed = args.test_suite in self_managed_tests
            
            if is_self_managed:
                # For self-managed tests, create a dummy client and run test once
                LOG.info(f"Test suite '{args.test_suite}' manages its own nodes, skipping node connection requirement")
                dummy_client = node_connector.get_client(list(node_connector.clients.keys())[0]) if node_connector.clients else None
                if not dummy_client:
                    # Create a dummy client if no nodes are connected
                    from .core.client.gravity_client import GravityClient
                    dummy_client = GravityClient("http://127.0.0.1:8545", "dummy")
                
                test_helper = RunHelper(
                    client=dummy_client,
                    working_dir=str(output_dir),
                    faucet_account=account_manager.get_faucet() if account_manager else None
                )
                
                # Determine test modules to run
                if args.test_suite == "epoch_consistency":
                    test_modules = ["cases.epoch_consistency"]
                elif args.test_suite == "validator_add_remove":
                    test_modules = ["cases.validator_add_remove"]
                else:
                    test_modules = []
                
                # Execute tests
                for module in test_modules:
                    await run_test_module(module, test_helper, test_results)
            else:
                # Use successfully connected nodes
                connected_nodes = list(node_connector.clients.keys())
                
                if not connected_nodes:
                    LOG.error("No nodes connected. Please ensure nodes are running and accessible.")
                    sys.exit(1)
                
                for node_id in connected_nodes:
                    client = node_connector.get_client(node_id)
                    
                    # Initialize test helper
                    test_helper = RunHelper(
                        client=client,
                        working_dir=str(output_dir),
                        faucet_account=account_manager.get_faucet()
                    )
                    
                    # Determine test modules to run
                    if args.test_suite == "all":
                        test_modules = [
                            "cases.basic_transfer",
                            "cases.contract",
                            "cases.erc20",
                            "cases.cross_chain_deposit"
                        ]
                    elif args.test_suite == "basic":
                        test_modules = ["cases.basic_transfer"]
                    elif args.test_suite == "contract":
                        test_modules = ["cases.contract"]
                    elif args.test_suite == "erc20":
                        test_modules = ["cases.erc20"]
                    elif args.test_suite == "cross_chain_deposit":
                        test_modules = ["cases.cross_chain_deposit"]
                    elif args.test_suite == "randomness":
                        test_modules = [
                            "cases.randomness_smoke",
                            "cases.randomness_basic",
                            "cases.randomness_correctness",
                            "cases.randomness_reconfiguration",
                            "cases.randomness_multi_contract",
                            "cases.randomness_api_completeness"
                        ]
                    elif args.test_suite == "randomness_basic":
                        test_modules = ["cases.randomness_basic"]
                    elif args.test_suite == "randomness_correctness":
                        test_modules = ["cases.randomness_correctness"]
                    elif args.test_suite == "randomness_smoke":
                        test_modules = ["cases.randomness_smoke"]
                    elif args.test_suite == "randomness_reconfiguration":
                        test_modules = ["cases.randomness_reconfiguration"]
                    elif args.test_suite == "randomness_multi_contract":
                        test_modules = ["cases.randomness_multi_contract"]
                    elif args.test_suite == "randomness_api_completeness":
                        test_modules = ["cases.randomness_api_completeness"]
                    elif args.test_suite == "randomness_stress":
                        test_modules = ["cases.randomness_stress"]
                    elif args.test_suite == "epoch_consistency":
                        test_modules = ["cases.epoch_consistency"]
                    elif args.test_suite == "validator_add_remove":
                        test_modules = ["cases.validator_add_remove"]
                    else:
                        test_modules = [f"cases.{args.test_suite}"]
                    
                    LOG.info(f"Running tests on node {node_id}: {test_modules}")
                    
                    # Execute tests
                    for module in test_modules:
                        await run_test_module(module, test_helper, test_results)
                    
            # 7. Generate test report
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
            
            # 8. Save test results
            results_file = output_dir / "test_results.json"
            try:
                # Convert TestResult objects to dictionaries
                serializable_results = []
                for result in test_results:
                    if hasattr(result, 'to_dict'):
                        serializable_results.append(result.to_dict())
                    elif isinstance(result, dict):
                        serializable_results.append(result)
                    else:
                        # Fallback: try to extract basic info
                        serializable_results.append({
                            "test_name": getattr(result, 'test_name', 'unknown'),
                            "success": getattr(result, 'success', False),
                            "error": getattr(result, 'error', None)
                        })
                
                with open(results_file, 'w') as f:
                    json.dump({
                        "timestamp": asyncio.get_event_loop().time(),
                        "total_tests": total_tests,
                        "passed": passed_tests,
                        "failed": failed_tests,
                        "results": serializable_results
                    }, f, indent=2)
                LOG.info(f"Test results saved to: {results_file}")
            except Exception as e:
                LOG.error(f"Failed to save test results: {e}")
            
            # 9. Save generated test accounts
            await account_manager._save_accounts_async()
            
            # 10. Return exit code
            return 1 if failed_tests > 0 else 0
    
    except Exception as e:
        LOG.error(f"Failed to initialize node connector: {e}")
        sys.exit(1)


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))