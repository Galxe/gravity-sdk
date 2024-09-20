source env.sh

log_suffix=$(date +"%Y-%d-%m:%H:%M:%S")

function start_node() {
    export RUST_BACKTRACE=1
    cur_node_path=$1
    node_path=${ROOT_PATH}/${cur_node_path}
    rm -rf $node_path/data/quorumstoreDB
    rm -rf $node_path/data/consensus_db
    rm -rf $node_path/data/rand_db
    # temporarily set these two round to zero
    jq 'walk(if type == "object" and (.last_voted_round? // .highest_timeout_round? // .preferred_round? // .one_chain_round?) then .last_voted_round |= 0 | .preferred_round |= 0 | .one_chain_round |= 0 | .highest_timeout_round |= 0 else . end)' ${node_path}/data/secure_storage.json > secure_storage.json
    mv secure_storage.json ${node_path}/data/
    pid=$(${ROOT_PATH}/target/debug/gravity-sdk --node-config-path ${node_path}/genesis/validator.yaml --mockdb-config-path ${ROOT_PATH}/nodes_config.json > ${node_path}/logs/debug.log & echo $!)
    echo $pid > ${cur_node_path}.pid
}

node_arg=$1

if [ -e ${node_arg}.pid ]; then
    echo ${node_arg} is started
    exit 1
fi

mkdir -p ${ROOT_PATH}/${node_arg}/logs

start_node ${node_arg}
