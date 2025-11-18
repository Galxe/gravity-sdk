#!/bin/bash

# Deploy validator nodes
for num in {1..4}; do
    echo "Deploying node$num..."
    ./deploy_utils/deploy.sh --node "node$num" "$@"
done

# Deploy VFN nodes
for num in {1..2}; do
    vfn_name="vfn$num"
    echo "Deploying $vfn_name..."
    ./deploy_utils/deploy.sh --node "$vfn_name" "$@"
done

echo "Cluster deployment completed!"

