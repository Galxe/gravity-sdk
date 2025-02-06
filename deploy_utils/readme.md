# Node Startup Documentation
## Node Configuration

The system consists of 4 nodes with corresponding ports as follows:

- Node1: 2024
- Node2: 2025
- Node3: 2026
- Node4: 6180

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

In the bin directory, enter either the `bench` or `peth` or `kvstore` directory to compile.

example:

```
cd bin/bench
cargo build

cd bin/peth
cargo build
```

## Single Node Cluster Deployment

### Deploy Node

Deploy to /tmp directory by default:

```
./deploy_utils/deploy.sh --mode single --node node1
```

### Start Node

```
cd /tmp/node1
./script/start.sh --node node1
```

### Stop Node

```
./script/stop.sh
```

In single node deployment mode, only node1 can be started by default. If you need to start other nodes, you need to modify configurations like `deploy_utils/single_node_config.json` and `deploy_utils/single_node_discovery`.

## Multi-Node Cluster Deployment

### Warning

If you want to switch from single-node cluster mode to multi-node cluster mode, you need to clean up the single-node cluster first to avoid conflicts.

```
cd /tmp/node1
./script/stop.sh
cd ../
rm -rf node1
```

### Start node1

Execute the following command to start node1:

```
./deploy_utils/deploy.sh --mode cluster --node node1

cd /tmp/node1
./script/start.sh --node node1
./script/stop.sh
```

### Start node2

Execute the following command to start node2:

```
./deploy_utils/deploy.sh --mode cluster --node node2

cd /tmp/node2
./script/start.sh --node node2
./script/stop.sh
```

Just change the `--node` parameter to corresponding `nodeX`.

### Start node3 and node4

The startup process for node3 and node4 is the same as node1/2.

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

### Log Locations

The logs consist of two parts: `consensus_log` and `execution_logs`. The former includes all logs from the Aptos component, while the latter contains Reth details.
You can set the Aptos log by modifying the `log_file_path` in the corresponding node's `validator.yaml` file.
For execution_logs, you can set the log file configuration by using the `--log.file.directory ${your_directory}` parameter in the command line.