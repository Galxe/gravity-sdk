source env.sh

echo ${ROOT_PATH}
log_suffix=$(date +"%Y-%d-%m:%H:%M:%S")

function start_node() {
    node_path=$1
    port=$2
    node_path=${ROOT_PATH}/${node_path}
    rm -rf $node_path/data/quorumstoreDB
    rm -rf $node_path/data/consensus_db
    rm -rf $node_path/data/rand_db
    # temporarily set these two round to zero
    jq 'walk(if type == "object" and (.last_voted_round? // .highest_timeout_round?) then .last_voted_round |= 0 | .highest_timeout_round |= 0 else . end)' ${node_path}/data/secure_storage.json > secure_storage.json
    mv secure_storage.json ${node_path}/data/
    ${ROOT_PATH}/target/debug/gravity_consensus --data-path ${node_path}/data --port ${port} --discovery-path ${node_path}/discovery > logs/${port}.${log_suffix}.log &
}

start_node node1 2024
start_node node2 6180