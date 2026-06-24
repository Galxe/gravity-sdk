#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CLUSTER_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
PROJECT_ROOT="$(cd "$CLUSTER_DIR/.." && pwd)"
CONFIG_FILE="${CLUSTER_TOML:-$CLUSTER_DIR/cluster.toml}"
REPORT_FILE="${CHAOS_REPORT_FILE:-$SCRIPT_DIR/reports/chaos-$(date +%Y%m%d-%H%M%S).jsonl}"
RECOVERY_TIMEOUT="${RECOVERY_TIMEOUT:-300}"
RECOVERY_INTERVAL="${RECOVERY_INTERVAL:-10}"
STALL_HOLD="${STALL_HOLD:-60}"
LOAD_CMD="${CHAOS_LOAD_CMD:-}"
CUSTOM_LOAD_PID=""
BENCH_PID=""
TX_PID=""
BENCH_LOG_FILE="${CHAOS_BENCH_LOG_FILE:-${REPORT_FILE%.jsonl}.gravity_bench.log}"
TX_HISTORY_FILE="${CHAOS_TX_HISTORY_FILE:-${CHAOS_HISTORY_FILE:-${REPORT_FILE%.jsonl}.history.jsonl}}"
CHAOS_START_TS="${CHAOS_START_TS:-$(date +%s)}"
SCENARIO_FAILED=0
if [ "${CHAOS_BACKEND:-}" = "docker" ]; then
    export CHAOS_DOCKER_RPC_NETWORK="${CHAOS_DOCKER_RPC_NETWORK:-gravity-chaos}"
fi

usage() {
    cat <<'EOF'
Usage:
  scenarios.sh [--config <cluster.toml>] <scenario> [args]

Scenarios:
  majority-failure              Stop >1/3 validator stake, assert stall, recover
  kill-under-load <node>        Restart one node while load is running
  flap <node> [count] [iv_s]    Repeatedly restart one node
  rolling-restart [wait_s]      Restart validators one by one
  partition-kill <partitioned> <killed>
                                Partition one node, restart another, heal, recover
  partition-random [rounds=3] [hold_s=180] [recover_s=60]
                                Randomly partition one quorum-safe validator, heal, repeat
  partition-no-quorum-split [hold_s=180] [left_csv] [right_csv]
                                Split validators into two no-quorum Docker partitions
  partition-2-2 [hold_s=180] [left_csv] [right_csv]
                                Backward-compatible alias for partition-no-quorum-split
  partition-majority-minority [hold_s=180] [minority_csv=random]
                                Split validators into majority quorum and minority side
  partition-3-1 [hold_s=180] [minority_node=random]
                                Backward-compatible alias for partition-majority-minority
  partition-asym [rounds=2] [hold_s=180] [recover_s=60] [direction=random] [node=random]
                                Directional in/out partition for one validator
  partition-load [mode=random] [hold_s=180] [recover_s=60]
                                Run managed load during single, no-quorum, or majority/minority partition
  delay-load-spike <node|random> [duration_s=300] [lat=200ms] [jitter=50ms] [interval_s=60]
                                Apply delay while managed load is running
  partition-mixed [duration_s=1800] [hold_s=180] [recover_s=60]
                                Longer randomized mix of single, no-quorum, majority/minority, and asym faults
  soak [duration_s] [interval_s]
                                Long steady-state oracle test, optional load

Environment:
  CHAOS_REPORT_FILE   JSONL report path
  CHAOS_LOAD_CMD      Optional load command for kill-under-load
                    Also used by soak when set
  CHAOS_BENCH_ENABLE 1 starts gravity_bench as background pressure
  CHAOS_BENCH_CONFIG gravity_bench config path; defaults to existing
                    GRAVITY_ARTIFACTS_DIR/faucet_bench_config.toml,
                    cluster/output/faucet_bench_config.toml, or a bench_config.toml
                    next to the cluster config
  CHAOS_BENCH_RECOVER
                    Default: 1; passes --recover to gravity_bench
  CHAOS_BENCH_LOG_FILE
                    Default: <report>.gravity_bench.log
  CHAOS_TX_ENABLE    1 starts the built-in EVM tx workload for kill-under-load/soak
  CHAOS_TX_HISTORY_FILE
                    JSONL tx/head/nemesis history path
  CHAOS_TX_PRIVATE_KEYS
                    Comma-separated funded EVM private keys for built-in workload
  RECOVERY_TIMEOUT    Default: 300
  RECOVERY_INTERVAL   Default: 10
  STALL_HOLD          Default: 60
  MAX_PARTITION_ADVANCE
                    Default: 10 for no-quorum split partitions
  CHAOS_DOCKER_RPC_NETWORK
                    Default: gravity-chaos when CHAOS_BACKEND=docker
EOF
}

log() {
    printf '[scenario] %s\n' "$*"
}

die() {
    printf '[scenario][error] %s\n' "$*" >&2
    exit 1
}

cluster_py() {
    python3 "$SCRIPT_DIR/lib/cluster.py" "$@"
}

chaos() {
    bash "$SCRIPT_DIR/chaos.sh" --config "$CONFIG_FILE" "$@"
}

oracle() {
    LOG_SINCE="$CHAOS_START_TS" bash "$SCRIPT_DIR/oracle.sh" --config "$CONFIG_FILE" "$@"
}

snapshot_failure() {
    local scenario="$1"
    local out_dir
    SCENARIO_FAILED=1
    out_dir="$(CHAOS_SNAPSHOT_DIR="${CHAOS_SNAPSHOT_DIR:-}" bash "$SCRIPT_DIR/snapshot.sh" --config "$CONFIG_FILE" 2>/dev/null || true)"
    if [ -n "$out_dir" ]; then
        record_json "$scenario" snapshot "$out_dir"
        log "snapshot: $out_dir"
    fi
}

record_json() {
    local scenario="$1"
    local result="$2"
    local detail="${3:-}"
    mkdir -p "$(dirname "$REPORT_FILE")"
    SCENARIO="$scenario" RESULT="$result" DETAIL="$detail" REPORT_FILE="$REPORT_FILE" \
    CHAOS_LOOP_ROUND="${CHAOS_LOOP_ROUND:-}" \
    CHAOS_LOOP_SELECTED_SCENARIO="${CHAOS_LOOP_SELECTED_SCENARIO:-}" python3 <<'PY'
import json
import os
import time

entry = {
    "ts": time.time(),
    "scenario": os.environ["SCENARIO"],
    "result": os.environ["RESULT"],
}
detail = os.environ.get("DETAIL") or ""
if detail:
    entry["detail"] = detail
if os.environ.get("CHAOS_LOOP_ROUND"):
    try:
        entry["loop_round"] = int(os.environ["CHAOS_LOOP_ROUND"])
    except ValueError:
        entry["loop_round"] = os.environ["CHAOS_LOOP_ROUND"]
if os.environ.get("CHAOS_LOOP_SELECTED_SCENARIO"):
    entry["loop_selected_scenario"] = os.environ["CHAOS_LOOP_SELECTED_SCENARIO"]
with open(os.environ["REPORT_FILE"], "a", encoding="utf-8") as fh:
    fh.write(json.dumps(entry, sort_keys=True) + "\n")
PY
}

