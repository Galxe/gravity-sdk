#!/bin/bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE=$SCRIPT_DIR/..

log_suffix=$(date +"%Y-%d-%m:%H:%M:%S")

node_arg=$1
bin_name=$2

if [ -e ${WORKSPACE}/script/node.pid ]; then
    pid=$(cat ${WORKSPACE}/script/node.pid)
    if [ -d "/proc/$pid" ]; then
        echo ${node_arg} is started
        exit 1
    fi
fi

function start_node() {
    export RUST_BACKTRACE=1
    reth_rpc_port=$1
    authrpc_port=$2
    http_port=$3
    metric_port=$4
    rm -rf $node_path/data/quorumstoreDB
    rm -rf $node_path/data/consensus_db
    rm -rf $node_path/data/rand_db

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
            --http.port ${http_port} \
            --port ${reth_rpc_port} \
            --authrpc.port ${authrpc_port} \
            --metrics ${metric_port} \
            --dev \
            --datadir ${WORKSPACE}/data/reth \
            --datadir.static-files ${WORKSPACE}/data/reth \
            --gravity_node_config ${WORKSPACE}/genesis/validator.yaml \
            --log.file.directory ${WORKSPACE}/execution_logs/ \
            > ${WORKSPACE}/logs/debug.log &
        echo $!
    )
    echo $pid >${WORKSPACE}/script/node.pid
}

port1=""
port2=""
port3=""
port4=""
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
else
    port1="16180"
    port2="8554"
    port3="8548"
    port4="9004"
fi

echo "start $node_arg ${port1} ${port2} ${port3} ${port4} ${bin_name}"
start_node ${port1} ${port2} ${port3} ${port4}
