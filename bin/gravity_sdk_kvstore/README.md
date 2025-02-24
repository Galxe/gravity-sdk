# README for `gravity_sdk_kvstore`

## Overview

`gravity_sdk_kvstore` is a lightweight, Rust-based key-value store server, designed to emulate certain functionalities of Celestia. It provides three HTTP endpoints (`add_txn`, `get_receipt`, and `get_value`) for network interaction. This document guides you through the compilation, deployment, and usage of the server.

**Note:** This code serves as a minimum viable implementation for demonstrating how to build a DApp using `gravity-sdk`. It does not include account balance validation, comprehensive error handling, or robust runtime fault tolerance. Current limitations and future tasks include:

* **Block Synchronization:** Block synchronization is not yet implemented. A basic Recover API is required for block synchronization functionality.
* **State Persistence:** The server does not load persisted state data on restart, leading to state resets after each restart.
* **Execution Pipeline:** Although the execution layer pipeline is designed with five stages, it currently executes blocks serially instead of in a pipelined manner.

---

## Installation

### Building the Binary

To compile the project, execute the following command after cloning the repository:

```bash
cargo build --release
```

This command generates the optimized executable gravity_sdk_kvstore in the target/release/ directory.

---

## Deployment

### Single-Node Deployment

To deploy and start the server in single-node mode, perform the following steps from the project root:

```bash
rm -rf /tmp/node1

./deploy_utils/deploy.sh --mode single --node node1 -v debug -b gravity_sdk_kvstore

./target/release/gravity_sdk_kvstore \
    --genesis_path $genesis_path \
    --listen_url $url \
    --gravity_node_config $validator.yaml \
    --log_dir $log_dir
```

### Cluster Deployment

For a cluster deployment with four nodes, execute the following commands:

```bash
rm -rf /tmp/node1 /tmp/node2 /tmp/node3 /tmp/node4

./deploy_utils/deploy.sh --mode cluster --node node1 -v debug -b gravity_sdk_kvstore
./deploy_utils/deploy.sh --mode cluster --node node2 -v debug -b gravity_sdk_kvstore
./deploy_utils/deploy.sh --mode cluster --node node3 -v debug -b gravity_sdk_kvstore
./deploy_utils/deploy.sh --mode cluster --node node4 -v debug -b gravity_sdk_kvstore

./target/release/gravity_sdk_kvstore \
    --genesis_path $genesis_path \
    --listen_url $url_node1 \
    --gravity_node_config $validator_node1 \
    --log_dir $log_dir_node1
./target/release/gravity_sdk_kvstore \
    --genesis_path $genesis_path \
    --listen_url $url_node2 \
    --gravity_node_config $validator_node2 \
    --log_dir $log_dir_node2
./target/release/gravity_sdk_kvstore \
    --genesis_path $genesis_path \
    --listen_url $url_node3 \
    --gravity_node_config $validator_node3 \
    --log_dir $log_dir_node3
./target/release/gravity_sdk_kvstore \
    --genesis_path $genesis_path \
    --listen_url $url_node4 \
    --gravity_node_config $validator_node4 \
    --log_dir $log_dir_node4
```

### Explanation:
1. **`deploy.sh` Script**: Prepares the environment and deploys the server in single-node mode.
2. **Command-Line Arguments for `kvstore`**:
   - `--gravity_node_config`: Specifies the path to the node configuration file (e.g., validator.yaml).
   - `--listen_url`: Defines the server's listening address and port (e.g., 127.0.0.1:8545).
   - `--log_dir`: Sets the path to the log directory for the key-value store.
   - `--genesis_path`: Provides the path to the genesis file.

After executing these steps, the server will be operational and ready to receive requests.

---

## Usage

Interact with the server using the following HTTP endpoints, assuming the server address is 127.0.0.1:9006.

### add_txn

Submit a transaction to the system via the add_txn endpoint. Upon successful submission, the transaction hash is returned.

```shell
curl -X POST -H "Content-Type: application/json" -d '{
  "unsigned": {
    "nonce": 1,
    "kind": {
      "Transfer": {
        "receiver": "0x1234567890abcdef1234567890abcdef12345678",
        "amount": 100
      }
    }
  },
  "signature": "your_signature_here"
}' http://127.0.0.1:9006/add_txn
```

### get_receipt

Retrieve the transaction receipt using the transaction hash.

``` bash
curl -X POST -H "Content-Type: application/json" -d '{
  "transaction_hash": "your_transaction_hash_here"
}' http://127.0.0.1:9006/get_receipt
```

### get_value

Set a key-value pair under an account namespace and retrieve it using the get_value endpoint.

```bash
curl -X POST -H "Content-Type: application/json" -d '[
  "$(openssl rand -hex 20)",
  "key"
]' http://127.0.0.1:9006/get_value
```

---

## Troubleshooting

- Ensure that the `gravity_sdk_kvstore` binary is correctly built and accessible.
- Verify that the `gravity_node_config` file path is valid and points to the correct configuration.

---

## Notes
- This is a lightweight implementation for demonstration and learning purposes.
- For production-grade systems, consider additional features such as authentication, and clustering.

---

## License
This project is open-source and available under the MIT license. Feel free to contribute or modify as needed!