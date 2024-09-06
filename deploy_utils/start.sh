source env.sh

echo ${ROOT_PATH}
log_suffix=$(date +"%Y-%d-%m:%H:%M:%S")

function start_node() {
    cur_node_path=$1
    port=$2
    node_path=${ROOT_PATH}/${node_path}
    rm -rf $node_path/data/quorumstoreDB
    rm -rf $node_path/data/consensus_db
    rm -rf $node_path/data/rand_db
    # temporarily set these two round to zero
    jq 'walk(if type == "object" and (.last_voted_round? // .highest_timeout_round? // .preferred_round? // .one_chain_round?) then .last_voted_round |= 0 | .preferred_round |= 0 | .one_chain_round |= 0 | .highest_timeout_round |= 0 else . end)' ${node_path}/data/secure_storage.json > secure_storage.json
    mv secure_storage.json ${node_path}/data/
    pid=$(${ROOT_PATH}/target/debug/gravity_consensus --data-path ${node_path}/data --port ${port} --discovery-path ${node_path}/discovery > logs/${port}.${log_suffix}.log & echo $!)
    echo $pid > ${cur_node_path}.pid
}

node_arg=$1
port_arg=$2

start_node ${node_arg} ${port_arg}
