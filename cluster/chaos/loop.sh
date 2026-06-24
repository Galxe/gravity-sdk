#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CLUSTER_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CONFIG_FILE="${CLUSTER_TOML:-$CLUSTER_DIR/cluster.toml}"
REPORT_FILE="${CHAOS_REPORT_FILE:-$SCRIPT_DIR/reports/chaos-loop-$(date +%Y%m%d-%H%M%S).jsonl}"
LOOP_INTERVAL="${LOOP_INTERVAL:-60}"
LOOP_MAX_ROUNDS="${LOOP_MAX_ROUNDS:-0}"
PAUSE_ON_FAILURE="${PAUSE_ON_FAILURE:-1}"
LOOP_FREEZE_ON_FAILURE="${LOOP_FREEZE_ON_FAILURE:-$PAUSE_ON_FAILURE}"
LOOP_DRY_RUN="${LOOP_DRY_RUN:-0}"
LOOP_SUMMARY_FILE="${LOOP_SUMMARY_FILE:-${CHAOS_SUMMARY_FILE:-}}"
LOOP_STARTED_TS="$(date +%s)"
SUMMARY_WRITTEN=0

if [ "${CHAOS_BACKEND:-}" = "docker" ]; then
    DEFAULT_LOOP_SCENARIO_WEIGHTS="rolling-restart=8,flap=8,kill-under-load=8,majority-failure=5,partition-random=12,partition-no-quorum-split=14,partition-majority-minority=14,partition-load=22"
    if [ "${CHAOS_DOCKER_ENABLE_NET_TOOLS_SCENARIOS:-0}" = "1" ]; then
        DEFAULT_LOOP_SCENARIO_WEIGHTS="${DEFAULT_LOOP_SCENARIO_WEIGHTS},partition-asym=10,delay-load-spike=12"
    fi
else
    DEFAULT_LOOP_SCENARIO_WEIGHTS="rolling-restart=20,flap=20,kill-under-load=15,majority-failure=10,partition-kill=10,partition-random=15,partition-asym=10"
fi
LOOP_SCENARIO_WEIGHTS="${LOOP_SCENARIO_WEIGHTS:-$DEFAULT_LOOP_SCENARIO_WEIGHTS}"

LOOP_ROLLING_WAIT="${LOOP_ROLLING_WAIT:-30}"
LOOP_FLAP_COUNT="${LOOP_FLAP_COUNT:-10}"
LOOP_FLAP_INTERVAL="${LOOP_FLAP_INTERVAL:-10}"
LOOP_PARTITION_HOLD="${LOOP_PARTITION_HOLD:-180}"
LOOP_RECOVER_WAIT="${LOOP_RECOVER_WAIT:-60}"
LOOP_DELAY_DURATION="${LOOP_DELAY_DURATION:-300}"
LOOP_DELAY_LATENCY="${LOOP_DELAY_LATENCY:-200ms}"
LOOP_DELAY_JITTER="${LOOP_DELAY_JITTER:-50ms}"
LOOP_DELAY_ORACLE_INTERVAL="${LOOP_DELAY_ORACLE_INTERVAL:-60}"

ROUND_SCENARIO=""
ROUND_TARGET=""
ROUND_FAULT=""
ROUND_ASSERTIONS=""
ROUND_COMMAND=()

usage() {
    cat <<'EOF'
Usage:
  loop.sh [--config <cluster.toml>]

Runs weighted random chaos scenarios until interrupted.

Environment:
  CHAOS_REPORT_FILE       JSONL report path
  LOOP_INTERVAL           Seconds between rounds, default 60
  LOOP_MAX_ROUNDS         0 means unlimited
  PAUSE_ON_FAILURE        1 pauses after a failed round, default 1
  LOOP_FREEZE_ON_FAILURE  1 asks scenarios to preserve fault state on failure,
                          default follows PAUSE_ON_FAILURE
  LOOP_DRY_RUN            1 records selected rounds without running scenarios
  LOOP_SCENARIO_WEIGHTS   Comma-separated name=weight list
  LOOP_SUMMARY_FILE       Optional standalone JSON summary output path
  CHAOS_DOCKER_ENABLE_NET_TOOLS_SCENARIOS
                          Docker only: include partition-asym and
                          delay-load-spike in default weights when containers
                          have iptables/tc support

Default Docker weights:
  rolling-restart=8,flap=8,kill-under-load=8,majority-failure=5,
  partition-random=12,partition-no-quorum-split=14,
  partition-majority-minority=14,partition-load=22

Useful tuning:
  LOOP_PARTITION_HOLD=180 LOOP_RECOVER_WAIT=60
  LOOP_DELAY_DURATION=300 LOOP_DELAY_LATENCY=200ms LOOP_DELAY_JITTER=50ms
EOF
}

