#!/usr/bin/env python3
"""
Gravity E2E Test Framework - Main Entry Point

This module provides the CLI interface for running E2E tests against
the Gravity blockchain network.
"""
import argparse
import asyncio
import json
import logging
import sys
from pathlib import Path

from .core.node_connector import NodeConnector
from .helpers.account_manager import TestAccountManager
from .helpers.test_helpers import RunHelper
from .utils.logging import setup_logging
from .cluster.manager import Cluster # New import

# Import test cases to trigger registration
from .tests import test_cases  # noqa: F401
from .tests.test_registry import (
    get_test,
    get_tests_to_run,
    get_available_choices,
    is_self_managed,
    list_suites,
    list_tests,
)
from .tests.test_cases import DEFAULT_TESTS

LOG = logging.getLogger(__name__)


async def run_test(test_name: str, test_helper: RunHelper, cluster: Cluster = None) -> dict:
    """
    Run a single test by name.
    
    Args:
        test_name: The registered test name
        test_helper: RunHelper instance for the test
        cluster: Optional Cluster instance for cluster_ops tests
    
    Returns:
        Test result dictionary or TestResult object
    """
    test_func = get_test(test_name)
    if not test_func:
        LOG.error(f"Test '{test_name}' not found in registry")
        return {
            "test_name": test_name,
            "success": False,
            "error": f"Test '{test_name}' not found"
        }
    
    try:
        LOG.info(f"Running test: {test_name}")
        
        # Dependency Injection based on arguments
        # Simple check: if test_func accepts 'cluster', pass it
        import inspect
        sig = inspect.signature(test_func)
        kwargs = {"run_helper": test_helper}
        
        if "cluster" in sig.parameters:
            if cluster is None:
                raise ValueError(f"Test '{test_name}' requires 'cluster' fixture but no --cluster-config provided")
            kwargs["cluster"] = cluster
            
        result = await test_func(**kwargs)
        return result
    except Exception as e:
        LOG.error(f"Test {test_name} failed: {e}", exc_info=True)
        return {
            "test_name": test_name,
            "success": False,
            "error": str(e)
        }


