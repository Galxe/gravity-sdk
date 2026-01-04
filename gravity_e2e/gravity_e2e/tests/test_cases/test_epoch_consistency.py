"""
Epoch consistency tests

Tests data consistency after epoch switching with different scenarios:
- Slow epoch switching (2-minute intervals, 3 epochs)
- Fast epoch switching (10-second intervals, 10 epochs)
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
from gravity_e2e.utils.epoch_utils import (
    EpochConfig,
    run_epoch_consistency_test,
    collect_epoch_data,
    validate_epoch_consistency,
)
from gravity_e2e.core.client.gravity_http_client import GravityHttpClient
from gravity_e2e.core.node_manager import NodeManager

LOG = logging.getLogger(__name__)


# Configuration for slow epoch switching scenario (default genesis)
SLOW_EPOCH_CONFIG = EpochConfig(
    num_epochs=3,
    check_interval=120,  # 2 minutes between checks
    epoch_timeout=600,   # 10 minutes timeout per epoch
    node_startup_delay=10
)

# Configuration for fast epoch switching scenario (fast genesis)
FAST_EPOCH_CONFIG = EpochConfig(
    num_epochs=10,
    check_interval=10,   # 10 seconds between checks
    epoch_timeout=600,   # 10 minutes timeout per epoch
    node_startup_delay=10
)


@test_case
async def test_epoch_consistency_slow(
    run_helper: RunHelper,
    test_result: TestResult
):
    """
    Test epoch switching data consistency with slow epoch intervals.

    Uses default genesis configuration with longer epoch duration.
    Monitors 3 epochs with 2-minute check intervals.

    Test steps:
    1. Deploy node1
    2. Start node1
    3. Wait for 3 epochs (epoch 1, 2, 3), checking every 2 minutes
    4. Record data for each epoch
    5. Validate for N = [1, 2]:
       - Epoch N ledger_info.block_number == Epoch N+1 round 1 block.block_number - 1
       - Epoch N+1 round 1 QC commit_info_block_id != Epoch N ledger_info.block_hash
    """
    LOG.info("=" * 70)
    LOG.info("Test: Epoch Consistency Test (Slow Epochs)")
    LOG.info("=" * 70)
    LOG.info(f"Configuration: {SLOW_EPOCH_CONFIG.num_epochs} epochs, "
             f"{SLOW_EPOCH_CONFIG.check_interval}s intervals")

    result = await run_epoch_consistency_test(
        config=SLOW_EPOCH_CONFIG,
        http_url="http://127.0.0.1:1024",
        node_name="node1",
        install_dir="/tmp",
        deploy_node=True
    )

    if not result.success:
        LOG.error(f"Test failed: {result.error}")
        test_result.mark_failure(result.error or "Unknown error")
        raise AssertionError(result.error)

    LOG.info("\nAll validations passed!")
    LOG.info("=" * 70)

    # Prepare success data
    success_data = {
        "validation_rounds": f"N={result.epochs_validated}",
        "scenario": "slow_epoch"
    }
    for epoch_num, epoch_data in result.epoch_data.items():
        success_data[f"epoch{epoch_num}_block_number"] = epoch_data.ledger_info["block_number"]

    test_result.mark_success(**success_data)


@test_case
async def test_epoch_consistency_fast(
    run_helper: RunHelper,
    test_result: TestResult
):
    """
    Test epoch switching data consistency with fast epoch intervals.

    Uses fast genesis configuration with shorter epoch duration.
    Monitors 10 epochs with 10-second check intervals.

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
    LOG.info("Test: Epoch Consistency Extended Test (Fast Epochs)")
    LOG.info("=" * 70)
    LOG.info(f"Configuration: {FAST_EPOCH_CONFIG.num_epochs} epochs, "
             f"{FAST_EPOCH_CONFIG.check_interval}s intervals")

    result = await run_epoch_consistency_test(
        config=FAST_EPOCH_CONFIG,
        http_url="http://127.0.0.1:1024",
        node_name="node1",
        install_dir="/tmp",
        deploy_node=True
    )

    if not result.success:
        LOG.error(f"Test failed: {result.error}")
        test_result.mark_failure(result.error or "Unknown error")
        raise AssertionError(result.error)

    LOG.info("\nAll validations passed!")
    LOG.info("=" * 70)

    # Prepare success data
    success_data = {
        "validation_rounds": f"N={result.epochs_validated}",
        "scenario": "fast_epoch"
    }
    for epoch_num, epoch_data in result.epoch_data.items():
        success_data[f"epoch{epoch_num}_block_number"] = epoch_data.ledger_info["block_number"]

    test_result.mark_success(**success_data)


# Pytest-compatible test functions
@pytest.mark.slow
@pytest.mark.epoch
@pytest.mark.self_managed
@pytest.mark.asyncio
async def test_epoch_consistency(run_helper: RunHelper, test_result: TestResult):
    """Pytest wrapper for slow epoch consistency test"""
    await test_epoch_consistency_slow(run_helper=run_helper, test_result=test_result)


@pytest.mark.slow
@pytest.mark.epoch
@pytest.mark.self_managed
@pytest.mark.asyncio
async def test_epoch_consistency_extended(run_helper: RunHelper, test_result: TestResult):
    """Pytest wrapper for fast epoch consistency test (extended)"""
    await test_epoch_consistency_fast(run_helper=run_helper, test_result=test_result)


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

    parser = argparse.ArgumentParser(description="Run epoch consistency tests")
    parser.add_argument(
        "--scenario",
        choices=["slow", "fast", "both"],
        default="slow",
        help="Test scenario: slow (3 epochs, 2min intervals), fast (10 epochs, 10s intervals), or both"
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

        if args.scenario in ("slow", "both"):
            LOG.info("\n" + "=" * 80)
            LOG.info("Running SLOW epoch scenario...")
            LOG.info("=" * 80)
            result = await test_epoch_consistency_slow(run_helper=run_helper)
            results.append(("slow", result))

        if args.scenario in ("fast", "both"):
            LOG.info("\n" + "=" * 80)
            LOG.info("Running FAST epoch scenario...")
            LOG.info("=" * 80)
            result = await test_epoch_consistency_fast(run_helper=run_helper)
            results.append(("fast", result))

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
