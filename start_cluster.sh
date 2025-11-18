#!/bin/bash

install_dir="/tmp"

# Start validator nodes
for num in {1..4}; do
    echo "Starting node$num..."
    cd "$install_dir/node$num" && ./script/start.sh
    sleep 2
done

# Start VFN nodes
for num in {1..2}; do
    echo "Starting vfn$num..."
    cd "$install_dir/vfn$num" && ./script/start.sh
    sleep 2
done

echo "Cluster started!"
