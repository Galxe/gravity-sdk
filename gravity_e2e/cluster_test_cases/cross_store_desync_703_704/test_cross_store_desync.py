"""
Deterministic e2e reproduction for gravity-audit #703 and #704.

Refs Galxe/gravity-audit#703, Galxe/gravity-audit#704
(see galxe/RESTART_RECOVERY_FINDINGS.md, findings F1 and F2).

THE BUG (F2, #704): reth's execution rocksdb and the aptos consensus DB persist
on INDEPENDENT stores with no cross-store write-ordering barrier. In the
commit_ledger/commit_blocks window reth can durably persist block N while the
consensus DB's BlockNumberSchema / LedgerInfoSchema is still at N-1. A crash in
that window leaves reth AHEAD of consensus.

On restart, recovery anchors on reth's height N but the consensus index does not
contain N, so either:
  * crates/api/src/bootstrap.rs:335-344 early-returns after setting
    latest_commit_block_number = N but BEFORE marking the block-buffer-manager
    Ready -> the buffer stays Uninitialized -> every get_ordered_blocks awaits
    ready_notifier forever = SILENT PERMANENT STALL (one error!, no panic); or
  * RecoveryData::new fails find_root ->
    aptos-core/consensus/src/persistent_liveness_storage.rs:550 panic!("")
    (the PartialRecoveryData / state-sync fallback is commented out and
    recovery_manager.rs:104 is todo!()) = CRASH-LOOP (F1, #703).

HOW WE INDUCE IT DETERMINISTICALLY (no need for a real power-loss race):
  1. Bring the single node live, let it commit past height H (> 5).
  2. node.stop() cleanly.
  3. Run `gravity_cli unwind --consensus-db-path <data/consensus_db>
     --target H-3` (see bin/gravity_cli/src/unwind.rs). unwind_to_block deletes
     consensus Block/QC/BlockNumber entries with block_number > target, and the
     ledger-info / rand state, WITHOUT touching <data/reth>. That is EXACTLY the
     "reth ahead of consensus" state the crash window would leave behind.
  4. node.start() again and observe the failure: a panic in
     consensus_log/validator.log, OR a silent stall (process up, RPC up, height
     never advances past reth's persisted H, buffer Uninitialized).

The test asserts the CORRECT post-fix behaviour (node detects reth > consensus,
auto-reconciles, and RESUMES producing blocks). On current code it cannot, so it
is marked xfail. The captured panic string / stall evidence is logged and
attached to the assertion message.
"""

import asyncio
import logging
import os
import subprocess
import time
from pathlib import Path

import pytest

from gravity_e2e.cluster.manager import Cluster

LOG = logging.getLogger(__name__)

# A real Rust panic always logs "panicked at". The recovery failure also logs
# the specific F1 string before the empty-message panic.
PANIC_MARKER = "panicked at"
RECOVERY_FAIL_MARKER = "Failed to construct recovery data"
# Bootstrap early-return / buffer-uninitialized evidence (bootstrap.rs:335-344).
STALL_MARKERS = (
    "Uninitialized",
    "latest_commit_block_number",
    "block buffer",
)

# gravity_cli / gravity_node live under the cargo target dir (quick-release).
# Honour CARGO_TARGET_DIR (the harness exports it to the shared checkout's
# target/, since a git worktree has no target/ of its own), else fall back to
# <repo-root>/target.
_REPO_ROOT = Path(__file__).resolve().parents[3]
_TARGET_DIR = Path(os.environ.get("CARGO_TARGET_DIR", _REPO_ROOT / "target"))
_GRAVITY_CLI = _TARGET_DIR / "quick-release" / "gravity_cli"

# How long to wait after restart for the node to resume block production. A
# healthy (fixed) node resumes within a few seconds; the buggy node stalls
# forever, so a flat height across this whole window IS the stall evidence.
RESUME_TIMEOUT = 45
STALL_OBSERVE = 30


def _consensus_log(node) -> Path:
    return node._infra_path / "consensus_log" / "validator.log"


def _scan_log(path: Path, markers) -> str | None:
    if not path.exists():
        return None
    try:
        with open(path, "r", errors="ignore") as f:
            for line in f:
                for m in markers:
                    if m in line:
                        return line.strip()
    except OSError:
        return None
    return None


