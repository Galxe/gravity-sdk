#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_FILE="${1:-$SCRIPT_DIR/cluster.toml}"
OUTPUT_DIR="$SCRIPT_DIR/output"

source "$SCRIPT_DIR/utils/common.sh"

PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Configure node function (Rendering logic)
configure_node() {
    local node_id="$1"
    local host="$2"
    local p2p_port="$3"
    local rpc_port="$4"
    local metrics_port="$5"
    local data_dir="$6"
    local genesis_path="$7"
    local binary_path="$8"
    local identity_src="$9"
    local waypoint_src="${10}"
    
    local config_dir="$data_dir/config"
    
    log_info "  [$node_id] configuring..."
    
    # Create config dir
    mkdir -p "$config_dir"
    
    # Copy identity and waypoint from artifacts
    cp "$identity_src" "$config_dir/validator-identity.yaml"
    cp "$waypoint_src" "$config_dir/waypoint.txt"
    
    # Calculate derived ports
    local vfn_port=$((p2p_port + 10))
    local inspection_port=$((10000 + ${node_id##node} - 1))
    local https_port=$((1024 + ${node_id##node} - 1))
    local authrpc_port=$((8551 + ${node_id##node} - 1))
    local p2p_port_reth=$((12024 + ${node_id##node} - 1))
    
    # Export variables for envsubst
    # Note: DATA_DIR and CONFIG_DIR used in templates
    export NODE_ID="$node_id"
    export HOST="$host"
    export P2P_PORT="$p2p_port"
    export RPC_PORT="$rpc_port"
    export METRICS_PORT="$metrics_port"
    export DATA_DIR="$data_dir"
    export CONFIG_DIR="$config_dir"
    export GENESIS_PATH="$genesis_path"
    export VFN_PORT="$vfn_port"
    export INSPECTION_PORT="$inspection_port"
    export HTTPS_PORT="$https_port"
    export AUTHRPC_PORT="$authrpc_port"
    export P2P_PORT_RETH="$p2p_port_reth"
    export BINARY_PATH="$binary_path"
    
    # Generate validator.yaml from template
    envsubst < "$SCRIPT_DIR/templates/validator.yaml.tpl" > "$config_dir/validator.yaml"
    
    # Generate reth_config.json from template
    envsubst < "$SCRIPT_DIR/templates/reth_config.json.tpl" > "$config_dir/reth_config.json"
    
    # Generate start script for this node
    cat > "$data_dir/script/start.sh" << 'START_SCRIPT'
#!/bin/bash
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE="$SCRIPT_DIR/.."

if [ -e "${WORKSPACE}/script/node.pid" ]; then
    pid=$(cat "${WORKSPACE}/script/node.pid")
    if [ -d "/proc/$pid" ]; then
        echo "Node is already running with PID $pid"
        exit 1
    fi
fi

reth_config="${WORKSPACE}/config/reth_config.json"

if ! command -v jq &> /dev/null; then
    echo "Error: 'jq' is required but not installed."
    exit 1
fi

reth_args_array=()
while IFS= read -r key && IFS= read -r value; do
    if [ -z "$value" ] || [ "$value" == "null" ]; then
        reth_args_array+=( "--${key}" )
    else
        reth_args_array+=( "--${key}=${value}" )
    fi
done < <(jq -r '.reth_args | to_entries[] | .key, .value' "$reth_config")

env_vars_array=()
while IFS= read -r key && IFS= read -r value; do
    if [ -n "$value" ] && [ "$value" != "null" ]; then
        env_vars_array+=( "${key}=${value}" )
    fi
done < <(jq -r '.env_vars | to_entries[] | .key, .value' "$reth_config")

export RUST_BACKTRACE=1
pid=$(
    env ${env_vars_array[*]} BINARY_PATH node \
        ${reth_args_array[*]} \
        > "${WORKSPACE}/logs/debug.log" 2>&1 &
    echo $!
)
echo $pid > "${WORKSPACE}/script/node.pid"
echo "Started node with PID $pid"
START_SCRIPT

    # Replace BINARY_PATH placeholder
    sed -i "s|BINARY_PATH|$binary_path|g" "$data_dir/script/start.sh"
    chmod +x "$data_dir/script/start.sh"
    
    # Generate stop script
    cat > "$data_dir/script/stop.sh" << 'STOP_SCRIPT'
#!/bin/bash
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE="$SCRIPT_DIR/.."

if [ -e "${WORKSPACE}/script/node.pid" ]; then
    pid=$(cat "${WORKSPACE}/script/node.pid")
    if [ -d "/proc/$pid" ]; then
        kill "$pid"
        echo "Stopped node (PID: $pid)"
    else
        echo "Node not running (stale PID file)"
    fi
    rm -f "${WORKSPACE}/script/node.pid"
else
    echo "No PID file found"
fi
STOP_SCRIPT
    chmod +x "$data_dir/script/stop.sh"
}


main() {
    if [ ! -f "$CONFIG_FILE" ]; then
        log_error "Config file not found: $CONFIG_FILE"
        exit 1
    fi
    
    if [ ! -d "$OUTPUT_DIR" ]; then
        log_error "Artifacts directory not found: $OUTPUT_DIR"
        log_error "Please run 'make init' first."
        exit 1
    fi

    log_info "Deploying from $OUTPUT_DIR using config $CONFIG_FILE"
    export CONFIG_FILE
    
    # Parse TOML
    config_json=$(parse_toml)
    
    base_dir=$(echo "$config_json" | jq -r '.cluster.base_dir')
    binary_path=$(echo "$config_json" | jq -r '.build.binary_path')
    
    # Resolve binary path
    if [[ "$binary_path" != /* ]]; then
        binary_path="$(cd "$SCRIPT_DIR" && realpath "$binary_path")"
    fi
     
    # Find/Validate binary logic (reused from init.sh concept)
    if [ ! -f "$binary_path" ]; then
        log_warn "Configured binary not found at: $binary_path"
        FOUND_BIN=$(find_binary "gravity_node" "$PROJECT_ROOT") || true
        if [ -n "$FOUND_BIN" ]; then
            binary_path="$FOUND_BIN"
            log_info "Found binary at: $binary_path"
        else
            log_error "gravity_node binary not found. Build it first."
            exit 1
        fi
    fi

    # Clean old environment
    if [ -d "$base_dir" ]; then
        log_warn "Cleaning old environment at $base_dir..."
        rm -rf "$base_dir"
    fi
    mkdir -p "$base_dir"
    
    # Deploy Genesis
    if [ -f "$OUTPUT_DIR/genesis.json" ]; then
        cp "$OUTPUT_DIR/genesis.json" "$base_dir/genesis.json"
    else
        log_error "Genesis file not found in artifacts!"
        exit 1
    fi
    genesis_path="$base_dir/genesis.json"
    
    # Deploy Nodes
    node_count=$(echo "$config_json" | jq '.nodes | length')
    log_info "Deploying $node_count nodes..."
    
    for i in $(seq 0 $((node_count - 1))); do
        node=$(echo "$config_json" | jq ".nodes[$i]")
        node_id=$(echo "$node" | jq -r '.id')
        host=$(echo "$node" | jq -r '.host')
        p2p_port=$(echo "$node" | jq -r '.p2p_port')
        rpc_port=$(echo "$node" | jq -r '.rpc_port')
        metrics_port=$(echo "$node" | jq -r '.metrics_port')
        data_dir=$(echo "$node" | jq -r '.data_dir // empty')
        
        if [ -z "$data_dir" ]; then
            data_dir="$base_dir/$node_id"
        fi
        
        # Prepare dirs
        mkdir -p "$data_dir"/{config,data,logs,execution_logs,consensus_log,script}
        
        
        # Artifact sources
        identity_src="$OUTPUT_DIR/$node_id/config/validator-identity.yaml"
        waypoint_src="$OUTPUT_DIR/waypoint.txt"
        
        if [ ! -f "$identity_src" ]; then
            log_error "Identity not found for $node_id at $identity_src"
            exit 1
        fi
        
        configure_node \
            "$node_id" \
            "$host" \
            "$p2p_port" \
            "$rpc_port" \
            "$metrics_port" \
            "$data_dir" \
            "$genesis_path" \
            "$binary_path" \
            "$identity_src" \
            "$waypoint_src"
    done
    
    log_success "Deployment complete! Environment ready at $base_dir"
}

log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }

main "$@"
