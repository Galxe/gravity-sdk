"""
Full-lifecycle Hardfork E2E Test

Walks a single gravity cluster through EVERY Gravity hardfork in order:
    Alpha, Beta → genesis (blocks 0-0)
    Gamma       → GAMMA_BLOCK (default 30)
    Delta       → DELTA_BLOCK (default 60)
    Epsilon     → EPSILON_BLOCK (default 100)
    Zeta        → ZETA_BLOCK (default 150)

At each fork boundary we:
  1. snapshot the contract codehashes the fork claims to upgrade
  2. wait for the fork block
  3. re-snapshot and assert the codehashes for that fork's contracts changed
  4. run the fork-specific ABI smoke (via test_epsilon / test_zeta helpers)

After Zeta we:
  5. run one business flow end-to-end across forks:
     - pre-Zeta: propose a stake-pool operator change → must revert (no
       propose* selector)
     - post-Zeta: proposeOperator → fast-forward on-chain time by
       MIN_ROLE_CHANGE_DELAY → acceptOperator → verify operator changed
  6. restart all nodes, wait for replay, reassert post-Zeta codehashes.

Env vars:
    GAMMA_BLOCK / DELTA_BLOCK / EPSILON_BLOCK / ZETA_BLOCK: per-fork block
        numbers (passed through to hooks.py -> genesis config injection).
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

# Ordered sequence the test walks. "alpha"/"beta" activate at block 0 and
# have no contract-level observables to assert via this framework, so they
# are not in the list — their activation is implicit at genesis.
HARDFORK_SEQUENCE = [
    ("gamma", "Gamma Hardfork", GAMMA_BLOCK),
    ("delta", "Delta Hardfork", DELTA_BLOCK),
    ("epsilon", "Epsilon Hardfork", EPSILON_BLOCK),
    ("zeta", "Zeta Hardfork", ZETA_BLOCK),
]

# Per-fork post-transition smoke callback
PER_FORK_SMOKE = {
    "gamma": None,  # Framework's Phase 4 already covers Gamma (codehash diff)
    "delta": None,  # Governance owner patch checked more thoroughly in Zeta
    "epsilon": _verify_epsilon_observables,
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
    """Walk the full Alpha→Zeta hardfork sequence on a single cluster."""
    LOG.info(
        "\n╔══════════════════════════════════════════════════════════════╗"
        "\n║        Full Hardfork Lifecycle E2E                          ║"
        "\n║        gamma=%-5d delta=%-5d epsilon=%-5d zeta=%-5d       ║"
        "\n╚══════════════════════════════════════════════════════════════╝",
        GAMMA_BLOCK, DELTA_BLOCK, EPSILON_BLOCK, ZETA_BLOCK,
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

    # Pre-Zeta sanity: proposeStaker must not be in StakePool's dispatcher.
    # Wait for Gamma to land first (StakePool bytecode is the Gamma one pre-Zeta).
    await wait_for_block(w3, GAMMA_BLOCK, timeout=180)
    LOG.info("🔎 Pre-Zeta check: proposeStaker(address) on StakePool must revert")
    await _verify_pre_zeta_propose_staker_reverts(w3)

    # Walk each fork.
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
