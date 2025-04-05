## Gravity SDK: The First Open Source Pipeline Blockchain SDK

![logo](https://framerusercontent.com/images/KFAvs9kq8cPUVXIEBaW6UuhuK0.svg)

[Readme](./readme.md)
| [Book](./book)

## Introduction

Gravity SDK is an open-source, modular blockchain framework that builds upon the world's most
production-ready blockchain, Aptos. It is designed to modularize the existing architecture of the
Aptos blockchain, borrowing battle-tested components such as the mempool with Quorum Store, the
AptosBFT consensus engine to create a the world's first pipelined blockchain SDK.

The decision to base Gravity SDK on Aptos stems from several key factors:

- **State-of-the-Art Blockchain Foundation**: Aptos is the state-of-the-art PoS blockchain of the
  PBFT-family consensus. By introducing
  [Order Votes (AIP-89)](https://github.com/aptos-foundation/AIPs/blob/main/aips/aip-89.md),
  AptosBFT has reduced the consensus latency to 3 hops, which is the theoretically optimal limit on
  a BFT based consensus protocol.
- **Performance Optimized for Extreme Demands**: Aptos has been meticulously optimized for
  performance, achieving an impressive throughput of roughly 160,000 transactions per second with a
  finality time of under one second.
- **Battle-Tested Reliability**: Aptos has already proven its reliability and robustness through
  real-world deployment in production environments, demonstrating its ability to handle demanding
  workloads with ease.
- **Rapid and Continuous Innovation**: Aptos continues to evolve at an exceptional pace. Over the
  past two years,
  [more than 100 Aptos Improvement Proposals (AIPs)](https://github.com/aptos-foundation/AIPs/wiki/Index-of-AIPs)
  have been proposed, discussed, implemented, and deployed on the Aptos mainnet. This relentless
  commitment to improvement keeps Aptos at the cutting edge of blockchain technology, serving as the
  North Star that continually guides the development of Gravity SDK and the chains built upon it.
- **Avoiding Reinvention**: Building on Aptos allows us to leverage its mature and well-tested
  foundation, avoiding the unnecessary complexities and risks associated with starting from scratch.
  Other attempts to outperform Aptos by reinventing the wheel lack both theoretical grounding and
  convincing innovations.
- **Synergetic Evolution** Aptos is a continuously evolving project, introducing features like the
  randomness API and other cutting-edge capabilities. By integrating closely with Aptos, Gravity SDK
  ensures that these innovations can be seamlessly incorporated, creating a synergistic relationship
  between Aptos and chains built on Gravity SDK, e.g. Gravity Chain. On the other hand, Gravity SDK
  also contributes back to Aptos, by modularizing the structure and introducing restaking-powered
  PoS security modules.

Blockchains built on Gravity SDK uses the Gravity Consensus Engine Interface (GCEI) to interact with
the pipelined consensus engine. This interface is designed to be compatible with any execution
layer, although Gravity SDK offers primary support around Gravity reth. We will dive into more
details GCEI in the following section.

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

### Gravity Consent Engine Interface (GCEI) Protocol Integration

- Standardized consensus-execution interface
- Robust recovery mechanisms
- Asynchronous communication support

## BlockBufferManager: The Consensus-Execution Bridge

The BlockBufferManager serves as the core communication bridge implementing the GCEI (Gravity Consensus Execution Interface) protocol within the Gravity-SDK.

Its primary purpose is to decouple the Consensus Layer from the Execution Layer, enabling them to operate concurrently while maintaining synchronization. It achieves this by:

Providing asynchronous, buffered channels for exchanging data (transactions, ordered blocks, execution results, commit confirmations).
Rigorously managing the lifecycle state of blocks (Ordered -> Computed -> Commited) as they progress through the system.
This component acts as a managed, shared interface, ensuring smooth and efficient coordination between the distinct processes of block ordering (Consensus) and block execution (Execution).


### Block Lifecycle Perspective

From a block’s lifecycle standpoint, GCEI structures the interaction between the execution and consensus layers into
three stages:

1. **`Pre-Consensus Stage`**

    - **Forming a Candidate Block**: The consensus layer requests pending transactions from the execution layer (via
      send_pending_txns), then packages them into a candidate block.

2. **`Consensus Stage`**

    - **Ordering & Execution**: The consensus layer proposes the new block, and upon acceptance, sends it to the
      execution layer (recv_ordered_block) for transaction execution.
    - **State Commitment**: After execution, the execution layer calculates the resulting BlockHash and informs the
      consensus layer (send_executed_block_hash). The consensus layer seeks a 2f+1 majority on this state commitment.

3. **`Post-Consensus Stage`**

    - **Finalization & Commitment**: Once the block’s state is finalized, the consensus layer instructs the execution
      layer to persist the finalized block hash by invoking commit_block_info. This action commits the block to the
      blockchain storage.

```
    Stage     Execution Layer    GCEI Protocol       Consensus Layer       Block Status
┌────────────┐       │                  │                      │          B[n-1] B[n] B[n+1]
│    Pre-    │       │                  │   Request Batch      │           ┌──┐  ┌──┐  ┌──┐
│ Consensus  │       │                  │<─────────────────────│           │✓ │  │QC│  │N │
│            │       │<─────────────-───│     Get Batch        │           └──┘  └──┘  └──┘
│            │       │   Return Batch   │                      │          ✓: committed
│            │       │─────────────────>│    Return Batch      │          N: new block
│            │       │                  │─────────────────────>│       
│            │       │                  │                      │
├────────────┤       │                  │                      │          B[n-1] B[n] B[n+1]
│            │       │                  │    Order Block       │           ┌──┐  ┌──┐  ┌──┐
│ Consensus  │       │                  │<─────────────────────│           │✓ │  │⚡ │  │QC│
│            │       │<─────────────────│    Execute Block     │           └──┘  └──┘  └──┘
│            │       │   Return Result  │                      │          ⚡:  executable
│            │       │─────────────────>│    Return Result     │          QC: has a quorum cert
│            │       │                  │─────────────────────>│
│            │       │                  │                      │
├────────────┤       │                  │                      │          B[n-1] B[n] B[n+1]
│   Post-    │       │                  │    Commit Block      │           ┌──┐  ┌──┐  ┌──┐
│ Consensus  │       │                  │<─────────────────────│           │✓ │  │✓ │  │QC│
│            │       │<─────────────────│    Commit Block      │           └──┘  └──┘  └──┘
│            │       │                  │                      │          
└────────────┘       │                  │                      │
```

## Getting Started

To help you get started with Gravity-SDK, we provide detailed instructions for deploying and running nodes.

### Prerequisites

Ensure you have the necessary development environment set up, including the required dependencies for compiling and
running the SDK.
Familiarity with blockchain concepts, especially consensus algorithms and execution layers, is recommended.

### Compiling the Binaries

To compile the binaries for the project, we use a `Makefile` to manage the build process. The `Makefile` allows you to
compile different components with customizable options.

Here’s how to use the `Makefile` to compile the binaries:

#### Prerequisites

- Ensure that you have [Rust](https://www.rust-lang.org/learn/get-started) installed, as it is required for building the
  project.
- The `Makefile` controls the build process for the following binaries:
    - `gravity_node`
    - `bench`
    - `kvstore`

#### Steps to Compile

1. **Clone the Repository and Navigate to the Project Directory**
   Ensure you have cloned the repository and navigated to the root directory of the project.

2. **Choose the Mode and Features (Optional)**
   The build mode can be set to `release` or `debug` (default: `release`). If you need specific features enabled, use
   the `FEATURE` variable.

   Example:
   ```bash
   make MODE=debug FEATURE=some_feature
   ```

3. **Build the Project**
   To compile the project, run the following command:
   ```bash
   make
   ```

   This will compile the binary specified in the `BINARY` variable, which defaults to `gravity_node`. To build another
   binary (e.g., `bench` or `kvstore`), set the `BINARY` variable as follows:

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

We encourage contributions to the Gravity-SDK project. Whether you want to report an issue, suggest a new feature, or
submit a pull request, we welcome your input! Please see our [Contributing Guidelines](CONTRIBUTING.md) for more
information on how to get involved.

