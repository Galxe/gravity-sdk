"""
Pure helpers for the storage_v2_fresh_sync case (TC9).

Deterministic and unit-tested in
gravity_e2e/tests/unit/test_storage_fresh_sync_case.py (loaded by file
path; this directory is not a package). The pytest case keeps
orchestration and assertions; this module keeps the derivable facts:

- the topology constants (which nodes are legacy / SF, and each
  fullnode's pinned upstream — the SF x non-SF coverage matrix);
- resolve_sf_mode: the [sf] mode knob with its env override
  (GRAVITY_SF_MODE), and which modes the case can actually execute today;
- sf_start_extra_args: the form-B argument set, reserved until greth
  wires --storage.v2 into init_genesis AND Node.start() can inject
  per-node args (design doc sf-fresh-sync-design.md §2/Q1).
"""

from __future__ import annotations

from typing import Mapping, Optional, Sequence, Tuple

# Legacy core: born on v1.7.5, rolling-upgraded, stays on the legacy
# layout (upgraded datadirs never flip — TC2's criterion).
LEGACY_NODE_IDS: Tuple[str, ...] = ("node1", "node2", "vfn1", "pfn1")

# SF nodes: born on merge v2.3.0, SF layout from (effectively) block 0.
SF_NODE_IDS: Tuple[str, ...] = (
    "sf_val1",
    "sf_vfn1",
    "sf_vfn2",
    "sf_pfn1",
    "sf_pfn2",
)

# Every fullnode's pinned upstream (static seeds + discovery none in
# cluster.toml.tpl) — the coverage matrix's edges. Validators discover
# each other via consensus and have no pinned upstream.
PINNED_UPSTREAMS: Mapping[str, str] = {
    "vfn1": "node1",      # legacy vfn  <- legacy validator
    "pfn1": "vfn1",       # legacy pfn  <- legacy vfn (the tx entry)
    "sf_vfn1": "node2",   # SF vfn      <- legacy validator
    "sf_vfn2": "sf_val1", # SF vfn      <- SF validator
    "sf_pfn1": "sf_vfn1", # SF pfn      <- SF vfn
    "sf_pfn2": "vfn1",    # SF pfn      <- legacy vfn
}

# SF fullnodes whose upstream exists from phase 1 (first batch); sf_pfn1
# must wait for sf_vfn1, sf_vfn2 must wait for sf_val1.
SF_FIRST_BATCH: Tuple[str, ...] = ("sf_vfn1", "sf_pfn2")

# All txs enter here (the production pfn -> vfn -> validator path).
TX_ENTRY_NODE = "pfn1"

# Cross-checked against genesis.toml.tpl by the unit tests: the case's
# waits must match the rendered epoch, and the join stake must equal the
# genesis validators' stake (the L3 all-votes-quorum prerequisite).
EPOCH_INTERVAL_S = 120
GENESIS_STAKE_WEI = 2 * 10**18
JOIN_STAKE_ETH = "2.0"

# The three-account fund-flow model (test module constraint (4)): the
# [faucet_init] cascade sweeps the faucet's on-chain balance at suite
# init, so the faucet is a gas-only governance wallet afterwards;
# bench[0] signs sf_val1's join/staking (manager._ensure_evm_account
# assigns accounts.csv rows to VALIDATOR-role nodes in order); bench[1]
# is the bank for every foreground value transfer. Must match
# [faucet_init] num_accounts (unit-test enforced).
JOIN_BENCH_INDEX = 0
BANK_BENCH_INDEX = 1
FAUCET_INIT_NUM_ACCOUNTS = 2