log() {
    printf '[loop] %s\n' "$*"
}

cluster_py() {
    python3 "$SCRIPT_DIR/lib/cluster.py" "$@"
}

json_bool() {
    [ "$1" = "1" ] && printf 'true' || printf 'false'
}

record_loop_event() {
    local result="$1"
    local round="${2:-}"
    local scenario="${3:-}"
    local target="${4:-}"
    local fault="${5:-}"
    local assertions="${6:-}"
    local duration_s="${7:-}"
    local rc="${8:-}"
    local command="${9:-}"
    local snapshot_path="${10:-}"

    mkdir -p "$(dirname "$REPORT_FILE")"
    RESULT="$result" ROUND="$round" SCENARIO_NAME="$scenario" TARGET="$target" \
    FAULT="$fault" ASSERTIONS="$assertions" DURATION_S="$duration_s" RC="$rc" \
    COMMAND="$command" SNAPSHOT_PATH="$snapshot_path" REPORT_FILE="$REPORT_FILE" \
    LOOP_DRY_RUN_JSON="$(json_bool "$LOOP_DRY_RUN")" \
    LOOP_FREEZE_JSON="$(json_bool "$LOOP_FREEZE_ON_FAILURE")" python3 <<'PY'
import json
import os
import time

def maybe_float(value: str):
    if not value:
        return None
    try:
        return float(value)
    except ValueError:
        return value

def maybe_int(value: str):
    if not value:
        return None
    try:
        return int(value)
    except ValueError:
        return value

entry = {
    "ts": time.time(),
    "scenario": "loop",
    "result": os.environ["RESULT"],
    "dry_run": os.environ["LOOP_DRY_RUN_JSON"] == "true",
    "freeze_on_failure": os.environ["LOOP_FREEZE_JSON"] == "true",
}
if os.environ.get("ROUND"):
    entry["round"] = maybe_int(os.environ["ROUND"])
if os.environ.get("SCENARIO_NAME"):
    entry["selected_scenario"] = os.environ["SCENARIO_NAME"]
if os.environ.get("TARGET"):
    entry["target"] = os.environ["TARGET"]
if os.environ.get("FAULT"):
    entry["fault"] = os.environ["FAULT"]
if os.environ.get("ASSERTIONS"):
    entry["assertions"] = os.environ["ASSERTIONS"]
if os.environ.get("DURATION_S"):
    entry["duration_s"] = maybe_float(os.environ["DURATION_S"])
if os.environ.get("RC"):
    entry["rc"] = maybe_int(os.environ["RC"])
if os.environ.get("COMMAND"):
    entry["command"] = os.environ["COMMAND"]
if os.environ.get("SNAPSHOT_PATH"):
    entry["snapshot_path"] = os.environ["SNAPSHOT_PATH"]
with open(os.environ["REPORT_FILE"], "a", encoding="utf-8") as fh:
    fh.write(json.dumps(entry, sort_keys=True) + "\n")
PY
}

latest_snapshot_path() {
    [ -f "$REPORT_FILE" ] || return 0
    python3 - "$REPORT_FILE" <<'PY'
import json
import sys

latest = ""
with open(sys.argv[1], encoding="utf-8") as fh:
    for line in fh:
        try:
            row = json.loads(line)
        except json.JSONDecodeError:
            continue
        if row.get("result") == "snapshot":
            latest = str(row.get("detail") or "")
print(latest)
PY
}

