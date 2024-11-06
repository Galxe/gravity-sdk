
## Gravity-SDK

[Readme](./readme.md) 
| [GravitySDKBook](docs/GravitySDKBook.md)

Introduction
Gravity-SDK is a high-performance, modular consensus algorithm component designed as an alternative to Tendermint. It integrates the Aptos-BFT consensus algorithm, offering a highly customizable and scalable framework for blockchain systems. Its design enables efficient, secure, and fault-tolerant consensus processes, making it suitable for various blockchain ecosystems.

Why Use Gravity-SDK?
Gravity-SDK is built to address the performance and flexibility demands of modern blockchain systems. With a focus on modularity and efficiency, it provides blockchain developers with a robust solution for consensus, while ensuring compatibility with widely-used blockchain protocols like Ethereum.

Key Advantages
Efficient Consensus:
Utilizes the Aptos-BFT algorithm, which provides high efficiency and fault tolerance, ensuring consensus is reached even in the event of node failures.
Modular Architecture:
The SDK is designed to be easily integrated into different blockchain platforms. The modular components allow developers to customize and extend functionality based on specific requirements.
Pipeline Optimization:
By separating the consensus process into two distinct pipelines—one for transaction batch consensus and another for execution result consensus—Gravity-SDK optimizes workflow and improves overall performance.
Unified External Interface:
The SDK adopts the GCEI Protocol to provide a unified interface, simplifying communication between consensus and execution layers and ensuring seamless integration into various blockchain architectures.
Ethereum Compatibility:
Gravity-SDK is fully compatible with Ethereum's Engine API, making it interoperable with the Ethereum ecosystem and ensuring flexibility for multi-chain projects.
Core Features
Gravity-SDK offers several key features that make it a powerful tool for blockchain developers:

Batch Consensus:
The SDK processes batches of transactions and submits them for consensus. This feature is crucial for blockchains that prioritize high throughput and efficient batch processing.
Execution Result Consensus:
After transaction execution, results are submitted to the consensus layer to reach agreement. This ensures that both transaction data and execution outcomes are securely agreed upon by the network.
Architecture and Protocols
GCEI Protocol
The GCEI (Gravity Consensus Execution Interface) protocol is the communication bridge between the consensus and execution modules in Gravity-SDK. It standardizes the interaction between the two layers, ensuring that consensus and execution processes are properly synchronized.

Execution Layer API
request_batch: Requests a batch of transactions from the execution engine for processing.
send_orderblock: Sends the ordered block to the execution engine for execution.
commit: Submits execution results to the consensus layer for final agreement.
Consensus Layer API
recv_batch: Receives a transaction batch from the consensus layer.
recv_compute_res: Receives the execution result from the execution engine.
Optimized Pipeline Design
Gravity-SDK divides the consensus process into two pipelines:

Batch Consensus Pipeline: Handles the consensus on incoming transaction batches.
Result Consensus Pipeline: Handles the consensus on the results of the executed transactions.
This separation optimizes the overall workflow by reducing bottlenecks and ensuring that both transaction batches and execution results are processed efficiently.

Getting Started
To help you get started with Gravity-SDK, we provide detailed instructions for deploying and running nodes.

Prerequisites
Ensure you have the necessary development environment set up, including the required dependencies for compiling and running the SDK.
Familiarity with blockchain concepts, especially consensus algorithms and execution layers, is recommended.
Quick Start Guide
For step-by-step instructions on how to deploy a network of multiple nodes, refer to the following guide:

- [Deploy 4 nodes](deploy_utils/readme.md)

This guide provides a comprehensive walkthrough of setting up a four-node network

## Contributing

We encourage contributions to the Gravity-SDK project. Whether you want to report an issue, suggest a new feature, or submit a pull request, we welcome your input! Please see our [Contributing Guidelines](CONTRIBUTING.md) for more information on how to get involved.