record_phase() {
    local scenario="$1"
    local phase="$2"
    local phase_result="$3"
    local duration_s="$4"
    local detail="${5:-}"
    mkdir -p "$(dirname "$REPORT_FILE")"
    SCENARIO="$scenario" PHASE="$phase" PHASE_RESULT="$phase_result" \
    DURATION_S="$duration_s" DETAIL="$detail" REPORT_FILE="$REPORT_FILE" \
    CHAOS_LOOP_ROUND="${CHAOS_LOOP_ROUND:-}" \
    CHAOS_LOOP_SELECTED_SCENARIO="${CHAOS_LOOP_SELECTED_SCENARIO:-}" python3 <<'PY'
import json
import os
import time

entry = {
    "ts": time.time(),
    "scenario": os.environ["SCENARIO"],
    "result": "phase",
    "phase": os.environ["PHASE"],
    "phase_result": os.environ["PHASE_RESULT"],
}
try:
    entry["duration_s"] = float(os.environ["DURATION_S"])
except ValueError:
    entry["duration_s"] = os.environ["DURATION_S"]
detail = os.environ.get("DETAIL") or ""
if detail:
    entry["detail"] = detail
if os.environ.get("CHAOS_LOOP_ROUND"):
    try:
        entry["loop_round"] = int(os.environ["CHAOS_LOOP_ROUND"])
    except ValueError:
        entry["loop_round"] = os.environ["CHAOS_LOOP_ROUND"]
if os.environ.get("CHAOS_LOOP_SELECTED_SCENARIO"):
    entry["loop_selected_scenario"] = os.environ["CHAOS_LOOP_SELECTED_SCENARIO"]
with open(os.environ["REPORT_FILE"], "a", encoding="utf-8") as fh:
    fh.write(json.dumps(entry, sort_keys=True) + "\n")
PY
}

wait_oracle() {
    local deadline=$((SECONDS + RECOVERY_TIMEOUT))
    while [ "$SECONDS" -lt "$deadline" ]; do
        if oracle >/tmp/gravity-chaos-oracle.json 2>/tmp/gravity-chaos-oracle.err; then
            cat /tmp/gravity-chaos-oracle.json
            return 0
        fi
        sleep "$RECOVERY_INTERVAL"
    done
    cat /tmp/gravity-chaos-oracle.err >&2 || true
    cat /tmp/gravity-chaos-oracle.json >&2 || true
    return 1
}

timed_wait_oracle() {
    local scenario="$1"
    local phase="${2:-recovery-oracle}"
    local detail="${3:-}"
    local started=$SECONDS
    if wait_oracle >/dev/null; then
        record_phase "$scenario" "$phase" pass "$((SECONDS - started))" "$detail"
        return 0
    fi
    record_phase "$scenario" "$phase" fail "$((SECONDS - started))" "$detail"
    return 1
}

timed_heal() {
    local scenario="$1"
    shift || true
    local detail target started
    target="${*:-all}"
    detail="target=$target"
    started=$SECONDS
    if chaos heal "$@"; then
        record_phase "$scenario" heal pass "$((SECONDS - started))" "$detail"
        return 0
    fi
    record_phase "$scenario" heal fail "$((SECONDS - started))" "$detail"
    return 1
}

node_rpc_port() {
    cluster_py node-field --config "$CONFIG_FILE" "$1" rpc_port
}

node_rpc_host() {
    cluster_py node-field --config "$CONFIG_FILE" "$1" host
}

rpc_height() {
    local node="$1"
    local host port
    host="$(node_rpc_host "$node")"
    port="$(node_rpc_port "$node")"
    if [ "${CHAOS_BACKEND:-}" = "docker" ] && [ -n "${CHAOS_DOCKER_RPC_NETWORK:-}" ]; then
        docker run --rm --network "$CHAOS_DOCKER_RPC_NETWORK" \
            "${CHAOS_DOCKER_RPC_IMAGE:-curlimages/curl:8.10.1}" \
            -sS -m "${RPC_TIMEOUT:-3}" \
            -X POST -H 'Content-Type: application/json' \
            --data '{"jsonrpc":"2.0","method":"eth_blockNumber","id":1}' \
            "http://${host}:${port}" 2>/dev/null |
            jq -r '.result // empty'
        return
    fi
    curl -sS --noproxy '*' -m "${RPC_TIMEOUT:-3}" \
        -X POST -H 'Content-Type: application/json' \
        --data '{"jsonrpc":"2.0","method":"eth_blockNumber","id":1}' \
        "http://${host}:${port}" 2>/dev/null |
        jq -r '.result // empty'
}

height_dec() {
    local height="${1#0x}"
    printf '%d\n' "$((16#$height))"
}

docker_observation_container() {
    local node="$1"
    local candidate="${CHAOS_DOCKER_PREFIX:-}${node}"
    if docker inspect "$candidate" >/dev/null 2>&1; then
        printf '%s\n' "$candidate"
        return 0
    fi
    docker ps -a --format '{{.Names}}' | awk -v n="$node" '
        $0 == n { print; exit }
        $0 ~ "(^|[_-])" n "($|[_-])" { print; exit }
    '
}

docker_log_height_dec() {
    local node="$1"
    local container
    container="$(docker_observation_container "$node")"
    [ -n "$container" ] || return 1
    docker exec "$container" sh -lc \
        "grep -h 'Canonical chain committed number=' /gravity/data/execution_logs/dev/reth.log 2>/dev/null | tail -1 | sed -n 's/.*number=\\([0-9][0-9]*\\).*/\\1/p'"
}

observed_height_dec() {
    local node="$1"
    local height
    height="$(rpc_height "$node" || true)"
    if [ -n "$height" ]; then
        height_dec "$height"
        return 0
    fi
    if [ "${CHAOS_BACKEND:-}" = "docker" ]; then
        docker_log_height_dec "$node"
        return
    fi
    return 1
}

random_validator() {
    cluster_py nodes --config "$CONFIG_FILE" --validators |
        python3 -c 'import random,sys; ns=sys.stdin.read().split(); print(random.choice(ns))'
}

random_direction() {
    python3 -c 'import random; print(random.choice(["in", "out"]))'
}

choose_single_victim() {
    local node="${1:-}"
    if [ -n "$node" ] && [ "$node" != "random" ]; then
        cluster_py choose-single-victim --config "$CONFIG_FILE" --node "$node"
    else
        cluster_py choose-single-victim --config "$CONFIG_FILE"
    fi
}