write_loop_summary() {
    [ "$SUMMARY_WRITTEN" -eq 0 ] || return 0
    SUMMARY_WRITTEN=1
    mkdir -p "$(dirname "$REPORT_FILE")"
    REPORT_FILE="$REPORT_FILE" START_TS="$LOOP_STARTED_TS" LOOP_SUMMARY_FILE="$LOOP_SUMMARY_FILE" python3 <<'PY'
import json
import os
import time
from pathlib import Path

path = os.environ["REPORT_FILE"]
rows = []
if os.path.exists(path):
    with open(path, encoding="utf-8") as fh:
        for line in fh:
            try:
                rows.append(json.loads(line))
            except json.JSONDecodeError:
                pass

round_rows = [
    row for row in rows
    if row.get("scenario") == "loop" and row.get("result") in {"round_pass", "round_fail", "round_inconclusive"}
]
phase_rows = [
    row for row in rows
    if row.get("result") == "phase" and row.get("loop_round") is not None
]
durations = [float(row["duration_s"]) for row in round_rows if isinstance(row.get("duration_s"), (int, float))]
def percentile(values, pct):
    if not values:
        return None
    ordered = sorted(values)
    idx = int(round((len(ordered) - 1) * pct / 100.0))
    return ordered[idx]

summary = {
    "ts": time.time(),
    "scenario": "loop",
    "result": "summary",
    "duration_s": time.time() - float(os.environ["START_TS"]),
    "rounds": len(round_rows),
    "pass": sum(1 for row in round_rows if row.get("result") == "round_pass"),
    "fail": sum(1 for row in round_rows if row.get("result") == "round_fail"),
    "inconclusive": sum(1 for row in round_rows if row.get("result") == "round_inconclusive"),
    "duration_p50_s": percentile(durations, 50),
    "duration_p95_s": percentile(durations, 95),
    "by_selected_scenario": {},
    "by_phase": {},
}
for row in round_rows:
    name = row.get("selected_scenario") or "unknown"
    bucket = summary["by_selected_scenario"].setdefault(name, {"rounds": 0, "pass": 0, "fail": 0, "inconclusive": 0})
    bucket["rounds"] += 1
    if row.get("result") == "round_pass":
        bucket["pass"] += 1
    elif row.get("result") == "round_fail":
        bucket["fail"] += 1
    else:
        bucket["inconclusive"] += 1

phase_durations = {}
for row in phase_rows:
    phase = row.get("phase") or "unknown"
    bucket = summary["by_phase"].setdefault(phase, {"count": 0, "pass": 0, "fail": 0, "duration_p50_s": None, "duration_p95_s": None})
    bucket["count"] += 1
    if row.get("phase_result") == "pass":
        bucket["pass"] += 1
    elif row.get("phase_result") == "fail":
        bucket["fail"] += 1
    if isinstance(row.get("duration_s"), (int, float)):
        phase_durations.setdefault(phase, []).append(float(row["duration_s"]))

for phase, values in phase_durations.items():
    summary["by_phase"][phase]["duration_p50_s"] = percentile(values, 50)
    summary["by_phase"][phase]["duration_p95_s"] = percentile(values, 95)

with open(path, "a", encoding="utf-8") as fh:
    fh.write(json.dumps(summary, sort_keys=True) + "\n")
if os.environ.get("LOOP_SUMMARY_FILE"):
    summary_path = Path(os.environ["LOOP_SUMMARY_FILE"])
    summary_path.parent.mkdir(parents=True, exist_ok=True)
    summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
print(json.dumps(summary, sort_keys=True))
PY
}

pause_after_failure() {
    local detail="$1"
    record_loop_event paused "$round" "$ROUND_SCENARIO" "$ROUND_TARGET" "$ROUND_FAULT" "$ROUND_ASSERTIONS" "" "" "" "$(latest_snapshot_path)"
    if [ "$PAUSE_ON_FAILURE" != "1" ]; then
        return
    fi
    log "paused after failure: $detail"
    if [ "$LOOP_FREEZE_ON_FAILURE" = "1" ]; then
        log "freeze-on-failure is enabled; scenario cleanup will preserve fault state when possible"
    fi
    log "inspect the snapshot/report, then press Ctrl+C to stop or send USR1 to resume: kill -USR1 $$"
    PAUSED=1
    trap 'PAUSED=0; log "resume signal received"' USR1
    while [ "$PAUSED" -eq 1 ]; do
        sleep 5
    done
}

pick_validator() {
    cluster_py nodes --config "$CONFIG_FILE" --validators |
        python3 -c 'import random,sys; xs=[x.strip() for x in sys.stdin if x.strip()]; print(random.choice(xs) if xs else "")'
}

pick_two_validators() {
    cluster_py nodes --config "$CONFIG_FILE" --validators |
        python3 -c 'import random,sys; xs=[x.strip() for x in sys.stdin if x.strip()]; print("\n".join(random.sample(xs, min(2, len(xs)))))'
}

