#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUTPUT_DIR="${GRAVITY_ARTIFACTS_DIR:-$SCRIPT_DIR/output}"

source "$SCRIPT_DIR/utils/common.sh"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Function: Prepare Node Keys
prepare_node_keys() {
    local node_id="$1"
    local node_config_dir="$2"
    
    mkdir -p "$node_config_dir"
    local identity_file="$node_config_dir/identity.yaml"
    
    log_info "  [$node_id] Checking identity..."
    
    if [ -f "$identity_file" ]; then
        if grep -q "consensus_public_key" "$identity_file"; then
             log_info "  [$node_id] Identity key exists."
             return 0
        else
             log_warn "  [$node_id] Old key format detected. Regenerating..."
        fi
    fi
    
    log_info "  [$node_id] Generating identity key..."
    "$GRAVITY_CLI" genesis generate-key --output-file="$identity_file" > /dev/null
}

main() {
    log_info "Initializing node keys..."
    
    # Create output dir if not exists
    mkdir -p "$OUTPUT_DIR"
    
    # Find gravity_cli
    GRAVITY_CLI=$(find_binary "gravity_cli" "$PROJECT_ROOT") || true
    if [ -z "$GRAVITY_CLI" ]; then
        log_error "gravity_cli not found! Please build it first."
        exit 1
    fi
    log_info "Using gravity_cli: $GRAVITY_CLI"
    
    # Accept genesis.toml path as argument, or use defaults
    local input_config="${1:-}"
    local config_dir=""
    
    if [ -n "$input_config" ] && [ -f "$input_config" ]; then
        config_dir="$(dirname "$input_config")"
    else
        config_dir="$SCRIPT_DIR"
    fi
    
    # Try genesis.toml first (for genesis validators), then fall back to cluster.toml
    GENESIS_CONFIG="$config_dir/genesis.toml"
    CLUSTER_CONFIG="$config_dir/cluster.toml"
    
    # If input is explicitly a genesis.toml or cluster.toml, use it directly
    if [ -n "$input_config" ] && [ -f "$input_config" ]; then
        if [[ "$input_config" == *"genesis.toml" ]]; then
            GENESIS_CONFIG="$input_config"
        else
            CLUSTER_CONFIG="$input_config"
        fi
    fi
    
    nodes_to_process=()
    
    if [ -f "$GENESIS_CONFIG" ]; then
        log_info "Reading genesis validators from genesis.toml..."
        export CONFIG_FILE="$GENESIS_CONFIG"
        config_json=$(parse_toml)
        
        # Get genesis validators
        genesis_validators=$(echo "$config_json" | jq -c '.genesis_validators // []')
        validator_count=$(echo "$genesis_validators" | jq 'length')
        
        for i in $(seq 0 $((validator_count - 1))); do
            node_id=$(echo "$genesis_validators" | jq -r ".[$i].id")
            nodes_to_process+=("$node_id")
        done
        log_info "Found $validator_count genesis validators"
    fi
    
    if [ -f "$CLUSTER_CONFIG" ]; then
        log_info "Reading nodes from cluster.toml..."
        export CONFIG_FILE="$CLUSTER_CONFIG"
        config_json=$(parse_toml)
        
        # Get nodes that need validator identity (genesis or validator role)
        node_count=$(echo "$config_json" | jq '.nodes | length')
        
        for i in $(seq 0 $((node_count - 1))); do
            node=$(echo "$config_json" | jq ".nodes[$i]")
            node_id=$(echo "$node" | jq -r '.id')
            role=$(echo "$node" | jq -r '.role // empty')
            
            # Add to list if not already there (from genesis.toml)
            if [[ ! " ${nodes_to_process[*]} " =~ " ${node_id} " ]]; then
                nodes_to_process+=("$node_id")
            fi
        done
    fi
    
    if [ ${#nodes_to_process[@]} -eq 0 ]; then
        log_error "No nodes found! Create genesis.toml or cluster.toml first."
        log_info "Copy genesis.toml.example to genesis.toml and/or cluster.toml.example to cluster.toml"
        exit 1
    fi
    
    # Generate keys for all nodes
    log_info "Generating keys for ${#nodes_to_process[@]} nodes..."
    for node_id in "${nodes_to_process[@]}"; do
        node_conf_out="$OUTPUT_DIR/$node_id/config"
        prepare_node_keys "$node_id" "$node_conf_out"
    done
    
    echo ""
    log_info "Init complete!"
    log_info "Node keys generated in: $OUTPUT_DIR"
    log_info "Run 'make genesis' next to generate genesis.json."
}

main "$@"
