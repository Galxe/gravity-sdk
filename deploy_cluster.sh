#!/bin/bash

# Deploy validator nodes
for num in {1..4}; do
    echo "Deploying node$num..."
    ./deploy_utils/deploy.sh --node "node$num" "$@"
done

# Deploy VFN nodes
for num in {1..2}; do
    echo "Deploying vfn$num..."
    ./deploy_utils/deploy.sh --node "vfn$num" "$@"
done

echo "Cluster deployment completed!"

