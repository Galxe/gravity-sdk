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


@pytest.mark.skip(
    reason="Genesis baseline is gravity-testnet-v1.4.0 which already ships the "
    "post-Gamma bytecode set, so the hardfork is a no-op on this cluster — "
    "Phase 4's bytecode-diff assertion has nothing to observe. Coverage for the "
    "Zeta-on-v1.5 transition lives in test_zeta.py + test_full_lifecycle.py."
)
@pytest.mark.asyncio
async def test_gamma(cluster: Cluster):
    config = HardforkTestConfig(
        name="gamma",
        display_name="Gamma Hardfork",
        hardfork_block=GAMMA_BLOCK,
        contracts=get_contracts_for_hardfork("gamma"),
    )
    await run_hardfork_lifecycle_test(cluster, config)
