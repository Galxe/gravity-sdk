# README for `kvstore`

## Overview
`kvstore` is a simple, Redis-like key-value store server implemented in Rust. It supports basic commands such as `SET` and `GET` over a network connection. This document describes how to compile, deploy, and use the program.

---

## Installation

### Build the Binary
After cloning the project, you can compile the binary using:
```bash
cargo build --release
```
This will generate the executable binary named `kvstore` in the `target/release` directory.

---

## Deployment

To deploy and start the server in single-node mode, execute the following steps from the project root:

```bash
rm -rf /tmp/node1
./deploy_utils/deploy.sh --mode single --node node1 --bin_version release
./kvstore --gravity_node_config /tmp/node1/genesis/validator.yaml --listen-url 127.0.0.1:8545 --log-dir /tmp/node1/kvlogs
```

Or you can deploy and start in the cluster mode. For example if you have four server then you can:

```bash
rm -rf /tmp/node1
rm -rf /tmp/node2
rm -rf /tmp/node3
rm -rf /tmp/node4
./deploy_utils/deploy.sh --mode cluster --node node1 --bin_version release
./deploy_utils/deploy.sh --mode cluster --node node2 --bin_version release
./deploy_utils/deploy.sh --mode cluster --node node3 --bin_version release
./deploy_utils/deploy.sh --mode cluster --node node4 --bin_version release
./kvstore --gravity_node_config /tmp/node1/genesis/validator.yaml --listen-url 127.0.0.1:8545 --log-dir /tmp/node1/kvlogs
./kvstore --gravity_node_config /tmp/node2/genesis/validator.yaml --listen-url 127.0.0.1:8546 --log-dir /tmp/node2/kvlogs
./kvstore --gravity_node_config /tmp/node3/genesis/validator.yaml --listen-url 127.0.0.1:8547 --log-dir /tmp/node3/kvlogs
./kvstore --gravity_node_config /tmp/node4/genesis/validator.yaml --listen-url 127.0.0.1:8548 --log-dir /tmp/node4/kvlogs
```

### Explanation:
1. **`deploy.sh` Script**: Prepares the environment and deploys the server in single-node mode.
2. **Command-Line Arguments for `kvstore`**:
   - `--gravity_node_config`: Path to the configuration file (e.g., `validator.yaml`).
   - `--listen-url`: The server's listening address and port (e.g., `127.0.0.1:8545`).
   - `--log-dir`: Path to the log directory for kv store.

Once the above steps are complete, the server will be running and ready to accept connections.

---

## Usage

### Connecting to the Server
You can use `nc` (Netcat) to interact with the server over the specified `listen-url`. For example:
```bash
nc 127.0.0.1 8545
```

### Commands
The server supports Redis-like commands. Examples:

#### Invalid Command
```plaintext
set hello word
Unknown command
```

#### Valid Commands
```plaintext
SET hello world
OK
GET hello
world
```

- **`SET <key> <value>`**: Stores a value for a given key.
- **`GET <key>`**: Retrieves the value associated with the given key.

---

## Troubleshooting

- Ensure that the `kvstore` binary is correctly built and accessible.
- Verify that the `gravity_node_config` file path is valid and points to the correct configuration.
- Ensure the `db-path` directory is writable and accessible.

---

## Notes
- This is a lightweight implementation for demonstration and learning purposes.
- For production-grade systems, consider additional features such as persistence, authentication, and clustering.

---

## License
This project is open-source and available under the MIT license. Feel free to contribute or modify as needed!