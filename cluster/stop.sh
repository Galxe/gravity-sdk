#!/bin/bash
set -e

# ============================================================
# Gravity Cluster Stop Script
#
# Usage:
#   ./stop.sh [--config cluster.toml] [--nodes node1,node2,...]
#
# Stops all nodes in the cluster or specified nodes only.
# ============================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_FILE="$SCRIPT_DIR/cluster.toml"
SPECIFIC_NODES=""

# Parse arguments
while [[ "$#" -gt 0 ]]; do
    case $1 in
        --config)
            CONFIG_FILE="$2"
            shift
            ;;
        --nodes)
            SPECIFIC_NODES="$2"
            shift
            ;;
        *)
            echo "Unknown parameter: $1"
            exit 1
            ;;
    esac
    shift
done

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info()  { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Parse TOML using Python
parse_toml() {
    python3 << 'PYTHON_SCRIPT'
import json
import sys
import os

try:
    import tomllib
    def load_toml(f):
        return tomllib.load(f)
    open_mode = 'rb'
except ImportError:
    import toml
    def load_toml(f):
        return toml.load(f)
    open_mode = 'r'

config_file = os.environ.get('CONFIG_FILE', 'cluster.toml')

try:
    with open(config_file, open_mode) as f:
        config = load_toml(f)
    print(json.dumps(config))
except Exception as e:
    print(f"Error: {e}", file=sys.stderr)
    sys.exit(1)
PYTHON_SCRIPT
}

# Check if node should be stopped
should_stop_node() {
    local node_id="$1"
    
    if [ -z "$SPECIFIC_NODES" ]; then
        return 0  # Stop all nodes
    fi
    
    IFS=',' read -ra nodes_array <<< "$SPECIFIC_NODES"
    for n in "${nodes_array[@]}"; do
        if [ "$n" == "$node_id" ]; then
            return 0
        fi
    done
    return 1
}

# Stop a node
stop_node() {
    local node_id="$1"
    local data_dir="$2"
    
    local pid_file="$data_dir/script/node.pid"
    
    if [ ! -f "$pid_file" ]; then
        log_warn "$node_id: No PID file found (not running?)"
        return 0
    fi
    
    local pid=$(cat "$pid_file")
    
    # Use kill -0 to check if process exists (works on macOS and Linux)
    if ! kill -0 "$pid" 2>/dev/null; then
        log_warn "$node_id: Process $pid not running (stale PID file)"
        rm -f "$pid_file"
        return 0
    fi
    
    log_info "Stopping $node_id (PID: $pid)..."
    kill "$pid" 2>/dev/null || true
    
    # Wait for graceful shutdown
    for i in {1..10}; do
        if ! kill -0 "$pid" 2>/dev/null; then
            rm -f "$pid_file"
            log_info "$node_id stopped"
            return 0
        fi
        sleep 0.5
    done
    
    # Force kill if still running
    log_warn "$node_id: Force killing..."
    kill -9 "$pid" 2>/dev/null || true
    rm -f "$pid_file"
    log_info "$node_id stopped (forced)"
}

# Main
main() {
    if [ ! -f "$CONFIG_FILE" ]; then
        log_error "Config file not found: $CONFIG_FILE"
        exit 1
    fi
    
    export CONFIG_FILE
    config_json=$(parse_toml)
    
    base_dir=$(echo "$config_json" | jq -r '.cluster.base_dir')
    cluster_name=$(echo "$config_json" | jq -r '.cluster.name')
    
    log_info "Stopping cluster: $cluster_name"
    
    # Process each node
    node_count=$(echo "$config_json" | jq '.nodes | length')
    stopped=0
    
    for i in $(seq 0 $((node_count - 1))); do
        node=$(echo "$config_json" | jq ".nodes[$i]")
        node_id=$(echo "$node" | jq -r '.id')
        data_dir=$(echo "$node" | jq -r '.data_dir // empty')
        
        if [ -z "$data_dir" ]; then
            data_dir="$base_dir/$node_id"
        fi
        
        if should_stop_node "$node_id"; then
            stop_node "$node_id" "$data_dir"
            ((stopped++)) || true
        fi
    done
    
    echo ""
    log_info "Stopped $stopped node(s)"
}

main "$@"
