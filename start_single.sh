#!/bin/bash

# 添加彩色日志功能
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
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

log_debug() {
    echo -e "${BLUE}[DEBUG]${NC} $1"
}

# 固定参数
NODE="node1"
CHAIN="/home/gravity/xj/gravity.json"
MOCK_CONSENSUS=false
DISABLE_EVM=false
USE_PARALLEL_STATE_ROOT=1
export USE_PARALLEL_STATE_ROOT=1
export RETH_TXPOOL_BATCH_INSERT=1
export BATCH_INSERT_TIME=30

# 解析参数
while [[ $# -gt 0 ]]; do
  case $1 in
    --mock)
      MOCK_CONSENSUS=true
      shift
      ;;
    --evm)
      DISABLE_EVM=true
      shift
      ;;
    *)
      log_error "未知参数: $1"
      echo "用法: $0 [--mock] [--evm]"
      exit 1
      ;;
  esac
done

# 终止之前运行的gravity进程
log_info "正在终止现有的gravity进程..."
pkill -9 "gravity" || log_warn "没有找到正在运行的gravity进程"

# 部署节点
log_info "正在部署节点 $NODE..."
./deploy_utils/deploy.sh --mode single --node $NODE -v quick-release
# 设置环境变量
log_info "正在配置环境变量..."
export USE_STORAGE_CACHE=1

# 处理EVM设置
if [ "$DISABLE_EVM" = true ]; then
  log_info "已禁用GrEVM - 设置EVM_DISABLE_GREVM=1"
  export EVM_DISABLE_GREVM=1
else
  log_info "已启用GrEVM - 清除EVM_DISABLE_GREVM"
  unset EVM_DISABLE_GREVM
fi

# 处理Mock共识设置
if [ "$MOCK_CONSENSUS" = true ]; then
  log_info "已启用模拟共识模式 - 设置MOCK_CONSENSUS=true"
  export MOCK_CONSENSUS=true
else
  log_info "使用真实共识模式 - 设置MOCK_CONSENSUS=false"
  export MOCK_CONSENSUS=false
fi

# 启动节点
log_info "正在启动节点 $NODE，链配置: $CHAIN"
log_debug "环境配置: USE_PARALLEL_STATE_ROOT=$USE_PARALLEL_STATE_ROOT, USE_STORAGE_CACHE=1, MOCK_CONSENSUS=$MOCK_CONSENSUS, EVM_DISABLE_GREVM=$(if [ "$DISABLE_EVM" = true ]; then echo "1"; else echo "未设置"; fi)"
bash /tmp/$NODE/script/start.sh --node $NODE --chain $CHAIN

log_info "节点启动完成"
