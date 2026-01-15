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
    -v "${SCRIPT_DIR}/2_mins.json:/2_mins.json:ro" \
    rust:1.88.0-bookworm \
    bash -c '
set -e

echo "[1/6] Installing dependencies..."
apt-get update && apt-get install -y --no-install-recommends \
    libzstd-dev clang llvm build-essential pkg-config libssl-dev libudev-dev procps git jq curl python3 python3-pip python3-venv nodejs npm > /dev/null 2>&1
ln -sf /usr/bin/python3 /usr/bin/python

echo "[2/6] Cloning ${GIT_REF}..."
git clone --depth 50 --branch "${GIT_REF}"  "${CLONE_URL}" /app
cd /app
echo "Checked out: $(git rev-parse --short HEAD)"

echo "[2.5/6] Configuring Genesis..."
cp /2_mins.json ./
echo "Genesis file size: $(wc -c < /app/2_mins.json) bytes"

echo "Updating reth_config.json template..."
echo "reth_config.json exists: $(test -f deploy_utils/node1/config/reth_config.json && echo yes || echo no)"
if jq ".reth_args.chain = \"/app/2_mins.json\"" deploy_utils/node1/config/reth_config.json > deploy_utils/node1/config/reth_config.json.tmp; then
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

echo ""
echo "[5.5/7] Running gravity_e2e tests..."
cd /app/gravity_e2e

# Install Python dependencies
python3 -m pip install -r requirements.txt --quiet

# Install Foundry (for tests that need forge to compile contracts)
echo "Installing Foundry..."
curl -L https://foundry.paradigm.xyz | bash
export PATH="$HOME/.foundry/bin:$PATH"
foundryup --quiet

# Compile ERC20 test contracts
if [ -d "tests/contracts/erc20-test" ]; then
    echo "Compiling ERC20 test contracts..."
    cd tests/contracts/erc20-test
    forge build --quiet || echo "Warning: forge build failed for erc20-test"
    cd /app/gravity_e2e
    # Copy compiled contracts to contracts_data
    mkdir -p contracts_data
    cp tests/contracts/erc20-test/out/SimpleStorage.sol/SimpleStorage.json contracts_data/ 2>/dev/null || true
    cp tests/contracts/erc20-test/out/SimpleToken.sol/SimpleToken.json contracts_data/ 2>/dev/null || true
fi

# Compile RandomDice contract if exists
if [ -d "/app/examples/randomness" ]; then
    echo "Compiling RandomDice contract..."
    cd /app/examples/randomness
    forge build --quiet || echo "Warning: forge build failed, randomness tests may skip"
    cd /app/gravity_e2e
fi

# Create nodes config for local node
mkdir -p configs
cat > configs/nodes.json << NODES_EOF
{
  "nodes": [
    {
      "id": "local_node",
      "host": "localhost",
      "rpc_port": 8545,
      "type": "validator"
    }
  ]
}
NODES_EOF

# Create minimal test accounts config
cat > configs/test_accounts.json << ACCOUNTS_EOF
{
  "faucet": {
    "private_key": "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
    "address": "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
  }
}
ACCOUNTS_EOF

# Run specific test suites (basic, contract, erc20, randomness)
echo "Running gravity_e2e selected tests..."
E2E_OUTPUT=$(python3 -m gravity_e2e --test-suite basic --test-suite contract --test-suite erc20 --test-suite randomness --nodes-config configs/nodes.json --accounts-config configs/test_accounts.json 2>&1)
E2E_EXIT_CODE=$?

# Display output
echo "$E2E_OUTPUT"

# Check for failures in output - sum all "Failed: X" values from each suite
FAILED_COUNT=$(echo "$E2E_OUTPUT" | grep -o "Failed: [0-9]*" | grep -o "[0-9]*" | awk '{sum += $1} END {print sum}')
if [ -n "$FAILED_COUNT" ] && [ "$FAILED_COUNT" -gt 0 ]; then
    echo ""
    echo "============================================================"
    echo "gravity_e2e tests completed with $FAILED_COUNT FAILED tests!"
    echo "============================================================"
    # Extract failed test names
    echo "$E2E_OUTPUT" | grep -A100 "Failed tests:" | head -20
    kill $NODE_PID 2>/dev/null || true
    exit 1
fi

if [ $E2E_EXIT_CODE -ne 0 ]; then
    echo "gravity_e2e tests FAILED with exit code $E2E_EXIT_CODE!"
    kill $NODE_PID 2>/dev/null || true
    exit 1
fi
echo "gravity_e2e tests PASSED!"

cd /

echo "[6/7] Running benchmark (100 accounts, 100 TPS, 5 min)..."
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

