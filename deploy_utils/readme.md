# Node Startup Documentation [中文版](https://github.com/Galxe/gravity-sdk/deploy_utils/readme_cn.md)
## Node Configuration

This system consists of 4 nodes with corresponding ports as follows:

- node1: 2024
- node2: 2025
- node3: 2026
- node4: 6180

## Prerequisites

Before starting the nodes, you need to modify all data paths in the following configuration files to your own local paths:

1. node1/genesis/validator.yaml
2. node2/genesis/validator.yaml
3. node3/genesis/validator.yaml
4. node4/genesis/validator.yaml

For example, modify configurations similar to the following to your local path:

```yaml
base:
  role: "validator"
  data_dir: "/tmp/node1/data"
  waypoint:
    from_file: "/tmp/node1/genesis/waypoint.txt"
```

## Compilation

Execute the following command in the project root directory to compile:

```
cargo build
```

## Single Node Cluster Deployment

### Deploy Node

Deploy to the /tmp directory by default:

```
./deploy_utils/test_deploy.sh node1
```

### Start Node

```
cd /tmp/node1
./script/start.sh
```

### Stop Node

```
./script/stop.sh
```

## Multi-Node Cluster Deployment

### Start node1

Follow the single node cluster process to start node1.

### Start node2

Execute the following command to start node2:

```
./deploy_utils/test_deploy.sh node2

cd /tmp/node2

./script/start.sh
./script/stop.sh
```

### Start node3 and node4

The startup process for node3 and node4 is the same as node1.

## Important Notes

1. Ensure all paths in configuration files are correctly modified before starting the nodes.
2. For multi-node cluster deployment, ensure proper communication between nodes.
3. If port conflicts occur, check and modify the corresponding port configurations.

## Troubleshooting

If you encounter startup issues, please check:
1. Whether the configuration file paths are correct
2. Whether the ports are occupied
3. System firewall settings

For further assistance, please check the log files or contact technical support.