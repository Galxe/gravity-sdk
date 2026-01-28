#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_FILE="${1:-$SCRIPT_DIR/cluster.toml}"
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
    
    # Step 1: Keys (only for validator nodes)
    log_info "Step 1: generating node keys..."
    node_count=$(echo "$config_json" | jq '.nodes | length')
    
    for i in $(seq 0 $((node_count - 1))); do
        node=$(echo "$config_json" | jq ".nodes[$i]")
        node_id=$(echo "$node" | jq -r '.id')
        role=$(echo "$node" | jq -r '.role // empty')
        
        # Validate role is specified
        if [ -z "$role" ]; then
            log_error "Node $node_id must specify 'role' (genesis, validator, or vfn)"
            exit 1
        fi
        
        # Skip VFN nodes - they don't need validator identity
        # Both 'genesis' and 'validator' roles need validator identity
        if [ "$role" == "vfn" ]; then
            log_info "  [$node_id] Skipping VFN node (no validator identity needed)"
            continue
        fi
        
        # Structure: output/nodeX/config/identity.yaml
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
    
    # Organize Config Files
    GEN_CONFIG_DIR="$OUTPUT_DIR/genesis_config"
    mkdir -p "$GEN_CONFIG_DIR"
    
    val_genesis_path="$GEN_CONFIG_DIR/validator_genesis.json"
    if [ -f "$OUTPUT_DIR/validator_genesis.json" ]; then
        mv "$OUTPUT_DIR/validator_genesis.json" "$val_genesis_path"
    else
        log_error "Failed to generate validator_genesis.json"
        exit 1
    fi
    
    if [ -f "$OUTPUT_DIR/faucet_alloc.json" ]; then
        mv "$OUTPUT_DIR/faucet_alloc.json" "$GEN_CONFIG_DIR/faucet_alloc.json"
    fi

    # Step 3: Genesis Generation (Call External)
    GEN_SCRIPT="$PROJECT_ROOT/external/gravity_chain_core_contracts/scripts/generate_genesis.sh"
    EXTERNAL_DIR="$PROJECT_ROOT/external"
    GENESIS_CONTRACT_DIR="$EXTERNAL_DIR/gravity_chain_core_contracts"
    
    # Read repo config from cluster.toml (with defaults)
    GENESIS_REPO=$(echo "$config_json" | jq -r '.dependencies.genesis_contracts.repo // "https://github.com/Galxe/gravity_chain_core_contracts.git"')
    GENESIS_REF=$(echo "$config_json" | jq -r '.dependencies.genesis_contracts.ref // "main"')
    
    # Auto-clone
    if [ ! -d "$GENESIS_CONTRACT_DIR" ]; then
        log_warn "gravity_chain_core_contracts not found. Cloning from $GENESIS_REPO..."
        mkdir -p "$EXTERNAL_DIR"
        git clone "$GENESIS_REPO" "$GENESIS_CONTRACT_DIR"
    fi
    
    # Checkout specified ref (commit, branch, or tag)

    # Auto-install dependencies if missing
    if [ ! -d "$GENESIS_CONTRACT_DIR/node_modules" ]; then
        log_info "Installing dependencies for gravity_chain_core_contracts..."
        (
            cd "$GENESIS_CONTRACT_DIR"
            if command -v yarn &> /dev/null; then
                yarn install
            elif command -v npm &> /dev/null; then
                npm install
            else
                log_error "Neither yarn nor npm found. Cannot install dependencies."
                exit 1
            fi
        )
    fi
    
    if [ -f "$GEN_SCRIPT" ]; then
         if ! command -v forge &> /dev/null; then
             log_error "Forge not found! verify dependency."
             exit 1
         fi
         
         log_info "Step 3: Generating genesis from contract..."
         GEN_DIR="$(dirname "$GEN_SCRIPT")"
         ABS_VAL_GENESIS_PATH="$(cd "$(dirname "$val_genesis_path")" && pwd)/$(basename "$val_genesis_path")"
         
       # Prepare genesis template (inject faucet if present)
    local genesis_template="$GENESIS_CONTRACT_DIR/genesis-tool/config/genesis_template.json"
    local faucet_alloc="$GEN_CONFIG_DIR/faucet_alloc.json"
    local final_template="$GEN_CONFIG_DIR/genesis_template_merged.json"
    
    if [ -f "$faucet_alloc" ]; then
        log_info "Injecting faucet allocation from cluster.toml..."
        # Use jq to merge faucet alloc into the template's alloc
        jq -s '.[0] * {alloc: (.[0].alloc + .[1])}' "$genesis_template" "$faucet_alloc" > "$final_template"
        genesis_template="$final_template"
    fi

    # Generate Genesis using the (possibly modified) template
    log_info "Generating genesis block..."
    cd "$GENESIS_CONTRACT_DIR"
    
    # Run the generation script with the custom template
    CONFIG_FILE="$ABS_VAL_GENESIS_PATH" \
    OUTPUT_DIR="$GENESIS_CONTRACT_DIR/output" \
    ./scripts/generate_genesis.sh \
        --config "$ABS_VAL_GENESIS_PATH" \
        --template "$genesis_template"

    log_info "Deploying Genesis..."
    
    # Copy artifacts back
    if [ -f "$GENESIS_CONTRACT_DIR/genesis.json" ]; then
        cp "$GENESIS_CONTRACT_DIR/genesis.json" "$OUTPUT_DIR/genesis.json"
        
        # Copy intermediate files for verification
        if [ -f "$GENESIS_CONTRACT_DIR/output/genesis_config_modified.json" ]; then
            cp "$GENESIS_CONTRACT_DIR/output/genesis_config_modified.json" "$GEN_CONFIG_DIR/debug_genesis_config.json"
        fi
        if [ -f "$GENESIS_CONTRACT_DIR/account_alloc.json" ]; then
            cp "$GENESIS_CONTRACT_DIR/account_alloc.json" "$GEN_CONFIG_DIR/debug_account_alloc.json"
        fi
        
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
        
    echo ""
    log_info "Init complete!"
    log_info "Main artifacts:"
    log_info "  - $OUTPUT_DIR/genesis.json"
    log_info "  - $OUTPUT_DIR/waypoint.txt"
    log_info "Config artifacts in: $GEN_CONFIG_DIR"
    log_info "Run 'make deploy' next."
}

main "$@"
