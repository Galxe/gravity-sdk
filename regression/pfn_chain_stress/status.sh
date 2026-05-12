#!/usr/bin/env bash
# Show cluster health: per-node block height + peer count.
#
# Usage:
#   ./status.sh                          # single cluster
#   ./status.sh --parallel=3             # A, B, C
#   ./status.sh --topology=simple ...    # only probe node1/vfn1/pfn1
#
# Flags should match what was used with run.sh.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

unset http_proxy https_proxy HTTP_PROXY HTTPS_PROXY all_proxy ALL_PROXY no_proxy NO_PROXY

PARALLEL=1
TOPOLOGY="chain"
for arg in "$@"; do
    case "$arg" in
        --parallel=*) PARALLEL="${arg#--parallel=}" ;;
        --topology=chain|--topology=simple) TOPOLOGY="${arg#--topology=}" ;;
        -h|--help) sed -n '3,9p' "$0"; exit 0 ;;
        *) echo "[status] unknown arg: $arg" >&2; exit 2 ;;
    esac
done

if [[ "$TOPOLOGY" == "chain" ]]; then
    NODES=(node1 vfn1 pfn1 pfn2 pfn3)
else
    NODES=(node1 vfn1 pfn1)
fi

cluster_ids() {
    if [[ "$PARALLEL" -eq 1 ]]; then echo "single"; else
        for (( i=0; i<PARALLEL; i++ )); do
            printf '%s ' "$(printf "\\$(printf '%03o' $((65+i)))")"
        done
    fi
}

port_offset_for() {
    local id="$1"
    if [[ "$id" == "single" ]]; then echo 0; else
        local letter_ord; letter_ord=$(printf '%d' "'$id")
        echo $(( (letter_ord - 65) * 10000 ))
    fi
}

rpc_port_for() {
    local id="$1" node="$2"
    local offset; offset="$(port_offset_for "$id")"
    case "$node" in
        node1) echo $((18545 + offset)) ;;
        vfn1)  echo $((18546 + offset)) ;;
        pfn1)  echo $((18547 + offset)) ;;
        pfn2)  echo $((18548 + offset)) ;;
        pfn3)  echo $((18549 + offset)) ;;
    esac
}

for id in $(cluster_ids); do
    if [[ "$id" == "single" ]]; then proj="pfn_stress"; else
        lc="$(echo "$id" | tr 'A-Z' 'a-z')"
        proj="pfn_stress_${lc}"
    fi
    echo "── cluster=$id project=$proj ──"
    COMPOSE_PROJECT_NAME="$proj" docker compose ps 2>/dev/null || true

    echo
    printf '%-6s %-8s %-12s %-8s\n' node port block peers
    for n in "${NODES[@]}"; do
        port="$(rpc_port_for "$id" "$n")"
        bn=$(curl -s --noproxy '*' -m 2 -X POST -H 'Content-Type: application/json' \
            --data '{"jsonrpc":"2.0","method":"eth_blockNumber","id":1}' \
            "http://127.0.0.1:$port" 2>/dev/null \
            | sed -n 's/.*"result":"\(0x[0-9a-fA-F]*\)".*/\1/p')
        pc=$(curl -s --noproxy '*' -m 2 -X POST -H 'Content-Type: application/json' \
            --data '{"jsonrpc":"2.0","method":"net_peerCount","id":1}' \
            "http://127.0.0.1:$port" 2>/dev/null \
            | sed -n 's/.*"result":"\(0x[0-9a-fA-F]*\)".*/\1/p')
        printf '%-6s %-8s %-12s %-8s\n' "$n" "$port" "${bn:-?}" "${pc:-?}"
    done
    echo
done
