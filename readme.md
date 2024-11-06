
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

### Quick Start Guide
For step-by-step instructions on how to deploy a network of multiple nodes, refer to the following guide:

- [Deploy 4 nodes](deploy_utils/readme.md)

This guide provides a comprehensive walkthrough of setting up a four-node network

## Contributing

We encourage contributions to the Gravity-SDK project. Whether you want to report an issue, suggest a new feature, or submit a pull request, we welcome your input! Please see our [Contributing Guidelines](CONTRIBUTING.md) for more information on how to get involved.

