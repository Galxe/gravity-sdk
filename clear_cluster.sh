#!/bin/bash

install_dir="/tmp"

# Stop all validator nodes
for num in {1..4}; do
    if [[ -f "$install_dir/node$num/script/stop.sh" ]]; then
        echo "Stopping node$num..."
        cd "$install_dir/node$num" && ./script/stop.sh
    fi
done

# Stop all VFN nodes
for num in {1..2}; do
    if [[ -f "$install_dir/vfn$num/script/stop.sh" ]]; then
        echo "Stopping vfn$num..."
        cd "$install_dir/vfn$num" && ./script/stop.sh
    fi
done

# Clear validator node directories
for num in {1..4}; do
    rm -rf "$install_dir/node$num"
done

# Clear VFN node directories
for num in {1..2}; do
    rm -rf "$install_dir/vfn$num"
done

echo "Cluster cleared!"