async def main():
    """Main execution flow"""
    # Build available choices dynamically
    available_choices = ["all"] + list(set(list_suites() + list_tests()))
    
    parser = argparse.ArgumentParser(description="Gravity Node E2E Test Framework")
    parser.add_argument("--nodes-config", default="configs/nodes.json",
                       help="Path to nodes configuration file")
    parser.add_argument("--accounts-config", default="configs/test_accounts.json",
                       help="Path to accounts configuration file")
    parser.add_argument("--cluster-config", default=None,
                       help="Path to cluster.toml configuration for Cluster Ops tests")
    parser.add_argument("--test-suite", default="all",
                       help=f"Test suite to run. Available: {', '.join(available_choices[:10])}...")
    parser.add_argument("--list-tests", action="store_true",
                       help="List all available tests and suites")
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
    
    # Handle --list-tests
    if args.list_tests:
        print("\nAvailable Test Suites:")
        for suite in sorted(list_suites()):
            print(f"  - {suite}")
        print("\nAvailable Individual Tests:")
        for test in sorted(list_tests()):
            managed = " (self-managed)" if is_self_managed(test) else ""
            print(f"  - {test}{managed}")
        return 0
    
    # Create output directory
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    
    # Get tests to run
    tests_to_run = get_tests_to_run(args.test_suite)
    
    # Handle special case for "all" - use default tests
    if args.test_suite == "all":
        tests_to_run = DEFAULT_TESTS
    
    if not tests_to_run:
        LOG.error(f"No tests found for suite '{args.test_suite}'")
        LOG.info(f"Available suites: {list_suites()}")
        LOG.info(f"Available tests: {list_tests()}")
        return 1
    
    LOG.info(f"Tests to run: {tests_to_run}")
    
    # Check if any tests are self-managed
    has_self_managed = any(is_self_managed(t) for t in tests_to_run)
    all_self_managed = all(is_self_managed(t) for t in tests_to_run)
    
    test_results = []
    
    try:
        async with NodeConnector(args.nodes_config) as node_connector:
            # Determine nodes to test
            test_nodes = []
            
            if not all_self_managed:
                if args.cluster:
                    try:
                        test_nodes = node_connector.get_cluster_nodes(args.cluster)
                        LOG.info(f"Testing cluster '{args.cluster}' with nodes: {test_nodes}")
                    except Exception as e:
                        LOG.error(f"Failed to load cluster '{args.cluster}': {e}")
                        sys.exit(1)
                elif args.node_id:
                    test_nodes = [n.strip() for n in args.node_id.split(",")]
                    LOG.info(f"Testing specified nodes: {test_nodes}")
                elif args.node_type:
                    test_nodes = node_connector.get_nodes_by_type(args.node_type)
                    LOG.info(f"Testing all {args.node_type} nodes: {test_nodes}")
                else:
                    test_nodes = node_connector.list_nodes()
                    LOG.info(f"Testing all nodes: {test_nodes}")
                
                # Connect to specified nodes
                LOG.info("Connecting to nodes...")
                connection_results = await node_connector.connect_all(target_nodes=test_nodes)
                
                failed_connections = [node_id for node_id, success in connection_results.items() if not success]
                if failed_connections:
                    LOG.error(f"Failed to connect to nodes: {failed_connections}")
            
            # Initialize account manager
            try:
                account_manager = TestAccountManager(args.accounts_config)
            except Exception as e:
                LOG.error(f"Failed to load account configuration: {e}")
                sys.exit(1)
            
            # Perform health check if we have connected nodes
            if not all_self_managed:
                LOG.info("Performing health check...")
                health_status = await node_connector.health_check()
                LOG.info("Node health status:")
                for node_id, status in health_status.items():
                    if status["status"] == "healthy":
                        LOG.info(f"  {node_id}: OK (block {status['block_number']})")
                    else:
                        LOG.error(f"  {node_id}: {status['status']} - {status.get('error', 'Unknown error')}")
            
            # Initialize Cluster if config provided
            cluster_manager = None
            if args.cluster_config:
                try:
                    cluster_manager = Cluster(Path(args.cluster_config))
                    LOG.info(f"Initialized Cluster Manager with config: {args.cluster_config}")
                except Exception as e:
                    LOG.error(f"Failed to initialize Cluster: {e}")
                    sys.exit(1)

            # Run self-managed tests first (they don't need pre-connected nodes)
            self_managed_tests = [t for t in tests_to_run if is_self_managed(t)]
            regular_tests = [t for t in tests_to_run if not is_self_managed(t)]
            
            # Run self-managed tests
            if self_managed_tests:
                LOG.info(f"Running {len(self_managed_tests)} self-managed tests...")
                
                # Create a dummy client for self-managed tests
                from .core.client.gravity_client import GravityClient
                dummy_client = GravityClient("http://127.0.0.1:8545", "dummy")
                
                test_helper = RunHelper(
                    client=dummy_client,
                    working_dir=str(output_dir),
                    faucet_account=account_manager.get_faucet()
                )
                
                for test_name in self_managed_tests:
                    result = await run_test(test_name, test_helper, cluster=cluster_manager)
                    test_results.append(result)
            
            # Run regular tests on connected nodes
            if regular_tests:
                connected_nodes = list(node_connector.clients.keys())
                
                if not connected_nodes:
                    LOG.error("No nodes connected. Please ensure nodes are running and accessible.")
                    sys.exit(1)
                
                for node_id in connected_nodes:
                    client = node_connector.get_client(node_id)
                    
                    test_helper = RunHelper(
                        client=client,
                        working_dir=str(output_dir),
                        faucet_account=account_manager.get_faucet()
                    )
                    
                    LOG.info(f"Running {len(regular_tests)} tests on node {node_id}")
                    
                    for test_name in regular_tests:
                        # Cluster manager passed here too, though regular tests might not use it
                        result = await run_test(test_name, test_helper, cluster=cluster_manager)
                        test_results.append(result)
            
            # Generate test report
            total_tests = len(test_results)
            passed_tests = sum(1 for r in test_results if _is_success(r))
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
                    if not _is_success(result):
                        name = _get_test_name(result)
                        error = _get_error(result)
                        LOG.error(f"  {name}: {error}")
            
            # Save test results
            results_file = output_dir / "test_results.json"
            try:
                serializable_results = [_to_dict(r) for r in test_results]
                
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
            
            # Save generated test accounts
            await account_manager._save_accounts_async()
            
            return 1 if failed_tests > 0 else 0
    
    except Exception as e:
        LOG.error(f"Failed to initialize node connector: {e}")
        sys.exit(1)


def _is_success(result) -> bool:
    """Check if a test result indicates success."""
    if isinstance(result, dict):
        return result.get('success', False)
    return getattr(result, 'success', False)


def _get_test_name(result) -> str:
    """Get test name from result."""
    if isinstance(result, dict):
        return result.get('test_name', 'Unknown')
    return getattr(result, 'test_name', 'Unknown')


def _get_error(result) -> str:
    """Get error message from result."""
    if isinstance(result, dict):
        return result.get('error', 'Unknown error')
    return getattr(result, 'error', 'Unknown error')


def _to_dict(result) -> dict:
    """Convert result to dictionary."""
    if hasattr(result, 'to_dict'):
        return result.to_dict()
    elif isinstance(result, dict):
        return result
    else:
        return {
            "test_name": getattr(result, 'test_name', 'unknown'),
            "success": getattr(result, 'success', False),
            "error": getattr(result, 'error', None)
        }


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
