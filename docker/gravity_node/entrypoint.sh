#!/bin/bash
set -e

# Gravity Node Docker Entrypoint
# 
# 环境变量:
#   RUST_LOG: 日志级别 (默认: info)
#   RUST_BACKTRACE: 错误回溯 (默认: 1)
#   BATCH_INSERT_TIME: 批量插入时间 (默认: 20)
#   CONFIG_PATH_PREFIX: 配置文件中的路径前缀 (用于替换)

GRAVITY_HOME="${GRAVITY_NODE_HOME:-/gravity_node}"
CONFIG_DIR="${GRAVITY_HOME}/config"
RETH_CONFIG="${CONFIG_DIR}/reth_config.json"
VALIDATOR_CONFIG="${CONFIG_DIR}/validator.yaml"
BIN_PATH="${GRAVITY_HOME}/bin/gravity_node"

# 运行时配置目录 (可写)
RUNTIME_CONFIG_DIR="${GRAVITY_HOME}/runtime_config"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# 检查必要文件
check_requirements() {
    log_info "Checking requirements..."
    
    if [ ! -f "$BIN_PATH" ]; then
        log_error "Binary not found: $BIN_PATH"
        exit 1
    fi
    
    if [ ! -f "$RETH_CONFIG" ]; then
        log_error "Config not found: $RETH_CONFIG"
        exit 1
    fi
    
    if ! command -v jq &> /dev/null; then
        log_error "'jq' is required but not installed"
        exit 1
    fi
    
    log_info "All requirements satisfied"
}

# 准备运行时配置 (处理路径替换)
prepare_runtime_config() {
    log_info "Preparing runtime configuration..."
    
    mkdir -p "$RUNTIME_CONFIG_DIR"
    
    # 检测配置文件中的路径前缀
    local detected_prefix=""
    
    # 从 reth_config.json 检测路径前缀
    if grep -q '"/tmp/node' "$RETH_CONFIG"; then
        detected_prefix=$(grep -oE '"/tmp/node[0-9]*' "$RETH_CONFIG" | head -1 | tr -d '"')
        log_info "Detected path prefix in config: $detected_prefix"
    fi
    
    # 如果检测到非标准路径，进行替换
    if [ -n "$detected_prefix" ] && [ "$detected_prefix" != "/gravity_node" ]; then
        log_info "Replacing path prefix: $detected_prefix -> /gravity_node"
        
        # 替换 reth_config.json
        sed "s|${detected_prefix}|/gravity_node|g" "$RETH_CONFIG" > "${RUNTIME_CONFIG_DIR}/reth_config.json"
        RETH_CONFIG="${RUNTIME_CONFIG_DIR}/reth_config.json"
        
        # 替换 validator.yaml
        if [ -f "$VALIDATOR_CONFIG" ]; then
            sed "s|${detected_prefix}|/gravity_node|g" "$VALIDATOR_CONFIG" > "${RUNTIME_CONFIG_DIR}/validator.yaml"
        fi
        
        # 复制其他配置文件
        for f in "$CONFIG_DIR"/*; do
            fname=$(basename "$f")
            if [ "$fname" != "reth_config.json" ] && [ "$fname" != "validator.yaml" ]; then
                if [ -f "$f" ]; then
                    cp "$f" "${RUNTIME_CONFIG_DIR}/"
                fi
            fi
        done
        
        log_info "Runtime config prepared at: $RUNTIME_CONFIG_DIR"
    else
        log_info "Config paths are already correct, using original config"
    fi
}

# 解析 reth_config.json 生成命令行参数
parse_reth_args() {
    local config_file="$1"
    local args=""
    
    # 解析 reth_args
    while IFS= read -r key && IFS= read -r value; do
        if [ -z "$value" ] || [ "$value" == "null" ]; then
            args="$args --${key}"
        else
            args="$args --${key}=${value}"
        fi
    done < <(jq -r '.reth_args | to_entries[] | .key, .value' "$config_file")
    
    echo "$args"
}

# 解析环境变量
parse_env_vars() {
    local config_file="$1"
    
    while IFS= read -r key && IFS= read -r value; do
        if [ -n "$value" ] && [ "$value" != "null" ]; then
            export "${key}=${value}"
            log_info "Set env: ${key}=${value}"
        fi
    done < <(jq -r '.env_vars | to_entries[] | .key, .value' "$config_file")
}

# 启动节点
start_node() {
    log_info "Starting Gravity Node..."
    log_info "Config: $RETH_CONFIG"
    log_info "Data dir: ${GRAVITY_HOME}/data"
    
    # 解析配置
    RETH_ARGS=$(parse_reth_args "$RETH_CONFIG")
    parse_env_vars "$RETH_CONFIG"
    
    log_info "Reth args: $RETH_ARGS"
    
    # 设置环境变量
    export RUST_BACKTRACE="${RUST_BACKTRACE:-1}"
    export RUST_LOG="${RUST_LOG:-info}"
    
    # 启动节点 (前台运行，让 Docker 管理进程)
    exec "$BIN_PATH" node $RETH_ARGS
}

# 主流程
main() {
    log_info "==================================="
    log_info "  Gravity Node Docker Container"
    log_info "==================================="
    log_info "GRAVITY_HOME: $GRAVITY_HOME"
    log_info "CONFIG_DIR: $CONFIG_DIR"
    log_info "RUST_LOG: ${RUST_LOG:-info}"
    
    check_requirements
    prepare_runtime_config
    start_node
}

# 捕获信号，优雅退出
trap 'log_info "Received shutdown signal, stopping..."; exit 0' SIGTERM SIGINT

main "$@"
