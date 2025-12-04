# Gravity Node Docker Image
# 
# Usage:
#   构建: docker build -f docker/gravity_node/validator.Dockerfile -t gravity_node:latest .
#   运行: docker-compose -f docker/gravity_node/docker-compose.yaml up -d

FROM ubuntu:24.04

# 构建参数
ARG BUILD_TYPE=quick-release
ARG RUST_LOG=info

# 安装必要工具
RUN apt-get update && apt-get install -y \
    ca-certificates \
    jq \
    curl \
    sed \
    && rm -rf /var/lib/apt/lists/*

# 设置工作目录
WORKDIR /gravity_node

# 创建目录结构
RUN mkdir -p \
    /gravity_node/bin \
    /gravity_node/config \
    /gravity_node/runtime_config \
    /gravity_node/data \
    /gravity_node/data/reth \
    /gravity_node/logs \
    /gravity_node/execution_logs \
    && touch /gravity_node/consensus_log

# 复制二进制文件 (默认使用 release 版本)
COPY target/${BUILD_TYPE}/gravity_node /gravity_node/bin/gravity_node
RUN chmod +x /gravity_node/bin/gravity_node

# 复制默认配置文件 (可被 volume 覆盖)
COPY docker/gravity_node/config/validator-identity.yaml /gravity_node/config/validator-identity.yaml
COPY docker/gravity_node/config/validator.yaml /gravity_node/config/validator.yaml
COPY docker/gravity_node/config/reth_config.json /gravity_node/config/reth_config.json
COPY docker/gravity_node/config/waypoint.txt /gravity_node/config/waypoint.txt

# 复制启动脚本
COPY docker/gravity_node/entrypoint.sh /gravity_node/entrypoint.sh
RUN chmod +x /gravity_node/entrypoint.sh

# 环境变量
ENV RUST_BACKTRACE=1
ENV RUST_LOG=${RUST_LOG}
ENV GRAVITY_NODE_HOME=/gravity_node

# 暴露端口
EXPOSE 8545   
# RPC HTTP
EXPOSE 8551   
# Auth RPC
EXPOSE 9001   
# Metrics
EXPOSE 12024  
# Reth P2P
EXPOSE 2024   
# Gravity Network
EXPOSE 10000  
# Inspection Service

# 健康检查
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD curl -sf http://localhost:8545 -X POST -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' || exit 1

# 入口点
ENTRYPOINT ["/gravity_node/entrypoint.sh"]
