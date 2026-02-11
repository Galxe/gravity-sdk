#!/bin/bash
set -e

# ============================================================
# Gravity E2E Docker Runner (CI/CD)
#
# Usage:
#   ./gravity_e2e/run_docker.sh [suite1] [suite2] ... [--exclude suite] [pytest_args]
#
# Examples:
#   ./gravity_e2e/run_docker.sh                    # Run all test suites
#   ./gravity_e2e/run_docker.sh single_node        # Run only single_node suite
#   ./gravity_e2e/run_docker.sh single_node -k test_transfer
#   ./gravity_e2e/run_docker.sh --exclude fuzzy_cluster  # Run all except fuzzy_cluster
#
# Description:
#   Runs the complete E2E pipeline inside Docker (no host mount):
#   1. Copy source code into container
#   2. Build gravity_node + gravity_cli
#   3. Run cluster init/deploy/start + pytest via runner.py
# ============================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

DOCKER_IMAGE="rust:1.88.0-bookworm"
ARGS="$@"

echo "===== Gravity E2E Docker Runner ====="
echo "Repo Root: $REPO_ROOT"
echo "Image: $DOCKER_IMAGE"
echo "Args: ${ARGS:-<all suites>}"
echo "======================================"

# Pipe repo into container via tar (no volume mount, no permission issues)
tar -C "$REPO_ROOT" \
    --exclude='target' \
    --exclude='.git' \
    --exclude='external/gravity_bench' \
    --exclude='external/gravity_chain_core_contracts' \
    -cf - . \
| docker run --rm -i \
    -e RUST_BACKTRACE=1 \
    "$DOCKER_IMAGE" \
    bash -c "
set -e
mkdir -p /app && cd /app && tar xf -

echo '===== Phase 1: Environment Setup ====='

echo '[Step 1] Installing system dependencies...'
apt-get update >/dev/null 2>&1
apt-get install -y --no-install-recommends \\
    clang llvm build-essential pkg-config libssl-dev libudev-dev \\
    procps git jq curl python3 python3-pip python3-venv \\
    nodejs npm protobuf-compiler bc gettext-base >/dev/null 2>&1

ln -sf /usr/bin/python3 /usr/bin/python

echo '[Step 2] Installing Foundry...'
curl -L https://foundry.paradigm.xyz 2>/dev/null | bash >/dev/null 2>&1
export PATH=\"\$HOME/.foundry/bin:\$PATH\"
foundryup >/dev/null 2>&1
echo '  Foundry installed: '\$(forge --version | head -1)

echo '[Step 3] Installing Python dependencies...'
pip install -r /app/gravity_e2e/requirements.txt --quiet --break-system-packages

echo ''
echo '===== Phase 2: Building Binaries ====='

echo '[Step 4] Building gravity_node (quick-release)...'
export RUSTFLAGS='--cfg tokio_unstable'
cargo build --bin gravity_node --profile quick-release 2>&1 | tail -5

echo '[Step 5] Building gravity_cli (release)...'
cargo build --bin gravity_cli --release 2>&1 | tail -5

echo ''
echo '===== Phase 3: Running E2E Tests ====='

echo '[Step 6] Running runner.py...'
export PYTHONPATH=/app:/app/gravity_e2e:\$PYTHONPATH
cd /app/gravity_e2e
python3 runner.py --force-init $ARGS

echo ''
echo '===== E2E Tests Completed Successfully ====='
"
