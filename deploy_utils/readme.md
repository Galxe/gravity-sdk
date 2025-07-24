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

In the bin directory, enter either the `bench` or `gravity_node` or `kvstore` directory to compile.

example:

```
cd bin/bench
cargo build

cd bin/gravity_node
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

## Docker & deploy_new.sh Deployment

### 1. Deploy with deploy_new.sh

#### Deploy Node

Assuming you have prepared the config directory (e.g., `my_config/`) and built the binary:

```
./deploy_utils/deploy_new.sh --config_dir ./my_config --deploy_dir /tmp/gravity_node
```
- `--config_dir`: Directory containing identity.yaml, validator.yaml, reth_config.json, etc.
- `--deploy_dir`: Target deployment directory, should match the path in your config files (e.g., `/tmp/gravity_node`)

#### Start Node

```
cd /tmp/gravity_node
./script/start.sh
```

#### Stop Node

```
cd /tmp/gravity_node
./script/stop.sh
```

### 2. Deploy with Docker or Docker Compose

#### Build Docker Image

From the project root directory:

```
docker build -f docker/gravity_node/validator.Dockerfile -t gravity_node:latest .
```

#### Prepare Host Directories

Before starting, create the required host directories:

```
mkdir -p /tmp/node1/data /tmp/node1/logs /tmp/node1/execution_logs /tmp/node1/consensus_log
```

#### Start with Docker

```
docker run -d \
  --name gravity_node1 \
  -v /tmp/node1/data:/gravity_node/data \
  -v /tmp/node1/logs:/gravity_node/logs \
  -v /tmp/node1/execution_logs:/gravity_node/execution_logs \
  -v /tmp/node1/consensus_log:/gravity_node/consensus_log \
  -v /your/config/dir:/gravity_node/config \
  -p 8545:8545 -p 8551:8551 -p 9001:9001 \
  gravity_node:latest
```
- `/your/config/dir` should contain identity.yaml, validator.yaml, reth_config.json, etc.

#### Start with Docker Compose

Example `docker-compose.yaml`:

```yaml
version: '3.8'
services:
  gravity_node1:
    build:
      context: .
      dockerfile: docker/gravity_node/validator.Dockerfile
    container_name: gravity_node1
    volumes:
      - /tmp/node1/data:/gravity_node/data
      - /tmp/node1/logs:/gravity_node/logs
      - /tmp/node1/execution_logs:/gravity_node/execution_logs
      - /tmp/node1/consensus_log:/gravity_node/consensus_log
      - ./docker/gravity_node/config:/gravity_node/config
    environment:
      - RUST_LOG=info
    ports:
      - "8545:8545"
      - "8551:8551"
      - "9001:9001"
    command: ["/gravity_node/script/start.sh"]
```

Start the service:

```
docker compose -f docker/gravity_node/docker-compose.yaml up -d
```

Stop the service:

```
docker compose -f docker/gravity_node/docker-compose.yaml down
```

### Notes

1. Ensure all config file paths are correct before starting the nodes.
2. For multi-node deployment, prepare separate data/logs/config directories for each node and add multiple service blocks in the compose file.
3. If port conflicts occur, adjust the host ports in the `ports` section.
4. Log files are separated into `consensus_log` (Aptos) and `execution_logs` (Reth). You can configure log file locations via config files and command-line parameters.

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