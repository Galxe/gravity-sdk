"""
Validator add and remove tests

Tests adding and removing validators from the validator set with two scenarios:
- Immediate node start: Start node3 immediately after validator join
- Delayed node start: Wait before starting node3 after validator join
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
import pytest

from gravity_e2e.helpers.test_helpers import RunHelper, TestResult, test_case
from gravity_e2e.utils.validator_utils import (
    ValidatorTestConfig,
    ValidatorJoinParams,
    ValidatorTestResult,
    DEFAULT_VALIDATOR_PARAMS,
    run_validator_add_remove_test,
)

LOG = logging.getLogger(__name__)


# Test configurations
IMMEDIATE_START_CONFIG = ValidatorTestConfig(
    node1_name="node1",
    node3_name="node3",
    install_dir="/tmp",
    http_url_node1="http://127.0.0.1:1024",
    http_url_node3="http://127.0.0.1:1026",
    rpc_url="http://127.0.0.1:8545",
    bin_version="quick-release",
    node_startup_delay=10,
    validator_change_wait=120,  # 2 minutes
)

DELAYED_START_CONFIG = ValidatorTestConfig(
    node1_name="node1",
    node3_name="node3",
    install_dir="/tmp",
    http_url_node1="http://127.0.0.1:1024",
    http_url_node3="http://127.0.0.1:1026",
    rpc_url="http://127.0.0.1:8545",
    bin_version="quick-release",
    node_startup_delay=10,
    validator_change_wait=120,  # 2 minutes
)


@test_case
async def test_validator_add_remove_immediate(
    run_helper: RunHelper,
    test_result: TestResult
):
    """
    Test adding and removing validators with immediate node3 startup.

    Test steps:
    1. Deploy node1 and node3
    2. Start node1
    3. Wait and verify validator count == 1
    4. Add validator (node3) using gravity_cli
    5. Start node3 immediately
    6. Wait 2 minutes and verify validator count == 2
    7. Remove validator (node3) using gravity_cli
    8. Wait 2 minutes and verify validator count == 1
    9. Stop nodes
    """
    LOG.info("=" * 70)
    LOG.info("Test: Validator Add and Remove (Immediate Start)")
    LOG.info("=" * 70)

    result = await run_validator_add_remove_test(
        config=IMMEDIATE_START_CONFIG,
        validator_params=DEFAULT_VALIDATOR_PARAMS,
        delayed_node3_start=False,
        verification_http_url_after_add=IMMEDIATE_START_CONFIG.http_url_node1
    )

    if not result.success:
        LOG.error(f"Test failed: {result.error}")
        test_result.mark_failure(result.error or "Unknown error")
        raise AssertionError(result.error)

    LOG.info("\nAll validations passed!")
    LOG.info("=" * 70)

    test_result.mark_success(
        initial_validator_count=result.initial_validator_count,
        after_add_validator_count=result.after_add_validator_count,
        after_remove_validator_count=result.after_remove_validator_count,
        scenario="immediate_start"
    )


@test_case
async def test_validator_add_remove_delayed(
    run_helper: RunHelper,
    test_result: TestResult
):
    """
    Test adding and removing validators with delayed node3 startup.

    Test steps:
    1. Deploy node1 and node3
    2. Start node1
    3. Wait and verify validator count == 1
    4. Add validator (node3) using gravity_cli
    5. Wait 2 minutes before starting node3
    6. Start node3
    7. Wait 2 minutes after starting node3
    8. Verify validator count == 2 (using node3's HTTP endpoint)
    9. Remove validator (node3) using gravity_cli
    10. Wait 2 minutes and verify validator count == 1
    11. Stop nodes
    """
    LOG.info("=" * 70)
    LOG.info("Test: Validator Add and Remove (Delayed Start)")
    LOG.info("=" * 70)

    result = await run_validator_add_remove_test(
        config=DELAYED_START_CONFIG,
        validator_params=DEFAULT_VALIDATOR_PARAMS,
        delayed_node3_start=True,
        pre_node3_start_delay=120,  # Wait 2 minutes before starting node3
        post_node3_start_delay=120,  # Wait 2 minutes after starting node3
        verification_http_url_after_add=DELAYED_START_CONFIG.http_url_node3  # Use node3's endpoint
    )

    if not result.success:
        LOG.error(f"Test failed: {result.error}")
        test_result.mark_failure(result.error or "Unknown error")
        raise AssertionError(result.error)

    LOG.info("\nAll validations passed!")
    LOG.info("=" * 70)

    test_result.mark_success(
        initial_validator_count=result.initial_validator_count,
        after_add_validator_count=result.after_add_validator_count,
        after_remove_validator_count=result.after_remove_validator_count,
        scenario="delayed_start",
        delayed_startup=True
    )


# Pytest-compatible test functions (aliases for registry compatibility)
@pytest.mark.slow
@pytest.mark.validator
@pytest.mark.self_managed
@test_case
async def test_validator_add_remove(run_helper: RunHelper, test_result: TestResult):
    """Pytest wrapper for immediate validator add/remove test"""
    await test_validator_add_remove_immediate(run_helper=run_helper, test_result=test_result)


# Allow direct execution
if __name__ == "__main__":
    import argparse

    # Add project path
    sys.path.insert(0, str(Path(__file__).parent.parent.parent.parent))

    from gravity_e2e.helpers.test_helpers import RunHelper
    from gravity_e2e.core.client.gravity_client import GravityClient

    # Setup logging
    logging.basicConfig(
        level=logging.INFO,
        format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
    )

    parser = argparse.ArgumentParser(description="Run validator add/remove tests")
    parser.add_argument(
        "--scenario",
        choices=["immediate", "delayed", "both"],
        default="immediate",
        help="Test scenario: immediate (start node3 immediately), delayed (wait before starting node3), or both"
    )
    args = parser.parse_args()

    async def run_direct():
        """Run tests directly"""
        dummy_client = GravityClient("http://127.0.0.1:8545", "dummy_node")
        run_helper = RunHelper(
            client=dummy_client,
            working_dir=str(Path(__file__).parent.parent.parent.parent),
            faucet_account=None
        )

        results = []

        if args.scenario in ("immediate", "both"):
            LOG.info("\n" + "=" * 80)
            LOG.info("Running IMMEDIATE start scenario...")
            LOG.info("=" * 80)
            result = await test_validator_add_remove_immediate(run_helper=run_helper)
            results.append(("immediate", result))

        if args.scenario in ("delayed", "both"):
            LOG.info("\n" + "=" * 80)
            LOG.info("Running DELAYED start scenario...")
            LOG.info("=" * 80)
            result = await test_validator_add_remove_delayed(run_helper=run_helper)
            results.append(("delayed", result))

        # Summary
        LOG.info("\n" + "=" * 80)
        LOG.info("TEST SUMMARY")
        LOG.info("=" * 80)
        all_passed = True
        for scenario, result in results:
            status = "PASSED" if result.success else "FAILED"
            LOG.info(f"  {scenario.upper()}: {status}")
            if not result.success:
                all_passed = False

        sys.exit(0 if all_passed else 1)

    asyncio.run(run_direct())