choose_no_quorum_split() {
    local left="${1:-}"
    local right="${2:-}"
    if [ -n "$left" ] || [ -n "$right" ]; then
        cluster_py choose-no-quorum-split --config "$CONFIG_FILE" --left "$left" --right "$right"
    else
        cluster_py choose-no-quorum-split --config "$CONFIG_FILE"
    fi
}

choose_majority_minority_split() {
    local minority="${1:-}"
    if [ -n "$minority" ] && [ "$minority" != "random" ]; then
        cluster_py choose-majority-minority-split --config "$CONFIG_FILE" --minority "$minority"
    else
        cluster_py choose-majority-minority-split --config "$CONFIG_FILE"
    fi
}

cleanup_network_partitions() {
    if [ "${CHAOS_FREEZE_ON_FAILURE:-0}" = "1" ] && [ "$SCENARIO_FAILED" = "1" ]; then
        log "freeze-on-failure enabled; leaving network fault in place"
        return 0
    fi
    chaos heal >/dev/null 2>&1 || true
}

resolve_bench_config() {
    if [ -n "${CHAOS_BENCH_CONFIG:-}" ]; then
        [ -f "$CHAOS_BENCH_CONFIG" ] || die "CHAOS_BENCH_CONFIG not found: $CHAOS_BENCH_CONFIG"
        printf '%s\n' "$CHAOS_BENCH_CONFIG"
        return 0
    fi

    local candidates=()
    if [ -n "${GRAVITY_ARTIFACTS_DIR:-}" ]; then
        candidates+=("$GRAVITY_ARTIFACTS_DIR/faucet_bench_config.toml")
    fi
    candidates+=(
        "$CLUSTER_DIR/output/faucet_bench_config.toml"
        "$(dirname "$CONFIG_FILE")/bench_config.toml"
    )

    local candidate
    for candidate in "${candidates[@]}"; do
        if [ -f "$candidate" ]; then
            printf '%s\n' "$candidate"
            return 0
        fi
    done

    return 1
}

start_custom_load_if_configured() {
    if [ -n "$LOAD_CMD" ]; then
        log "starting load command: $LOAD_CMD"
        bash -lc "$LOAD_CMD" &
        CUSTOM_LOAD_PID="$!"
    fi
}

start_bench_if_configured() {
    if [ "${CHAOS_BENCH_ENABLE:-0}" != "1" ]; then
        return 0
    fi

    mkdir -p "$(dirname "$BENCH_LOG_FILE")"
    if [ -n "${CHAOS_BENCH_CMD:-}" ]; then
        log "starting gravity_bench command: $CHAOS_BENCH_CMD"
        log "gravity_bench log: $BENCH_LOG_FILE"
        bash -lc "$CHAOS_BENCH_CMD" >"$BENCH_LOG_FILE" 2>&1 &
        BENCH_PID="$!"
        return 0
    fi

    local bench_dir bench_config recover_arg
    bench_dir="${CHAOS_BENCH_DIR:-$PROJECT_ROOT/external/gravity_bench}"
    [ -d "$bench_dir" ] || die "gravity_bench dir not found: $bench_dir; set CHAOS_BENCH_DIR or CHAOS_BENCH_CMD"

    bench_config="$(resolve_bench_config)" || die "bench config not found; set CHAOS_BENCH_CONFIG or CHAOS_BENCH_CMD"
    bench_config="$(cd "$(dirname "$bench_config")" && pwd)/$(basename "$bench_config")"
    recover_arg=""
    if [ "${CHAOS_BENCH_RECOVER:-1}" = "1" ]; then
        recover_arg="--recover"
    fi

    log "starting gravity_bench: config=$bench_config recover=${CHAOS_BENCH_RECOVER:-1}"
    log "gravity_bench log: $BENCH_LOG_FILE"
    (
        cd "$bench_dir"
        if [ -n "${CHAOS_BENCH_BIN:-}" ]; then
            "$CHAOS_BENCH_BIN" --config "$bench_config" $recover_arg ${CHAOS_BENCH_EXTRA_ARGS:-}
        elif [ -x "$bench_dir/target/release/gravity_bench" ]; then
            "$bench_dir/target/release/gravity_bench" --config "$bench_config" $recover_arg ${CHAOS_BENCH_EXTRA_ARGS:-}
        else
            cargo run --release --bin gravity_bench -- --config "$bench_config" $recover_arg ${CHAOS_BENCH_EXTRA_ARGS:-}
        fi
    ) >"$BENCH_LOG_FILE" 2>&1 &
    BENCH_PID="$!"
}

start_tx_if_configured() {
    if [ "${CHAOS_TX_ENABLE:-0}" = "1" ]; then
        log "starting built-in tx workload: history=$TX_HISTORY_FILE"
        python3 "$SCRIPT_DIR/lib/tx_workload.py" \
            --config "$CONFIG_FILE" \
            --history-file "$TX_HISTORY_FILE" &
        TX_PID="$!"
    fi
}

start_load_if_configured() {
    start_custom_load_if_configured
    start_bench_if_configured
    start_tx_if_configured

    if [ -z "$CUSTOM_LOAD_PID" ] && [ -z "$BENCH_PID" ] && [ -z "$TX_PID" ]; then
        if pgrep -f '[g]ravity_bench' >/dev/null 2>&1; then
            log "detected existing gravity_bench load"
        else
            log "no CHAOS_LOAD_CMD and no gravity_bench detected; continuing without load"
        fi
    fi
}

stop_one_load() {
    local label="$1"
    local pid="$2"
    [ -n "$pid" ] || return 0
    log "stopping $label pid=$pid"
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
}

stop_load_if_started() {
    if [ -n "$CUSTOM_LOAD_PID" ]; then
        stop_one_load "load command" "$CUSTOM_LOAD_PID"
        CUSTOM_LOAD_PID=""
    fi
    if [ -n "$BENCH_PID" ]; then
        stop_one_load "gravity_bench" "$BENCH_PID"
        BENCH_PID=""
    fi
    if [ -n "$TX_PID" ]; then
        stop_one_load "built-in tx workload" "$TX_PID"
        TX_PID=""
    fi
}

check_one_load_alive() {
    local scenario="$1"
    local label="$2"
    local pid="$3"
    local detail="$4"
    local rc
    [ -n "$pid" ] || return 0
    if kill -0 "$pid" 2>/dev/null; then
        return 0
    fi
    set +e
    wait "$pid"
    rc=$?
    set -e
    record_json "$scenario" fail "$label exited early rc=$rc $detail"
    snapshot_failure "$scenario"
    return 1
}

