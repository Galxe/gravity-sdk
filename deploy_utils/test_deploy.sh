#!/bin/bash

bin_name="gravity-reth"
node_arg=""

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
    *)
        echo "Unknown parameter: $1"
        exit 1
        ;;
    esac
    shift
done

if [[ "$bin_name" != "gravity-reth" && "$bin_name" != "bench" ]]; then
    echo "Error: bin_name must be either 'gravity-reth' or 'bench'."
    exit 1
fi

if [[ -z "$node_arg" ]]; then
    echo "Error: --node parameter is required."
    exit 1
fi

rm -rf /tmp/$node_arg

mkdir -p /tmp/$node_arg
mkdir -p /tmp/$node_arg/genesis
mkdir -p /tmp/$node_arg/bin
mkdir -p /tmp/$node_arg/data
mkdir -p /tmp/$node_arg/logs
mkdir -p /tmp/$node_arg/script

cp -r $node_arg/genesis /tmp/$node_arg
cp -r nodes_config.json /tmp/$node_arg/genesis/
cp -r discovery /tmp/$node_arg
cp target/debug/$bin_name /tmp/$node_arg/bin
cp deploy_utils/start.sh /tmp/$node_arg/script
cp deploy_utils/stop.sh /tmp/$node_arg/script
