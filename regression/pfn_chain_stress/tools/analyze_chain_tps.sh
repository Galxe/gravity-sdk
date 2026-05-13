#!/usr/bin/env bash
# Chain-level TPS analyzer for the pfn_chain_stress regression suite.
#
# Usage:
#   analyze_chain_tps.sh <RPC_URL> <UNIX_TS_START> <UNIX_TS_END>
#
# Computes sustained chain TPS over [start, end] by:
#   1. Binary-searching block numbers whose header timestamp brackets the
#      requested wall-clock window (chain progress is monotone, so the
#      timestamp -> block_number map is monotone-nondecreasing).
#   2. Summing transaction counts across the resulting block range.
#   3. Dividing by (last_ts - first_ts) — using actual block timestamps,
#      not the requested window, so we report the true throughput observed
#      in the matched range and don't undercount when the window endpoints
#      fall between blocks.
#
# Output: a single human-readable line with start/end blocks, tx total,
# elapsed seconds, and TPS. Returns 0 on success; non-zero if no block
# range fits the window (e.g. chain hasn't reached UNIX_TS_END yet).
#
# Designed to be invoked optionally by ../run.sh after a bench completes;
# the analyzer is run on the last node in the topology chain so the
# numbers reflect what a downstream PFN actually sees on its local DB.

set -euo pipefail

if [[ $# -ne 3 ]]; then
    echo "usage: $0 <RPC_URL> <UNIX_TS_START> <UNIX_TS_END>" >&2
    exit 2
fi

RPC_URL="$1"
WIN_START="$2"
WIN_END="$3"

[[ "$WIN_START" =~ ^[0-9]+$ ]] || { echo "[analyze] WIN_START must be unix ts" >&2; exit 2; }
[[ "$WIN_END"   =~ ^[0-9]+$ ]] || { echo "[analyze] WIN_END must be unix ts" >&2; exit 2; }
[[ "$WIN_END" -gt "$WIN_START" ]] || { echo "[analyze] WIN_END must be > WIN_START" >&2; exit 2; }

# Direct-connect; bypass any ambient http_proxy (5x slower on localhost).
CURL=(curl -s --noproxy '*' -m 10 -X POST -H 'Content-Type: application/json')

rpc() {
    # $1 = method, $2 = params JSON array (e.g. '["latest", false]')
    local method="$1" params="$2"
    "${CURL[@]}" --data "{\"jsonrpc\":\"2.0\",\"method\":\"$method\",\"params\":$params,\"id\":1}" "$RPC_URL"
}

hex_to_dec() {
    # Strip 0x and convert; handles empty/null cleanly.
    local h="${1#0x}"
    [[ -z "$h" ]] && { echo 0; return; }
    printf '%d\n' "$((16#$h))"
}

block_ts() {
    # Echo the timestamp of block $1 as a decimal unix ts, or empty on error.
    local n_hex
    n_hex=$(printf '0x%x' "$1")
    rpc eth_getBlockByNumber "[\"$n_hex\", false]" \
        | python3 -c 'import sys, json; r=json.load(sys.stdin).get("result") or {}; t=r.get("timestamp"); print(int(t,16) if t else "", end="")'
}

# ── Probe latest block to bound the binary search ─────────────────────────
latest_hex=$(rpc eth_blockNumber '[]' | python3 -c 'import sys, json; r=json.load(sys.stdin).get("result") or "0x0"; print(r)')
latest=$(hex_to_dec "$latest_hex")
if [[ "$latest" -le 0 ]]; then
    echo "[analyze] no blocks on chain yet (latest=$latest)" >&2
    exit 1
fi

# Earliest non-genesis block we can read; use block 1 (genesis ts is often 0).
EARLIEST=1
ts1=$(block_ts "$EARLIEST")
tsL=$(block_ts "$latest")
if [[ -z "$ts1" || -z "$tsL" ]]; then
    echo "[analyze] failed to read header timestamps at $RPC_URL" >&2
    exit 1
fi

# Sanity: window must overlap the chain's observed time range.
if [[ "$WIN_END" -lt "$ts1" || "$WIN_START" -gt "$tsL" ]]; then
    echo "[analyze] requested window [$WIN_START,$WIN_END] outside chain range [$ts1,$tsL]" >&2
    exit 1
fi

# ── Binary search: smallest block whose ts >= target ──────────────────────
# Standard lower_bound. Returns a block in [EARLIEST, latest+1].
search_lb() {
    local target="$1" lo="$EARLIEST" hi="$((latest + 1))" mid mts
    while [[ "$lo" -lt "$hi" ]]; do
        mid=$(( (lo + hi) / 2 ))
        mts=$(block_ts "$mid")
        if [[ -z "$mts" ]]; then
            # Block missing (pruned?). Treat as larger so we shrink the upper half.
            hi="$mid"
            continue
        fi
        if [[ "$mts" -lt "$target" ]]; then
            lo=$(( mid + 1 ))
        else
            hi="$mid"
        fi
    done
    echo "$lo"
}

start_blk=$(search_lb "$WIN_START")
# end_blk = largest block with ts <= WIN_END = (lower_bound(WIN_END+1)) - 1
end_blk_lb=$(search_lb "$(( WIN_END + 1 ))")
end_blk=$(( end_blk_lb - 1 ))

if [[ "$end_blk" -lt "$start_blk" ]]; then
    echo "[analyze] no blocks in window [$WIN_START,$WIN_END]" >&2
    exit 1
fi

# ── Sum tx counts across [start_blk, end_blk] ─────────────────────────────
start_ts_actual=$(block_ts "$start_blk")
end_ts_actual=$(block_ts "$end_blk")
elapsed=$(( end_ts_actual - start_ts_actual ))
[[ "$elapsed" -le 0 ]] && elapsed=1   # single-block window edge case

total_tx=0
# Use eth_getBlockTransactionCountByNumber for cheaper reads (no full tx list).
for (( n = start_blk; n <= end_blk; n++ )); do
    h=$(printf '0x%x' "$n")
    cnt_hex=$(rpc eth_getBlockTransactionCountByNumber "[\"$h\"]" \
        | python3 -c 'import sys, json; r=json.load(sys.stdin).get("result") or "0x0"; print(r)')
    cnt=$(hex_to_dec "$cnt_hex")
    total_tx=$(( total_tx + cnt ))
done

tps=$(python3 -c "print(f'{$total_tx / $elapsed:.1f}')")

printf '[analyze] blocks=[%d..%d] window_actual=[%d..%d] elapsed=%ds txs=%d tps=%s\n' \
    "$start_blk" "$end_blk" "$start_ts_actual" "$end_ts_actual" "$elapsed" "$total_tx" "$tps"
