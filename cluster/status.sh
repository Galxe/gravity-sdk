#!/bin/bash

# ============================================================
# Gravity Cluster Status Script
#
# Usage:
#   ./status.sh [--config cluster.toml]
#
# Shows the status of all nodes in the cluster.
# ============================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_FILE="$SCRIPT_DIR/cluster.toml"

# Parse arguments
while [[ "$#" -gt 0 ]]; do
    case $1 in
        --config)
            CONFIG_FILE="$2"
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
CYAN='\033[0;36m'
NC='\033[0m'

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

# Get block number from RPC
get_block_number() {
    local host="$1"
    local port="$2"
    
    local response
    response=$(curl -s --max-time 2 -X POST \
        -H "Content-Type: application/json" \
        --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
        "http://${host}:${port}" 2>/dev/null)
    
    if [ $? -eq 0 ] && [ -n "$response" ]; then
        local hex=$(echo "$response" | jq -r '.result // empty' 2>/dev/null)
        if [ -n "$hex" ]; then
            # Convert hex to decimal
            printf "%d" "$hex" 2>/dev/null || echo "-"
            return
        fi
    fi
    echo "-"
}

# Check node status
check_node() {
    local node_id="$1"
    local host="$2"
    local rpc_port="$3"
    local data_dir="$4"
    
    local pid_file="$data_dir/script/node.pid"
    local status="Stopped"
    local pid="-"
    local block="-"
    local status_color="$RED"
    
    if [ -f "$pid_file" ]; then
        pid=$(cat "$pid_file")
        if [ -d "/proc/$pid" ]; then
            status="Running"
            status_color="$GREEN"
            block=$(get_block_number "$host" "$rpc_port")
        else
            status="Stale"
            status_color="$YELLOW"
        fi
    fi
    
    printf "│ %-8s │ ${status_color}%-8s${NC} │ %-15s │ %-8s │ %-8s │\n" \
        "$node_id" "$status" "${host}:${rpc_port}" "$pid" "$block"
}

# Main
main() {
    if [ ! -f "$CONFIG_FILE" ]; then
        echo -e "${RED}[ERROR]${NC} Config file not found: $CONFIG_FILE"
        exit 1
    fi
    
    export CONFIG_FILE
    config_json=$(parse_toml)
    
    base_dir=$(echo "$config_json" | jq -r '.cluster.base_dir')
    cluster_name=$(echo "$config_json" | jq -r '.cluster.name')
    node_count=$(echo "$config_json" | jq '.nodes | length')
    
    echo ""
    echo -e "${CYAN}Cluster: $cluster_name ($node_count nodes)${NC}"
    echo "┌──────────┬──────────┬─────────────────┬──────────┬──────────┐"
    echo "│ Node     │ Status   │ RPC Endpoint    │ PID      │ Block #  │"
    echo "├──────────┼──────────┼─────────────────┼──────────┼──────────┤"
    
    for i in $(seq 0 $((node_count - 1))); do
        node=$(echo "$config_json" | jq ".nodes[$i]")
        
        node_id=$(echo "$node" | jq -r '.id')
        host=$(echo "$node" | jq -r '.host')
        rpc_port=$(echo "$node" | jq -r '.rpc_port')
        data_dir=$(echo "$node" | jq -r '.data_dir // empty')
        
        if [ -z "$data_dir" ]; then
            data_dir="$base_dir/$node_id"
        fi
        
        check_node "$node_id" "$host" "$rpc_port" "$data_dir"
    done
    
    echo "└──────────┴──────────┴─────────────────┴──────────┴──────────┘"
    echo ""
}

main "$@"
