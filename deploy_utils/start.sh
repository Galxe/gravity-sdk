#!/bin/bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE=$SCRIPT_DIR/..

log_suffix=$(date +"%Y-%d-%m:%H:%M:%S")

bin_name="gravity_node"
node_arg=""
chain="dev"
log_level="info"

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

function start_node() {
    export RUST_BACKTRACE=1
    reth_rpc_port=$1
    authrpc_port=$2
    http_port=$3
    metric_port=$4

    # temporarily set these two round to zero
    jq 'walk(
        if type == "object" and (
            .last_voted_round? // 
            .highest_timeout_round? // 
            .preferred_round? // 
            .one_chain_round?
        ) then 
            .last_voted_round |= 0 | 
            .preferred_round |= 0 | 
            .one_chain_round |= 0 | 
            .highest_timeout_round |= 0 
        else 
            . 
        end
    )' ${WORKSPACE}/data/secure_storage.json > ${WORKSPACE}/data/secure_storage.json


    echo ${WORKSPACE}

    pid=$(
        ${WORKSPACE}/bin/${bin_name} node \
            --chain ${chain} \
            --http \
            --http.port ${http_port} \
            --http.corsdomain "*" \
            --http.api "debug,eth,net,trace,txpool,web3,rpc" \
            --http.addr 0.0.0.0 \
            --dev \
            --port ${reth_rpc_port} \
            --authrpc.port ${authrpc_port} \
            --authrpc.addr 0.0.0.0 \
            --metrics 0.0.0.0:${metric_port} \
            --log.file.filter ${log_level} \
            --datadir ${WORKSPACE}/data/reth \
            --datadir.static-files ${WORKSPACE}/data/reth \
            --gravity_node_config ${WORKSPACE}/genesis/validator.yaml \
            --log.file.directory ${WORKSPACE}/execution_logs/ \
            --rpc.max-subscriptions-per-connection 20000 \
            --rpc.max-connections 20000 \
            --txpool.max-pending-txns 1000000 \
            --txpool.pending-max-count 18446744073709551615 \
            --txpool.pending-max-size 17592186044415 \
            --txpool.basefee-max-count 18446744073709551615 \
            --txpool.basefee-max-size 17592186044415 \
            --txpool.queued-max-count 18446744073709551615 \
            --txpool.queued-max-size 17592186044415 \
             --http.disable_compression \
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
