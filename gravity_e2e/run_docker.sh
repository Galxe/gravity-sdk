#!/bin/bash
set -euo pipefail

# ============================================================
# Gravity E2E Docker Runner (CI/CD)
#
# Usage:
#   ./gravity_e2e/run_docker.sh [options] [suite1] [suite2] ... [--exclude suite] [pytest_args]
#
# Options:
#   --build-only   Build gravity_node + gravity_cli into host
#                  target/quick-release/ and exit (no tests).
#   --skip-build   Do not cargo-build; require prebuilt binaries at
#                  host target/quick-release/{gravity_node,gravity_cli}.
#
# Examples:
#   ./gravity_e2e/run_docker.sh                    # Build + all suites
#   ./gravity_e2e/run_docker.sh --build-only       # Build once for CI
#   ./gravity_e2e/run_docker.sh --skip-build single_node
#   ./gravity_e2e/run_docker.sh single_node -k test_transfer
#
# Description:
#   Source is piped into Docker via tar (no full host mount — avoids
#   permission issues). Prebuilt binaries use a bind mount of
#   target/quick-release only.
# ============================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

DOCKER_IMAGE="rust:1.88.0-bookworm"
BUILD_ONLY=0
SKIP_BUILD=0
ARGS=()

while [ "$#" -gt 0 ]; do
    case "$1" in
        --build-only)
            BUILD_ONLY=1
            shift
            ;;
        --skip-build)
            SKIP_BUILD=1
            shift
            ;;
        --)
            shift
            ARGS+=("$@")
            break
            ;;
        *)
            ARGS+=("$1")
            shift
            ;;
    esac
done

if [ "${BUILD_ONLY}" -eq 1 ] && [ "${SKIP_BUILD}" -eq 1 ]; then
    echo "error: --build-only and --skip-build are mutually exclusive" >&2
    exit 2
fi

if [ "${BUILD_ONLY}" -eq 1 ] && [ "${#ARGS[@]}" -gt 0 ]; then
    echo "error: --build-only does not accept suite / pytest args: ${ARGS[*]}" >&2
    exit 2
fi

echo "===== Gravity E2E Docker Runner ====="
echo "Repo Root: $REPO_ROOT"
echo "Image: $DOCKER_IMAGE"
echo "Build only: ${BUILD_ONLY}"
echo "Skip build: ${SKIP_BUILD}"
echo "Args: ${ARGS[*]:-<all suites>}"
echo "======================================"

PREBUILT_DIR="${REPO_ROOT}/target/quick-release"
mkdir -p "${PREBUILT_DIR}"

if [ "${SKIP_BUILD}" -eq 1 ]; then
    for bin in gravity_node gravity_cli; do
        if [ ! -x "${PREBUILT_DIR}/${bin}" ]; then
            echo "error: --skip-build requires executable ${PREBUILT_DIR}/${bin}" >&2
            exit 1
        fi
    done
fi

source_tar() {
    tar -C "$REPO_ROOT" \
        --exclude='target' \
        --exclude='.git' \
        --exclude='external/gravity_bench' \
        --exclude='external/gravity_chain_core_contracts' \
        -cf - .
}

# ---------------------------------------------------------------------------
# --build-only: compile once, write binaries to host target/quick-release
# ---------------------------------------------------------------------------
if [ "${BUILD_ONLY}" -eq 1 ]; then
    echo "===== Build-only → host target/quick-release ====="
    source_tar | docker run --rm -i \
        -e RUST_BACKTRACE=1 \
        -v "${PREBUILT_DIR}:/out" \
        "$DOCKER_IMAGE" \
        bash -c '
set -euo pipefail
mkdir -p /app && cd /app && tar xf -

echo "[Setup] Installing build dependencies..."
apt-get update >/dev/null 2>&1
apt-get install -y --no-install-recommends \
    clang llvm build-essential pkg-config libssl-dev libudev-dev \
    git curl ca-certificates >/dev/null 2>&1

export RUSTFLAGS="--cfg tokio_unstable -C debug-assertions=yes"

echo "[Build] gravity_node (quick-release)..."
cargo build --bin gravity_node --profile quick-release 2>&1 | tail -20

echo "[Build] gravity_cli (quick-release)..."
cargo build --bin gravity_cli --profile quick-release 2>&1 | tail -20

install -m 755 target/quick-release/gravity_node /out/gravity_node
install -m 755 target/quick-release/gravity_cli /out/gravity_cli
ls -la /out/gravity_node /out/gravity_cli
echo "===== Build-only completed ====="
'
    test -x "${PREBUILT_DIR}/gravity_node"
    test -x "${PREBUILT_DIR}/gravity_cli"
    echo "Host artifacts:"
    ls -la "${PREBUILT_DIR}/gravity_node" "${PREBUILT_DIR}/gravity_cli"
    exit 0
fi

# ---------------------------------------------------------------------------
# Test path: optional --skip-build
# ---------------------------------------------------------------------------
# Safely quote runner args for nested bash -c
RUNNER_ARGS=""
if [ "${#ARGS[@]}" -gt 0 ]; then
    printf -v RUNNER_ARGS '%q ' "${ARGS[@]}"
fi

DOCKER_VOL_ARGS=()
PHASE2_SCRIPT=""
if [ "${SKIP_BUILD}" -eq 1 ]; then
    DOCKER_VOL_ARGS+=(-v "${PREBUILT_DIR}:/prebuilt:ro")
    PHASE2_SCRIPT='
echo ""
echo "===== Phase 2: Using prebuilt binaries (no cargo build) ====="
mkdir -p /app/target/quick-release
cp -a /prebuilt/gravity_node /prebuilt/gravity_cli /app/target/quick-release/
chmod +x /app/target/quick-release/gravity_node /app/target/quick-release/gravity_cli
ls -la /app/target/quick-release/gravity_node /app/target/quick-release/gravity_cli
'
else
    PHASE2_SCRIPT='
echo ""
echo "===== Phase 2: Building Binaries ====="
export RUSTFLAGS="--cfg tokio_unstable -C debug-assertions=yes"

echo "[Step 4] Building gravity_node (quick-release)..."
cargo build --bin gravity_node --profile quick-release 2>&1 | tail -5

echo "[Step 5] Building gravity_cli (quick-release)..."
cargo build --bin gravity_cli --profile quick-release 2>&1 | tail -5
'
fi

source_tar | docker run --rm -i \
    -e RUST_BACKTRACE=1 \
    "${DOCKER_VOL_ARGS[@]+"${DOCKER_VOL_ARGS[@]}"}" \
    "$DOCKER_IMAGE" \
    bash -c "
set -euo pipefail
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

${PHASE2_SCRIPT}

echo ''
echo '===== Phase 2b: E2E Solidity test contracts ====='
if [ -f /app/gravity_e2e/tests/contracts/randomness/foundry.toml ]; then
    (cd /app/gravity_e2e/tests/contracts/randomness && forge build)
fi

echo ''
echo '===== Phase 3: Running E2E Tests ====='
echo '[Step 7] Running runner.py...'
export PYTHONPATH=/app:/app/gravity_e2e:\$PYTHONPATH
cd /app/gravity_e2e
python3 runner.py --force-init --exclude long_test ${RUNNER_ARGS}

echo ''
echo '===== E2E Tests Completed Successfully ====='
"
