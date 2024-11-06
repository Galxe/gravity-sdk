
## Gravity-SDK

[Readme](./readme.md) 
| [GravitySDKBook](docs/GravitySDKBook.md)

## What is Gravity-SDK?

Gravity-SDK is a consensus algorithm component designed as an alternative to Tendermint, aimed at providing a high-performance and modular consensus solution for blockchain systems. It leverages the Aptos-BFT consensus algorithm, offering a highly customizable and scalable framework that enables efficient blockchain consensus processes.

## 🌟 Features

Gravity-SDK provides the following key features for the consensus module within a blockchain:

Consensus on Receiving Batches: Processes and receives transaction batches, which then enter the consensus process.
Consensus on Execution Results: Submits execution results to the consensus layer to reach agreement.
With its modular architecture, Gravity-SDK offers a flexible consensus mechanism suitable for various blockchain scenarios.

## Key Advantages

Efficient Aptos-BFT Consensus Algorithm:
Gravity-SDK employs the Aptos-BFT consensus algorithm for highly efficient fault-tolerant consensus.
Modular Architecture:
The SDK is designed with a modular structure, making it easy to integrate into different blockchain systems.
Optimized Pipeline Consensus:
The SDK divides the consensus process into two separate pipelines: one for batch consensus and another for result consensus, optimizing the flow and improving performance.
Unified Interface:
Gravity-SDK uses the GCEI protocol for a unified external interface, allowing seamless integration into a variety of blockchain architectures.
Compatibility with ETH Engine API:
Gravity-SDK is compatible with Ethereum's Engine API, facilitating interoperability with the Ethereum ecosystem.

## GCEI Protocol

Gravity-SDK uses the GCEI protocol as the communication bridge between its consensus and execution modules. The GCEI protocol is divided into two main parts: the Execution Layer API and the Consensus Layer API.

Core-API
Execution Layer API
request_batch: Requests a batch of transactions from the execution engine.
send_orderblock: Sends the ordered block to the execution engine.
commit: Submits the execution results to the consensus layer.
Consensus Layer API
recv_batch: Receives a batch of transactions from the consensus layer.
recv_compute_res: Receives the execution result from the execution layer.

## 🚀 Quick Start

To lauch mutilpe nodes, please refer to the documentation listed below:
- [Deploy 4 nodes](deploy_utils/readme.md)


t each method of the trait according to your specific consensus requirements.

## 🤝 Contributing

We welcome contributions to the gravity-sdk project! Please see our [Contributing Guidelines](CONTRIBUTING.md) for more information on how to get involved.

---

gravity-sdk - Empowering blockchain projects with flexible, efficient consensus mechanisms.


