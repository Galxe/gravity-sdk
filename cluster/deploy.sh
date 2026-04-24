#!/bin/bash
set -eo pipefail

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

# Resolve a binary from a source JSON object.
# The source object supports three types (like Cargo.toml):
#   { "bin_path": "/path/to/gravity_node" }         — pre-built binary
#   { "project_path": "../" }                         — local project, cargo build
#   { "github": "Owner/Repo", "rev": "v1.0.0" }       — clone + cargo build
# Args: $1=source_json, $2=cache_dir, $3=label (for logs)
# Prints the resolved binary path to stdout.
resolve_source() {
    # Wrapper: _resolve_source_impl prints log_info to stdout (via echo),
    # but we only want the binary path on stdout. So we redirect impl's
    # stdout→stderr and capture the binary path via fd3.
    local _result
    _result=$(_resolve_source_impl "$@" 3>&1 1>&2)
    echo "$_result"
}

_resolve_source_impl() {
    local source_json="$1"
    local cache_dir="$2"
    local label="${3:-build}"

    local src_bin_path src_project_path src_github src_rev
    src_bin_path=$(echo "$source_json" | jq -r '.bin_path // empty')
    src_project_path=$(echo "$source_json" | jq -r '.project_path // empty')
    src_github=$(echo "$source_json" | jq -r '.github // empty')
    src_rev=$(echo "$source_json" | jq -r '.rev // empty')

    # Type 1: pre-built binary
    if [ -n "$src_bin_path" ]; then
        if [[ "$src_bin_path" != /* ]]; then
            src_bin_path="$(cd "$SCRIPT_DIR" && realpath "$src_bin_path")"
        fi
        if [ ! -f "$src_bin_path" ]; then
            log_error "[$label] Binary not found: $src_bin_path"
            exit 1
        fi
        log_info "[$label] Using binary: $src_bin_path"
        echo "$src_bin_path" >&3
        return
    fi

    # Type 2: local project — cargo build
    if [ -n "$src_project_path" ]; then
        if [[ "$src_project_path" != /* ]]; then
            src_project_path="$(cd "$SCRIPT_DIR" && realpath "$src_project_path")"
        fi
        if [ ! -f "$src_project_path/Cargo.toml" ]; then
            log_error "[$label] No Cargo.toml found in project_path: $src_project_path"
            exit 1
        fi
        local built_binary="$src_project_path/target/quick-release/gravity_node"
        if [ ! -f "$built_binary" ]; then
            log_info "[$label] Building gravity_node from $src_project_path..."
            RUSTFLAGS="--cfg tokio_unstable" cargo build \
                --manifest-path "$src_project_path/Cargo.toml" \
                --bin gravity_node \
                --profile quick-release 2>&1 | tail -20
        fi
        if [ ! -f "$built_binary" ]; then
            log_error "[$label] Build failed: $built_binary not found"
            exit 1
        fi
        log_info "[$label] Using built binary: $built_binary"
        echo "$built_binary" >&3
        return
    fi

    # Type 3: github clone + build
    if [ -n "$src_github" ] && [ -n "$src_rev" ]; then
        local safe_rev
        safe_rev=$(echo "$src_rev" | tr '/' '_')
        local repo_cache="$cache_dir/${src_github//\//_}-${safe_rev}"
        local cached_binary="$repo_cache/target/quick-release/gravity_node"

        if [ -f "$cached_binary" ]; then
            log_info "[$label] Using cached build: $cached_binary"
            echo "$cached_binary" >&3
            return
        fi

        local clone_url
        if [ -n "${GITHUB_TOKEN:-}" ]; then
            clone_url="https://x-access-token:${GITHUB_TOKEN}@github.com/${src_github}.git"
        else
            clone_url="https://github.com/${src_github}.git"
        fi

        log_info "[$label] Cloning ${src_github} @ ${src_rev}..."
        if [ ! -d "$repo_cache/.git" ]; then
            mkdir -p "$repo_cache"
            git clone --depth 1 --branch "$src_rev" \
                "$clone_url" "$repo_cache" 2>/dev/null || {
                rm -rf "$repo_cache"
                mkdir -p "$repo_cache"
                git clone "$clone_url" "$repo_cache"
                git -C "$repo_cache" checkout "$src_rev"
            }
        fi

        log_info "[$label] Building gravity_node from source (this may take a while)..."
        RUSTFLAGS="--cfg tokio_unstable" CARGO_TARGET_DIR="$repo_cache/target" cargo build \
            --manifest-path "$repo_cache/Cargo.toml" \
            --bin gravity_node \
            --profile quick-release 2>&1 | tail -20

        if [ ! -f "$cached_binary" ]; then
            log_error "[$label] Build failed: $cached_binary not found"
            exit 1
        fi

        log_info "[$label] Build complete: $cached_binary"
        echo "$cached_binary" >&3
        return
    fi

    log_error "[$label] No valid source configured (need bin_path, project_path, or github+rev)"
    exit 1
}

# Resolve per-node identity source and export template env vars.
# Reads `identity.source` ("file" | "gcp_secret", default "file") and
# `identity.secret` (required when source = gcp_secret) from the node's
# cluster.toml entry. When source = file, materializes identity.yaml under
# $config_dir; when source = gcp_secret, leaves disk clean and wires the
# resource path into validator.yaml via SAFETY_RULES_IDENTITY_* and
# NETWORK_IDENTITY_* envs that the YAML templates expand.
#
# Prereq: gravity_node must be built with `--features gcp-secret-manager`
# when any node uses source = gcp_secret; otherwise the node panics at
# startup with "GCP Secret Manager support not compiled in".
setup_identity() {
    local node_json="$1"
    local node_id="$2"
    local config_dir="$3"
    local identity_src="$4"

    local source secret
    source=$(echo "$node_json" | jq -r '.identity.source // "file"')
    secret=$(echo "$node_json" | jq -r '.identity.secret // empty')

    case "$source" in
        file)
            cp "$identity_src" "$config_dir/identity.yaml"
            export SAFETY_RULES_IDENTITY_VARIANT=from_file
            export SAFETY_RULES_IDENTITY_KEY=identity_blob_path
            export SAFETY_RULES_IDENTITY_VALUE="$config_dir/identity.yaml"
            export NETWORK_IDENTITY_TYPE=from_file
            export NETWORK_IDENTITY_FIELD=path
            export NETWORK_IDENTITY_VALUE="$config_dir/identity.yaml"
            ;;
        gcp_secret)
            if [ -z "$secret" ]; then
                log_error "  [$node_id] identity.source = gcp_secret requires identity.secret (projects/<P>/secrets/<S>[/versions/<V>])"
                exit 1
            fi
            log_info "  [$node_id] identity: GCP Secret Manager ($secret) — not writing identity.yaml to disk"
            export SAFETY_RULES_IDENTITY_VARIANT=from_gcp_secret
            export SAFETY_RULES_IDENTITY_KEY=identity_blob_secret
            export SAFETY_RULES_IDENTITY_VALUE="$secret"
            export NETWORK_IDENTITY_TYPE=from_gcp_secret
            export NETWORK_IDENTITY_FIELD=resource
            export NETWORK_IDENTITY_VALUE="$secret"
            ;;
        *)
            log_error "  [$node_id] unknown identity.source '$source' (expected 'file' or 'gcp_secret')"
            exit 1
            ;;
    esac
}

# Configure node function (Rendering logic)
configure_node() {
    local node_id="$1"
    local data_dir="$2"
    local genesis_path="$3"
    local binary_path="$4"
    local identity_src="$5"
    local waypoint_src="$6"
    local role="$7"
    local node_json="$8"

    local config_dir="$data_dir/config"

    log_info "  [$node_id] [$role] configuring..."

    # Create config dir
    mkdir -p "$config_dir"

    # Resolve identity source (file vs GCP Secret Manager) and waypoint
    setup_identity "$node_json" "$node_id" "$config_dir" "$identity_src"
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

    # Generate reth_config.json from template (supports override via env var, e.g. mainnet hardening)
    local reth_tpl="${RETH_CONFIG_TPL:-$SCRIPT_DIR/templates/reth_config.json.tpl}"
    if [ ! -f "$reth_tpl" ]; then
        log_error "reth config template not found: $reth_tpl"
        exit 1
    fi
    envsubst < "$reth_tpl" > "$config_dir/reth_config.json"
    log_info "  Using reth config: $reth_tpl"

    # Render relayer_config.json from template (supports per-test-case override via env var)
    local relayer_tpl="${RELAYER_CONFIG_TPL:-$SCRIPT_DIR/templates/relayer_config.json.tpl}"
    if [ -f "$relayer_tpl" ]; then
        envsubst < "$relayer_tpl" > "$config_dir/relayer_config.json"
        log_info "  Using relayer config: $relayer_tpl (rpc_url=$RELAYER_RPC_URL)"
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
    if kill -0 "$pid" 2>/dev/null; then
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
    env ${env_vars_array[*]} ${WORKSPACE}/bin/gravity_node node \
        ${reth_args_array[*]} \
        > "${WORKSPACE}/logs/debug.log" 2>&1 &
    echo $!
)
echo $pid > "${WORKSPACE}/script/node.pid"
echo "Started node with PID $pid"
START_SCRIPT

    chmod +x "$data_dir/script/start.sh"
    
    # Generate stop script
    cat > "$data_dir/script/stop.sh" << 'STOP_SCRIPT'
#!/bin/bash
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE="$SCRIPT_DIR/.."

if [ -e "${WORKSPACE}/script/node.pid" ]; then
    pid=$(cat "${WORKSPACE}/script/node.pid")
    if kill -0 "$pid" 2>/dev/null; then
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

# Configure PFN (public-fullnode) function
configure_pfn() {
    local node_id="$1"
    local data_dir="$2"
    local genesis_path="$3"
    local binary_path="$4"
    local identity_src="$5"
    local waypoint_src="$6"

    local config_dir="$data_dir/config"

    log_info "  [$node_id] [pfn] configuring..."

    mkdir -p "$config_dir"
    cp "$identity_src" "$config_dir/identity.yaml"
    cp "$waypoint_src" "$config_dir/waypoint.txt"

    export NODE_ID="$node_id"
    export DATA_DIR="$data_dir"
    export CONFIG_DIR="$config_dir"
    export GENESIS_PATH="$genesis_path"
    export BINARY_PATH="$binary_path"

    # PFN listens on Public network at P2P_PORT (no Vfn network at all).
    envsubst < "$SCRIPT_DIR/templates/public_full_node.yaml.tpl" > "$config_dir/public_full_node.yaml"

    local reth_tpl="${RETH_CONFIG_PFN_TPL:-$SCRIPT_DIR/templates/reth_config_pfn.json.tpl}"
    if [ ! -f "$reth_tpl" ]; then
        log_error "reth pfn config template not found: $reth_tpl"
        exit 1
    fi
    envsubst < "$reth_tpl" > "$config_dir/reth_config.json"
    log_info "  Using reth pfn config: $reth_tpl"

    if [ "$WS_PORT" != "null" ]; then
        local tmp="$config_dir/reth_config.json.tmp"
        jq --argjson port "$WS_PORT" \
           --arg origins "$RPC_WS_ORIGINS" \
           --arg api "$RPC_WS_API" \
           '.reth_args += {"ws":"", "ws.port":$port, "ws.addr":"0.0.0.0", "ws.origins":$origins, "ws.api":$api}' \
           "$config_dir/reth_config.json" > "$tmp" && mv "$tmp" "$config_dir/reth_config.json"
    fi

    cat > "$data_dir/script/start.sh" << 'START_SCRIPT'
#!/bin/bash
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE="$SCRIPT_DIR/.."

if [ -e "${WORKSPACE}/script/node.pid" ]; then
    pid=$(cat "${WORKSPACE}/script/node.pid")
    if kill -0 "$pid" 2>/dev/null; then
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
    env ${env_vars_array[*]} ${WORKSPACE}/bin/gravity_node node \
        ${reth_args_array[*]} \
        > "${WORKSPACE}/logs/debug.log" 2>&1 &
    echo $!
)
echo $pid > "${WORKSPACE}/script/node.pid"
echo "Started PFN node with PID $pid"
START_SCRIPT
    chmod +x "$data_dir/script/start.sh"

    cat > "$data_dir/script/stop.sh" << 'STOP_SCRIPT'
#!/bin/bash
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE="$SCRIPT_DIR/.."

if [ -e "${WORKSPACE}/script/node.pid" ]; then
    pid=$(cat "${WORKSPACE}/script/node.pid")
    if kill -0 "$pid" 2>/dev/null; then
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
    local node_json="$7"

    local config_dir="$data_dir/config"

    log_info "  [$node_id] [vfn] configuring..."

    # Create config dir
    mkdir -p "$config_dir"

    # Resolve identity source (file vs GCP Secret Manager) and waypoint
    setup_identity "$node_json" "$node_id" "$config_dir" "$identity_src"
    cp "$waypoint_src" "$config_dir/waypoint.txt"
    
    # Export paths
    export NODE_ID="$node_id"
    export DATA_DIR="$data_dir"
    export CONFIG_DIR="$config_dir"
    export GENESIS_PATH="$genesis_path"
    export BINARY_PATH="$binary_path"
    
    # Generate validator_full_node.yaml from template
    envsubst < "$SCRIPT_DIR/templates/validator_full_node.yaml.tpl" > "$config_dir/validator_full_node.yaml"

    # Generate reth_config.json from template (supports override via env var)
    local reth_tpl="${RETH_CONFIG_VFN_TPL:-$SCRIPT_DIR/templates/reth_config_vfn.json.tpl}"
    if [ ! -f "$reth_tpl" ]; then
        log_error "reth vfn config template not found: $reth_tpl"
        exit 1
    fi
    envsubst < "$reth_tpl" > "$config_dir/reth_config.json"
    log_info "  Using reth vfn config: $reth_tpl"

    # Optionally enable WebSocket RPC (only when ws_port is set in cluster.toml)
    if [ "$WS_PORT" != "null" ]; then
        local tmp="$config_dir/reth_config.json.tmp"
        jq --argjson port "$WS_PORT" \
           --arg origins "$RPC_WS_ORIGINS" \
           --arg api "$RPC_WS_API" \
           '.reth_args += {"ws":"", "ws.port":$port, "ws.addr":"0.0.0.0", "ws.origins":$origins, "ws.api":$api}' \
           "$config_dir/reth_config.json" > "$tmp" && mv "$tmp" "$config_dir/reth_config.json"
    fi

    # Render relayer_config.json from template
    local relayer_tpl="${RELAYER_CONFIG_TPL:-$SCRIPT_DIR/templates/relayer_config.json.tpl}"
    if [ -f "$relayer_tpl" ]; then
        envsubst < "$relayer_tpl" > "$config_dir/relayer_config.json"
        log_info "  Using relayer config: $relayer_tpl (rpc_url=$RELAYER_RPC_URL)"
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
    if kill -0 "$pid" 2>/dev/null; then
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
    env ${env_vars_array[*]} ${WORKSPACE}/bin/gravity_node node \
        ${reth_args_array[*]} \
        > "${WORKSPACE}/logs/debug.log" 2>&1 &
    echo $!
)
echo $pid > "${WORKSPACE}/script/node.pid"
echo "Started VFN node with PID $pid"
START_SCRIPT

    chmod +x "$data_dir/script/start.sh"
    
    # Generate stop script
    cat > "$data_dir/script/stop.sh" << 'STOP_SCRIPT'
#!/bin/bash
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE="$SCRIPT_DIR/.."

if [ -e "${WORKSPACE}/script/node.pid" ]; then
    pid=$(cat "${WORKSPACE}/script/node.pid")
    if kill -0 "$pid" 2>/dev/null; then
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
    
    # Build artifacts dir for source builds (clone + compile cache)
    local artifacts_dir="$base_dir/artifacts"
    mkdir -p "$artifacts_dir"
    
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
    # Read genesis source paths from config (with defaults)
    genesis_path=$(echo "$config_json" | jq -r '.genesis_source.genesis_path // "./output/genesis.json"')
    waypoint_path=$(echo "$config_json" | jq -r '.genesis_source.waypoint_path // "./output/waypoint.txt"')
    
    # Read relayer RPC URL from config (with default)
    export RELAYER_RPC_URL=$(echo "$config_json" | jq -r '.relayer.relayer_rpc_url // "https://sepolia.drpc.org"')

    # Cluster-level RPC settings (defaults preserve current "open" behavior for dev/e2e)
    export RPC_HTTP_CORSDOMAIN=$(echo "$config_json" | jq -r '.rpc.http_corsdomain // "*"')
    export RPC_HTTP_API=$(echo "$config_json" | jq -r '.rpc.http_api // "debug,eth,net,trace,txpool,web3,rpc"')
    export RPC_WS_ORIGINS=$(echo "$config_json" | jq -r '.rpc.ws_origins // "*"')
    export RPC_WS_API=$(echo "$config_json" | jq -r '.rpc.ws_api // "debug,eth,net,trace,txpool,web3,rpc"')
    
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
    log_info "Scanning node sources..."
    local seen_keys=()
    local seen_vals=()
    for i in $(seq 0 $((node_count - 1))); do
        local src_github src_rev node_id
        node_id=$(echo "$config_json" | jq -r ".nodes[$i].id")
        src_github=$(echo "$config_json" | jq -r ".nodes[$i].source.github // empty")
        src_rev=$(echo "$config_json" | jq -r ".nodes[$i].source.rev // empty")
        if [ -n "$src_github" ] && [ -n "$src_rev" ]; then
            local key="${src_github}@${src_rev}"
            local found=0
            for idx in "${!seen_keys[@]}"; do
                if [ "${seen_keys[$idx]}" == "$key" ]; then
                    seen_vals[$idx]="${seen_vals[$idx]}, $node_id"
                    log_info "  Source: $key (shared, also used by $node_id)"
                    found=1
                    break
                fi
            done
            if [ $found -eq 0 ]; then
                seen_keys+=("$key")
                seen_vals+=("$node_id")
                log_info "  Source: $key (first seen in $node_id)"
            fi
        fi
    done

    # Pre-build unique github sources
    if [ ${#seen_keys[@]} -gt 0 ]; then
        log_info "Pre-building ${#seen_keys[@]} unique github source(s)..."
        for idx in "${!seen_keys[@]}"; do
            local key="${seen_keys[$idx]}"
            local val="${seen_vals[$idx]}"
            local src='{"github":"'"${key%%@*}"'","rev":"'"${key#*@}"'"}'
            log_info "  Building $key (used by: $val)..."
            resolve_source "$src" "$artifacts_dir" "$key" > /dev/null
        done
        log_info "All github sources pre-built."
    fi
    
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
        export WS_PORT=$(echo "$node" | jq -r '.ws_port // "null"')
        export METRICS_PORT=$(echo "$node" | jq -r '.metrics_port')
        export INSPECTION_PORT=$(echo "$node" | jq -r '.inspection_port')
        export HTTPS_PORT=$(echo "$node" | jq -r '.https_port // "null"')
        export AUTHRPC_PORT=$(echo "$node" | jq -r '.authrpc_port')
        export P2P_PORT_RETH=$(echo "$node" | jq -r '.reth_p2p_port')
        
        role=$(echo "$node" | jq -r '.role // empty')

        # Validate role is specified
        if [ -z "$role" ]; then
            log_error "Node $NODE_ID must specify 'role' (genesis, validator, vfn, or pfn)"
            exit 1
        fi

        # Resolve discovery_method. Per-node override wins; otherwise the
        # default is role-driven — validator/vfn use onchain, pfn emits nothing
        # (seed-only). Valid values: onchain | none. Omit to take the default.
        discovery_method=$(echo "$node" | jq -r '.discovery_method // empty')
        if [ -z "$discovery_method" ]; then
            case "$role" in
                genesis|validator|vfn) discovery_method="onchain" ;;
                pfn)                   discovery_method="" ;;
                *)                     discovery_method="" ;;
            esac
        fi
        if [ -n "$discovery_method" ]; then
            export DISCOVERY_METHOD_NETWORK_BLOCK="  discovery_method:
    ${discovery_method}"
            export DISCOVERY_METHOD_FULLNODE_BLOCK="    discovery_method:
      ${discovery_method}"
        else
            export DISCOVERY_METHOD_NETWORK_BLOCK=""
            export DISCOVERY_METHOD_FULLNODE_BLOCK=""
        fi

        data_dir=$(echo "$node" | jq -r '.data_dir // empty')
        if [ -z "$data_dir" ]; then
            data_dir="$base_dir/$NODE_ID"
        fi
        
        # Resolve per-node binary from source config (required)
        local node_source
        node_source=$(echo "$node" | jq -c '.source // empty')
        if [ -z "$node_source" ]; then
            log_error "Node $NODE_ID must specify 'source' (bin_path, project_path, or github+rev)"
            exit 1
        fi
        node_binary=$(resolve_source "$node_source" "$artifacts_dir" "$NODE_ID")

        # Prepare dirs
        mkdir -p "$data_dir"/{bin,config,data,logs,execution_logs,consensus_log,script}
        
        # Create hardlink for gravity_node binary in each node's bin dir
        ln -f "$node_binary" "$data_dir/bin/gravity_node"
        
        waypoint_src="$OUTPUT_DIR/waypoint.txt"

        identity_source=$(echo "$node" | jq -r '.identity.source // "file"')

        if [ "$role" == "pfn" ]; then
            # Public Full Node. Listens on Public network at P2P_PORT; dials a
            # VFN/IVFN via a static seed on the Public network (public_seed_of
            # in cluster.toml). On-chain discovery is enabled but typically
            # points at a Vfn-network address for the validator's fullnode,
            # which is wrong net for PFN — seeds are the dependable path.
            identity_src="$OUTPUT_DIR/$NODE_ID/config/identity.yaml"

            # Only require a local identity.yaml when the node will actually
            # consume it from disk. GCP Secret Manager sources pull at startup.
            if [ "$identity_source" = "file" ] && [ ! -f "$identity_src" ]; then
                log_error "Identity not found for $NODE_ID at $identity_src"
                exit 1
            fi

            if [ "$P2P_PORT" == "null" ]; then
                log_error "PFN $NODE_ID requires p2p_port (Public listener)"
                exit 1
            fi

            public_seed_of=$(echo "$node" | jq -r '.public_seed_of // empty')
            if [ -n "$public_seed_of" ]; then
                target_identity="$OUTPUT_DIR/$public_seed_of/config/identity.yaml"
                if [ ! -f "$target_identity" ]; then
                    log_error "public_seed_of=$public_seed_of but identity not found at $target_identity"
                    exit 1
                fi
                target_peer_id=$(awk -F': ' '/^account_address:/{gsub(/["\x27]/,"",$2); print $2}' "$target_identity")
                target_network_pk=$(awk -F': ' '/^network_public_key:/{gsub(/["\x27]/,"",$2); print $2}' "$target_identity")
                target_peer_id=${target_peer_id#0x}
                target_network_pk=${target_network_pk#0x}
                if [ -z "$target_peer_id" ] || [ -z "$target_network_pk" ]; then
                    log_error "public_seed_of=$public_seed_of: failed to parse peer_id/network_pk from $target_identity"
                    exit 1
                fi
                target_host=$(echo "$config_json" | jq -r --arg id "$public_seed_of" \
                    '.nodes[] | select(.id == $id) | (.internal_host // .host)')
                target_public_port=$(echo "$config_json" | jq -r --arg id "$public_seed_of" \
                    '.nodes[] | select(.id == $id) | .public_port')
                if [ -z "$target_host" ] || [ "$target_host" = "null" ] || \
                   [ -z "$target_public_port" ] || [ "$target_public_port" = "null" ]; then
                    log_error "public_seed_of=$public_seed_of: missing host/public_port in cluster.toml"
                    exit 1
                fi
                target_proto="dns"
                if [[ "$target_host" =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
                    target_proto="ip4"
                fi
                # ValidatorFullNode is the role for the upstream peer that serves
                # this PFN. It lands in `upstream_roles(Public, FullNode)` —
                # see gravity-aptos/config/src/network_id.rs:176-180.
                export PFN_SEEDS_BLOCK="    seeds:
      '0x${target_peer_id}':
        addresses:
          - '/${target_proto}/${target_host}/tcp/${target_public_port}/noise-ik/${target_network_pk}/handshake/0'
        role: ValidatorFullNode"
                log_info "  [$NODE_ID] public_seed_of=$public_seed_of, seeds -> ${target_host}:${target_public_port} (proto=${target_proto})"
            else
                export PFN_SEEDS_BLOCK=""
            fi

            configure_pfn \
                "$NODE_ID" \
                "$data_dir" \
                "$genesis_path" \
                "$node_binary" \
                "$identity_src" \
                "$waypoint_src"
        elif [ "$role" == "vfn" ]; then
            # VFN node
            identity_src="$OUTPUT_DIR/$NODE_ID/config/identity.yaml"

            # Only require a local identity.yaml when the node will actually
            # consume it from disk. GCP Secret Manager sources pull at startup.
            if [ "$identity_source" = "file" ] && [ ! -f "$identity_src" ]; then
                log_error "Identity not found for $NODE_ID at $identity_src"
                exit 1
            fi

            # Optional: build VFN_SEEDS_BLOCK if shadow_of is set.
            # See _local/drafts/vfn-shadow/e2e.md §6.3.
            shadow_of=$(echo "$node" | jq -r '.shadow_of // empty')
            if [ -n "$shadow_of" ]; then
                target_identity="$OUTPUT_DIR/$shadow_of/config/identity.yaml"
                if [ ! -f "$target_identity" ]; then
                    log_error "shadow_of=$shadow_of but identity not found at $target_identity"
                    exit 1
                fi
                target_peer_id=$(awk -F': ' '/^account_address:/{gsub(/["\x27]/,"",$2); print $2}' "$target_identity")
                target_network_pk=$(awk -F': ' '/^network_public_key:/{gsub(/["\x27]/,"",$2); print $2}' "$target_identity")
                target_peer_id=${target_peer_id#0x}
                target_network_pk=${target_network_pk#0x}
                if [ -z "$target_peer_id" ] || [ -z "$target_network_pk" ]; then
                    log_error "shadow_of=$shadow_of: failed to parse peer_id/network_pk from $target_identity"
                    exit 1
                fi
                # Intra-cluster p2p dial uses internal_host first, falls back to host.
                target_host=$(echo "$config_json" | jq -r --arg id "$shadow_of" \
                    '.nodes[] | select(.id == $id) | (.internal_host // .host)')
                target_vfn_port=$(echo "$config_json" | jq -r --arg id "$shadow_of" \
                    '.nodes[] | select(.id == $id) | .vfn_port')
                if [ -z "$target_host" ] || [ "$target_host" = "null" ] || \
                   [ -z "$target_vfn_port" ] || [ "$target_vfn_port" = "null" ]; then
                    log_error "shadow_of=$shadow_of: missing host/vfn_port in cluster.toml"
                    exit 1
                fi
                target_proto="dns"
                if [[ "$target_host" =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
                    target_proto="ip4"
                fi
                export VFN_SEEDS_BLOCK="    seeds:
      '0x${target_peer_id}':
        addresses:
          - '/${target_proto}/${target_host}/tcp/${target_vfn_port}/noise-ik/${target_network_pk}/handshake/0'
        role: Validator"
                log_info "  [$NODE_ID] shadow_of=$shadow_of, seeds -> ${target_host}:${target_vfn_port} (proto=${target_proto})"
            else
                export VFN_SEEDS_BLOCK=""
            fi

            # Optional: second full_node_networks entry on Public so PFN peers
            # can dial this VFN. Only emitted when cluster.toml sets public_port.
            public_port=$(echo "$node" | jq -r '.public_port // empty')
            if [ -n "$public_port" ] && [ "$public_port" != "null" ]; then
                # Public listener is seed-accept-only: no on-chain discovery
                # (fullnode_address encodes the Vfn listener, not this Public
                # port) and no outbound dialing. PFNs reach us via their own
                # static seeds pointing here.
                export PUBLIC_NETWORK_BLOCK="  - network_id: public
    listen_address: \"/ip4/0.0.0.0/tcp/${public_port}\"
    identity:
      type: \"from_file\"
      path: ${data_dir}/config/identity.yaml
    discovery_method:
      none"
                log_info "  [$NODE_ID] Public listener enabled on port ${public_port}"
            else
                export PUBLIC_NETWORK_BLOCK=""
            fi

            configure_vfn \
                "$NODE_ID" \
                "$data_dir" \
                "$genesis_path" \
                "$node_binary" \
                "$identity_src" \
                "$waypoint_src" \
                "$node"
        else
            # Validator node (includes both 'genesis' and 'validator' roles)
            identity_src="$OUTPUT_DIR/$NODE_ID/config/identity.yaml"

            # Only require a local identity.yaml when the node will actually
            # consume it from disk. GCP Secret Manager sources pull at startup.
            if [ "$identity_source" = "file" ] && [ ! -f "$identity_src" ]; then
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
                "$node_binary" \
                "$identity_src" \
                "$waypoint_src" \
                "$role" \
                "$node"
        fi
    done
    
    log_success "Deployment complete! Environment ready at $base_dir"
}

log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }

main "$@"
