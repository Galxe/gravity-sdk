#!/bin/bash
set -e

# Cross-platform sed -i
if [[ "$OSTYPE" == "darwin"* ]]; then
    SED_INPLACE=(sed -i '')
else
    SED_INPLACE=(sed -i)
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_FILE="${1:-$SCRIPT_DIR/cluster.toml}"
OUTPUT_DIR="${GRAVITY_ARTIFACTS_DIR:-$SCRIPT_DIR/output}"

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
    local role="$7"
    
    local config_dir="$data_dir/config"
    
    log_info "  [$node_id] [$role] configuring..."
    
    # Create config dir
    mkdir -p "$config_dir"
    
    # Copy identity and waypoint from artifacts
    cp "$identity_src" "$config_dir/identity.yaml"
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
    
    # Copy relayer_config.json from template (supports per-test-case override via env var)
    local relayer_tpl="${RELAYER_CONFIG_TPL:-$SCRIPT_DIR/templates/relayer_config.json.tpl}"
    if [ -f "$relayer_tpl" ]; then
        cp "$relayer_tpl" "$config_dir/relayer_config.json"
        log_info "  Using relayer config: $relayer_tpl"
    else
        log_warn "  Relayer config template not found: $relayer_tpl (skipping)"
    fi
    
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
    "${SED_INPLACE[@]}" "s|BINARY_PATH|$binary_path|g" "$data_dir/script/start.sh"
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
    local identity_src="$5"
    local waypoint_src="$6"
    
    local config_dir="$data_dir/config"
    
    log_info "  [$node_id] [vfn] configuring..."
    
    # Create config dir
    mkdir -p "$config_dir"
    
    # Copy identity and waypoint from artifacts
    cp "$identity_src" "$config_dir/identity.yaml"
    cp "$waypoint_src" "$config_dir/waypoint.txt"
    
    # Export paths
    export NODE_ID="$node_id"
    export DATA_DIR="$data_dir"
    export CONFIG_DIR="$config_dir"
    export GENESIS_PATH="$genesis_path"
    export BINARY_PATH="$binary_path"
    
    # Generate validator_full_node.yaml from template
    envsubst < "$SCRIPT_DIR/templates/validator_full_node.yaml.tpl" > "$config_dir/validator_full_node.yaml"
    
    # Generate reth_config.json from template
    envsubst < "$SCRIPT_DIR/templates/reth_config_vfn.json.tpl" > "$config_dir/reth_config.json"
    
    # Copy relayer_config.json from template
    cp "$SCRIPT_DIR/templates/relayer_config.json.tpl" "$config_dir/relayer_config.json"
    
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
    "${SED_INPLACE[@]}" "s|BINARY_PATH|$binary_path|g" "$data_dir/script/start.sh"
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
    # Handle existing environment
    if [ -d "$base_dir" ] && [ "$(ls -A "$base_dir" 2>/dev/null)" ]; then
        log_warn "Existing deployment found at $base_dir:"
        ls -1 "$base_dir"
        echo ""
        read -p "[?] Clean old environment before deploying? [y/N] " -n 1 -r
        echo ""
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            log_warn "Cleaning old environment at $base_dir..."
            rm -rf "$base_dir"
        else
            log_info "Keeping existing environment, overwriting configs..."
        fi
    fi
    mkdir -p "$base_dir"
    
    # Find gravity_cli and create hardlink (or copy if cross-device)
    gravity_cli_path=$(find_binary "gravity_cli" "$PROJECT_ROOT") || true
    if [ -n "$gravity_cli_path" ]; then
        log_info "Found gravity_cli at: $gravity_cli_path"
        log_info "Copying gravity_cli to $base_dir..."
        cp "$gravity_cli_path" "$base_dir/gravity_cli"
        export GRAVITY_CLI="$gravity_cli_path"
    else
        log_error "gravity_cli not found - VFN identity generation may fail and hardlink not created"
        exit 1
    fi
    
    # Copy gravity_node binary (self-contained deployment)
    log_info "Copying gravity_node to $base_dir..."
    cp "$binary_path" "$base_dir/gravity_node"
    local_binary_path="$base_dir/gravity_node"
    
    # Read genesis source paths from config (with defaults)
    genesis_path=$(echo "$config_json" | jq -r '.genesis_source.genesis_path // "./output/genesis.json"')
    waypoint_path=$(echo "$config_json" | jq -r '.genesis_source.waypoint_path // "./output/waypoint.txt"')
    
    # Resolve relative paths (relative to CONFIG_FILE location, not SCRIPT_DIR)
    local config_dir="$(dirname "$CONFIG_FILE")"
    if [[ "$genesis_path" != /* ]]; then
        genesis_path="$(cd "$config_dir" && realpath "$genesis_path" 2>/dev/null || echo "$genesis_path")"
    fi
    if [[ "$waypoint_path" != /* ]]; then
        waypoint_path="$(cd "$config_dir" && realpath "$waypoint_path" 2>/dev/null || echo "$waypoint_path")"
    fi
    
    # Deploy Genesis
    if [ -f "$genesis_path" ]; then
        cp "$genesis_path" "$base_dir/genesis.json"
        log_info "Deployed genesis from: $genesis_path"
    else
        log_error "Genesis file not found: $genesis_path"
        exit 1
    fi
    genesis_path="$base_dir/genesis.json"
    
    node_count=$(echo "$config_json" | jq '.nodes | length')
    
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
        
        role=$(echo "$node" | jq -r '.role // empty')
        
        # Validate role is specified
        if [ -z "$role" ]; then
            log_error "Node $NODE_ID must specify 'role' (genesis, validator, or vfn)"
            exit 1
        fi
        
        data_dir=$(echo "$node" | jq -r '.data_dir // empty')
        if [ -z "$data_dir" ]; then
            data_dir="$base_dir/$NODE_ID"
        fi
        
        # Prepare dirs
        mkdir -p "$data_dir"/{config,data,logs,execution_logs,consensus_log,script}
        
        waypoint_src="$OUTPUT_DIR/waypoint.txt"
        
        if [ "$role" == "vfn" ]; then
            # VFN node
            identity_src="$OUTPUT_DIR/$NODE_ID/config/identity.yaml"
            
            if [ ! -f "$identity_src" ]; then
                log_error "Identity not found for $NODE_ID at $identity_src"
                exit 1
            fi
            
            configure_vfn \
                "$NODE_ID" \
                "$data_dir" \
                "$genesis_path" \
                "$local_binary_path" \
                "$identity_src" \
                "$waypoint_src"
        else
            # Validator node (includes both 'genesis' and 'validator' roles)
            identity_src="$OUTPUT_DIR/$NODE_ID/config/identity.yaml"
            
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
                "$local_binary_path" \
                "$identity_src" \
                "$waypoint_src" \
                "$role"
        fi
    done
    
    log_success "Deployment complete! Environment ready at $base_dir"
}

log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }

main "$@"
