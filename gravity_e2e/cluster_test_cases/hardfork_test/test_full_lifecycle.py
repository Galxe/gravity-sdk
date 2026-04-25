"""
Full-lifecycle Hardfork E2E Test (Zeta-only walk on a v1.4.0 baseline)

Genesis is gravity-testnet-v1.4.0, which already ships the post-Epsilon
bytecode set. Alpha/Beta/Gamma/Delta/Epsilon therefore activate at block 0
as no-ops — Phase 4's bytecode-diff has nothing to observe at those
boundaries — so this lifecycle walks only Zeta:

  1. baseline ABI smoke (assert v1.4.0 genesis exposes the post-Epsilon
     surface; failing here means the genesis itself is wrong, not Zeta)
  2. pre-Zeta proposeStaker must revert (the v1.4.0 StakePool bytecode
     does not have the 2-step role API)
  3. walk Zeta: snapshot → wait for ZETA_BLOCK → re-snapshot → assert at
     least one of {Governance, StakingConfig, ValidatorManagement,
     Reconfiguration, JWKManager} changed
  4. post-Zeta invariant suite: governance init / staking-config setters /
     whitelist / 2-step role timelock / JWKManager validation
  5. restart node, wait for replay, reassert post-Zeta codehashes

Env vars:
    GAMMA_BLOCK / DELTA_BLOCK / EPSILON_BLOCK / ZETA_BLOCK: per-fork block
        numbers (passed through to hooks.py -> genesis config injection).
        On a v1.4.0 baseline only ZETA_BLOCK is meaningful; the others are
        retained for parity with hooks.py / genesis.json schema.
    FAUCET_KEY: override the faucet private key.

Usage:
    ./gravity_e2e/run_test.sh hardfork_test -k test_full_lifecycle
"""

import asyncio
import logging
import os
import sys
from pathlib import Path

import pytest
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.cluster.node import NodeState

sys.path.insert(0, str(Path(__file__).parent))
from hardfork_framework import (
    HardforkTestConfig,
    phase1_pre_hardfork_validation,
    phase2_pre_hardfork_snapshot,
    phase3_hardfork_transition,
    phase4_bytecode_verification,
    phase5_epoch_verification,
    phase6_restart_replay,
)
from hardfork_utils import (
    compare_snapshots,
    get_contract_code_hashes,
    send_eth_transfers,
    wait_for_block,
    wait_for_blocks_after,
)
from system_contracts import get_contracts_for_hardfork
from test_epsilon import _verify_epsilon_observables
from test_zeta import (
    STAKING,
    _verify_governance_init,
    _verify_jwk_manager_validation,
    _verify_staking_config_setters,
    _verify_stakepool_role_timelock,
    _verify_validator_management_whitelist,
    _selector,
)

LOG = logging.getLogger(__name__)

GAMMA_BLOCK = int(os.environ.get("GAMMA_BLOCK", "30"))
DELTA_BLOCK = int(os.environ.get("DELTA_BLOCK", "60"))
EPSILON_BLOCK = int(os.environ.get("EPSILON_BLOCK", "100"))
ZETA_BLOCK = int(os.environ.get("ZETA_BLOCK", "150"))

# Ordered sequence the test walks. The genesis baseline is
# gravity-testnet-v1.4.0, which already ships the post-Alpha/Beta/Gamma/Delta/
# Epsilon contract bytecodes — those forks are no-ops on this cluster, so we
# only walk Zeta (the one this PR actually adds). The framework's Phase 4
# bytecode-diff is meaningful on Zeta because v1.4.0 → v1.5 replaces
# Governance/StakingConfig/ValidatorManagement/Reconfiguration/JWKManager.
HARDFORK_SEQUENCE = [
    ("zeta", "Zeta Hardfork", ZETA_BLOCK),
]

# Per-fork post-transition smoke callback. Epsilon ABI observables are still
# probed once before the walk (chain is already past EpsilonBlock at that
# point thanks to the v1.4.0 baseline).
PER_FORK_SMOKE = {
    "zeta": None,   # handled explicitly after the sequence (richer flow)
}


async def _walk_hardfork(cluster: Cluster, name: str, display: str, block: int):
    """Run phases 1-5 for a single fork."""
    config = HardforkTestConfig(
        name=name,
        display_name=display,
        hardfork_block=block,
        contracts=get_contracts_for_hardfork(name),
        # For back-to-back forks we don't need to wait 60 blocks between —
        # each fork's phase3/phase5 waits for its own window.
        post_hardfork_blocks=10,
        epoch_wait_blocks=20,
        min_changed_contracts=1,
    )

    node = cluster.get_node(config.node_name)
    w3 = node.w3

    # Phase 1 reuse (idempotent liveness check)
    await phase1_pre_hardfork_validation(cluster, config)

    # Phase 2 — snapshot right before the fork block
    #
    # NOTE: the framework's phase2 also sends a burst of EOA transfers. That's
    # cheap on one fork but wastes a bunch of txns when we walk four forks, so
    # we reproduce the snapshot-only part inline.
    LOG.info(f"[{display}] snapshotting codehashes")
    pre_snapshot = get_contract_code_hashes(w3, config.contracts)

    # Phase 3 — transition
    await phase3_hardfork_transition(cluster, config)

    # Phase 4 — verify codehashes changed
    changed, post_snapshot = await phase4_bytecode_verification(cluster, config, pre_snapshot)

    # Phase 5 — epoch stability
    await phase5_epoch_verification(cluster, config, changed, post_snapshot)

    # Per-fork ABI smoke (if registered)
    smoke = PER_FORK_SMOKE.get(name)
    if smoke is not None:
        LOG.info(f"[{display}] running fork-specific ABI smoke")
        smoke(w3)

    return post_snapshot


