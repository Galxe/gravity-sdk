#!/bin/bash
set -e

# ============================================================
# Gravity E2E Docker Builder
#
# Usage:
#   ./gravity_e2e/build_docker.sh
#
# Description:
#   Launches a Docker container to build gravity_node and gravity_cli.
#   Artifacts are stored in target/ (mounted from host).
# ============================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Image used in scripts/e2e_test.sh
DOCKER_IMAGE="rust:1.88.0-bookworm"

echo "===== Building Gravity Binaries in Docker ====="
echo "Repo Root: $REPO_ROOT"
echo "Image: $DOCKER_IMAGE"
echo "==============================================="

docker run --rm -i \
    -v "$REPO_ROOT:/app" \
    -w /app \
    -e RUST_BACKTRACE=1 \
    "$DOCKER_IMAGE" \
    bash -c '
set -e

echo "[Setup] Installing system dependencies..."
apt-get update >/dev/null 2>&1
apt-get install -y --no-install-recommends \
    clang llvm build-essential pkg-config libssl-dev libudev-dev procps git jq curl \
    python3 python3-pip python3-venv >/dev/null 2>&1

echo "[Build] Starting build process..."
export RUSTFLAGS="--cfg tokio_unstable"
echo "Building gravity_node..."
cargo build --bin gravity_node --profile quick-release

echo "Building gravity_cli..."
cargo build --bin gravity_cli --profile quick-release

echo "Build complete!"
'