pick_weighted_scenario() {
    LOOP_SCENARIO_WEIGHTS="$LOOP_SCENARIO_WEIGHTS" python3 <<'PY'
import os
import random
import sys

items = []
for raw in os.environ["LOOP_SCENARIO_WEIGHTS"].replace(";", ",").split(","):
    raw = raw.strip()
    if not raw:
        continue
    if "=" in raw:
        name, weight = raw.split("=", 1)
    elif ":" in raw:
        name, weight = raw.split(":", 1)
    else:
        name, weight = raw, "1"
    try:
        weight_i = int(weight)
    except ValueError:
        print(f"invalid weight in LOOP_SCENARIO_WEIGHTS: {raw}", file=sys.stderr)
        raise SystemExit(2)
    if weight_i > 0:
        items.append((name.strip(), weight_i))
if not items:
    print("no enabled scenarios in LOOP_SCENARIO_WEIGHTS", file=sys.stderr)
    raise SystemExit(2)
total = sum(weight for _, weight in items)
pick = random.randint(1, total)
seen = 0
for name, weight in items:
    seen += weight
    if pick <= seen:
        print(name)
        break
PY
}

set_round() {
    local scenario="$1"
    ROUND_SCENARIO="$scenario"
    ROUND_TARGET=""
    ROUND_FAULT=""
    ROUND_ASSERTIONS=""
    ROUND_COMMAND=()

    local node nodes first second
    case "$scenario" in
        rolling-restart)
            ROUND_TARGET="validators"
            ROUND_FAULT="rolling-restart"
            ROUND_ASSERTIONS="oracle after each restart"
            ROUND_COMMAND=("$scenario" "$LOOP_ROLLING_WAIT")
            ;;
        flap)
            node="$(pick_validator)"
            [ -n "$node" ] || return 1
            ROUND_TARGET="$node"
            ROUND_FAULT="repeated stop/start"
            ROUND_ASSERTIONS="recovery oracle"
            ROUND_COMMAND=("$scenario" "$node" "$LOOP_FLAP_COUNT" "$LOOP_FLAP_INTERVAL")
            ;;
        kill-under-load)
            node="$(pick_validator)"
            [ -n "$node" ] || return 1
            ROUND_TARGET="$node"
            ROUND_FAULT="restart under managed load"
            ROUND_ASSERTIONS="recovery oracle and optional receipt checker"
            ROUND_COMMAND=("$scenario" "$node")
            ;;
        majority-failure)
            ROUND_TARGET="stake-weighted >1/3 validators"
            ROUND_FAULT="stop no-quorum victim set"
            ROUND_ASSERTIONS="remaining validators stall, then recovery oracle"
            ROUND_COMMAND=("$scenario")
            ;;
        partition-kill)
            nodes="$(pick_two_validators)"
            first="$(printf '%s\n' "$nodes" | sed -n '1p')"
            second="$(printf '%s\n' "$nodes" | sed -n '2p')"
            [ -n "$first" ] && [ -n "$second" ] || return 1
            ROUND_TARGET="partitioned=$first killed=$second"
            ROUND_FAULT="partition one validator and restart another"
            ROUND_ASSERTIONS="recovery oracle"
            ROUND_COMMAND=("$scenario" "$first" "$second")
            ;;
        partition-random)
            ROUND_TARGET="random-validator"
            ROUND_FAULT="single validator partition"
            ROUND_ASSERTIONS="majority advances, victim bounded, recovery oracle"
            ROUND_COMMAND=("$scenario" 1 "$LOOP_PARTITION_HOLD" "$LOOP_RECOVER_WAIT")
            ;;
        partition-asym)
            ROUND_TARGET="random-validator"
            ROUND_FAULT="asymmetric in/out partition"
            ROUND_ASSERTIONS="majority advances, recovery oracle"
            ROUND_COMMAND=("$scenario" 1 "$LOOP_PARTITION_HOLD" "$LOOP_RECOVER_WAIT" random random)
            ;;
        partition-2-2|partition-no-quorum-split)
            ROUND_TARGET="stake-aware no-quorum validator split"
            ROUND_FAULT="no-quorum split"
            ROUND_ASSERTIONS="both sides bounded, recovery oracle"
            if [ "$scenario" = "partition-2-2" ]; then
                ROUND_COMMAND=("partition-2-2" "$LOOP_PARTITION_HOLD")
            else
                ROUND_COMMAND=("partition-no-quorum-split" "$LOOP_PARTITION_HOLD")
            fi
            ;;
        partition-3-1|partition-majority-minority)
            ROUND_TARGET="stake-aware majority/minority split"
            ROUND_FAULT="majority/minority split"
            ROUND_ASSERTIONS="majority advances, minority bounded, recovery oracle"
            if [ "$scenario" = "partition-3-1" ]; then
                ROUND_COMMAND=("partition-3-1" "$LOOP_PARTITION_HOLD" random)
            else
                ROUND_COMMAND=("partition-majority-minority" "$LOOP_PARTITION_HOLD" random)
            fi
            ;;
        partition-load)
            ROUND_TARGET="stake-aware partition under load"
            ROUND_FAULT="partition under managed load"
            ROUND_ASSERTIONS="partition assertions, recovery oracle, optional receipt checker"
            ROUND_COMMAND=("$scenario" random "$LOOP_PARTITION_HOLD" "$LOOP_RECOVER_WAIT")
            ;;
        delay-load-spike)
            ROUND_TARGET="random-validator"
            ROUND_FAULT="network delay under managed load"
            ROUND_ASSERTIONS="periodic oracle, recovery oracle, optional receipt checker"
            ROUND_COMMAND=("$scenario" random "$LOOP_DELAY_DURATION" "$LOOP_DELAY_LATENCY" "$LOOP_DELAY_JITTER" "$LOOP_DELAY_ORACLE_INTERVAL")
            ;;
        *)
            log "unknown scenario in LOOP_SCENARIO_WEIGHTS: $scenario"
            return 1
            ;;
    esac
}

