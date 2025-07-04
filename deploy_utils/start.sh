#!/bin/bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE=$SCRIPT_DIR/..

log_suffix=$(date +"%Y-%d-%m:%H:%M:%S")

bin_name="gravity_node"
node_arg=""
chain="dev"
log_level="info"
JSON_FILE="test.json"

while [[ "$#" -gt 0 ]]; do
    case $1 in
    --bin_name)
        bin_name="$2"
        shift
        ;;
    --node)
        node_arg="$2"
        shift
        ;;
    --chain)
        chain="$2"
        shift
        ;;
    --log_level)
        log_level="$2"
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
    local json_file="$1"
    if [ ! -f "$json_file" ]; then
        echo "Error: JSON config file '$json_file' not found."
        exit 1
    fi
}

check_json_file_exists "$JSON_FILE"

# 3. Parse JSON and build argument array
parse_json_args() {
    local json_file="$1"
    json_args=()
    while IFS= read -r key && IFS= read -r value; do
        if [ -z "$value" ] || [ "$value" == "null" ]; then
            json_args+=( "--${key}" )
        else
            json_args+=( "--${key}" "${value}" )
        fi
    done < <(jq -r '. | to_entries[] | .key, .value' "$json_file")
}

echo "Parsing arguments from $JSON_FILE ..."
parse_json_args "$JSON_FILE"

function start_node() {
    export RUST_BACKTRACE=1
    reth_rpc_port=$1
    authrpc_port=$2
    http_port=$3
    metric_port=$4
    
    echo ${WORKSPACE}

    pid=$(
        ${WORKSPACE}/bin/${bin_name} node \
            "${json_args[@]}" \
	    > ${WORKSPACE}/logs/debug.log &
        echo $!
    )
    echo $pid >${WORKSPACE}/script/node.pid
}

port1="12024"
port2="8551"
port3="8545"
port4="9001"
if [ "$node_arg" == "node1" ]; then
    port1="12024"
    port2="8551"
    port3="8545"
    port4="9001"
elif [ "$node_arg" == "node2" ]; then
    port1="12025"
    port2="8552"
    port3="8546"
    port4="9002"
elif [ "$node_arg" == "node3" ]; then
    port1="12026"
    port2="8553"
    port3="8547"
    port4="9003"
elif [ "$node_arg" == "node4" ]; then
    port1="16180"
    port2="8554"
    port3="8548"
    port4="9004"
fi

echo "start $node_arg ${port1} ${port2} ${port3} ${port4} ${bin_name}"
start_node ${port1} ${port2} ${port3} ${port4}
