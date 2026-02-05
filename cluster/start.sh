#!/bin/bash
set -e

# ============================================================
# Gravity Cluster Start Script
#
# Usage:
#   ./start.sh [--config cluster.toml] [--nodes node1,node2,...]
#
# Starts all nodes in the cluster or specified nodes only.
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

# Check if node should be started
should_start_node() {
    local node_id="$1"
    
    if [ -z "$SPECIFIC_NODES" ]; then
        return 0  # Start all nodes
    fi
    
    IFS=',' read -ra nodes_array <<< "$SPECIFIC_NODES"
    for n in "${nodes_array[@]}"; do
        if [ "$n" == "$node_id" ]; then
            return 0
        fi
    done
    return 1
}

# Start a node
start_node() {
    local node_id="$1"
    local data_dir="$2"
    
    local start_script="$data_dir/script/start.sh"
    
    if [ ! -f "$start_script" ]; then
        log_error "Start script not found for $node_id: $start_script"
        log_error "Run ./init.sh first to generate configs."
        return 1
    fi
    
    # Check if already running
    local pid_file="$data_dir/script/node.pid"
    if [ -f "$pid_file" ]; then
        local pid=$(cat "$pid_file")
        # Use kill -0 to check if process exists (works on macOS and Linux)
        if kill -0 "$pid" 2>/dev/null; then
            log_warn "$node_id is already running (PID: $pid)"
            return 0
        fi
    fi
    
    log_info "Starting $node_id..."
    bash "$start_script"
    
    # Wait and verify
    sleep 1
    if [ -f "$pid_file" ]; then
        local pid=$(cat "$pid_file")
        if kill -0 "$pid" 2>/dev/null; then
            log_info "$node_id started successfully (PID: $pid)"
            return 0
        fi
    fi
    
    log_error "$node_id failed to start. Check logs at: $data_dir/logs/"
    return 1
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
    
    log_info "Starting cluster: $cluster_name"
    
    # Process each node
    node_count=$(echo "$config_json" | jq '.nodes | length')
    started=0
    failed=0
    
    for i in $(seq 0 $((node_count - 1))); do
        node=$(echo "$config_json" | jq ".nodes[$i]")
        node_id=$(echo "$node" | jq -r '.id')
        data_dir=$(echo "$node" | jq -r '.data_dir // empty')
        
        if [ -z "$data_dir" ]; then
            data_dir="$base_dir/$node_id"
        fi
        
        if should_start_node "$node_id"; then
            if start_node "$node_id" "$data_dir"; then
                ((started++)) || true
            else
                ((failed++)) || true
            fi
        fi
    done
    
    echo ""
    log_info "Started $started node(s), $failed failed"
    
    if [ $failed -gt 0 ]; then
        exit 1
    fi
}

main "$@"
