#!/bin/bash

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
WORKSPACE=$SCRIPT_DIR/..

log_suffix=$(date +"%Y-%d-%m:%H:%M:%S")

function start_node() {
    export RUST_BACKTRACE=1
    cur_node_path=$1
    rm -rf $node_path/data/quorumstoreDB
    rm -rf $node_path/data/consensus_db
    rm -rf $node_path/data/rand_db
    # temporarily set these two round to zero
    jq 'walk(if type == "object" and (.last_voted_round? // .highest_timeout_round? // .preferred_round? // .one_chain_round?) then .last_voted_round |= 0 | .preferred_round |= 0 | .one_chain_round |= 0 | .highest_timeout_round |= 0 else . end)' ${WORKSPACE}/data/secure_storage.json > ${WORKSPACE}/data/secure_storage.json
    pid=$(${WORKSPACE}/bin/gravity-sdk --node-config-path ${WORKSPACE}/genesis/validator.yaml --mockdb-config-path ${WORKSPACE}/genesis/nodes_config.json > ${WORKSPACE}/logs/debug.log & echo $!)
    echo $pid > ${WORKSPACE}/script/node.pid
}

node_arg=$1

if [ -e ${WORKSPACE}/script/node.pid ]; then
    pid=cat ${WORKSPACE}/script/node.pid
    if [ -d "/proc/$pid" ]; then
        echo ${node_arg} is started
        exit 1
    fi
fi

start_node ${node_arg}
