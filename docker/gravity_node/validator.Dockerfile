FROM ubuntu:24.04

# Install required tools
RUN apt-get update && apt-get install -y \
    clang \
    llvm \
    build-essential \
    pkg-config \
    libssl-dev \
    ansible \
    ca-certificates jq \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /gravity_node

# Create directories
RUN mkdir -p /gravity_node/data /gravity_node/logs /gravity_node/bin /gravity_node/script /gravity_node/config /gravity_node/execution_logs
RUN touch /gravity_node/consensus_log

# Copy binary and scripts
COPY target/debug/gravity_node /gravity_node/bin/gravity_node
COPY deploy_utils/start.sh /gravity_node/script/start.sh
COPY deploy_utils/stop.sh /gravity_node/script/stop.sh

# Copy config files (optional: can also use volume mount)
COPY docker/gravity_node/config/identity.yaml /gravity_node/config/identity.yaml
COPY docker/gravity_node/config/validator.yaml /gravity_node/config/validator.yaml
COPY docker/gravity_node/config/reth_config.json /gravity_node/config/reth_config.json
COPY docker/gravity_node/config/waypoint.txt /gravity_node/config/waypoint.txt
COPY docker/gravity_node/config/discovery /gravity_node/discovery
COPY docker/gravity_node/config/network_config.json /gravity_node/config/config.json

# Start command
CMD ["/gravity_node/script/start.sh"]