def _grep_panic(node) -> str | None:
    """Return the panic / recovery-failure line from validator.log, if any."""
    log = _consensus_log(node)
    hit = _scan_log(log, (RECOVERY_FAIL_MARKER,))
    if hit:
        return hit
    return _scan_log(log, (PANIC_MARKER,))


def _grep_stall_evidence(node) -> str | None:
    log = _consensus_log(node)
    return _scan_log(log, STALL_MARKERS)


@pytest.mark.asyncio
@pytest.mark.xfail(
    reason="gravity-audit#703/#704: reth-ahead-of-consensus cross-store desync. "
    "Recovery anchors on reth's height (gravity_node/src/main.rs:123 "
    "recover_block_number) but the unwound consensus index lacks it, so "
    "RecoveryData::new -> find_root fails with 'unable to find root' -> "
    "persistent_liveness_storage.rs:550 panic!(\"\") and the node CRASHES "
    "(the PartialRecoveryData/state-sync fallback on :551 is commented out and "
    "recovery_manager.rs:104 is todo!()). The sibling F2 path is the "
    "block_buffer_manager.rs:343 early-return (map non-empty but missing reth's "
    "height) leaving the buffer Uninitialized -> get_ordered_blocks awaits "
    "ready_notifier forever (silent stall). Remove this xfail once startup "
    "auto-reconciles reth_height > consensus_index instead of panicking/stalling.",
    strict=True,
    raises=AssertionError,
)
async def test_reth_ahead_of_consensus_recovers(cluster: Cluster):
    # ---- 1. Bring the single node live and commit past H -------------------
    assert await cluster.set_full_live(timeout=90), "node failed to become live"
    node = cluster.get_node("node1")
    assert node, "node1 not found"
    assert _GRAVITY_CLI.exists(), f"gravity_cli not found at {_GRAVITY_CLI}"

    assert await node.wait_for_block_increase(timeout=60, delta=6), (
        "node never committed past height 6 — cannot set up the desync"
    )
    h_before_stop = node.get_block_number()
    LOG.info(f"node committed to height {h_before_stop}; stopping cleanly")
    assert h_before_stop > 5, f"height {h_before_stop} too low to unwind"

    # ---- 2. Clean stop -----------------------------------------------------
    assert await node.stop(), "node failed to stop cleanly"
    assert not node.is_running(), "node still running after stop()"

    # GOTCHA: `gravity_cli unwind --consensus-db-path P` internally does
    # ConsensusDB::new(P, ...) which JOINS "consensus_db" onto P (see
    # consensusdb/mod.rs:112, CONSENSUS_DB_NAME). The node's real rocksdb lives
    # at <data>/consensus_db, so the path we must hand the tool is the PARENT
    # <data> dir — passing <data>/consensus_db makes the tool operate on an empty
    # <data>/consensus_db/consensus_db and silently no-op. So:
    data_dir = node._infra_path / "data"
    consensus_db = data_dir / "consensus_db"  # the actual rocksdb the node opens
    reth_db = data_dir / "reth"
    assert consensus_db.exists(), f"consensus_db missing at {consensus_db}"
    assert reth_db.exists(), f"reth db missing at {reth_db}"

    # reth's on-disk height is the anchor recovery will use. We deliberately do
    # NOT touch reth_db; only the consensus DB is rolled back.
    reth_size_before = sum(
        f.stat().st_size for f in reth_db.rglob("*") if f.is_file()
    )

    # ---- 3. Roll the CONSENSUS DB back, leaving reth ahead -----------------
    # Unwind aggressively (to floor(H/2), well below reth's H): deletes consensus
    # Block/QC/BlockNumber for block_number > target plus the stale ledger-info /
    # rand state, but does NOT touch reth. This leaves the consensus index well
    # behind reth so the recovery map no longer contains reth's height.
    target = max(1, h_before_stop // 2)
    LOG.info(
        f"unwinding CONSENSUS DB to block {target} (reth stays at ~{h_before_stop}) "
        f"to induce reth-ahead-of-consensus; passing parent dir {data_dir} "
        f"(tool appends 'consensus_db')"
    )
    proc = subprocess.run(
        [
            str(_GRAVITY_CLI),
            "unwind",
            "--consensus-db-path",
            str(data_dir),
            "--target",
            str(target),
        ],
        capture_output=True,
        text=True,
        timeout=120,
    )
    LOG.info(f"gravity_cli unwind stdout:\n{proc.stdout}")
    if proc.stderr.strip():
        LOG.info(f"gravity_cli unwind stderr:\n{proc.stderr}")
    assert proc.returncode == 0, (
        f"gravity_cli unwind failed (rc={proc.returncode}): {proc.stderr}"
    )
    # Confirm the tool hit the node's real DB, not an empty nested one.
    assert "consensus_db/consensus_db" not in proc.stdout, (
        "unwind operated on a NESTED consensus_db (wrong path) — it would no-op. "
        f"stdout: {proc.stdout}"
    )

    # Sanity: reth was untouched (still ahead of the unwound consensus index).
    reth_size_after = sum(
        f.stat().st_size for f in reth_db.rglob("*") if f.is_file()
    )
    LOG.info(
        f"reth db size {reth_size_before} -> {reth_size_after} bytes (unchanged; "
        f"reth still at ~{h_before_stop}, consensus now at {target})"
    )

    # ---- 4. Restart and observe recovery -----------------------------------
    if node.pid_file.exists():
        node.pid_file.unlink()
    LOG.info("restarting node with reth AHEAD of consensus...")
    started = await node.start()

    # Give recovery a moment to either panic or settle into the stall, then
    # watch for block progress past reth's persisted height.
    panic = None
    resumed = False
    base_height = None
    deadline = time.time() + RESUME_TIMEOUT
    while time.time() < deadline:
        panic = _grep_panic(node)
        if panic:
            break
        try:
            if node.is_running():
                bn = node.get_block_number()
                if base_height is None:
                    base_height = bn
                # Healthy recovery: height climbs past where it restarted.
                if bn > target + 1 and bn >= h_before_stop:
                    resumed = True
                    break
        except Exception:
            pass
        await asyncio.sleep(2)

    # If still up and not resumed, confirm the stall is *persistent* (flat
    # height) over an additional observation window — that is the F2 silent
    # stall, distinct from a slow-but-progressing node.
    stall_flat = False
    if not panic and not resumed and node.is_running():
        h0 = node.get_block_number()
        await asyncio.sleep(STALL_OBSERVE)
        h1 = node.get_block_number()
        stall_flat = h1 == h0
        LOG.info(
            f"stall observation: height {h0} -> {h1} over {STALL_OBSERVE}s "
            f"(flat={stall_flat})"
        )

    stall_log = _grep_stall_evidence(node)
    final_running = node.is_running()
    final_height = node.get_block_number() if final_running else None

    # ---- 5. Build the evidence string and assert CORRECT behaviour ---------
    if panic:
        evidence = (
            f"RECOVERY PANIC (F1/#703): validator.log -> {panic!r}. "
            f"Source: aptos-core/consensus/src/persistent_liveness_storage.rs:550 "
            f"panic!(\"\") after 'Failed to construct recovery data' / "
            f"RecoveryData::new -> find_root 'unable to find root' "
            f"(PartialRecoveryData fallback on :551 commented out; "
            f"recovery_manager.rs:104 todo!())."
        )
    elif stall_flat or (not resumed and not started):
        evidence = (
            f"SILENT STALL (F2/#704): node up={final_running}, height frozen at "
            f"{final_height} (reth persisted ~{h_before_stop}, consensus unwound "
            f"to {target}); no progress in {RESUME_TIMEOUT}+{STALL_OBSERVE}s. "
            f"Source: crates/block-buffer-manager/src/block_buffer_manager.rs:343 "
            f"early-return (latest_commit_block_number not in the recovered map) "
            f"leaves the buffer Uninitialized -> get_ordered_blocks awaits "
            f"ready_notifier forever."
            + (f" log: {stall_log!r}" if stall_log else "")
        )
    elif not final_running:
        evidence = (
            f"NODE DIED on restart (reth ahead of consensus): process not "
            f"running, no clean panic captured. start()={started}. "
            f"Inspect {_consensus_log(node)}."
        )
    else:
        evidence = "node resumed cleanly"

    LOG.info(f"OBSERVED: {evidence}")

    # CORRECT post-fix behaviour: the node detects reth_height > consensus_index,
    # reconciles, and RESUMES producing blocks past the desync point. Until that
    # path exists this assertion fails -> xfail.
    assert resumed and final_running and panic is None, (
        f"node did NOT recover from reth-ahead-of-consensus desync. {evidence}"
    )