check_loads_alive() {
    local scenario="$1"
    check_one_load_alive "$scenario" "load command" "$CUSTOM_LOAD_PID" "" || return 1
    check_one_load_alive "$scenario" "gravity_bench" "$BENCH_PID" "log=$BENCH_LOG_FILE" || return 1
    check_one_load_alive "$scenario" "built-in tx workload" "$TX_PID" "history=$TX_HISTORY_FILE" || return 1
}

check_receipts_if_configured() {
    local scenario="$1"
    local check_file status detail
    if [ "${CHAOS_TX_ENABLE:-0}" != "1" ]; then
        return 0
    fi
    log "checking built-in tx receipts: history=$TX_HISTORY_FILE"
    if ! python3 "$SCRIPT_DIR/lib/receipt_checker.py" \
        --config "$CONFIG_FILE" \
        --history-file "$TX_HISTORY_FILE" \
        --require-txs \
        --json > /tmp/gravity-chaos-receipt-check.json; then
        cat /tmp/gravity-chaos-receipt-check.json >&2 || true
        record_json "$scenario" fail "receipt checker failed history=$TX_HISTORY_FILE"
        snapshot_failure "$scenario"
        return 1
    fi
    cat /tmp/gravity-chaos-receipt-check.json
    check_file=/tmp/gravity-chaos-receipt-check.json
    status="$(python3 - "$check_file" <<'PY'
import json, sys
with open(sys.argv[1], encoding="utf-8") as fh:
    print(json.load(fh).get("status", "unknown"))
PY
)"
    if [ "$status" = "inconclusive" ]; then
        detail="$(python3 - "$check_file" <<'PY'
import json, sys
with open(sys.argv[1], encoding="utf-8") as fh:
    data = json.load(fh)
summary = data.get("history_summary", {})
print(
    "tx history inconclusive "
    f"submitted={summary.get('submitted')} receipts={summary.get('receipts')} "
    f"timeouts={summary.get('timeouts')} interrupted={summary.get('interrupted')}"
)
PY
)"
        record_json "$scenario" inconclusive "$detail"
        if [ "${CHAOS_TX_FAIL_ON_INCONCLUSIVE:-0}" = "1" ]; then
            snapshot_failure "$scenario"
            return 1
        fi
    fi
}

timed_check_receipts_if_configured() {
    local scenario="$1"
    local started=$SECONDS
    if [ "${CHAOS_TX_ENABLE:-0}" != "1" ]; then
        return 0
    fi
    if check_receipts_if_configured "$scenario" >/dev/null; then
        record_phase "$scenario" receipt-check pass "$((SECONDS - started))" "history=$TX_HISTORY_FILE"
        return 0
    fi
    record_phase "$scenario" receipt-check fail "$((SECONDS - started))" "history=$TX_HISTORY_FILE"
    return 1
}

scenario_majority_failure() {
    local victims victim_csv
    victims="$(cluster_py majority-victims --config "$CONFIG_FILE")"
    [ -n "$victims" ] || die "no majority victims selected"
    victim_csv="${victims// /,}"

    log "pre-check oracle"
    oracle >/dev/null

    log "stopping >1/3 validator stake: $victims"
    for node in $victims; do
        chaos kill "$node"
    done

    log "asserting remaining validators stall for ${STALL_HOLD}s"
    if ! cluster_py assert-stalled --config "$CONFIG_FILE" --exclude "$victim_csv" --hold "$STALL_HOLD"; then
        record_json majority-failure fail "remaining validators advanced without quorum"
        snapshot_failure majority-failure
        return 1
    fi

    log "recovering victims: $victims"
    for node in $victims; do
        chaos start "$node"
    done

    log "waiting for recovery oracle"
    if ! timed_wait_oracle majority-failure recovery-oracle "victims=$victims"; then
        record_json majority-failure fail "recovery oracle failed"
        snapshot_failure majority-failure
        return 1
    fi
    record_json majority-failure pass "victims=$victims"
}

scenario_kill_under_load() {
    local node="$1"
    [ -n "$node" ] || die "kill-under-load requires <node>"
    log "pre-check oracle"
    oracle >/dev/null

    start_load_if_configured
    trap stop_load_if_started EXIT

    log "restarting $node under load"
    chaos restart "$node" 30
    if ! check_loads_alive kill-under-load; then
        return 1
    fi

    log "waiting for recovery oracle"
    if ! timed_wait_oracle kill-under-load recovery-oracle "node=$node"; then
        record_json kill-under-load fail "node=$node recovery oracle failed"
        snapshot_failure kill-under-load
        return 1
    fi
    if ! check_loads_alive kill-under-load; then
        return 1
    fi
    stop_load_if_started
    if ! timed_check_receipts_if_configured kill-under-load; then
        return 1
    fi
    record_json kill-under-load pass "node=$node"
}

scenario_flap() {
    local node="$1"
    local count="${2:-10}"
    local interval="${3:-10}"
    [ -n "$node" ] || die "flap requires <node> [count] [iv_s]"

    log "pre-check oracle"
    oracle >/dev/null

    for i in $(seq 1 "$count"); do
        log "flap $node cycle $i/$count"
        chaos kill "$node"
        remaining=$((deadline - SECONDS))
        if [ "$remaining" -gt 0 ]; then
            if [ "$remaining" -lt "$interval" ]; then
                sleep "$remaining"
            else
                sleep "$interval"
            fi
        fi
        chaos start "$node"
        sleep "$interval"
    done

    log "waiting for recovery oracle"
    if ! timed_wait_oracle flap recovery-oracle "node=$node"; then
        record_json flap fail "node=$node recovery oracle failed"
        snapshot_failure flap
        return 1
    fi
    record_json flap pass "node=$node count=$count interval=$interval"
}

scenario_rolling_restart() {
    local wait_s="${1:-30}"
    local validators
    validators="$(cluster_py nodes --config "$CONFIG_FILE" --validators)"
    [ -n "$validators" ] || die "no validators found"

    log "pre-check oracle"
    oracle >/dev/null

    for node in $validators; do
        log "rolling restart $node"
        chaos restart "$node" "$wait_s"
        if ! timed_wait_oracle rolling-restart recovery-oracle "node=$node"; then
            record_json rolling-restart fail "node=$node recovery oracle failed"
            snapshot_failure rolling-restart
            return 1
        fi
    done

    record_json rolling-restart pass "wait_s=$wait_s"
}

scenario_partition_kill() {
    local partitioned="$1"
    local killed="$2"
    [ -n "$partitioned" ] && [ -n "$killed" ] || die "partition-kill requires <partitioned> <killed>"

    log "pre-check oracle"
    oracle >/dev/null

    log "partitioning $partitioned"
    chaos partition "$partitioned"

    log "restarting $killed while $partitioned is partitioned"
    chaos restart "$killed" 30

    log "healing network"
    timed_heal partition-kill

    log "waiting for recovery oracle"
    if ! timed_wait_oracle partition-kill recovery-oracle "partitioned=$partitioned killed=$killed"; then
        record_json partition-kill fail "partitioned=$partitioned killed=$killed recovery oracle failed"
        snapshot_failure partition-kill
        return 1
    fi

    record_json partition-kill pass "partitioned=$partitioned killed=$killed"
}

