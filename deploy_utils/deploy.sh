#!/bin/bash

bin_name="kvstore-v2"
node_arg=""
bin_version="debug"
mode="cluster"

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
    --bin_version)
        bin_version="$2"
        shift
        ;;
    --mode)
        mode="$2"
        shift
        ;;
    *)
        echo "Unknown parameter: $1"
        exit 1
        ;;
    esac
    shift
done

if [[ "$bin_name" != "gravity-reth" && "$bin_name" != "bench" && "$bin_name" != "kvstore-v2" ]]; then
    echo "Error: bin_name must be either 'gravity-reth' or 'bench'."
    exit 1
fi

if [[ "$bin_version" != "release" && "$bin_version" != "debug" ]]; then
    echo "Error: bin_version must be either 'release' or 'debug'."
    exit 1
fi

if [[ -z "$node_arg" ]]; then
    echo "Error: --node parameter is required."
    exit 1
fi

if [[ "$mode" != "cluster" && "$mode" != "single" ]]; then
    echo "Error: mode must be either 'cluster' or 'single'."
    exit 1
fi

if [[ "$bin_name" != "gravity-reth" ]]; then
    rm -rf /tmp/$node_arg
fi

mkdir -p /tmp/$node_arg
mkdir -p /tmp/$node_arg/genesis
mkdir -p /tmp/$node_arg/bin
mkdir -p /tmp/$node_arg/data
mkdir -p /tmp/$node_arg/logs
mkdir -p /tmp/$node_arg/script

cp -r $node_arg/genesis /tmp/$node_arg
if [[ "$mode" == "cluster" ]]; then
    cp -r deploy_utils/four_nodes_config.json /tmp/$node_arg/genesis/nodes_config.json
    cp -r deploy_utils/four_nodes_discovery /tmp/$node_arg/discovery
else
    if [[ "$node_arg" != "node1" ]]; then
        echo "Error: if 'mode' is 'single', 'node' must be 'node1'."
        exit
    fi
    cp -r deploy_utils/single_node_config.json /tmp/$node_arg/genesis/nodes_config.json
    cp -r deploy_utils/single_node_discovery /tmp/$node_arg/discovery
fi

cp target/$bin_version/$bin_name /tmp/$node_arg/bin
cp deploy_utils/start.sh /tmp/$node_arg/script
cp deploy_utils/stop.sh /tmp/$node_arg/script
