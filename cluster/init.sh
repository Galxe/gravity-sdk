#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_FILE="${1:-$SCRIPT_DIR/cluster.toml}"
OUTPUT_DIR="$SCRIPT_DIR/output"

source "$SCRIPT_DIR/utils/common.sh"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Function: Prepare Node Keys
prepare_node_keys() {
    local node_id="$1"
    local node_config_dir="$2"
    
    mkdir -p "$node_config_dir"
    local identity_file="$node_config_dir/validator-identity.yaml"
    
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
    log_info "Initializing cluster artifacts..."
    
    if [ ! -f "$CONFIG_FILE" ]; then
        log_error "Config file not found: $CONFIG_FILE"
        exit 1
    fi
    export CONFIG_FILE
    
    # Clean output dir
    rm -rf "$OUTPUT_DIR"
    mkdir -p "$OUTPUT_DIR"
    
    # Parse TOML
    config_json=$(parse_toml)
    
    # Need gravity_cli
    GRAVITY_CLI=$(find_binary "gravity_cli" "$PROJECT_ROOT") || true
    if [ -z "$GRAVITY_CLI" ]; then
        log_error "gravity_cli not found! Please build it first."
        exit 1
    fi
    log_info "Using gravity_cli: $GRAVITY_CLI"
    
    # Step 1: Keys
    log_info "Step 1: generating node keys..."
    node_count=$(echo "$config_json" | jq '.nodes | length')
    
    for i in $(seq 0 $((node_count - 1))); do
        node=$(echo "$config_json" | jq ".nodes[$i]")
        node_id=$(echo "$node" | jq -r '.id')
        
        # Structure: output/nodeX/config/validator-identity.yaml
        # This structure matches what aggregate_genesis.py expects relative to a base_dir
        node_conf_out="$OUTPUT_DIR/$node_id/config"
        prepare_node_keys "$node_id" "$node_conf_out"
    done
    
    # Step 2: Aggregate
    log_info "Step 2: Aggregating configuration..."
    
    # We patch the 'base_dir' in the config JSON to point to our OUTPUT_DIR
    # This tricks aggregate_genesis.py into looking for keys in OUTPUT_DIR
    modified_json=$(echo "$config_json" | jq --arg out "$OUTPUT_DIR" '.cluster.base_dir = $out')
    
    python3 "$SCRIPT_DIR/utils/aggregate_genesis.py" "$modified_json"
    
    val_genesis_path="$OUTPUT_DIR/validator_genesis.json"
    if [ ! -f "$val_genesis_path" ]; then
        log_error "Failed to generate $val_genesis_path"
        exit 1
    fi
    
    # Step 3: Genesis Generation (Call External)
    GEN_SCRIPT="$PROJECT_ROOT/external/gravity-genesis-contract/generate_genesis.sh"
    EXTERNAL_DIR="$PROJECT_ROOT/external"
    GENESIS_CONTRACT_DIR="$EXTERNAL_DIR/gravity-genesis-contract"
    GENESIS_REPO="https://github.com/Galxe/gravity-genesis-contract.git"  
    
    # Auto-clone
    if [ ! -d "$GENESIS_CONTRACT_DIR" ]; then
        log_warn "gravity-genesis-contract not found. Cloning..."
        mkdir -p "$EXTERNAL_DIR"
        git clone "$GENESIS_REPO" "$GENESIS_CONTRACT_DIR"
    fi
    
    if [ -f "$GEN_SCRIPT" ]; then
         if ! command -v forge &> /dev/null; then
             log_error "Forge not found! verify dependency."
             exit 1
         fi
         
         log_info "Step 3: Generating genesis from contract..."
         GEN_DIR="$(dirname "$GEN_SCRIPT")"
         ABS_VAL_GENESIS_PATH="$(cd "$(dirname "$val_genesis_path")" && pwd)/$(basename "$val_genesis_path")"
         
         (
            cd "$GEN_DIR"
            ./generate_genesis.sh --config "$ABS_VAL_GENESIS_PATH" > /dev/null
         )
         
         if [ -f "$GEN_DIR/genesis.json" ]; then
             cp "$GEN_DIR/genesis.json" "$OUTPUT_DIR/genesis.json"
             log_info "Genesis generated at $OUTPUT_DIR/genesis.json"
         else
             log_error "Genesis generation failed."
             exit 1
         fi
    else
        log_error "Genesis generation script missing even after clone attempt."
        exit 1
    fi

    # Step 4: Waypoint
    log_info "Step 4: Generating waypoint..."
    "$GRAVITY_CLI" genesis generate-waypoint \
        --input-file="$val_genesis_path" \
        --output-file="$OUTPUT_DIR/waypoint.txt"
        
    log_info "Init complete! Artifacts in $OUTPUT_DIR"
    log_info "Run 'make deploy' next."
}

main "$@"
