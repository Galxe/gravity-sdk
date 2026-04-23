"""
Gamma Hardfork E2E Test (Framework-based)

Uses the generic hardfork_framework to run the full 6-phase lifecycle test
for the gamma hardfork. To add a new hardfork test, copy this file and
update the HardforkTestConfig.

Usage:
    # Run via E2E runner:
    ./gravity_e2e/run_test.sh hardfork_test -k test_gamma

    # With custom gamma block:
    GAMMA_BLOCK=30 ./gravity_e2e/run_test.sh hardfork_test -k test_gamma
"""

import logging
import os
import sys
from pathlib import Path

import pytest

from gravity_e2e.cluster.manager import Cluster

sys.path.insert(0, str(Path(__file__).parent))
from hardfork_framework import HardforkTestConfig, run_hardfork_lifecycle_test
from system_contracts import get_contracts_for_hardfork

LOG = logging.getLogger(__name__)

GAMMA_BLOCK = int(os.environ.get("GAMMA_BLOCK", "500"))


@pytest.mark.asyncio
async def test_gamma(cluster: Cluster):
    """
    Gamma hardfork lifecycle test using the generic framework.

    This test verifies the full hardfork lifecycle:
    1. Pre-hardfork chain liveness
    2. Pre-hardfork contract snapshot
    3. Hardfork transition
    4. Post-hardfork bytecode verification
    5. Epoch change stability
    6. Node restart & replay
    """
    config = HardforkTestConfig(
        name="gamma",
        display_name="Gamma Hardfork",
        hardfork_block=GAMMA_BLOCK,
        contracts=get_contracts_for_hardfork("gamma"),
    )
    await run_hardfork_lifecycle_test(cluster, config)
