#!/bin/bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE=$SCRIPT_DIR/..

log_suffix=$(date +"%Y-%d-%m:%H:%M:%S")

bin_name="gravity_node"

while [[ "$#" -gt 0 ]]; do
    case $1 in
    --bin_name)
        bin_name="$2"
        shift
        ;;
    *)
        echo "Unknown parameter: $1"
        exit 1
        ;;
    esac
    shift
done

if [ -e ${WORKSPACE}/script/node.pid ]; then
    pid=$(cat ${WORKSPACE}/script/node.pid)
    if [ -d "/proc/$pid" ]; then
        echo ${pid} is running, stop it first
        exit 1
    fi
fi

reth_config="${WORKSPACE}/config/reth_config.json"

# 1. Check if jq is installed
check_jq_installed() {
    if ! command -v jq &> /dev/null; then
        echo "Error: 'jq' is required but not installed. Please install 'jq' first."
        exit 1
    fi
}

check_jq_installed

# 2. Check if JSON config file exists
check_json_file_exists() {
    local reth_config="$1"
    if [ ! -f "$reth_config" ]; then
        echo "Error: JSON config file '$reth_config' not found."
        exit 1
    fi
}

check_json_file_exists "$reth_config"

# 3. Parse JSON and build argument array
parse_json_args() {
    local reth_config="$1"
    
    # Parse reth_args
    reth_args_array=()
    while IFS= read -r key && IFS= read -r value; do
        if [ -z "$value" ] || [ "$value" == "null" ]; then
            reth_args_array+=( "--${key}" )
        else
            reth_args_array+=( "--${key}=${value}" )
        fi
    done < <(jq -r '.reth_args | to_entries[] | .key, .value' "$reth_config")
    reth_args_str="${reth_args_array[*]}"
    
    # Parse env_vars
    env_vars_array=()
    while IFS= read -r key && IFS= read -r value; do
        if [ -n "$value" ] && [ "$value" != "null" ]; then
            env_vars_array+=( "${key}=${value}" )
        fi
    done < <(jq -r '.env_vars | to_entries[] | .key, .value' "$reth_config")
    env_vars_str="${env_vars_array[*]}"
}

echo "Parsing arguments from $reth_config ..."
parse_json_args "$reth_config"

function start_node() {
    export RUST_BACKTRACE=1
    
    echo ${WORKSPACE}

    pid=$(
        env ${env_vars_str} ${WORKSPACE}/bin/${bin_name} node \
            ${reth_args_str} \
	    > ${WORKSPACE}/logs/debug.log &
        echo $!
    )
    echo $pid >${WORKSPACE}/script/node.pid
}

echo "start ${bin_name}"
start_node
