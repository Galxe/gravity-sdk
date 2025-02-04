#!/bin/bash

nodes=("node1" "node2" "node3" "node4")

for node in "${nodes[@]}"; do
    bash manager.sh action=stop node="$node" > "${node}.log" 2>&1 &
done


echo "start now, check the log"