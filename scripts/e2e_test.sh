#!/bin/bash
set -e

# ============================================================
# E2E Test Script
#
# Usage:
#   ./scripts/e2e_test.sh <branch_or_commit>
#
# Examples:
#   ./scripts/e2e_test.sh main
#   ./scripts/e2e_test.sh feature-branch
#   ./scripts/e2e_test.sh abc123def
#
# Environment Variables:
#   REPO              - GitHub repo (default: Galxe/gravity-sdk)
#   GITHUB_TOKEN      - Token for private repo access (optional for public)
#   DURATION          - How long to run the node (default: 60s)
#   BENCH_CONFIG_PATH - Path to bench_config.toml (default: ./bench_config.toml in scripts dir)
# ============================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BENCH_CONFIG_PATH="${BENCH_CONFIG_PATH:-${SCRIPT_DIR}/bench_config.toml}"

if [ ! -f "${BENCH_CONFIG_PATH}" ]; then
    echo "Error: bench_config.toml not found at ${BENCH_CONFIG_PATH}"
    exit 1
fi

GIT_REF="${1:-}"
if [ -z "${GIT_REF}" ]; then
    echo "Error: branch or commit is required"
    echo "Usage: $0 <branch_or_commit>"
    echo "Example: $0 main"
    exit 1
fi

REPO="${REPO:-Galxe/gravity-sdk}"
DURATION="${DURATION:-60}"

echo "===== Gravity E2E Test ====="
echo "Repo: ${REPO}"
echo "Ref: ${GIT_REF}"
echo "Duration: ${DURATION}s"
echo "Bench Config: ${BENCH_CONFIG_PATH}"
echo "============================"

# 构建 clone URL
if [ -n "${GITHUB_TOKEN}" ]; then
    CLONE_URL="https://x-access-token:${GITHUB_TOKEN}@github.com/${REPO}.git"
else
    CLONE_URL="https://github.com/${REPO}.git"
fi


docker run --rm -i \
    -p 9001:9001 \
    -e GIT_REF="${GIT_REF}" \
    -e CLONE_URL="${CLONE_URL}" \
    -e DURATION="${DURATION}" \
    -v "${BENCH_CONFIG_PATH}:/bench_config.toml:ro" \
    -v "${SCRIPT_DIR}/genesis.json:/app/genesis.json:ro" \
    rust:1.88.0-bookworm \
    bash -c '
set -e

echo "[1/6] Installing dependencies..."
apt-get update && apt-get install -y --no-install-recommends \
    clang llvm build-essential pkg-config libssl-dev libudev-dev procps git jq curl python3 python3-pip python3-venv nodejs npm > /dev/null 2>&1
ln -sf /usr/bin/python3 /usr/bin/python

echo "[2/6] Cloning ${GIT_REF}..."
git clone --depth 50 --branch "${GIT_REF}"  "${CLONE_URL}" /app
cd /app
echo "Checked out: $(git rev-parse --short HEAD)"

echo "[2.5/6] Configuring Genesis..."
echo "Genesis file size: $(wc -c < /app/genesis.json) bytes"

echo "Updating reth_config.json template..."
echo "reth_config.json exists: $(test -f deploy_utils/node1/config/reth_config.json && echo yes || echo no)"
if jq ".reth_args.chain = \"/app/genesis.json\"" deploy_utils/node1/config/reth_config.json > deploy_utils/node1/config/reth_config.json.tmp; then
    mv deploy_utils/node1/config/reth_config.json.tmp deploy_utils/node1/config/reth_config.json
    echo "jq command completed successfully"
    echo "New reth_config.json content preview:"
    head -5 deploy_utils/node1/config/reth_config.json || echo "Cannot read file"
else
    echo "jq command FAILED with exit code: $?"
    exit 1
fi


echo "[3/6] Building gravity_node (quick-release)..."
RUSTFLAGS="--cfg tokio_unstable" cargo build --bin gravity_node --profile quick-release

echo "[4/6] Deploying..."
rm -rf /tmp/node1
bash deploy_utils/deploy.sh --install_dir /tmp/ --mode single --node node1 -v quick-release

echo "[5/6] Running node..."
bash /tmp/node1/script/start.sh &
NODE_PID=$!
echo "Node started with PID $NODE_PID. Waiting for node to be ready..."
sleep 2

echo "Check node is up..."
curl -X POST -H "Content-Type: application/json" --data "{\"jsonrpc\":\"2.0\",\"method\":\"eth_blockNumber\",\"params\":[],\"id\":1}" http://localhost:8545

echo "[6/6] Running benchmark..."
cd /
git clone --depth 1 https://github.com/Galxe/gravity_bench.git /gravity_bench || true
cd /gravity_bench

echo "Using mounted benchmark config:"
cp /bench_config.toml ./
cat bench_config.toml
source setup.sh
# Run benchmark
cargo run --release 2>&1 || echo "Benchmark completed or failed"

echo "Benchmark execution finished. Displaying latest log..."
LATEST_LOG=$(ls -t log.*.log 2>/dev/null | head -n 1)
if [ -n "$LATEST_LOG" ]; then
    echo "Found log: $LATEST_LOG"
    tail "$LATEST_LOG"
    
    # Run verification script (located in /app/scripts/verify_benchmark.py)
    echo "Verifying benchmark results..."
    if python3 /app/scripts/verify_benchmark.py "$LATEST_LOG"; then
        echo "Verification PASSED."
    else
        echo "Verification FAILED."
        exit 1
    fi
else
    echo "No benchmark log file found."
    exit 1
fi

echo "Final block number check..."
curl -X POST -H "Content-Type: application/json" --data "{\"jsonrpc\":\"2.0\",\"method\":\"eth_blockNumber\",\"params\":[],\"id\":1}" http://localhost:8545
kill $NODE_PID 2>/dev/null || true

echo "===== E2E Test Completed ====="
'

echo "Done."

