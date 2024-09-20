# 节点启动文档 [English](https://github.com/Galxe/gravity-sdk/deploy_utils/readme.md)

## 节点配置

本系统包含4个节点,对应端口如下:

- node1: 2024
- node2: 2025
- node3: 2026
- node4: 6180

## 前置准备

在启动节点之前,需要修改以下配置文件中的所有数据路径为您自己的本地路径:

1. node1/genesis/validator.yaml
2. node2/genesis/validator.yaml
3. node3/genesis/validator.yaml
4. node4/genesis/validator.yaml

例如,将类似以下的配置修改为您的本地路径:

```yaml
base:
  role: "validator"
  data_dir: "/tmp/node1/data"
  waypoint:
    from_file: "/tmp/node1/genesis/waypoint.txt"
```

## 编译

在项目根目录执行以下命令进行编译:

```
cargo build
```

## 单节点集群部署

### 部署节点

默认部署到 /tmp 目录:

```
./deploy_utils/test_deploy.sh node1
```

### 启动节点

```
cd /tmp/node1
./script/start.sh
```

### 下线节点

```
./script/stop.sh
```

## 多节点集群部署

### 启动 node1

按照单节点集群的流程启动 node1。

### 启动 node2

执行以下命令启动 node2:

```
./deploy_utils/test_deploy.sh node2

cd /tmp/node2

./script/start.sh
./script/stop.sh
```

### 启动 node3 和 node4

node3 和 node4 的启动方式与 node1 相同。

## 注意事项

1. 确保在启动节点前已正确修改所有配置文件中的路径。
2. 多节点集群部署时,需要确保节点间能够正常通信。
3. 如遇到端口冲突,请检查并修改相应的端口配置。

## 故障排除

如果遇到启动问题,请检查:
1. 配置文件路径是否正确
2. 端口是否被占用
3. 系统防火墙设置

如需更多帮助,请查看日志文件或联系技术支持。
