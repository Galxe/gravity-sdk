#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CLUSTER_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CONFIG_FILE="${CLUSTER_TOML:-$CLUSTER_DIR/cluster.toml}"
HEIGHT_DIFF_MAX="${HEIGHT_DIFF_MAX:-10}"
SAMPLE_INTERVAL="${SAMPLE_INTERVAL:-10}"
COMMON_DEPTH="${COMMON_DEPTH:-5}"
LOG_SINCE="${LOG_SINCE:-$(date +%s)}"
LOG_MAX_MATCHES="${LOG_MAX_MATCHES:-20}"
VALIDATORS_ONLY=0
SKIP_PROCESS=0
if [ "${CHAOS_BACKEND:-}" = "docker" ]; then
    SKIP_PROCESS=1
    export CHAOS_DOCKER_RPC_NETWORK="${CHAOS_DOCKER_RPC_NETWORK:-gravity-chaos}"
fi
SKIP_ADVANCING=0
SKIP_LOG_SCAN=0

usage() {
    cat <<'EOF'
Usage:
  oracle.sh [--config <cluster.toml>] [--validators] [--skip-process] [--skip-advancing]

Checks:
  - node processes are alive
  - RPC is reachable
  - height spread <= HEIGHT_DIFF_MAX
  - >2/3 validator stake advances between samples
  - all nodes agree on a common block hash and state root
  - logs have no new panic/fatal/abort patterns

Environment:
  HEIGHT_DIFF_MAX   Default: 10
  SAMPLE_INTERVAL   Default: 10 seconds
  COMMON_DEPTH      Default: 5 blocks behind min height
  LOG_SINCE         Default: oracle start time (Unix seconds)
  LOG_MAX_MATCHES   Default: 20
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --config)
            CONFIG_FILE="$2"
            shift 2
            ;;
        --validators)
            VALIDATORS_ONLY=1
            shift
            ;;
        --skip-process)
            SKIP_PROCESS=1
            shift
            ;;
        --skip-advancing)
            SKIP_ADVANCING=1
            shift
            ;;
        --skip-log-scan)
            SKIP_LOG_SCAN=1
            shift
            ;;
        --log-since)
            LOG_SINCE="$2"
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

args=(oracle --config "$CONFIG_FILE"
    --height-diff-max "$HEIGHT_DIFF_MAX"
    --sample-interval "$SAMPLE_INTERVAL"
    --common-depth "$COMMON_DEPTH"
    --log-since "$LOG_SINCE"
    --log-max-matches "$LOG_MAX_MATCHES")

if [ "$VALIDATORS_ONLY" -eq 1 ]; then
    args+=(--validators)
fi
if [ "$SKIP_PROCESS" -eq 1 ]; then
    args+=(--skip-process)
fi
if [ "$SKIP_ADVANCING" -eq 1 ]; then
    args+=(--skip-advancing)
fi
if [ "$SKIP_LOG_SCAN" -eq 1 ]; then
    args+=(--skip-log-scan)
fi

python3 "$SCRIPT_DIR/lib/cluster.py" "${args[@]}"
