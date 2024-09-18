
# Gravity Consensus Engine Interface (GCEI)

Gravity-sdk is a high-performance, modular consensus engine interface designed to provide a standardized approach for integrating consensus mechanisms into blockchain projects. By encapsulating the Aptos consensus module and exposing a generic interface, GCEI offers a flexible solution for various blockchain initiatives. 

## 🌟 Features

- **Modular Design**: Easily integrate with different blockchain architectures.
- **High Performance**: Optimized for efficient consensus operations.
- **Flexible**: Adaptable to various consensus algorithms and blockchain structures.
- **Standardized Interface**: Consistent API for simplified integration and maintenance.

## 🚀 Quick Start

To lauch mutilpe nodes, please refer to the documentation listed below:
- [Deploy 4 nodes](deploy_utils/readme.md)

To use gravity-sdk in your project, implement the `GravityConsensusEngineInterface` trait:

```rust
pub trait GravityConsensusEngineInterface {
    fn init();
    fn submit_valid_transactions();
    fn polling_ordered_block();
    fn submit_compute_res();
    fn submit_block_head();
    fn polling_commit_block_ids();
    fn submit_commit_block_ids();
}
```

## 📘 API Reference

### `init()`
Initialize the consensus engine. Sets up initial state, network connections, and configurations.

### `submit_valid_transactions()`
Process incoming validated transactions, adding them to the local transaction pool.

### `polling_ordered_block()`
Retrieve and prepare newly ordered blocks for processing.

### `submit_compute_res()` / `submit_block_head()`
Submit computation results / block head back to the consensus mechanism after processing a block.


### `polling_commit_block_ids()`
Mark specified blocks as finalized and trigger related events such as persisting.

### `submit_persistent_block_ids()`
Return persistent IDs of execution layer for transaction cleanup by the consensus mechanism.

## 🛠 Implementation

To implement gravity-sdk in your project:

1. Import the gravity-sdk module.
2. Create a struct that implements the `GravityConsensusEngineInterface` trait.
3. Implement each method of the trait according to your specific consensus requirements.

## 🤝 Contributing

We welcome contributions to the gravity-sdk project! Please see our [Contributing Guidelines](CONTRIBUTING.md) for more information on how to get involved.

---

gravity-sdk - Empowering blockchain projects with flexible, efficient consensus mechanisms.


