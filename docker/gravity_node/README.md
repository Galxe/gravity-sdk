# Gravity Node Docker 部署

单节点 Docker 部署配置。

## 目录结构

```
docker/gravity_node/
├── validator.Dockerfile     # Docker 镜像定义
├── docker-compose.yaml      # Docker Compose 配置
├── entrypoint.sh            # 容器入口脚本
├── Makefile                 # 快捷命令
├── README.md                # 本文档
└── config/                  # 默认配置文件
    ├── validator-identity.yaml  # 节点身份
    ├── validator.yaml           # 验证者配置
    ├── reth_config.json         # Reth 配置
    └── waypoint.txt             # Waypoint
```

## 快速开始

### 1. 编译 Gravity Node

```bash
# 在项目根目录
cargo build --profile quick-release -p gravity_node
```

### 2. 构建 Docker 镜像

```bash
cd docker/gravity_node

# 构建 release 版本
make build

# 或构建 debug 版本
make build-debug
```

### 3. 启动节点

```bash
# 使用内置默认配置
make up

# 或使用外部配置 (如 deploy_utils/node1/config)
CONFIG_DIR=../../deploy_utils/node1/config make up-config
```

### 4. 查看状态

```bash
# 查看容器状态
make status

# 查看日志
make logs

# 健康检查
make health

# 测试 RPC
make rpc-test
```

### 5. 停止节点

```bash
make down

# 停止并清理数据
make clean
```

## 使用外部配置

支持使用 `deploy_utils/node1/config` 等外部配置目录：

```bash
# 使用相对路径
CONFIG_DIR=../../deploy_utils/node1/config make up-config

# 使用绝对路径
CONFIG_DIR=/path/to/your/config make up-config
```

**配置目录要求：**

配置目录中应包含以下文件：
- `reth_config.json` - Reth 配置
- `validator.yaml` - 验证者配置
- `validator-identity.yaml` - 节点身份密钥
- `waypoint.txt` - Waypoint

**路径自动替换：**

entrypoint.sh 会自动检测配置文件中的路径前缀（如 `/tmp/node1`），并替换为容器内路径 `/gravity_node`。

## 端口说明

| 端口 | 服务 | 说明 |
|------|------|------|
| 8545 | RPC HTTP | JSON-RPC 接口 |
| 8551 | Auth RPC | 认证 RPC 接口 |
| 9001 | Metrics | Prometheus 指标 |
| 12024 | Reth P2P | Reth 网络 |
| 2024 | Network | Gravity 共识网络 |
| 10000 | Inspection | 检查服务 |

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `BUILD_TYPE` | `quick-release` | 构建类型 (quick-release/debug) |
| `RUST_LOG` | `info` | 日志级别 |
| `IMAGE_TAG` | `latest` | 镜像标签 |
| `CONFIG_DIR` | `./config` | 配置目录路径 |
| `RPC_PORT` | `8545` | RPC 端口映射 |
| `METRICS_PORT` | `9001` | Metrics 端口映射 |

## 数据持久化

使用 Docker volumes 持久化数据：

- `gravity_data` - 区块链数据
- `gravity_logs` - 日志文件
- `gravity_exec_logs` - 执行日志
- `gravity_consensus_log` - 共识日志

## 常用命令

```bash
# 查看所有可用命令
make help

# 查看当前配置
make show-config

# 进入容器 shell
make shell

# 实时查看日志
make logs

# 重启节点
make restart
```

## 配置文件说明

### reth_config.json

```json
{
    "reth_args": {
        "chain": "dev",
        "http.port": 8545,
        "http.addr": "0.0.0.0",
        "datadir": "/gravity_node/data/reth",
        "gravity_node_config": "/gravity_node/config/validator.yaml",
        ...
    },
    "env_vars": {
        "BATCH_INSERT_TIME": 20
    }
}
```

### validator.yaml

```yaml
base:
  role: "validator"
  data_dir: "/gravity_node/data"
  waypoint:
    from_file: "/gravity_node/config/waypoint.txt"

consensus:
  safety_rules:
    backend:
      type: "on_disk_storage"
      path: /gravity_node/data/secure_storage.json
    initial_safety_rules_config:
      from_file:
        waypoint:
          from_file: /gravity_node/config/waypoint.txt
        identity_blob_path: /gravity_node/config/validator-identity.yaml
  ...

validator_network:
  network_id: validator
  listen_address: "/ip4/0.0.0.0/tcp/2024"
  ...
```

## 故障排查

### 容器无法启动

1. 检查二进制文件是否存在：
   ```bash
   ls -la ../../target/release/gravity_node
   ```

2. 检查构建日志：
   ```bash
   docker-compose build --no-cache
   ```

### 配置路径问题

1. 查看容器内的配置：
   ```bash
   make shell
   cat /gravity_node/config/reth_config.json
   cat /gravity_node/runtime_config/reth_config.json  # 运行时配置
   ```

2. 检查路径替换是否生效：
   ```bash
   make logs | grep "Detected path prefix"
   ```

### RPC 无响应

1. 检查容器状态：
   ```bash
   make status
   make health
   ```

2. 查看日志：
   ```bash
   make logs
   ```

### 清理重建

```bash
make clean
make build
make up
```
