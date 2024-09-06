jq 'walk(if type == "object" and (.last_voted_round? // .highest_timeout_round?) then .last_voted_round |= 0 | .highest_timeout_round |= 0 else . end)' node1/data/secure_storage.json > secure_storage.json
mv secure_storage.json node1/data/
./target/debug/gravity_consensus --data-path node1/data --port 2024 --discovery-path node1/discovery > 2024.log &
jq 'walk(if type == "object" and (.last_voted_round? // .highest_timeout_round?) then .last_voted_round |= 0 | .highest_timeout_round |= 0 else . end)' node2/data/secure_storage.json > secure_storage.json
mv secure_storage.json node2/data/
./target/debug/gravity_consensus --data-path node2/data --port 6180 --discovery-path node2/discovery > 6180.log &