scenario_partition_random() {
    local rounds="${1:-3}"
    local hold_s="${2:-180}"
    local recover_s="${3:-60}"
    local max_advance="${MAX_PARTITION_ADVANCE:-10}"
    local round victim probe majority_csv victim_stake majority_stake total_stake plan before_probe before_probe_dec before_victim after_probe after_probe_dec after_victim after_victim_dec before_victim_dec

    log "pre-check oracle"
    oracle >/dev/null
    trap cleanup_network_partitions EXIT

    for round in $(seq 1 "$rounds"); do
        plan="$(choose_single_victim)" || die "failed to choose quorum-safe partition victim"
        IFS=$'\t' read -r victim probe majority_csv victim_stake majority_stake total_stake <<< "$plan"
        [ -n "$victim" ] && [ -n "$probe" ] || die "failed to parse victim/probe validator"

        before_probe="$(rpc_height "$probe")"
        before_victim="$(rpc_height "$victim")"
        before_probe_dec="$(height_dec "$before_probe")"
        before_victim_dec="$(observed_height_dec "$victim")"

        log "round $round/$rounds partition victim=$victim hold=${hold_s}s probe=$probe majority=[$majority_csv] stake=${majority_stake}/${total_stake}"
        chaos partition "$victim"
        sleep "$hold_s"

        after_probe="$(rpc_height "$probe")"
        after_probe_dec="$(height_dec "$after_probe")"
        if [ "$after_probe_dec" -le "$before_probe_dec" ]; then
            record_json partition-random fail "round=$round victim=$victim majority did not advance"
            snapshot_failure partition-random
            return 1
        fi

        after_victim="$(rpc_height "$victim" || true)"
        after_victim_dec="$(observed_height_dec "$victim" || true)"
        if [ -n "$after_victim_dec" ]; then
            if [ $((after_victim_dec - before_victim_dec)) -gt "$max_advance" ]; then
                record_json partition-random fail "round=$round victim=$victim advanced while partitioned"
                snapshot_failure partition-random
                return 1
            fi
            if [ -n "$after_victim" ]; then
                log "victim=$victim reachable while partitioned height=$after_victim delta=$((after_victim_dec - before_victim_dec))"
            else
                log "victim=$victim rpc unreachable while partitioned observed_log_height=$after_victim_dec delta=$((after_victim_dec - before_victim_dec))"
            fi
        else
            log "victim=$victim unreachable while partitioned and no log height observed"
        fi

        log "healing victim=$victim"
        if ! timed_heal partition-random "$victim"; then
            record_json partition-random fail "round=$round victim=$victim heal failed"
            snapshot_failure partition-random
            return 1
        fi
        if ! timed_wait_oracle partition-random recovery-oracle "round=$round victim=$victim"; then
            record_json partition-random fail "round=$round victim=$victim recovery oracle failed"
            snapshot_failure partition-random
            return 1
        fi
        sleep "$recover_s"
        record_json partition-random pass "round=$round victim=$victim majority=$majority_csv victim_stake=$victim_stake majority_stake=$majority_stake total_stake=$total_stake hold_s=$hold_s"
    done

    trap - EXIT
}