# ── Chain-rate assumptions: the SINGLE source of truth ──────────────────
#
# TC9 slows the CHAIN's block production instead of touching the nodes
# under test: deep sync is tick-limited on the sync side (~1 block per
# 200 ms sync round ⇒ ~4.4 blk/s ceiling — possibly a driver defect, see
# the README's greth question) while the chain produces at ~3.8-3.9
# blk/s, leaving near-zero net convergence. Slowing production to ~1
# blk/s is an ENVIRONMENT parameter (e2e owns the chain's pace) and
# gives even the untouched sync driver an overwhelming advantage.
#
# The pacing knob, verified in source:
# - round_manager.rs:389-396 `process_new_round_event`: the proposer
#   sleeps APTOS_PROPOSER_SLEEP_MS (env, safe-parsed, default 200 ms)
#   UNCONDITIONALLY at the top of every new round — before the payload
#   pull — so it floors the per-block interval regardless of load (load
#   only makes blocks bigger, never faster). Measured: 200 ms default +
#   ~60 ms round overhead = the observed ~260 ms/block (~3.8-3.9 blk/s).
# - quorum_store_poll_time_ms is NOT a pacing knob in this fork:
#   quorum_store_client.rs:124 hardcodes `let done = true;`, so the poll
#   loop returns after a single pull and the `!done` wait branch (:138)
#   is unreachable — the config value is dead code.
#
# Injection: the case-local reth_config.json.tpl (validator-role
# template, auto-picked by runner.py's RETH_CONFIG_TPL override) bakes
# APTOS_PROPOSER_SLEEP_MS into .env_vars, so ALL validator-role nodes —
# node1, node2 AND sf_val1 — carry it from first boot (chain speed is a
# network property, and every validator rotates as proposer).
# vfn/pfn templates stay default (fullnodes never propose).
#
# ALL rate-derived constants below flow from these; a future re-pacing
# is a one-line change plus the unit-locked derivations.
PROPOSER_SLEEP_MS = 1000
ROUND_OVERHEAD_MS = 60  # measured: 260 ms observed at the 200 ms default
CHAIN_BLOCK_RATE_BPS = 1000.0 / (PROPOSER_SLEEP_MS + ROUND_OVERHEAD_MS)  # ~0.94
# The sync driver's own ceiling (~1 block per 200 ms sync round;
# measured 4.3-4.5 blk/s across attempts 5-7), floored conservatively.
SYNC_RATE_FLOOR_BPS = 4.0
# Net convergence floor for the catch-up budget: sync floor minus chain
# production (~3.06 blk/s — pending live calibration via the per-minute
# net_rate diagnostics).
NET_CATCHUP_FLOOR_BPS = SYNC_RATE_FLOOR_BPS - CHAIN_BLOCK_RATE_BPS
# Epoch geometry at the slowed rate (epoch_interval 120 s x ~0.94 blk/s).
BLOCKS_PER_EPOCH = int(EPOCH_INTERVAL_S * CHAIN_BLOCK_RATE_BPS)  # ~113
# Stall window for the progress-based waiter: one epoch-staircase step
# now takes <= BLOCKS_PER_EPOCH / SYNC_RATE_FLOOR_BPS (~28 s); 3x that
# for jitter, but never below two full epochs — epoch transitions (DKG
# etc.) can legitimately flatten progress around boundaries.
CATCHUP_STALL_WINDOW_S = max(
    2 * EPOCH_INTERVAL_S, 3 * BLOCKS_PER_EPOCH / SYNC_RATE_FLOOR_BPS
)
# gammaBlock for test_params.toml.example: must activate only after the
# fleet finishes upgrading (~render+19 min; the chain starts at
# ~render+5 min, so ~14 min of chain time ≈ 790 blocks at the slowed
# rate) yet be crossed comfortably within the run — 1200 is crossed at
# ~21 min of chain time (~render+26 min). Unit-locked against the
# example file.
GAMMA_BLOCK = 1200

SF_MODES: Tuple[str, ...] = ("migrate", "flag")
# Modes the case can execute today. "flag" (form B) needs greth's
# --storage.v2 -> init_genesis wiring plus per-node start-arg injection.
EXECUTABLE_SF_MODES: Tuple[str, ...] = ("migrate",)


def resolve_sf_mode(params: Mapping, environ: Mapping[str, str]) -> str:
    """The effective SF-enable mode: GRAVITY_SF_MODE overrides the params
    [sf].mode; default "migrate". Raises on unknown modes and on modes the
    case cannot execute yet (so a premature "flag" run fails at phase 0
    with a pointer instead of mid-run)."""
    mode = environ.get("GRAVITY_SF_MODE") or params.get("sf", {}).get(
        "mode", "migrate"
    )
    if mode not in SF_MODES:
        raise ValueError(f"[sf] mode must be one of {SF_MODES}, got {mode!r}")
    if mode not in EXECUTABLE_SF_MODES:
        raise NotImplementedError(
            f"[sf] mode {mode!r} is reserved: form B needs greth to wire "
            f"--storage.v2 into init_genesis (design doc Q1) and "
            f"Node.start() to inject per-node args; run mode 'migrate' "
            f"(form D) until then"
        )
    return mode


def sf_start_extra_args(mode: str) -> Sequence[str]:
    """Extra gravity_node args for an SF node's FIRST start under the
    given mode. Form D needs none (the flip happens offline via
    migrate-changesets); form B is the reserved wiring point."""
    if mode == "migrate":
        return ()
    if mode == "flag":
        # TODO(Q1/form B): return ("--storage.v2",) once greth wires the
        # flag into init_genesis; also needs Node.start() arg injection.
        raise NotImplementedError("form B is not wired yet (design doc Q1)")
    raise ValueError(f"unknown sf mode {mode!r}")


def wipe_targets(node_data_dir: str, node_logs: Optional[Sequence[str]] = None):
    """What must disappear for a runner-started node to become fresh
    again: the whole data dir (reth datadir + consensus/quorumstore DBs).
    Returned as a list so the caller (which owns the fs side effects) can
    log exactly what it removes. Node identity/config/binary live outside
    data/ and must survive."""
    targets = [node_data_dir]
    targets.extend(node_logs or ())
    return targets
