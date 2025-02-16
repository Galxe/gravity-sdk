
## Gravity-SDK

![logo](https://framerusercontent.com/images/KFAvs9kq8cPUVXIEBaW6UuhuK0.svg)

[Readme](./readme.md) 
| [Book](https://github.com/Galxe/gravity-sdk)



## Introduction
Gravity-SDK is a high-performance consensus algorithm component designed as a modern alternative to Tendermint. Built on the Aptos-BFT consensus algorithm, it provides a modular and scalable framework for blockchain systems requiring efficient consensus mechanisms.

## Architecture

The system implements a three-stage consensus pipeline:

1. **Pre-Consensus Stage (Quorum Store, WIP)**   
   - Broadcasts incoming transactions to peer nodes
   - Collects signatures to generate Proof of Store (POS) to verify transaction collection and storage

2. **Consensus Stage (Aptos-BFT Core)**
   - View Change: Switches to a new leader node when the current leader fails
   - Block Proposal and Validation: Leader node packages transactions into blocks and proposes them; other nodes validate
   - Quorum Certificate Formation: Collects signatures from sufficient validator nodes

3. **Post-Consensus Stage**
   - Aggregates transaction execution results across nodes
   - Manages digital signatures from participating nodes
   - Finalizes and commits blocks to the chain

## Features

### Advanced Consensus Mechanism
- Implements Aptos-BFT algorithm
- Ensures fault tolerance and high performance
- Supports dynamic validator sets

### Modular Design
- Pluggable components
- Customizable consensus parameters
- Extensible architecture

### Pipeline Processing
- Separate transaction batch consensus
- Independent execution result consensus
- Optimized throughput and latency

### GCEI Protocol Integration
- Standardized consensus-execution interface
- Robust recovery mechanisms
- Asynchronous communication support

## GCEI Protocol
The GCEI (Gravity Consensus Execution Interface) protocol is the communication bridge between the consensus and execution modules in Gravity-SDK. It standardizes the interaction between the two layers, ensuring that consensus and execution processes are properly synchronized.

### Execution Layer API
1. request_batch: Requests a batch of transactions from the execution engine for processing.
2. send_orderblock: Sends the ordered block to the execution engine for execution.
3. commit: Submits execution results to the consensus layer for final agreement.
### Consensus Layer API
1. recv_batch: Receives a transaction batch from the consensus layer.
2. recv_compute_res: Receives the execution result from the execution engine.


## Getting Started
To help you get started with Gravity-SDK, we provide detailed instructions for deploying and running nodes.

### Prerequisites
Ensure you have the necessary development environment set up, including the required dependencies for compiling and running the SDK.
Familiarity with blockchain concepts, especially consensus algorithms and execution layers, is recommended.

### Compiling the Binaries

To compile the binaries for the project, we use a `Makefile` to manage the build process. The `Makefile` allows you to compile different components with customizable options.

Here’s how to use the `Makefile` to compile the binaries:

#### Prerequisites
- Ensure that you have [Rust](https://www.rust-lang.org/learn/get-started) installed, as it is required for building the project.
- The `Makefile` controls the build process for the following binaries:
  - `gravity_node`
  - `bench`
  - `kvstore`

#### Steps to Compile

1. **Clone the Repository and Navigate to the Project Directory**
   Ensure you have cloned the repository and navigated to the root directory of the project.

2. **Choose the Mode and Features (Optional)**
   The build mode can be set to `release` or `debug` (default: `release`). If you need specific features enabled, use the `FEATURE` variable.

   Example:
   ```bash
   make MODE=debug FEATURE=some_feature
   ```

3. **Build the Project**
   To compile the project, run the following command:
   ```bash
   make
   ```

   This will compile the binary specified in the `BINARY` variable, which defaults to `gravity_node`. To build another binary (e.g., `bench` or `kvstore`), set the `BINARY` variable as follows:

   ```bash
   make BINARY=bench
   ```

4. **Clean the Build Artifacts (Optional)**
   If you want to clean up the build artifacts, you can run:
   ```bash
   make clean
   ```

   This will remove any previously compiled files from the `bin/` directory.

#### Customizing the Build
- **Build Mode**: The default build mode is `release`. You can set the mode to `debug` by using the `MODE` variable:
  ```bash
  make MODE=debug
  ```
- **Features**: You can specify additional features to include in the build by setting the `FEATURE` variable:
  ```bash
  make FEATURE=some_feature
  ```

This setup ensures that all required components are compiled based on the configuration you specify.


### Quick Start Guide
For step-by-step instructions on how to deploy a network of multiple nodes, refer to the following guide:

- [Deploy 4 nodes](deploy_utils/readme.md)

This guide provides a comprehensive walkthrough of setting up a four-node network

## Contributing

We encourage contributions to the Gravity-SDK project. Whether you want to report an issue, suggest a new feature, or submit a pull request, we welcome your input! Please see our [Contributing Guidelines](CONTRIBUTING.md) for more information on how to get involved.

