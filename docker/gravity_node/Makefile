# Gravity Node Docker 部署 Makefile
#
# 使用方法:
#   make build        - 构建 Docker 镜像
#   make up           - 启动节点 (使用内置配置)
#   make up-config    - 启动节点 (指定外部配置)
#   make down         - 停止节点
#   make logs         - 查看日志
#   make shell        - 进入容器
#   make clean        - 清理所有数据
#   make status       - 查看状态
#   make health       - 健康检查
#
# 使用外部配置示例:
#   CONFIG_DIR=../../deploy_utils/node1/config make up-config

.PHONY: build build-debug up up-config down restart logs shell clean status health rpc-test help

# 配置变量
COMPOSE_FILE := docker-compose.yaml
PROJECT_ROOT := ../..
IMAGE_NAME := gravity_node
IMAGE_TAG ?= latest
BUILD_TYPE ?= quick-release
RUST_LOG ?= info

# 外部配置目录 (相对于 docker/gravity_node 目录)
CONFIG_DIR ?= ./config

# 默认目标
.DEFAULT_GOAL := help

# 帮助信息
help:
	@echo "Gravity Node Docker 部署命令:"
	@echo ""
	@echo "  构建相关:"
	@echo "    make build        - 构建 Docker 镜像 (quick-release)"
	@echo "    make build-debug  - 构建 Docker 镜像 (debug)"
	@echo ""
	@echo "  运行相关:"
	@echo "    make up           - 启动节点 (使用内置配置)"
	@echo "    make up-config    - 启动节点 (使用外部配置)"
	@echo "    make down         - 停止节点"
	@echo "    make restart      - 重启节点"
	@echo ""
	@echo "  监控相关:"
	@echo "    make logs         - 查看日志 (实时)"
	@echo "    make status       - 查看容器状态"
	@echo "    make health       - 健康检查"
	@echo "    make rpc-test     - 测试 RPC 连接"
	@echo ""
	@echo "  调试相关:"
	@echo "    make shell        - 进入容器 shell"
	@echo ""
	@echo "  清理相关:"
	@echo "    make clean        - 停止并清理所有数据"
	@echo ""
	@echo "  环境变量:"
	@echo "    BUILD_TYPE=quick-release|debug  构建类型 (默认: quick-release)"
	@echo "    RUST_LOG=info|debug|...   日志级别 (默认: info)"
	@echo "    IMAGE_TAG=latest          镜像标签 (默认: latest)"
	@echo "    CONFIG_DIR=<path>         外部配置目录"
	@echo ""
	@echo "  使用外部配置示例:"
	@echo "    CONFIG_DIR=../../deploy_utils/node1/config make up-config"
	@echo "    CONFIG_DIR=/absolute/path/to/config make up-config"

# 构建镜像 (quick-release)
build:
	@echo ">>> 构建 Gravity Node 镜像 (quick-release)..."
	cd $(PROJECT_ROOT) && docker compose -f docker/gravity_node/$(COMPOSE_FILE) build \
		--build-arg BUILD_TYPE=quick-release \
		--build-arg RUST_LOG=$(RUST_LOG)
	@echo ">>> 构建完成: $(IMAGE_NAME):$(IMAGE_TAG)"

# 构建镜像 (debug)
build-debug:
	@echo ">>> 构建 Gravity Node 镜像 (debug)..."
	cd $(PROJECT_ROOT) && docker compose -f docker/gravity_node/$(COMPOSE_FILE) build \
		--build-arg BUILD_TYPE=debug \
		--build-arg RUST_LOG=debug
	@echo ">>> 构建完成: $(IMAGE_NAME):$(IMAGE_TAG)"

# 启动节点 (使用内置配置)
up:
	@echo ">>> 启动 Gravity Node (内置配置)..."
	cd $(PROJECT_ROOT) && docker compose -f docker/gravity_node/$(COMPOSE_FILE) up -d
	@$(MAKE) --no-print-directory _show_endpoints

