节点与对应的端口

node1 的端口是 6180

node2 的端口是 2024

node3 的端口是 2025

node4 的端口是 2026

启动之前需要修改下面配置文件中的所有数据路径为自己的路径

1. node1/genesis/validator.yaml
2. node2/genesis/validator.yaml
3. node3/genesis/validator.yaml
4. node4/genesis/validator.yaml

例如
```
base:
  role: "validator"
  data_dir: "/Users/lightman/repos/gravity-sdk/node1/data"
  waypoint:
    from_file: "/Users/lightman/repos/gravity-sdk/node1/genesis/waypoint.txt"
```


启动命令

cd deploy_utils

启动单节点

./start.sh node1

启动所有节点

./start_test_cluster.sh

下线节点

./stop.sh node1

下线所有节点

./stop.sh