scenario_partition_no_quorum_split() {
    local scenario_name="$1"
    shift
    local hold_s="${1:-180}"
    local left_arg="${2:-}"
    local right_arg="${3:-}"
    local max_advance="${MAX_PARTITION_ADVANCE:-10}"
    local node before after before_dec after_dec delta failed before_file plan left_csv right_csv left_stake right_stake total_stake

    log "pre-check oracle"
    oracle >/dev/null
    trap cleanup_network_partitions EXIT

    plan="$(choose_no_quorum_split "$left_arg" "$right_arg")" || die "failed to choose no-quorum split"
    IFS=$'\t' read -r left_csv right_csv left_stake right_stake total_stake <<< "$plan"
    [ -n "$left_csv" ] && [ -n "$right_csv" ] || die "failed to parse no-quorum split"

    before_file="$(mktemp "${TMPDIR:-/tmp}/gravity-partition-2-2-before.XXXXXX")"
    for node in ${left_csv//,/ } ${right_csv//,/ }; do
        before="$(observed_height_dec "$node")"
        printf '%s %s\n' "$node" "$before" >> "$before_file"
        log "before $node=$before"
    done

    log "no-quorum split left=[$left_csv] right=[$right_csv] stake=${left_stake}/${right_stake}/${total_stake} hold=${hold_s}s"
    chaos partition-split "$left_csv" "$right_csv"
    sleep "$hold_s"

    failed=0
    for node in ${left_csv//,/ } ${right_csv//,/ }; do
        before="$(awk -v node="$node" '$1 == node {print $2; exit}' "$before_file")"
        after="$(observed_height_dec "$node" || true)"
        if [ -z "$after" ]; then
            log "during split $node height unavailable"
            failed=1
            continue
        fi
        before_dec="$before"
        after_dec="$after"
        delta=$((after_dec - before_dec))
        log "during split $node=$after delta=$delta"
        if [ "$delta" -gt "$max_advance" ]; then
            failed=1
        fi
    done

    log "healing no-quorum split"
    if ! timed_heal "$scenario_name"; then
        record_json "$scenario_name" fail "heal failed"
        snapshot_failure "$scenario_name"
        return 1
    fi

    if [ "$failed" -ne 0 ]; then
        record_json "$scenario_name" fail "split advanced too far or became unreadable left=$left_csv right=$right_csv"
        snapshot_failure "$scenario_name"
        return 1
    fi

    if ! timed_wait_oracle "$scenario_name" recovery-oracle "left=$left_csv right=$right_csv"; then
        record_json "$scenario_name" fail "recovery oracle failed"
        snapshot_failure "$scenario_name"
        return 1
    fi

    record_json "$scenario_name" pass "left=$left_csv right=$right_csv left_stake=$left_stake right_stake=$right_stake total_stake=$total_stake hold_s=$hold_s"
    rm -f "$before_file"
    trap - EXIT
}

scenario_partition_2_2() {
    scenario_partition_no_quorum_split partition-2-2 "$@"
}

scenario_partition_majority_minority() {
    local scenario_name="$1"
    shift
    local hold_s="${1:-180}"
    local minority_arg="${2:-}"
    local max_advance="${MAX_PARTITION_ADVANCE:-10}"
    local majority_csv minority_csv majority_stake minority_stake total_stake plan before_majority before_majority_dec after_majority after_majority_dec after_minority_dec delta probe node before before_file

    log "pre-check oracle"
    oracle >/dev/null
    trap cleanup_network_partitions EXIT

    plan="$(choose_majority_minority_split "$minority_arg")" || die "failed to choose majority/minority split"
    IFS=$'\t' read -r majority_csv minority_csv probe majority_stake minority_stake total_stake <<< "$plan"
    [ -n "$majority_csv" ] && [ -n "$minority_csv" ] && [ -n "$probe" ] || die "failed to parse majority/minority split"

    before_majority_dec="$(observed_height_dec "$probe")"
    before_file="$(mktemp "${TMPDIR:-/tmp}/gravity-majority-minority-before.XXXXXX")"
    for node in ${minority_csv//,/ }; do
        before="$(observed_height_dec "$node")"
        printf '%s %s\n' "$node" "$before" >> "$before_file"
        log "before minority $node=$before"
    done

    log "majority/minority split majority=[$majority_csv] minority=[$minority_csv] stake=${majority_stake}/${minority_stake}/${total_stake} hold=${hold_s}s"
    chaos partition-split "$majority_csv" "$minority_csv"
    sleep "$hold_s"

    after_majority_dec="$(observed_height_dec "$probe" || true)"
    if [ -z "$after_majority_dec" ]; then
        record_json "$scenario_name" fail "majority probe height unavailable probe=$probe minority=$minority_csv"
        snapshot_failure "$scenario_name"
        return 1
    fi
    if [ "$after_majority_dec" -le "$before_majority_dec" ]; then
        record_json "$scenario_name" fail "majority did not advance probe=$probe minority=$minority_csv"
        snapshot_failure "$scenario_name"
        return 1
    fi

    for node in ${minority_csv//,/ }; do
        before="$(awk -v node="$node" '$1 == node {print $2; exit}' "$before_file")"
        after_minority_dec="$(observed_height_dec "$node" || true)"
        if [ -n "$after_minority_dec" ]; then
            delta=$((after_minority_dec - before))
            log "minority=$node observed_height=$after_minority_dec delta=$delta"
            if [ "$delta" -gt "$max_advance" ]; then
                record_json "$scenario_name" fail "minority=$node advanced too far delta=$delta"
                snapshot_failure "$scenario_name"
                return 1
            fi
        else
            log "minority=$node height unavailable during split"
        fi
    done

    log "healing majority/minority split"
    if ! timed_heal "$scenario_name"; then
        record_json "$scenario_name" fail "heal failed minority=$minority_csv"
        snapshot_failure "$scenario_name"
        return 1
    fi
    if ! timed_wait_oracle "$scenario_name" recovery-oracle "majority=$majority_csv minority=$minority_csv"; then
        record_json "$scenario_name" fail "recovery oracle failed minority=$minority_csv"
        snapshot_failure "$scenario_name"
        return 1
    fi

    record_json "$scenario_name" pass "majority=$majority_csv minority=$minority_csv majority_stake=$majority_stake minority_stake=$minority_stake total_stake=$total_stake hold_s=$hold_s"
    rm -f "$before_file"
    trap - EXIT
}

scenario_partition_3_1() {
    scenario_partition_majority_minority partition-3-1 "$@"
}

scenario_partition_asym() {
    local rounds="${1:-2}"
    local hold_s="${2:-180}"
    local recover_s="${3:-60}"
    local direction_arg="${4:-random}"
    local fixed_node="${5:-random}"
    local max_advance="${MAX_PARTITION_ADVANCE:-10}"
    local round victim direction probe majority_csv victim_stake majority_stake total_stake plan before_probe before_probe_dec before_victim_dec after_probe after_probe_dec after_victim_dec delta

    log "pre-check oracle"
    oracle >/dev/null
    trap cleanup_network_partitions EXIT

    for round in $(seq 1 "$rounds"); do
        plan="$(choose_single_victim "$fixed_node")" || die "failed to choose quorum-safe asym partition victim"
        IFS=$'\t' read -r victim probe majority_csv victim_stake majority_stake total_stake <<< "$plan"
        [ -n "$victim" ] && [ -n "$probe" ] || die "failed to parse asym partition target/probe"
        if [ "$direction_arg" = "random" ] || [ -z "$direction_arg" ]; then
            direction="$(random_direction)"
        else
            direction="$direction_arg"
        fi
        case "$direction" in
            in|out) ;;
            *) die "partition-asym direction must be random, in, or out" ;;
        esac
        before_probe="$(rpc_height "$probe")"
        before_probe_dec="$(height_dec "$before_probe")"
        before_victim_dec="$(observed_height_dec "$victim")"

        log "round $round/$rounds asym partition victim=$victim direction=$direction hold=${hold_s}s probe=$probe majority=[$majority_csv] stake=${majority_stake}/${total_stake}"
        chaos partition-asym "$victim" "$direction"
        sleep "$hold_s"

        after_probe="$(rpc_height "$probe")"
        after_probe_dec="$(height_dec "$after_probe")"
        if [ "$after_probe_dec" -le "$before_probe_dec" ]; then
            record_json partition-asym fail "round=$round victim=$victim direction=$direction majority did not advance"
            snapshot_failure partition-asym
            return 1
        fi

        after_victim_dec="$(observed_height_dec "$victim" || true)"
        if [ "$direction" = "in" ] && [ -n "$after_victim_dec" ]; then
            delta=$((after_victim_dec - before_victim_dec))
            log "victim=$victim direction=in observed_height=$after_victim_dec delta=$delta"
            if [ "$delta" -gt "$max_advance" ]; then
                record_json partition-asym fail "round=$round victim=$victim inbound partition advanced too far delta=$delta"
                snapshot_failure partition-asym
                return 1
            fi
        elif [ -n "$after_victim_dec" ]; then
            log "victim=$victim direction=$direction observed_height=$after_victim_dec"
        else
            log "victim=$victim direction=$direction height unavailable during partition"
        fi

        log "healing asym partition victim=$victim"
        if ! timed_heal partition-asym "$victim"; then
            record_json partition-asym fail "round=$round victim=$victim direction=$direction heal failed"
            snapshot_failure partition-asym
            return 1
        fi
        if ! timed_wait_oracle partition-asym recovery-oracle "round=$round victim=$victim direction=$direction"; then
            record_json partition-asym fail "round=$round victim=$victim direction=$direction recovery oracle failed"
            snapshot_failure partition-asym
            return 1
        fi
        sleep "$recover_s"
        record_json partition-asym pass "round=$round victim=$victim direction=$direction majority=$majority_csv victim_stake=$victim_stake majority_stake=$majority_stake total_stake=$total_stake hold_s=$hold_s"
    done

    trap - EXIT
}

scenario_partition_load() {
    local mode="${1:-random}"
    local hold_s="${2:-180}"
    local recover_s="${3:-60}"
    local max_advance="${MAX_PARTITION_ADVANCE:-10}"
    local victim probe plan left_csv right_csv majority_csv minority_csv before_probe before_probe_dec before_victim_dec after_probe after_probe_dec after_victim_dec delta failed node before_file before after left_stake right_stake majority_stake minority_stake total_stake victim_stake

    log "pre-check oracle"
    oracle >/dev/null
    if [ "${CHAOS_TX_ENABLE:-0}" = "1" ] && [ -z "${CHAOS_TX_RECEIPT_TIMEOUT:-}" ]; then
        export CHAOS_TX_RECEIPT_TIMEOUT=$((hold_s + recover_s + 60))
        log "defaulting CHAOS_TX_RECEIPT_TIMEOUT=${CHAOS_TX_RECEIPT_TIMEOUT}s for partition-load"
    fi
    start_load_if_configured
    trap 'cleanup_network_partitions; stop_load_if_started' EXIT

    case "$mode" in
        random|single)
            plan="$(choose_single_victim)" || die "failed to choose quorum-safe partition-load victim"
            IFS=$'\t' read -r victim probe majority_csv victim_stake majority_stake total_stake <<< "$plan"
            before_probe_dec="$(observed_height_dec "$probe")"
            before_victim_dec="$(observed_height_dec "$victim")"
            log "partition-load mode=single victim=$victim majority=[$majority_csv] stake=${majority_stake}/${total_stake} hold=${hold_s}s"
            chaos partition "$victim"
            sleep "$hold_s"
            check_loads_alive partition-load || return 1
            after_probe_dec="$(observed_height_dec "$probe" || true)"
            if [ -z "$after_probe_dec" ]; then
                record_json partition-load fail "mode=single majority probe height unavailable victim=$victim"
                snapshot_failure partition-load
                return 1
            fi
            if [ "$after_probe_dec" -le "$before_probe_dec" ]; then
                record_json partition-load fail "mode=single victim=$victim majority did not advance"
                snapshot_failure partition-load
                return 1
            fi
            after_victim_dec="$(observed_height_dec "$victim" || true)"
            if [ -n "$after_victim_dec" ]; then
                delta=$((after_victim_dec - before_victim_dec))
                log "victim=$victim observed_height=$after_victim_dec delta=$delta"
                if [ "$delta" -gt "$max_advance" ]; then
                    record_json partition-load fail "mode=single victim=$victim advanced too far delta=$delta"
                    snapshot_failure partition-load
                    return 1
                fi
            fi
            ;;
        2-2|split|no-quorum|no-quorum-split)
            plan="$(choose_no_quorum_split)" || die "failed to choose no-quorum partition-load split"
            IFS=$'\t' read -r left_csv right_csv left_stake right_stake total_stake <<< "$plan"
            before_file="$(mktemp "${TMPDIR:-/tmp}/gravity-partition-load-2-2-before.XXXXXX")"
            for node in ${left_csv//,/ } ${right_csv//,/ }; do
                before="$(observed_height_dec "$node")"
                printf '%s %s\n' "$node" "$before" >> "$before_file"
            done
            log "partition-load mode=no-quorum left=[$left_csv] right=[$right_csv] stake=${left_stake}/${right_stake}/${total_stake} hold=${hold_s}s"
            chaos partition-split "$left_csv" "$right_csv"
            sleep "$hold_s"
            check_loads_alive partition-load || return 1
            failed=0
            for node in ${left_csv//,/ } ${right_csv//,/ }; do
                before="$(awk -v node="$node" '$1 == node {print $2; exit}' "$before_file")"
                after="$(observed_height_dec "$node" || true)"
                if [ -z "$after" ]; then
                    failed=1
                    continue
                fi
                delta=$((after - before))
                log "no-quorum under load $node height=$after delta=$delta"
                if [ "$delta" -gt "$max_advance" ]; then
                    failed=1
                fi
            done
            rm -f "$before_file"
            if [ "$failed" -ne 0 ]; then
                record_json partition-load fail "mode=no-quorum split advanced too far or became unreadable left=$left_csv right=$right_csv"
                snapshot_failure partition-load
                return 1
            fi
            ;;
        3-1|majority-minority)
            plan="$(choose_majority_minority_split)" || die "failed to choose partition-load majority/minority split"
            IFS=$'\t' read -r majority_csv minority_csv probe majority_stake minority_stake total_stake <<< "$plan"
            probe="${majority_csv%%,*}"
            before_probe_dec="$(observed_height_dec "$probe")"
            before_file="$(mktemp "${TMPDIR:-/tmp}/gravity-partition-load-majority-minority-before.XXXXXX")"
            for node in ${minority_csv//,/ }; do
                before="$(observed_height_dec "$node")"
                printf '%s %s\n' "$node" "$before" >> "$before_file"
            done
            log "partition-load mode=majority-minority majority=[$majority_csv] minority=[$minority_csv] stake=${majority_stake}/${minority_stake}/${total_stake} hold=${hold_s}s"
            chaos partition-split "$majority_csv" "$minority_csv"
            sleep "$hold_s"
            check_loads_alive partition-load || return 1
            after_probe_dec="$(observed_height_dec "$probe" || true)"
            if [ -z "$after_probe_dec" ]; then
                record_json partition-load fail "mode=majority-minority majority probe height unavailable minority=$minority_csv"
                snapshot_failure partition-load
                return 1
            fi
            if [ "$after_probe_dec" -le "$before_probe_dec" ]; then
                record_json partition-load fail "mode=majority-minority majority did not advance minority=$minority_csv"
                snapshot_failure partition-load
                return 1
            fi
            for node in ${minority_csv//,/ }; do
                before="$(awk -v node="$node" '$1 == node {print $2; exit}' "$before_file")"
                after_victim_dec="$(observed_height_dec "$node" || true)"
                if [ -n "$after_victim_dec" ]; then
                    delta=$((after_victim_dec - before))
                    log "majority-minority under load minority=$node height=$after_victim_dec delta=$delta"
                    if [ "$delta" -gt "$max_advance" ]; then
                        record_json partition-load fail "mode=majority-minority minority=$node advanced too far delta=$delta"
                        snapshot_failure partition-load
                        return 1
                    fi
                fi
            done
            rm -f "$before_file"
            ;;
        *)
            die "partition-load mode must be random, single, no-quorum, 2-2, majority-minority, or 3-1"
            ;;
    esac

    log "healing partition-load mode=$mode"
    if ! timed_heal partition-load; then
        record_json partition-load fail "mode=$mode heal failed"
        snapshot_failure partition-load
        return 1
    fi
    if ! timed_wait_oracle partition-load recovery-oracle "mode=$mode"; then
        record_json partition-load fail "mode=$mode recovery oracle failed"
        snapshot_failure partition-load
        return 1
    fi
    sleep "$recover_s"
    stop_load_if_started
    if ! timed_check_receipts_if_configured partition-load; then
        return 1
    fi
    record_json partition-load pass "mode=$mode hold_s=$hold_s recover_s=$recover_s"
    trap - EXIT
}

scenario_delay_load_spike() {
    local node="${1:-random}"
    local duration="${2:-300}"
    local latency="${3:-200ms}"
    local jitter="${4:-50ms}"
    local interval="${5:-60}"
    local deadline round remaining sleep_s

    if [ "$node" = "random" ] || [ -z "$node" ]; then
        node="$(random_validator)"
    fi

    log "pre-check oracle"
    oracle >/dev/null
    start_load_if_configured
    trap 'cleanup_network_partitions; stop_load_if_started' EXIT

    log "delay-load-spike node=$node duration=${duration}s latency=$latency jitter=$jitter interval=${interval}s"
    if [ "${CHAOS_BACKEND:-}" = "docker" ]; then
        chaos delay "$node" "$latency" "$jitter"
    else
        chaos delay "$latency" "$jitter"
    fi

    deadline=$((SECONDS + duration))
    round=0
    while [ "$SECONDS" -lt "$deadline" ]; do
        remaining=$((deadline - SECONDS))
        sleep_s="$interval"
        if [ "$remaining" -lt "$sleep_s" ]; then
            sleep_s="$remaining"
        fi
        [ "$sleep_s" -gt 0 ] && sleep "$sleep_s"
        round=$((round + 1))
        check_loads_alive delay-load-spike || return 1
        if oracle >/tmp/gravity-chaos-delay-oracle.json 2>/tmp/gravity-chaos-delay-oracle.err; then
            record_json delay-load-spike pass "round=$round node=$node"
        else
            record_json delay-load-spike fail "round=$round node=$node oracle failed during delay"
            cat /tmp/gravity-chaos-delay-oracle.err >&2 || true
            cat /tmp/gravity-chaos-delay-oracle.json >&2 || true
            snapshot_failure delay-load-spike
            return 1
        fi
    done

    log "healing delay-load-spike node=$node"
    if [ "${CHAOS_BACKEND:-}" = "docker" ]; then
        if ! timed_heal delay-load-spike "$node"; then
            record_json delay-load-spike fail "node=$node heal failed"
            snapshot_failure delay-load-spike
            return 1
        fi
    else
        if ! timed_heal delay-load-spike; then
            record_json delay-load-spike fail "node=$node heal failed"
            snapshot_failure delay-load-spike
            return 1
        fi
    fi
    if ! timed_wait_oracle delay-load-spike recovery-oracle "node=$node"; then
        record_json delay-load-spike fail "node=$node recovery oracle failed"
        snapshot_failure delay-load-spike
        return 1
    fi
    stop_load_if_started
    if ! timed_check_receipts_if_configured delay-load-spike; then
        return 1
    fi
    record_json delay-load-spike complete "node=$node duration=$duration latency=$latency jitter=$jitter rounds=$round"
    trap - EXIT
}

scenario_partition_mixed() {
    local duration="${1:-1800}"
    local hold_s="${2:-180}"
    local recover_s="${3:-60}"
    local deadline=$((SECONDS + duration))
    local round=0
    local choice

    log "mixed partition duration=${duration}s hold=${hold_s}s recover=${recover_s}s"
    while [ "$SECONDS" -lt "$deadline" ]; do
        round=$((round + 1))
        if [ "${CHAOS_BACKEND:-}" = "docker" ]; then
            choice="$(python3 -c 'import random; print(random.randrange(4))')"
        else
            choice="$(python3 -c 'import random; print(random.randrange(2))')"
        fi
        case "$choice" in
            0)
                log "mixed round $round: random single-node partition"
                scenario_partition_random 1 "$hold_s" "$recover_s"
                ;;
            1)
                log "mixed round $round: asymmetric partition"
                scenario_partition_asym 1 "$hold_s" "$recover_s" random random
                ;;
            2)
                log "mixed round $round: no-quorum split"
                scenario_partition_no_quorum_split partition-no-quorum-split "$hold_s"
                sleep "$recover_s"
                ;;
            3)
                log "mixed round $round: majority/minority split"
                scenario_partition_majority_minority partition-majority-minority "$hold_s" random
                sleep "$recover_s"
                ;;
        esac
    done
    record_json partition-mixed complete "duration=$duration hold_s=$hold_s recover_s=$recover_s rounds=$round"
}

