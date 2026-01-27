#!/bin/bash
set -e

# ============================================================
# Gravity E2E Local Test Runner
#
# Usage:
#   ./gravity_e2e/run_test.sh
#
# Description:
#   Runs the Gravity E2E tests directly on the host machine.
#   It assumes:
#     1. 'gravity_node' and 'gravity_cli' are already built (use build_docker.sh or cargo build).
#     2. Python 3 and dependencies are installed.
#     3. Ports (8545, etc.) are free.
# ============================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Ensure python dependencies
if ! python3 -c "import tomli" >/dev/null 2>&1; then
    echo "Warning: 'tomli' module not found. Installing requirements..."
    pip install -r "$SCRIPT_DIR/requirements.txt" --break-system-packages 2>/dev/null || pip install -r "$SCRIPT_DIR/requirements.txt"
fi

echo "===== Running Gravity E2E Tests Locally ====="
echo "Repo Root: $REPO_ROOT"
echo "============================================="

# Add current directory to PYTHONPATH so module imports work
export PYTHONPATH="$REPO_ROOT:$PYTHONPATH"

# Execute runner
python3 "$SCRIPT_DIR/runner.py" "$@"