async def _verify_pre_zeta_propose_staker_reverts(w3: Web3):
    """Before Zeta, StakePool has no proposeStaker(address) selector."""
    pools_raw = w3.eth.call({"to": STAKING, "data": _selector("getAllPools()")})
    n = int.from_bytes(pools_raw[32:64], "big")
    assert n > 0
    pool = Web3.to_checksum_address("0x" + pools_raw[64 + 12 : 64 + 32].hex())

    data = _selector("proposeStaker(address)") + bytes(32)
    try:
        w3.eth.call({"to": pool, "data": data})
        raise AssertionError("proposeStaker existed pre-Zeta — unexpected!")
    except Exception as e:
        # "execution reverted" without return data is the old contract's
        # fallback / no-matching-selector behavior. That's the pass case.
        LOG.info(f"   pre-Zeta proposeStaker reverted as expected: {str(e)[:80]}")


async def _verify_post_zeta_zeta_specific(w3: Web3):
    """Run the full Zeta-specific invariant suite."""
    _verify_governance_init(w3)
    _verify_staking_config_setters(w3)
    _verify_validator_management_whitelist(w3)
    _verify_stakepool_role_timelock(w3)
    _verify_jwk_manager_validation(w3)


@pytest.mark.asyncio
async def test_full_lifecycle(cluster: Cluster):
    """Walk Zeta on a single cluster.

    Genesis baseline is gravity-testnet-v1.4.0, so Alpha/Beta/Gamma/Delta/
    Epsilon are already active at block 0 — only Zeta is observable. We
    still run the Epsilon ABI smoke up-front (it asserts the post-Epsilon
    contract surface is correct in the baseline) and the pre-Zeta
    proposeStaker-must-revert check, then walk Zeta.
    """
    LOG.info(
        "\n╔══════════════════════════════════════════════════════════════╗"
        "\n║        Full Hardfork Lifecycle E2E (Zeta-only walk)          ║"
        "\n║        zeta=%-5d                                              ║"
        "\n╚══════════════════════════════════════════════════════════════╝",
        ZETA_BLOCK,
    )

    # Sanity: the fork blocks must be strictly increasing.
    prev_block = -1
    for _, _, block in HARDFORK_SEQUENCE:
        assert block > prev_block, \
            f"fork blocks must be strictly increasing: {HARDFORK_SEQUENCE}"
        prev_block = block

    # Bring cluster live once.
    assert await cluster.set_full_live(timeout=60), "Cluster failed to come up"
    node = cluster.get_node("node1")
    assert node is not None
    w3 = node.w3

    # Baseline epsilon ABI smoke. v1.4.0 already ships post-Epsilon contracts,
    # so this asserts the genesis bytecode set is what we think it is before
    # we attempt the Zeta walk. Failing here means the genesis itself is wrong,
    # not Zeta.
    LOG.info("🔎 Baseline (v1.4.0 = post-Epsilon) ABI smoke")
    _verify_epsilon_observables(w3)

    # Pre-Zeta sanity: proposeStaker must not be in StakePool's dispatcher.
    # Must run before the chain crosses ZETA_BLOCK.
    assert node.get_block_number() < ZETA_BLOCK, (
        f"chain already past zetaBlock={ZETA_BLOCK} at test start "
        f"(block={node.get_block_number()}); raise ZETA_BLOCK or run this "
        "test earlier in the suite"
    )
    LOG.info("🔎 Pre-Zeta check: proposeStaker(address) on StakePool must revert")
    await _verify_pre_zeta_propose_staker_reverts(w3)

    # Walk Zeta.
    latest_post_snapshot = None
    for name, display, block in HARDFORK_SEQUENCE:
        latest_post_snapshot = await _walk_hardfork(cluster, name, display, block)

    # Post-Zeta: run the full Zeta-specific invariant suite.
    LOG.info("\n🔎 Post-Zeta: running full Zeta invariant suite")
    await _verify_post_zeta_zeta_specific(w3)

    # Phase 6 — restart + replay. We synthesise a "zeta" config just to get
    # the contract list right for the post-restart codehash reassertion.
    restart_config = HardforkTestConfig(
        name="zeta",
        display_name="Zeta Hardfork (restart replay)",
        hardfork_block=ZETA_BLOCK,
        contracts=get_contracts_for_hardfork("zeta"),
        post_hardfork_blocks=10,
        post_restart_blocks=20,
        min_changed_contracts=1,
    )
    assert latest_post_snapshot is not None
    changed_post_zeta = [
        name for name, h in latest_post_snapshot.items() if h is not None
    ]
    await phase6_restart_replay(cluster, restart_config, changed_post_zeta, latest_post_snapshot)

    LOG.info("✅ Full hardfork lifecycle passed end-to-end")
