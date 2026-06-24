#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage:
  report.sh <chaos-report.jsonl>

Prints a compact summary grouped by scenario/result.
EOF
}

[ "${1:-}" != "-h" ] && [ "${1:-}" != "--help" ] || { usage; exit 0; }
[ "$#" -eq 1 ] || { usage >&2; exit 1; }

REPORT_FILE="$1"
[ -f "$REPORT_FILE" ] || { echo "report not found: $REPORT_FILE" >&2; exit 1; }

python3 - "$REPORT_FILE" <<'PY'
import collections
import json
import sys
from datetime import datetime

path = sys.argv[1]
rows = []
with open(path, encoding="utf-8") as fh:
    for line in fh:
        line = line.strip()
        if not line:
            continue
        rows.append(json.loads(line))

if not rows:
    print("No report rows.")
    raise SystemExit(0)

by_key = collections.Counter((r.get("scenario"), r.get("result")) for r in rows)
by_scenario = collections.Counter(r.get("scenario") for r in rows)
loop_rounds = [
    r for r in rows
    if r.get("scenario") == "loop" and r.get("result") in {"round_pass", "round_fail", "round_inconclusive"}
]
phase_rows = [r for r in rows if r.get("result") == "phase"]

def percentile(values, pct):
    if not values:
        return None
    ordered = sorted(values)
    idx = int(round((len(ordered) - 1) * pct / 100.0))
    return ordered[idx]

print(f"Report: {path}")
print(f"Total rows: {len(rows)}")
print()
print("By scenario:")
for scenario, count in sorted(by_scenario.items()):
    print(f"  {scenario}: {count}")
print()
print("By scenario/result:")
for (scenario, result), count in sorted(by_key.items()):
    print(f"  {scenario}/{result}: {count}")

if loop_rounds:
    passed = sum(1 for r in loop_rounds if r.get("result") == "round_pass")
    failed = sum(1 for r in loop_rounds if r.get("result") == "round_fail")
    inconclusive_count = sum(1 for r in loop_rounds if r.get("result") == "round_inconclusive")
    durations = [
        float(r["duration_s"])
        for r in loop_rounds
        if isinstance(r.get("duration_s"), (int, float))
    ]
    by_selected = collections.Counter(r.get("selected_scenario", "unknown") for r in loop_rounds)
    print()
    print("Loop rounds:")
    print(f"  total={len(loop_rounds)} pass={passed} fail={failed} inconclusive={inconclusive_count}")
    if loop_rounds:
        pass_rate = passed / len(loop_rounds) * 100.0
        print(f"  pass_rate={pass_rate:.1f}%")
    if durations:
        print(f"  duration_p50={percentile(durations, 50):.1f}s duration_p95={percentile(durations, 95):.1f}s")
    print("  by selected scenario:")
    for scenario, count in sorted(by_selected.items()):
        print(f"    {scenario}: {count}")

if phase_rows:
    by_phase = collections.defaultdict(list)
    phase_failures = []
    for row in phase_rows:
        by_phase[row.get("phase", "unknown")].append(row)
        if row.get("phase_result") != "pass":
            phase_failures.append(row)
    print()
    print("Scenario phases:")
    for phase, phase_items in sorted(by_phase.items()):
        durations = [
            float(r["duration_s"])
            for r in phase_items
            if isinstance(r.get("duration_s"), (int, float))
        ]
        passed = sum(1 for r in phase_items if r.get("phase_result") == "pass")
        failed = sum(1 for r in phase_items if r.get("phase_result") == "fail")
        line = f"  {phase}: count={len(phase_items)} pass={passed} fail={failed}"
        if durations:
            line += f" p50={percentile(durations, 50):.1f}s p95={percentile(durations, 95):.1f}s"
        print(line)

ok_results = {
    "pass",
    "complete",
    "snapshot",
    "paused",
    "inconclusive",
    "round_start",
    "round_pass",
    "round_inconclusive",
    "summary",
    "phase",
}
failures = [r for r in rows if r.get("result") not in ok_results]
inconclusive = [r for r in rows if r.get("result") == "inconclusive"]
if phase_rows:
    phase_failures = [r for r in phase_rows if r.get("phase_result") != "pass"]
    if phase_failures:
        print()
        print("Phase Failures:")
        for row in phase_failures[-10:]:
            ts = datetime.fromtimestamp(row["ts"]).isoformat(timespec="seconds")
            detail = row.get("detail", "")
            print(
                f"  {ts} {row.get('scenario')} phase={row.get('phase')} "
                f"result={row.get('phase_result')} duration={row.get('duration_s')} {detail}"
            )
if inconclusive:
    print()
    print("Inconclusive:")
    for row in inconclusive[-10:]:
        ts = datetime.fromtimestamp(row["ts"]).isoformat(timespec="seconds")
        detail = row.get("detail", "")
        print(f"  {ts} {row.get('scenario')} {detail}")
if failures:
    print()
    print("Failures:")
    for row in failures[-10:]:
        ts = datetime.fromtimestamp(row["ts"]).isoformat(timespec="seconds")
        detail = row.get("detail", "")
        if row.get("scenario") == "loop":
            detail = (
                f"round={row.get('round')} selected={row.get('selected_scenario')} "
                f"target={row.get('target')} rc={row.get('rc')} snapshot={row.get('snapshot_path', '')}"
            )
        print(f"  {ts} {row.get('scenario')} {row.get('result')} {detail}")
PY
