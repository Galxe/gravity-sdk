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
- alpha_preflight_error / alpha_tail_wait_s: the Gravity Alpha hardfork
  timeline discipline. Mainnet activates NO gravity forks; greth v2.3.0
  gates behavior changes (system-tx gas exemption, SYSTEM_CALLER balance
  migration) on Alpha, so Alpha MUST NOT activate before every node runs
  the new binary. An alphaTime that predates existing v1.7.5 history makes
  the new binary re-execute those blocks under exempt semantics and
  diverge — symptom fingerprint: gravity_node aborts ~2 s after start with
  ``panicked at ...block_store.rs:773 / assertion `left == right` failed``
  on two 32-byte block hashes (observed live 2026-07-15 with the legacy
  ``alphaTime = 0`` config this case initially inherited).
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from typing import Iterable, List, Optional, Sequence

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


def alpha_preflight_error(
    alpha_time: Optional[int], now: float, min_lead_s: float
) -> Optional[str]:
    """Validate the Alpha timeline before the upgrade starts.

    Returns None when safe: alphaTime absent (mainnet posture — Alpha never
    activates) or at least ``min_lead_s`` in the future. Returns an
    operator-facing error string when Alpha is already active or would
    activate before the fleet can plausibly finish upgrading (this is the
    guard against the ``alphaTime = 0`` legacy-config artifact).
    """
    if alpha_time is None:
        return None
    lead = alpha_time - now
    if lead >= min_lead_s:
        return None
    return (
        f"alphaTime={alpha_time} activates in {lead:.0f}s (< required lead "
        f"{min_lead_s:.0f}s). Alpha gates v2.3.0 behavior changes (system-tx "
        f"gas exemption); it must NOT be active before every node runs the "
        f"new binary, or the new binary re-executes v1.7.5 history under "
        f"exempt semantics and aborts (block_store.rs:773 hash assert). "
        f"Set [hardforks] alphaTime to a later '+NNm' offset (or drop it) "
        f"and re-render."
    )


def alpha_tail_wait_s(
    alpha_time: Optional[int], now: float, max_wait_s: float
) -> Optional[float]:
    """Seconds to wait for the Alpha activation tail phase, or None to skip.

    None when alphaTime is absent (never activates) or still more than
    ``max_wait_s`` away (far-future config: the scheduled activation is not
    reachable within this run's budget). 0 when already activated.
    """
    if alpha_time is None:
        return None
    remaining = alpha_time - now
    if remaining > max_wait_s:
        return None
    return max(remaining, 0.0)


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
