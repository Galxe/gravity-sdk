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
    local data_dir="$2"
    local genesis_path="$3"
    local binary_path="$4"
    local identity_src="$5"
    local waypoint_src="$6"
    
    local config_dir="$data_dir/config"
    
    log_info "  [$node_id] configuring..."
    
    # Create config dir
    mkdir -p "$config_dir"
    
    # Copy identity and waypoint from artifacts
    cp "$identity_src" "$config_dir/validator-identity.yaml"
    cp "$waypoint_src" "$config_dir/waypoint.txt"
    
    # Export paths validation
    # (Port variables HOST, P2P_PORT etc expected to be exported by caller)
    export NODE_ID="$node_id"
    export DATA_DIR="$data_dir"
    export CONFIG_DIR="$config_dir"
    export GENESIS_PATH="$genesis_path"
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

# Configure VFN node function
configure_vfn() {
    local node_id="$1"
    local data_dir="$2"
    local genesis_path="$3"
    local binary_path="$4"
    local waypoint_src="$5"
    local seed_peer_id="$6"
    local seed_host="$7"
    local seed_vfn_port="$8"
    local seed_network_pk="$9"
    
    local config_dir="$data_dir/config"
    
    log_info "  [$node_id] configuring VFN..."
    
    # Create config dir
    mkdir -p "$config_dir"
    
    # Copy waypoint from artifacts
    cp "$waypoint_src" "$config_dir/waypoint.txt"
    
    # Generate VFN identity if not exists
    local identity_file="$config_dir/vfn-identity.yaml"
    if [ ! -f "$identity_file" ]; then
        log_info "  [$node_id] Generating VFN identity..."
        "$GRAVITY_CLI" genesis generate-key --output-file="$identity_file" > /dev/null
    fi
    
    # Export paths
    export NODE_ID="$node_id"
    export DATA_DIR="$data_dir"
    export CONFIG_DIR="$config_dir"
    export GENESIS_PATH="$genesis_path"
    export BINARY_PATH="$binary_path"
    export SEED_PEER_ID="$seed_peer_id"
    export SEED_HOST="$seed_host"
    export SEED_VFN_PORT="$seed_vfn_port"
    export SEED_NETWORK_PK="$seed_network_pk"
    
    # Generate validator_full_node.yaml from template
    envsubst < "$SCRIPT_DIR/templates/validator_full_node.yaml.tpl" > "$config_dir/validator_full_node.yaml"
    
    # Generate reth_config.json from template
    envsubst < "$SCRIPT_DIR/templates/reth_config_vfn.json.tpl" > "$config_dir/reth_config.json"
    
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
echo "Started VFN node with PID $pid"
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
     
    # Find/Validate binary checking
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
    
    # Find gravity_cli for VFN identity generation
    GRAVITY_CLI=$(find_binary "gravity_cli" "$PROJECT_ROOT") || true
    if [ -z "$GRAVITY_CLI" ]; then
        log_warn "gravity_cli not found - VFN identity generation may fail"
    fi
    export GRAVITY_CLI
    
    # Build validator info map (for VFN seed lookup)
    declare -A VALIDATOR_INFO
    node_count=$(echo "$config_json" | jq '.nodes | length')
    
    for i in $(seq 0 $((node_count - 1))); do
        node=$(echo "$config_json" | jq ".nodes[$i]")
        node_id=$(echo "$node" | jq -r '.id')
        role=$(echo "$node" | jq -r '.role // "validator"')
        
        if [ "$role" == "validator" ]; then
            host=$(echo "$node" | jq -r '.host')
            vfn_port=$(echo "$node" | jq -r '.vfn_port')
            identity_file="$OUTPUT_DIR/$node_id/config/validator-identity.yaml"
            
            if [ -f "$identity_file" ]; then
                peer_id=$(grep "account_address:" "$identity_file" | awk '{print $2}')
                # Extract network_public_key (strip 0x prefix if present)
                network_pk=$(grep "network_public_key:" "$identity_file" | awk '{print $2}' | sed 's/^0x//')
                VALIDATOR_INFO["$node_id"]="$peer_id|$host|$vfn_port|$network_pk"
            fi
        fi
    done
    
    # Deploy Nodes
    log_info "Deploying $node_count nodes..."
    
    for i in $(seq 0 $((node_count - 1))); do
        node=$(echo "$config_json" | jq ".nodes[$i]")
        
        # Extract and Export config
        export NODE_ID=$(echo "$node" | jq -r '.id')
        export HOST=$(echo "$node" | jq -r '.host')
        export P2P_PORT=$(echo "$node" | jq -r '.p2p_port')
        export VFN_PORT=$(echo "$node" | jq -r '.vfn_port // "null"')
        export RPC_PORT=$(echo "$node" | jq -r '.rpc_port')
        export METRICS_PORT=$(echo "$node" | jq -r '.metrics_port')
        export INSPECTION_PORT=$(echo "$node" | jq -r '.inspection_port')
        export HTTPS_PORT=$(echo "$node" | jq -r '.https_port // "null"')
        export AUTHRPC_PORT=$(echo "$node" | jq -r '.authrpc_port')
        export P2P_PORT_RETH=$(echo "$node" | jq -r '.reth_p2p_port')
        
        role=$(echo "$node" | jq -r '.role // "validator"')
        
        data_dir=$(echo "$node" | jq -r '.data_dir // empty')
        if [ -z "$data_dir" ]; then
            data_dir="$base_dir/$NODE_ID"
        fi
        
        # Prepare dirs
        mkdir -p "$data_dir"/{config,data,logs,execution_logs,consensus_log,script}
        
        waypoint_src="$OUTPUT_DIR/waypoint.txt"
        
        if [ "$role" == "vfn" ]; then
            # VFN node
            attached_to=$(echo "$node" | jq -r '.attached_to')
            
            if [ -z "$attached_to" ] || [ "$attached_to" == "null" ]; then
                log_error "VFN node $NODE_ID must specify 'attached_to' (validator node id)"
                exit 1
            fi
            
            if [ -z "${VALIDATOR_INFO[$attached_to]}" ]; then
                log_error "Validator '$attached_to' not found for VFN $NODE_ID"
                exit 1
            fi
            
            IFS='|' read -r seed_peer_id seed_host seed_vfn_port seed_network_pk <<< "${VALIDATOR_INFO[$attached_to]}"
            
            configure_vfn \
                "$NODE_ID" \
                "$data_dir" \
                "$genesis_path" \
                "$binary_path" \
                "$waypoint_src" \
                "$seed_peer_id" \
                "$seed_host" \
                "$seed_vfn_port" \
                "$seed_network_pk"
        else
            # Validator node
            identity_src="$OUTPUT_DIR/$NODE_ID/config/validator-identity.yaml"
            
            if [ ! -f "$identity_src" ]; then
                log_error "Identity not found for $NODE_ID at $identity_src"
                exit 1
            fi
            
            # Validate required ports (simple check)
            if [ "$P2P_PORT" == "null" ] || [ "$VFN_PORT" == "null" ]; then
                 log_error "Missing required ports in config for $NODE_ID"
                 exit 1
            fi
            
            configure_node \
                "$NODE_ID" \
                "$data_dir" \
                "$genesis_path" \
                "$binary_path" \
                "$identity_src" \
                "$waypoint_src"
        fi
    done
    
    log_success "Deployment complete! Environment ready at $base_dir"
}

log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }

main "$@"
