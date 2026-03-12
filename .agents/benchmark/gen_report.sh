#!/bin/bash
# gen_report.sh — Generate bench report from gravity_bench log file
# Usage: ./gen_report.sh <sdk_commit> <bench_commit> <log_file>
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/../../.env/bench.env"

SDK_COMMIT="$1"
BENCH_COMMIT="$2"
LOG_FILE="$3"

if [ -z "$LOG_FILE" ] || [ ! -f "$LOG_FILE" ]; then
  echo "Usage: $0 <sdk_commit> <bench_commit> <log_file>"
  exit 1
fi

# Extract bench-phase TPS values (only after "bench erc20 transfer" or "bench uniswap")
BENCH_TPS=$(awk '
  /bench erc20 transfer|bench uniswap/ { started=1; next }
  started && /TPS/ && /[0-9]+\.[0-9]+/ {
    match($0, /TPS[^0-9]*([0-9]+\.[0-9]+)/, m);
    if (m[1]+0 > 0) print m[1]
  }
' "$LOG_FILE")

TPS_STATS=$(echo "$BENCH_TPS" | awk '
  NR==1 { min=$1; max=$1; sum=0 }
  { sum+=$1; count++; if($1<min) min=$1; if($1>max) max=$1 }
  END {
    if (count>0) printf "avg=%.1f min=%.1f max=%.1f samples=%d", sum/count, min, max, count
    else print "avg=N/A min=N/A max=N/A samples=0"
  }
')

HALF_POOL=$((BENCH_MAX_POOL_SIZE / 2))

# Extract raw time series data (bench-phase only): TPS, Pending Txns, Pool Pending, Pool Queued
TIMESERIES=$(awk '
  /bench erc20 transfer|bench uniswap/ { started=1; next }
  !started { next }
  /TPS/ && /[0-9]+\.[0-9]+/ { match($0, /TPS[^0-9]*([0-9]+\.[0-9]+)/, m); if (m[1]+0 > 0) tps = m[1] }
  /Pending Txns/ {
    match($0, /Pending Txns[^0-9]*([0-9]+\.?[0-9]*K?)/, m)
    pt = m[1]; sub(/K$/, "", pt); if (m[1] ~ /K$/) pt = int(pt * 1000)
  }
  /Pool Pending/ {
    match($0, /Pool Pending[^0-9]*([0-9]+\.?[0-9]*K?)/, m)
    pp = m[1]; sub(/K$/, "", pp); if (m[1] ~ /K$/) pp = int(pp * 1000)
    match($0, /Pool Queued[^0-9]*([0-9]+\.?[0-9]*K?)/, m2)
    pq = m2[1]; sub(/K$/, "", pq); if (m2[1] ~ /K$/) pq = int(pq * 1000)
    n++
    printf "| %d | %.1f | %d | %d | %d |\n", n, tps, pt+0, pp+0, pq+0
  }
' "$LOG_FILE")

TIMESERIES_COUNT=$(echo "$TIMESERIES" | wc -l)

# Extract bench-phase duration from timestamps
BENCH_DURATION=$(awk '
  /bench erc20 transfer|bench uniswap/ { started=1; next }
  !started { next }
  /^[0-9]{4}-[0-9]{2}-[0-9]{2}T/ {
    match($0, /^([0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2})/, m)
    if (!first_ts) first_ts = m[1]
    last_ts = m[1]
  }
  END { print first_ts " → " last_ts }
' "$LOG_FILE")

# Calculate duration in seconds
BENCH_START_EPOCH=$(date -d "$(echo "$BENCH_DURATION" | awk -F' → ' '{print $1}')" +%s 2>/dev/null || echo "0")
BENCH_END_EPOCH=$(date -d "$(echo "$BENCH_DURATION" | awk -F' → ' '{print $2}')" +%s 2>/dev/null || echo "0")
if [ "$BENCH_START_EPOCH" -gt 0 ] && [ "$BENCH_END_EPOCH" -gt 0 ]; then
  BENCH_SECS=$((BENCH_END_EPOCH - BENCH_START_EPOCH))
else
  BENCH_SECS="N/A"
fi

LAST_SNAPSHOT=$(tail -50 "$LOG_FILE")

REPORT_DIR="$SCRIPT_DIR/../../bench_reports"
mkdir -p "$REPORT_DIR"
REPORT_FILE="$REPORT_DIR/bench_report_$(date +%Y%m%d_%H%M%S).md"

{
  echo "# Gravity Bench Report — $(date '+%Y-%m-%d %H:%M:%S')"
  echo ""
  echo "## Versions"
  echo "| Component | Commit |"
  echo "|-----------|--------|"
  echo "| gravity-sdk | ${SDK_COMMIT} |"
  echo "| gravity_bench | ${BENCH_COMMIT} |"
  echo ""
  echo "## Log Files"
  echo "| Location | Path |"
  echo "|----------|------|"
  echo "| Bench machine (remote) | \`${BENCH_USER}@${BENCH_INSTANCE}:${BENCH_DIR}/\$(log filename)\` |"
  echo "| Local copy (archived) | \`bench_reports/\$(log filename)\` |"
  echo ""
  echo "## Config"
  echo "| Parameter | Value |"
  echo "|-----------|-------|"
  echo "| target_tps | ${BENCH_TARGET_TPS} |"
  echo "| num_accounts | ${FAUCET_NUM_ACCOUNTS} |"
  echo "| num_senders | ${BENCH_NUM_SENDERS} |"
  echo "| duration_secs | ${BENCH_DURATION_SECS} |"
  echo "| num_tokens | ${BENCH_NUM_TOKENS} |"
  echo "| max_pool_size | ${BENCH_MAX_POOL_SIZE} |"
  echo "| deploy_machine | ${DEPLOY_INSTANCE} |"
  echo "| bench_machine | ${BENCH_INSTANCE} |"
  echo ""
  echo "## Bench-Phase TPS (faucet excluded)"
  echo ""
  echo "- **Duration**: ${BENCH_DURATION} (${BENCH_SECS}s)"
  echo "- **TPS**: ${TPS_STATS}"
  echo ""

  # Bottleneck analysis — dump raw data for human/LLM review
  if [ -n "$TIMESERIES" ]; then
    echo "## Bottleneck Analysis (bench-phase, ${TIMESERIES_COUNT} snapshots)"
    echo ""
    echo "> **How to read**: Look at Pending Txns and Pool Pending trends. If Pending Txns keeps growing and Pool Pending stays near/above \`max_pool_size\` (~${BENCH_MAX_POOL_SIZE}), the **chain is the bottleneck**. If Pending Txns fluctuates heavily or Pool Pending frequently drops to 0, the **pressure client is the bottleneck**. Pool Queued > 0 indicates nonce-gap issues."
    echo ">"
    echo "> \`max_pool_size = ${BENCH_MAX_POOL_SIZE}\` → half = ${HALF_POOL}. If data points < 50, consider extending \`duration_secs\`."
    echo ""
    echo "### Raw Time Series"
    echo ""
    echo "| # | TPS | Pending Txns | Pool Pending | Pool Queued |"
    echo "|---|-----|-------------|--------------|-------------|"
    echo "$TIMESERIES"
    echo ""
  fi


  echo "## Last Snapshot"
  echo ""
  echo '```'
  echo "${LAST_SNAPSHOT}"
  echo '```'
} > "$REPORT_FILE"

echo "Report saved to: $REPORT_FILE"
echo "Bench-phase TPS: $TPS_STATS"

