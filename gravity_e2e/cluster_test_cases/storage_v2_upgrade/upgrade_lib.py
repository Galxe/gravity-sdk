"""
Pure helpers for the storage_v2_upgrade case.

Everything here is deterministic and unit-tested in
gravity_e2e/tests/unit/test_storage_upgrade_case.py (loaded by file path;
this directory is not a package). The pytest case keeps orchestration and
assertions; this module keeps the derivable facts:

- build_upgrade_order: vfn-first rolling-upgrade order (mirrors
  cluster_test_cases/rolling_upgrade, which sorts vfn1 to the front).
- scan_log_lines / LogScanResult: the storage/decode error scan over node
  execution logs. Patterns are deliberately conservative — they target
  storage-corruption-class failures (RocksDB corruption reports, value
  decode failures, Rust panics), not generic ERROR lines, so healthy log
  noise cannot fail the case. Unwind mentions are collected separately:
  bounded unwinding can be legitimate crash recovery (TC3 kill -9), but an
  unbounded stream of unwind lines means the post-restart consistency
  check is looping — the case asserts a ceiling instead of zero.
- is_upgrade_target: hardlink-or-identical-size predicate shared by the
  binary swap assertion, the offline-probe binary check, and the phase-1
  "deployed binary must NOT already be the upgrade target" guard (deploy
  copies binaries, so samefile alone never fires there).
- alpha_preflight_error / upgrade_completion_error / alpha_tail_wait_s:
  the Gravity Alpha hardfork timeline discipline. Mainnet activates NO
  gravity forks; greth v2.3.0 gates behavior changes (system-tx gas
  exemption, SYSTEM_CALLER balance migration) on Alpha, so Alpha MUST NOT
  activate before every node runs the new binary. An alphaTime that
  predates existing v1.7.5 history makes the new binary re-execute those
  blocks under exempt semantics and diverge — symptom fingerprint:
  gravity_node aborts ~2 s after start with ``panicked at
  ...block_store.rs:773 / assertion `left == right` failed`` on two
  32-byte block hashes (observed live 2026-07-15 with the legacy
  ``alphaTime = 0`` config this case initially inherited). The schedule
  itself is fixed at render time (rolling_upgrade's mechanism: compute the
  fork point once before the run, never touch config mid-test); these
  guards make a blown schedule fail loudly instead of activating early.
- BlockSample / pre_alpha_debit_error / post_alpha_constancy_error: the
  wei-level SYSTEM_CALLER trajectory checks that prove the Alpha semantic
  flip directly — pre-activation every block debits the caller exactly
  ``gas_used * base_fee`` (empty blocks reconcile to the wei), post-
  activation the balance freezes.
"""

from __future__ import annotations

import os
import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Iterable, List, Mapping, Optional, Sequence, Union

# Storage-corruption-class patterns. Case-insensitive where wording varies.
# Every pattern is tied to a concrete failure family:
# - "panicked at": rust panic banner (std::panic output on stderr);
# - corrupt*: RocksDB "Corruption:" status strings and reth's
#   "corrupted"/"corrupt" wordings;
# - "failed to decode" / "decode error" / "DecodeError": reth-db value
#   decode failures (Compact/SCALE decode of table values);
# - "DatabaseError": reth-db error type surfacing in log lines.
ERROR_PATTERNS: Sequence[re.Pattern] = (
    re.compile(r"panicked at"),
    re.compile(r"corrupt", re.IGNORECASE),
    re.compile(r"failed to decode", re.IGNORECASE),
    re.compile(r"decode error", re.IGNORECASE),
    re.compile(r"DecodeError"),
    re.compile(r"DatabaseError"),
)

# Unwind-class lines (collected separately, thresholded by the case).
UNWIND_PATTERN = re.compile(r"unwind", re.IGNORECASE)

# Cap the recorded offenders so a pathological log cannot blow up memory /
# assertion output; the counts stay exact.
MAX_RECORDED_LINES = 50


@dataclass
class LogScanResult:
    """Outcome of scanning one log stream."""

    lines_scanned: int = 0
    error_count: int = 0
    unwind_count: int = 0
    error_lines: List[str] = field(default_factory=list)  # first offenders
    unwind_lines: List[str] = field(default_factory=list)  # first offenders

    def summary(self) -> str:
        head = (
            f"log scan: {self.lines_scanned} lines, "
            f"{self.error_count} storage/decode errors, "
            f"{self.unwind_count} unwind mentions"
        )
        parts = [head]
        if self.error_lines:
            parts.append("first error lines:")
            parts.extend(f"  {line}" for line in self.error_lines)
        if self.unwind_lines:
            parts.append("first unwind lines:")
            parts.extend(f"  {line}" for line in self.unwind_lines)
        return "\n".join(parts)


