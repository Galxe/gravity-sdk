#!/usr/bin/env bash
# Tear down stress-test cluster(s).
#
# Usage:
#   ./stop.sh                            # stop containers, keep data
#   ./stop.sh --clean                    # stop + wipe data volumes
#   ./stop.sh --parallel=3               # stop A,B,C parallel clusters
#   ./stop.sh --parallel=3 --clean       # ...and wipe their data
#
# `--parallel=N` must match what was passed to run.sh; without it stop.sh
# only addresses the single (`pfn_stress`) compose project.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

CLEAN=0
PARALLEL=1
for arg in "$@"; do
    case "$arg" in
        --clean) CLEAN=1 ;;
        --parallel=*)
            PARALLEL="${arg#--parallel=}"
            [[ "$PARALLEL" =~ ^[1-9][0-9]*$ ]] || { echo "[stop] --parallel must be a positive int"; exit 2; }
            [[ "$PARALLEL" -le 26 ]] || { echo "[stop] --parallel must be <=26 (single-letter cluster IDs)"; exit 2; } ;;
        -h|--help) sed -n '3,11p' "$0"; exit 0 ;;
        *) echo "[stop] unknown arg: $arg" >&2; exit 2 ;;
    esac
done

clean_flag=""
[[ $CLEAN -eq 1 ]] && clean_flag="-v"

cluster_ids() {
    if [[ "$PARALLEL" -eq 1 ]]; then echo "single"; else
        for (( i=0; i<PARALLEL; i++ )); do
            printf '%s ' "$(printf "\\$(printf '%03o' $((65+i)))")"
        done
    fi
}

for id in $(cluster_ids); do
    if [[ "$id" == "single" ]]; then
        proj="pfn_stress"
        cfg="./config"
    else
        lc="$(echo "$id" | tr 'A-Z' 'a-z')"
        proj="pfn_stress_${lc}"
        cfg="./config-${id}"
        # Stop any detached parallel bench container too.
        docker kill "pfn_stress_bench_${lc}" 2>/dev/null || true
    fi
    echo "[stop] $proj (config=$cfg) clean=$CLEAN"
    COMPOSE_PROJECT_NAME="$proj" CONFIG_DIR="$cfg" \
        docker compose down $clean_flag --remove-orphans 2>/dev/null || true
done
echo "[stop] done."
