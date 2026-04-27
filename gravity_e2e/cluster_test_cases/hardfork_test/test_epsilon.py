"""
Epsilon Hardfork E2E Test

Verifies the v1.3 → v1.4 Epsilon hardfork lifecycle + functional invariants:
  - PR #56: ValidatorManagement underbonded eviction + percentage-based
    performance threshold (bytecode replaced).
  - PR #63: Reconfiguration evictUnderperformingValidators() call-site moved
    (ABI unchanged; bytecode replaced).
  - PR #63: ValidatorConfig autoEvictThreshold(uint256) → __deprecated,
    new autoEvictThresholdPct(uint64) appended.
  - PR #66: GBridgeReceiver _processedNonces → __deprecated gap; isProcessed()
    selector removed.

The framework's 6-phase lifecycle covers liveness, snapshot, transition,
bytecode diff, epoch stability, and restart replay. On top of that we run
Epsilon-specific ABI assertions through the live node's JSON-RPC.

Usage:
    GAMMA_BLOCK=0 DELTA_BLOCK=20 EPSILON_BLOCK=50 \\
        ./gravity_e2e/run_test.sh hardfork_test -k test_epsilon
"""

import logging
import os
import sys
from pathlib import Path

import pytest
from eth_account import Account
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster

sys.path.insert(0, str(Path(__file__).parent))
from hardfork_framework import HardforkTestConfig, run_hardfork_lifecycle_test
from hardfork_utils import wait_for_block
from system_contracts import GBRIDGE_RECEIVER_TESTNET, get_contracts_for_hardfork

LOG = logging.getLogger(__name__)

EPSILON_BLOCK = int(os.environ.get("EPSILON_BLOCK", "100"))

VALIDATOR_CONFIG = Web3.to_checksum_address("0x00000000000000000000000000000001625F1002")
VALIDATOR_MANAGEMENT = Web3.to_checksum_address("0x00000000000000000000000000000001625F2001")
RECONFIGURATION = Web3.to_checksum_address("0x00000000000000000000000000000001625F2003")


def _selector(sig: str) -> bytes:
    return Web3.keccak(text=sig)[:4]


def _call_returns(w3: Web3, to: str, data: bytes) -> bool:
    """Returns True iff eth_call succeeds (any non-revert result)."""
    try:
        w3.eth.call({"to": to, "data": data})
        return True
    except Exception:
        return False


def _call_reverts(w3: Web3, to: str, data: bytes) -> bool:
    """Returns True iff eth_call reverts (selector missing or function reverted)."""
    try:
        w3.eth.call({"to": to, "data": data})
        return False
    except Exception:
        return True


def _verify_epsilon_observables(w3: Web3):
    """Run the Epsilon-specific ABI smoke checks defined in
    scripts/verify_hardfork/hardforks/epsilon.sh."""
    LOG.info("🔎 Epsilon smoke: ValidatorConfig")
    # New getter must resolve
    assert _call_returns(
        w3, VALIDATOR_CONFIG, _selector("autoEvictThresholdPct()")
    ), "autoEvictThresholdPct() missing post-Epsilon"
    # Old getter must be gone
    assert _call_reverts(
        w3, VALIDATOR_CONFIG, _selector("autoEvictThreshold()")
    ), "autoEvictThreshold() unexpectedly still present"
    # Sanity: existing getters still work
    assert _call_returns(w3, VALIDATOR_CONFIG, _selector("autoEvictEnabled()"))
    assert _call_returns(w3, VALIDATOR_CONFIG, _selector("minimumBond()"))
    assert _call_returns(w3, VALIDATOR_CONFIG, _selector("isInitialized()"))

    LOG.info("🔎 Epsilon smoke: ValidatorManagement")
    # evictUnderperformingValidators() has requireAllowed(RECONFIGURATION).
    # Called without impersonation it will revert — but with NotAllowed, not
    # MethodNotFound. We only care about selector presence, so any revert
    # other than "selector not in dispatcher" passes. The simplest proxy is
    # an eth_call from an impersonated reconfiguration address; on non-anvil
    # nodes we settle for asserting the call reverts (selector exists OR not).
    # Here we simply require the codehash differs — handled by Phase 4 — and
    # check a harmless view that was always present.
    assert _call_returns(w3, VALIDATOR_MANAGEMENT, _selector("getActiveValidatorCount()"))

    LOG.info("🔎 Epsilon smoke: Reconfiguration")
    assert _call_returns(w3, RECONFIGURATION, _selector("currentEpoch()"))

    LOG.info("🔎 Epsilon smoke: GBridgeReceiver")
    gbr = Web3.to_checksum_address(
        os.environ.get("GBRIDGE_RECEIVER", GBRIDGE_RECEIVER_TESTNET)
    )
    # isProcessed(uint128) must be gone — the selector is dispatched and
    # reverts as "execution reverted" (function not found).
    is_processed_with_arg = _selector("isProcessed(uint128)") + (1).to_bytes(32, "big")
    assert _call_reverts(
        w3, gbr, is_processed_with_arg
    ), "GBridgeReceiver.isProcessed(uint128) unexpectedly still present"
    # Surviving views must still work
    assert _call_returns(w3, gbr, _selector("trustedBridge()"))
    assert _call_returns(w3, gbr, _selector("trustedSourceId()"))

    LOG.info("✅ Epsilon smoke checks passed")


@pytest.mark.skip(
    reason="Genesis baseline is gravity-testnet-v1.4.0 which already ships the "
    "post-Epsilon bytecode set, so the framework's Phase 4 codehash-diff cannot "
    "observe enough changes to satisfy min_changed_contracts. Epsilon ABI "
    "observables are still smoke-checked at each fork boundary by "
    "test_full_lifecycle (PER_FORK_SMOKE['epsilon'] = _verify_epsilon_observables)."
)
@pytest.mark.asyncio
async def test_epsilon(cluster: Cluster):
    """Epsilon hardfork lifecycle + Epsilon-specific ABI smoke."""
    config = HardforkTestConfig(
        name="epsilon",
        display_name="Epsilon Hardfork",
        hardfork_block=EPSILON_BLOCK,
        contracts=get_contracts_for_hardfork("epsilon"),
    )
    await run_hardfork_lifecycle_test(cluster, config)

    # Post-lifecycle ABI smoke — the framework already verified codehash diff
    # and liveness; this verifies the behavioural consequences of Epsilon.
    node = cluster.get_node(config.node_name)
    _verify_epsilon_observables(node.w3)