command_string() {
    printf '%q ' bash "$SCRIPT_DIR/scenarios.sh" --config "$CONFIG_FILE" "${ROUND_COMMAND[@]}"
}

run_round() {
    local started rc command
    started="$(date +%s)"
    command="$(command_string)"
    record_loop_event round_start "$round" "$ROUND_SCENARIO" "$ROUND_TARGET" "$ROUND_FAULT" "$ROUND_ASSERTIONS" "" "" "$command" ""
    log "round $round scenario=$ROUND_SCENARIO target=$ROUND_TARGET"
    if [ "$LOOP_DRY_RUN" = "1" ]; then
        log "dry-run command: $command"
        return 0
    fi

    set +e
    CHAOS_REPORT_FILE="$REPORT_FILE" \
    CHAOS_FREEZE_ON_FAILURE="$LOOP_FREEZE_ON_FAILURE" \
    CHAOS_LOOP_ROUND="$round" \
    CHAOS_LOOP_SELECTED_SCENARIO="$ROUND_SCENARIO" \
        bash "$SCRIPT_DIR/scenarios.sh" --config "$CONFIG_FILE" "${ROUND_COMMAND[@]}"
    rc=$?
    set -e
    ROUND_RC="$rc"
    ROUND_DURATION=$(( $(date +%s) - started ))
    return "$rc"
}

finish_round() {
    local result="$1"
    local rc="$2"
    local duration="$3"
    record_loop_event "$result" "$round" "$ROUND_SCENARIO" "$ROUND_TARGET" "$ROUND_FAULT" "$ROUND_ASSERTIONS" "$duration" "$rc" "$(command_string)" "$(latest_snapshot_path)"
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --config)
            CONFIG_FILE="$2"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown parameter: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

trap write_loop_summary EXIT

log "report file: $REPORT_FILE"
log "scenario weights: $LOOP_SCENARIO_WEIGHTS"
log "pause_on_failure=$PAUSE_ON_FAILURE freeze_on_failure=$LOOP_FREEZE_ON_FAILURE dry_run=$LOOP_DRY_RUN"

round=0
while true; do
    round=$((round + 1))
    if [ "$LOOP_MAX_ROUNDS" -gt 0 ] && [ "$round" -gt "$LOOP_MAX_ROUNDS" ]; then
        log "reached LOOP_MAX_ROUNDS=$LOOP_MAX_ROUNDS"
        break
    fi

    scenario="$(pick_weighted_scenario)"
    if ! set_round "$scenario"; then
        record_loop_event round_fail "$round" "$scenario" "" "selection failed" "" 0 2 "" ""
        pause_after_failure "round=$round scenario=$scenario selection failed"
        sleep "$LOOP_INTERVAL"
        continue
    fi

    ROUND_RC=0
    ROUND_DURATION=0
    if run_round; then
        if [ "$LOOP_DRY_RUN" = "1" ]; then
            ROUND_DURATION=0
        fi
        finish_round round_pass 0 "$ROUND_DURATION"
    else
        rc="$ROUND_RC"
        duration="$ROUND_DURATION"
        finish_round round_fail "$rc" "$duration"
        pause_after_failure "round=$round scenario=$ROUND_SCENARIO rc=$rc"
    fi

    sleep "$LOOP_INTERVAL"
done
