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
    rust:1.88.0-bookworm \
    bash -c '
set -e

echo "===== Phase 1: Environment Setup ====="

echo "[1/7] Installing system dependencies..."
apt-get update && apt-get install -y --no-install-recommends \
    libzstd-dev clang llvm build-essential pkg-config libssl-dev libudev-dev procps git jq curl python3 python3-pip python3-venv nodejs npm gettext-base protobuf-compiler bc > /dev/null 2>&1
ln -sf /usr/bin/python3 /usr/bin/python

echo "[1.1/7] Installing Foundry..."
curl -L https://foundry.paradigm.xyz | bash
export PATH="$HOME/.foundry/bin:$PATH"
foundryup

echo "[2/7] Cloning ${GIT_REF}..."
git clone --depth 50 --branch "${GIT_REF}"  "${CLONE_URL}" /app
cd /app
echo "Checked out: $(git rev-parse --short HEAD)"

echo "===== Phase 2: Preparation (Fast Fail) ====="

echo "[2.5/7] Preparing E2E test environment..."
cd /app/gravity_e2e

echo "  - Installing Python dependencies..."
python3 -m pip install -r requirements.txt --quiet --break-system-packages
echo "  - Python dependencies installed."

echo "  - Creating test configuration files..."
mkdir -p configs
cat > configs/nodes.json << NODES_EOF
{
  "nodes": {
    "local_node": {
      "type": "validator",
      "role": "primary",
      "host": "localhost",
      "rpc_port": 8545,
      "metrics_port": 9001
    }
  }
}
NODES_EOF

cat > configs/test_accounts.json << ACCOUNTS_EOF
{
  "faucet": {
    "private_key": "0x047a5466f6f9e08c8bcc56213d6530d517c1ef126eefbbdf85ffe8d893ed0e9f",
    "address": "0xAEd2a948892475F800A337427B3275D190EA3e94"
  }
}
ACCOUNTS_EOF
echo "  - Test configurations created."

echo "  - Compiling test contracts..."
if [ -d "tests/contracts/erc20-test" ]; then
    echo "    Compiling ERC20 test contracts..."
    cd tests/contracts/erc20-test
    forge build --quiet || { echo "ERROR: forge build failed for erc20-test"; exit 1; }
    cd /app/gravity_e2e
    mkdir -p contracts_data
    cp tests/contracts/erc20-test/out/SimpleStorage.sol/SimpleStorage.json contracts_data/ 2>/dev/null || true
    cp tests/contracts/erc20-test/out/SimpleToken.sol/SimpleToken.json contracts_data/ 2>/dev/null || true
else
    echo "    WARN: tests/contracts/erc20-test not found, skipping"
fi

if [ -d "tests/contracts/randomness" ]; then
    echo "    Compiling RandomDice contract..."
    cd tests/contracts/randomness
    forge build --quiet || { echo "ERROR: forge build failed for randomness"; exit 1; }
    cd /app/gravity_e2e
    cp tests/contracts/randomness/out/RandomDice.sol/RandomDice.json contracts_data/ 2>/dev/null || true
else
    echo "    WARN: tests/contracts/randomness not found, skipping"
fi
echo "  - Test contracts compiled."

cd /app

echo "[2.6/7] Creating cluster configuration..."
# Use the cluster.toml from gravity_e2e/cluster_test_cases/single_node
cp /app/gravity_e2e/cluster_test_cases/single_node/cluster.toml /app/single_node.toml

# Fix binary_path for Docker environment
sed -i "s|binary_path = \"../target/quick-release/gravity_node\"|binary_path = \"/app/target/quick-release/gravity_node\"|" /app/single_node.toml

echo "  - Cluster configuration created."

echo ""
echo "===== Phase 2 Complete: Preparation Passed ====="
echo ""

echo "===== Phase 3: Building Binaries (Long) ====="

echo "[3/7] Building binaries..."
echo "  Building gravity_node..."
RUSTFLAGS="--cfg tokio_unstable" cargo build --bin gravity_node --profile quick-release
echo "  Building gravity_cli..."
RUSTFLAGS="--cfg tokio_unstable" cargo build --bin gravity_cli --profile quick-release

echo "[4/7] Initializing Cluster..."
export PATH=$PATH:/app/target/quick-release
npm config set registry https://registry.npmmirror.com
npm config set fetch-retries 5
npm config set fetch-retry-mintimeout 20000
bash cluster/init.sh /app/single_node.toml

echo "Injecting faucet account into genesis..."
GENESIS_FILE="cluster/output/genesis.json"
FAUCET_ADDR="0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
jq ".alloc[\"${FAUCET_ADDR}\"] = {\"balance\": \"0x21e19e0c9bab2400000\"}" "$GENESIS_FILE" > "$GENESIS_FILE.tmp" && mv "$GENESIS_FILE.tmp" "$GENESIS_FILE"
echo "Genesis injected with faucet account."

echo ""
echo "===== Phase 4: Deployment ====="

echo "[5/7] Deploying Cluster..."
bash cluster/deploy.sh /app/single_node.toml

echo "[6/7] Starting Cluster..."
bash cluster/start.sh --config /app/single_node.toml

# Function to stop cluster on exit
cleanup() {
    echo "Stopping cluster..."
    bash /app/cluster/stop.sh --config /app/single_node.toml
}
trap cleanup EXIT

echo "Waiting for node to be ready..."
sleep 5

echo "Check node is up..."
curl -X POST -H "Content-Type: application/json" --data "{\"jsonrpc\":\"2.0\",\"method\":\"eth_blockNumber\",\"params\":[],\"id\":1}" http://localhost:8545

echo ""
echo "===== Phase 5: Running Tests ====="

echo "[7/7] Running gravity_e2e tests..."
cd /app/gravity_e2e


suites="staking basic contract erc20 randomness"
for suite in $suites; do
    echo "------------------------------------------------------------"
    echo "Running test suite: $suite"
    echo "------------------------------------------------------------"
    python3 -m gravity_e2e --test-suite "$suite" --nodes-config configs/nodes.json --accounts-config configs/test_accounts.json
    echo "Test suite $suite PASSED"
    echo ""
done

echo "gravity_e2e tests PASSED!"

cd /

echo "Final block number check..."
curl -X POST -H "Content-Type: application/json" --data "{\"jsonrpc\":\"2.0\",\"method\":\"eth_blockNumber\",\"params\":[],\"id\":1}" http://localhost:8545

echo ""
echo "===== E2E Test Completed ====="


echo "Done."
'