def scan_log_lines(lines: Iterable[str]) -> LogScanResult:
    """Scan log lines for storage-corruption-class errors and unwind noise.

    Pure over any iterable of strings (list in tests, open file handle in
    the case). Never raises on content; the pass/fail semantics live in
    the caller's assertions.
    """
    result = LogScanResult()
    for raw_line in lines:
        line = raw_line.rstrip("\n")
        result.lines_scanned += 1
        if any(pattern.search(line) for pattern in ERROR_PATTERNS):
            result.error_count += 1
            if len(result.error_lines) < MAX_RECORDED_LINES:
                result.error_lines.append(line)
        if UNWIND_PATTERN.search(line):
            result.unwind_count += 1
            if len(result.unwind_lines) < MAX_RECORDED_LINES:
                result.unwind_lines.append(line)
    return result


def is_upgrade_target(binary: Union[str, Path], new_binary: Union[str, Path]) -> bool:
    """True when the deployed ``binary`` is (a copy of) the upgrade target:
    same inode (hardlink) or an identical-size file.

    The size fallback matters twice: swap_node_binary's cross-device copy
    fallback (copy2 preserves size+mtime) must still count as swapped, and
    — the phase-1 guard — deploy.sh COPIES the ``[source]`` binary into
    each node dir, so a misconfigured [source] pointing at the upgrade
    target itself produces a same-content copy that ``samefile`` alone
    would never flag (and the whole case would silently degrade into a
    same-version no-op).
    """
    return os.path.samefile(binary, new_binary) or (
        Path(binary).stat().st_size == Path(new_binary).stat().st_size
    )


def alpha_preflight_error(
    alpha_time: Optional[int],
    now: float,
    min_lead_s: float,
    max_lead_s: Optional[float] = None,
) -> Optional[str]:
    """Validate the render-time Alpha schedule before the upgrade starts.

    Returns None when safe: alphaTime absent (mainnet posture — Alpha never
    activates) or between ``min_lead_s`` and ``max_lead_s`` in the future.
    Returns an operator-facing error string when Alpha is already active or
    would activate before the fleet can plausibly finish upgrading (the
    guard against the ``alphaTime = 0`` legacy-config artifact), or when it
    is scheduled beyond the case budget — a scheduled activation is
    mandatory coverage, never silently skipped.
    """
    if alpha_time is None:
        return None
    lead = alpha_time - now
    if lead < min_lead_s:
        return (
            f"alphaTime={alpha_time} activates in {lead:.0f}s (< required lead "
            f"{min_lead_s:.0f}s). Alpha gates v2.3.0 behavior changes (system-tx "
            f"gas exemption); it must NOT be active before every node runs the "
            f"new binary, or the new binary re-executes v1.7.5 history under "
            f"exempt semantics and aborts (block_store.rs:773 hash assert). "
            f"Re-render with the component schedule from "
            f"test_params.toml.example (or drop alphaTime entirely)."
        )
    if max_lead_s is not None and lead > max_lead_s:
        return (
            f"alphaTime={alpha_time} is {lead:.0f}s away (> case budget "
            f"{max_lead_s:.0f}s) — the activation crossing is mandatory "
            f"coverage when alphaTime is scheduled and cannot be reached in "
            f"this run. Use the component schedule from "
            f"test_params.toml.example, or drop alphaTime entirely for the "
            f"pure mainnet posture (no gravity fork ever activates)."
        )
    return None


def upgrade_completion_error(
    alpha_time: Optional[int], completion_time: float
) -> Optional[str]:
    """THE operational rule, machine-enforced: every node must finish
    upgrading strictly before Alpha activates.

    Returns None when alphaTime is absent or the fleet completed in time;
    otherwise an error string — the run must fail immediately (an early
    activation means some blocks may have been produced/validated by
    binaries with diverging semantics), never continue silently.
    """
    if alpha_time is None or completion_time < alpha_time:
        return None
    return (
        f"rolling upgrade completed at {completion_time:.0f} but "
        f"alphaTime={alpha_time} had already passed "
        f"({completion_time - alpha_time:.0f}s late): Alpha must activate "
        f"only AFTER every node runs the new binary. The run overshot the "
        f"render-time schedule — increase upgrade_budget/margin under "
        f"[hardforks.alphaTime] in test_params.toml and re-render."
    )


