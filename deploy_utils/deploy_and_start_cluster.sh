#!/bin/bash

bin_name="gravity-reth"
node_arg=""
bin_version="debug"

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

if [[ "$bin_version" != "release" && "$bin_version" != "debug" ]]; then
    echo "Error: bin_version must be either 'release' or 'debug'."
    exit 1
fi

if [[ -z "$node_arg" ]]; then
    echo "Error: --node parameter is required."
    exit 1
fi

rm -rf /tmp/$node_arg

./deploy_utils/deploy.sh --mode cluster --node $node_arg --bin_version $bin_version

cd bash /tmp/$node_arg/script/
bash start.sh --node $node_arg
cd -