scenario_soak() {
    local duration="${1:-21600}"
    local interval="${2:-60}"
    local deadline=$((SECONDS + duration))
    local round=0

    log "soak duration=${duration}s interval=${interval}s"
    start_load_if_configured
    trap stop_load_if_started EXIT

    while [ "$SECONDS" -lt "$deadline" ]; do
        round=$((round + 1))
        log "soak oracle round $round"
        if oracle >/tmp/gravity-chaos-soak-oracle.json 2>/tmp/gravity-chaos-soak-oracle.err; then
            record_json soak pass "round=$round"
        else
            record_json soak fail "round=$round oracle failed"
            cat /tmp/gravity-chaos-soak-oracle.err >&2 || true
            cat /tmp/gravity-chaos-soak-oracle.json >&2 || true
            snapshot_failure soak
            return 1
        fi
        if ! check_loads_alive soak; then
            return 1
        fi
        sleep "$interval"
    done

    stop_load_if_started
    if ! timed_check_receipts_if_configured soak; then
        return 1
    fi
    record_json soak complete "duration=$duration interval=$interval rounds=$round"
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
            break
            ;;
    esac
done

[ "$#" -gt 0 ] || { usage; exit 1; }
scenario="$1"
shift

log "report file: $REPORT_FILE"

case "$scenario" in
    majority-failure)
        scenario_majority_failure "$@"
        ;;
    kill-under-load)
        scenario_kill_under_load "$@"
        ;;
    flap)
        scenario_flap "$@"
        ;;
    rolling-restart)
        scenario_rolling_restart "$@"
        ;;
    partition-kill)
        scenario_partition_kill "$@"
        ;;
    partition-random)
        scenario_partition_random "$@"
        ;;
    partition-2-2)
        scenario_partition_2_2 "$@"
        ;;
    partition-no-quorum-split)
        scenario_partition_no_quorum_split partition-no-quorum-split "$@"
        ;;
    partition-3-1)
        scenario_partition_3_1 "$@"
        ;;
    partition-majority-minority)
        scenario_partition_majority_minority partition-majority-minority "$@"
        ;;
    partition-asym)
        scenario_partition_asym "$@"
        ;;
    partition-load)
        scenario_partition_load "$@"
        ;;
    delay-load-spike)
        scenario_delay_load_spike "$@"
        ;;
    partition-mixed)
        scenario_partition_mixed "$@"
        ;;
    soak)
        scenario_soak "$@"
        ;;
    *)
        die "unknown scenario: $scenario"
        ;;
esac