def alpha_tail_wait_s(alpha_time: Optional[int], now: float) -> Optional[float]:
    """Seconds until the scheduled Alpha activation, or None to skip.

    None only when alphaTime is absent (mainnet posture — never activates);
    0 when the chain wall-clock already passed activation (the crossing is
    then verified retroactively). Unreachable schedules were already
    rejected by alpha_preflight_error, so a scheduled fork always waits.
    """
    if alpha_time is None:
        return None
    return max(alpha_time - now, 0.0)


# ---------------------------------------------------------------------------
# SYSTEM_CALLER balance trajectory (Alpha semantic-flip direct proof)
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class BlockSample:
    """Per-block facts needed to reconcile the SYSTEM_CALLER debit."""

    number: int
    gas_used: int
    base_fee: int
    # USER txs in the block. v2.3.0's RPC lists the per-block system txs
    # in the body too (sender == SYSTEM_CALLER; observed live run 2) —
    # the sampler filters them out, so tx_count == 0 means the header
    # gas is purely system gas and the debit must reconcile wei-exact.
    tx_count: int
    balance: int  # SYSTEM_CALLER balance at this block (post-state), wei


def pre_alpha_debit_error(
    samples: Sequence[BlockSample], min_exact: int = 2
) -> Optional[str]:
    """Verify the PRE-Alpha semantic on consecutive block samples: every
    block debits SYSTEM_CALLER, and on empty blocks (no user txs — header
    gas is purely the system txs') the debit reconciles to the wei as
    ``gas_used * base_fee``.

    Returns None when the trajectory matches; an error string otherwise.
    Requires at least ``min_exact`` empty-block exact reconciliations so a
    run cannot pass on strict-decrease alone.
    """
    if len(samples) < 2:
        return f"need at least 2 consecutive block samples, got {len(samples)}"
    exact = 0
    for prev, cur in zip(samples, samples[1:]):
        if cur.number != prev.number + 1:
            return (
                f"samples must be consecutive blocks: {prev.number} then "
                f"{cur.number}"
            )
        delta = prev.balance - cur.balance
        if delta <= 0:
            return (
                f"SYSTEM_CALLER balance did not decrease over block "
                f"{cur.number} (delta={delta} wei): pre-Alpha every block "
                f"debits the system-tx base-fee bill — a frozen balance here "
                f"means exempt semantics were already active"
            )
        if cur.tx_count == 0:
            expected = cur.gas_used * cur.base_fee
            if delta != expected:
                return (
                    f"block {cur.number} (no user txs) debited {delta} wei, "
                    f"expected gas_used*base_fee = {cur.gas_used}*"
                    f"{cur.base_fee} = {expected} wei"
                )
            exact += 1
    if exact < min_exact:
        return (
            f"only {exact} empty-block exact debit reconciliations "
            f"(need >= {min_exact}); sample a quieter window"
        )
    return None


def post_alpha_constancy_error(balances: Mapping[int, int]) -> Optional[str]:
    """Verify the POST-Alpha semantic: SYSTEM_CALLER balance frozen across
    the given blocks (system txs are gas-exempt from activation on).

    ``balances`` maps block number -> balance. Returns None when constant;
    an error string otherwise.
    """
    if len(balances) < 2:
        return f"need at least 2 post-activation blocks, got {len(balances)}"
    if len(set(balances.values())) == 1:
        return None
    return (
        "SYSTEM_CALLER balance still moving after Alpha activation (gas "
        f"exemption not in effect?): {dict(sorted(balances.items()))}"
    )


def build_upgrade_order(node_ids: Sequence[str], first: str) -> List[str]:
    """Rolling-upgrade order: ``first`` (the VFN) up front, the rest in
    their given (cluster.toml) order — the same order rolling_upgrade
    produces via its stable sort.

    Raises ValueError when ``first`` is not among the nodes or ids repeat,
    so a topology typo fails before any node is touched.
    """
    ids = list(node_ids)
    if len(set(ids)) != len(ids):
        raise ValueError(f"duplicate node ids: {ids}")
    if first not in ids:
        raise ValueError(f"upgrade-first node {first!r} not in {ids}")
    return [first] + [node_id for node_id in ids if node_id != first]
