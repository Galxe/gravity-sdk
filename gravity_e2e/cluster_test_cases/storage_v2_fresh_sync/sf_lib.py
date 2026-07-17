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