# 启动节点 (使用外部配置)
up-config:
	@echo ">>> 启动 Gravity Node (外部配置: $(CONFIG_DIR))..."
	@if [ ! -d "$(CONFIG_DIR)" ]; then \
		echo "错误: 配置目录不存在: $(CONFIG_DIR)"; \
		exit 1; \
	fi
	@echo ">>> 配置文件:"
	@ls -la $(CONFIG_DIR)/
	cd $(PROJECT_ROOT) && CONFIG_DIR=$(shell cd $(CONFIG_DIR) && pwd) \
		docker compose -f docker/gravity_node/$(COMPOSE_FILE) up -d
	@$(MAKE) --no-print-directory _show_endpoints

# 显示端点信息
_show_endpoints:
	@echo ""
	@echo ">>> 节点已启动"
	@echo ""
	@echo "  RPC 端点:    http://localhost:8545"
	@echo "  Metrics:     http://localhost:9001/metrics"
	@echo "  Inspection:  http://localhost:10000"
	@echo ""
	@echo "  查看日志: make logs"
	@echo "  健康检查: make health"

# 停止节点
down:
	@echo ">>> 停止 Gravity Node..."
	cd $(PROJECT_ROOT) && docker compose -f docker/gravity_node/$(COMPOSE_FILE) down
	@echo ">>> 节点已停止"

# 重启节点
restart:
	@echo ">>> 重启 Gravity Node..."
	cd $(PROJECT_ROOT) && docker compose -f docker/gravity_node/$(COMPOSE_FILE) restart
	@echo ">>> 节点已重启"

# 查看日志
logs:
	cd $(PROJECT_ROOT) && docker compose -f docker/gravity_node/$(COMPOSE_FILE) logs -f

# 进入容器
shell:
	docker exec -it gravity_node /bin/bash

# 查看状态
status:
	@echo ">>> 容器状态:"
	@docker ps -a --filter name=gravity_node --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"
	@echo ""
	@echo ">>> 数据卷:"
	@docker volume ls --filter name=gravity

# 健康检查
health:
	@echo ">>> 健康检查..."
	@docker inspect --format='{{.State.Health.Status}}' gravity_node 2>/dev/null || echo "容器未运行"
	@echo ""
	@echo ">>> 最近健康检查日志:"
	@docker inspect --format='{{range .State.Health.Log}}{{.Output}}{{end}}' gravity_node 2>/dev/null | tail -5 || true

# RPC 测试
rpc-test:
	@echo ">>> 测试 RPC 连接..."
	@curl -s http://localhost:8545 \
		-X POST \
		-H "Content-Type: application/json" \
		-d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' | jq . || echo "RPC 连接失败"
	@echo ""
	@echo ">>> 获取 Chain ID..."
	@curl -s http://localhost:8545 \
		-X POST \
		-H "Content-Type: application/json" \
		-d '{"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}' | jq . || echo "RPC 连接失败"

# 清理所有数据
clean:
	@echo ">>> 停止并清理所有数据..."
	cd $(PROJECT_ROOT) && docker compose -f docker/gravity_node/$(COMPOSE_FILE) down -v
	@echo ">>> 清理完成"

# 查看配置
show-config:
	@echo ">>> 当前配置:"
	@echo "  CONFIG_DIR: $(CONFIG_DIR)"
	@echo "  BUILD_TYPE: $(BUILD_TYPE)"
	@echo "  RUST_LOG: $(RUST_LOG)"
	@echo "  IMAGE_TAG: $(IMAGE_TAG)"
	@echo ""
	@if [ -d "$(CONFIG_DIR)" ]; then \
		echo ">>> 配置文件列表:"; \
		ls -la $(CONFIG_DIR)/; \
	else \
		echo ">>> 配置目录不存在: $(CONFIG_DIR)"; \
	fi
