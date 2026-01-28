#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_FILE="${1:-$SCRIPT_DIR/cluster.toml}"
OUTPUT_DIR="${GRAVITY_ARTIFACTS_DIR:-$SCRIPT_DIR/output}"

source "$SCRIPT_DIR/utils/common.sh"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

main() {
    if [ ! -f "$CONFIG_FILE" ]; then
        log_error "Config file not found: $CONFIG_FILE"
        exit 1
    fi
    export CONFIG_FILE
    
    # Parse TOML
    config_json=$(parse_toml)
    
    # Check if faucet enabled
    num_accounts=$(echo "$config_json" | jq -r '.faucet_init.num_accounts // 0')
    
    if [ "$num_accounts" -le 0 ]; then
        log_info "No faucet accounts requested (faucet_init.num_accounts <= 0). Skipping."
        exit 0
    fi
    
    log_info "Initializing $num_accounts faucet accounts..."
    
    # Extract Node Config
    nodes=$(echo "$config_json" | jq -r '.nodes')
    if [ "$nodes" == "null" ] || [ $(echo "$nodes" | jq 'length') -eq 0 ]; then
        log_error "No nodes found in config"
        exit 1
    fi
    
    # Use first node
    rpc_host=$(echo "$nodes" | jq -r '.[0].host // "127.0.0.1"')
    rpc_port=$(echo "$nodes" | jq -r '.[0].rpc_port // 8545')
    rpc_url="http://$rpc_host:$rpc_port"
    
    # Extract faucet config
    private_key=$(echo "$config_json" | jq -r '.faucet_init.private_key // "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"')
    eth_balance_per_acc=$(echo "$config_json" | jq -r '.faucet_init.eth_balance // "1000000000000000000"')
    
    # Calculate total required balance (num_accounts * per_acc + buffer for gas)
    # Gas overhead ~ 0.003 ETH per account in worst case (deep tree)? 
    # Let's add 0.01 ETH per account as buffer to be safe, plus fixed buffer.
    # actually, gravity_bench subtracts (total_txns * gas_price).
    # gas_price is hardcoded to 0.0021 ETH.
    # total_txns approx 1.1 * num_accounts.
    # So overhead is approx 1.1 * 0.0021 * num_accounts ~= 0.00231 * num_accounts.
    # We'll use python for big int math.
    
    fauce_eth_balance=$(python3 -c "
num = $num_accounts
per_acc = int('$eth_balance_per_acc')
# Buffer: 0.01 ETH (10^16) per account for gas
buffer_per_acc = 10**16 
total = num * (per_acc + buffer_per_acc)
print(total)
")
    
    # Chain ID (try to read from genesis.json in artifacts)
    chain_id=""
    if [ -f "$OUTPUT_DIR/genesis.json" ]; then
        genesis_chain_id=$(jq -r '.config.chainId // empty' "$OUTPUT_DIR/genesis.json")
        if [ ! -z "$genesis_chain_id" ]; then
            chain_id=$genesis_chain_id
        fi
    fi

    if [ -z "$chain_id" ]; then
        log_error "Could not determine chain_id from artifacts/genesis.json. Cannot generate faucet config."
        exit 1
    fi

    # Prepare Gravity Bench
    GRAVITY_BENCH_DIR="$PROJECT_ROOT/external/gravity_bench"
    
    # Check dependencies in config
    bench_repo=$(echo "$config_json" | jq -r '.dependencies.gravity_bench.repo // "https://github.com/Galxe/gravity_bench.git"')
    bench_ref=$(echo "$config_json" | jq -r '.dependencies.gravity_bench.ref // "main"')

    if [ ! -d "$GRAVITY_BENCH_DIR" ]; then
        log_info "gravity_bench not found. Cloning from $bench_repo..."
        mkdir -p "$(dirname "$GRAVITY_BENCH_DIR")"
        git clone "$bench_repo" "$GRAVITY_BENCH_DIR"
        
        (
            cd "$GRAVITY_BENCH_DIR"
            log_info "Checking out $bench_ref..."
            git checkout "$bench_ref"
            
            log_info "Initializing submodules..."
            git submodule update --init --recursive

            # Install python dependencies for deploy.py if needed
            if [ -f "requirements.txt" ]; then
                 log_info "Installing gravity_bench requirements..."
                 pip install -r requirements.txt || true
            fi
        )
    fi
    
    # Ensure gravity_bench is set up (contracts cloned, etc.)
    if [ -f "$GRAVITY_BENCH_DIR/setup.sh" ]; then
        log_info "Running gravity_bench setup.sh..."
        (
            cd "$GRAVITY_BENCH_DIR"
            # setup.sh expects to be run in its dir
            bash setup.sh
        )
    else
        log_warn "setup.sh not found in gravity_bench. Manual setup might be required."
    fi
    
    # Generate bench config
    bench_config_path="$OUTPUT_DIR/faucet_bench_config.toml"
    accounts_csv="$OUTPUT_DIR/accounts.csv"
    contracts_json="$OUTPUT_DIR/contracts.json" # Dummy path, deploy.py will create/overwrite
    log_path="$OUTPUT_DIR/faucet_bench.log"
    
    cat > "$bench_config_path" <<EOF
contract_config_path = "$contracts_json"
num_tokens = 0
target_tps = 1
enable_swap_token = false
address_pool_type = "random"
log_path = "$log_path"

[[nodes]]
rpc_url = "$rpc_url"
chain_id = $chain_id

[faucet]
private_key = "$private_key"
faucet_level = 3
wait_duration_secs = 2
fauce_eth_balance = "$fauce_eth_balance"

[accounts]
num_accounts = $num_accounts

[performance]
num_senders = 10
max_pool_size = 1000
duration_secs = 60
EOF

    log_info "Generated bench config at $bench_config_path"
    
    log_info "Running gravity_bench..."
    (
        cd "$GRAVITY_BENCH_DIR"
        cargo run --release --quiet -- \
            --config "$bench_config_path" \
            --faucet-only \
            --accounts-output "$accounts_csv"
    )
    
    if [ $? -eq 0 ]; then
        log_info "Faucet init complete. Accounts saved to $accounts_csv"
    else
        log_error "gravity_bench failed."
        exit 1
    fi
}

main "$@"
