# GCEI Protocol Compatibility

## Abstract

This document provides a detailed specification for integrating the **Generic Consensus-Execution Interface (GCEI)** with two widely used consensus protocols: **Tendermint** and the **Ethereum Engine API**. It outlines the protocol flows, state transitions, and interface mappings required to ensure seamless and consistent block processing across different consensus mechanisms. The goal is to establish a standardized execution model that promotes interoperability and uniformity in block finalization and state persistence across various blockchain systems.

## 1. Protocol Overview

The **Generic Consensus-Execution Interface (GCEI)** serves as a bridge between the consensus layer and the execution layer, standardizing the communication and data exchange mechanisms between these two critical components in a blockchain system.

### 1.1 GCEI Core Interface

The GCEI interface defines a set of asynchronous methods used for requesting and processing blocks, as well as for committing finalized blocks to the blockchain. The interface is designed to be thread-safe and can be implemented in a multi-threaded environment.

```rust
#[async_trait]
pub trait ExecutionApi: Send + Sync {
    /// Requests a batch of blocks for execution based on the current state of the blockchain.
    async fn request_block_batch(&self, state_block_hash: BlockHashState) -> BlockBatch;

    /// Sends an ordered block batch to the execution layer for processing.
    async fn send_ordered_block(&self, ordered_block: BlockBatch);

    /// Receives the executed block hash after the execution process is complete.
    async fn recv_executed_block_hash(&self) -> HashValue;

    /// Commits the finalized block hash, marking it as part of the immutable state.
    async fn commit_block_hash(&self, block_ids: Vec<BlockId>);
}
```

### 1.2 Block State Transitions

The block state transitions define the lifecycle of a block as it moves through the consensus and execution layers. This ensures that each block passes through the necessary stages before being finalized and committed to the blockchain.

```plaintext
[Pending] -> [Ordered] -> [Executed] -> [Finalized]
```

- **Pending**: The block is in a temporary state, awaiting consensus ordering.
- **Ordered**: The block has been ordered by the consensus layer and is ready for execution.
- **Executed**: The block has been processed by the execution layer, and its state has been updated.
- **Finalized**: The block has been confirmed by the consensus layer and committed to the blockchain.

---

## 2. Protocol Mappings

### 2.1 Tendermint Integration

In the case of **Tendermint**, the GCEI interface maps onto Tendermint's consensus protocol, which relies on **ABCI (Application Blockchain Interface)** interactions for communication between the consensus and execution layers.

#### Interface Correlations

The table below illustrates how the GCEI methods correspond to Tendermint's primary and secondary functions:

| GCEI Interface         | Tendermint Primary   | Tendermint Secondary   |
|------------------------|---------------------|------------------------|
| request_block_batch     | PrepareProposal     | GetMempool, CheckTx     |
| send_ordered_block      | ProcessProposal     | DeliverTx              |
| recv_executed_block_hash| EndBlock            | DeliverTx.Response      |
| commit_block_hash       | Commit              | CommitState            |

#### State Flow

The following sequence outlines the flow of block processing in Tendermint when integrated with GCEI:

```plaintext
PrepareProposal:
  Input: RequestPrepareProposal {
    max_tx_bytes: i64,
    txs: Vec<Vec<u8>>,
    local_last_commit: CommitInfo,
  }
  Output: ResponsePrepareProposal {
    block: BlockBatch,
  }

ProcessProposal:
  Input: RequestProcessProposal {
    txs: Vec<Vec<u8>>,
    proposed_last_commit: CommitInfo,
  }
  Output: ResponseProcessProposal {
    status: ProposalStatus,
  }
```

- **PrepareProposal**: The Tendermint consensus layer requests a proposal, which includes a batch of transactions that are then ordered by GCEI.
- **ProcessProposal**: Once the ordered transactions are received, the execution layer processes them and updates the block state.
- **EndBlock & Commit**: After execution, the final block state is returned, and the block is committed to the blockchain.

### 2.2 Ethereum Engine API Integration

For the **Ethereum Engine API**, the GCEI interface integrates with the consensus (fork-choice) and execution layers of Ethereum, providing a standardized mechanism for handling block proposals and state commitments.

#### Interface Correlations

The table below maps GCEI functions to the corresponding Ethereum Engine API methods:

| GCEI Interface         | Engine API Primary     | Engine API Secondary   |
|------------------------|------------------------|------------------------|
| request_block_batch     | engine_forkchoiceUpdated | engine_getPayload      |
| send_ordered_block      | engine_newPayload      | engine_executePayload  |
| recv_executed_block_hash| engine_executePayload  | PayloadStatus          |
| commit_block_hash       | engine_consensusValidated| engine_forkchoiceUpdated|

#### State Flow

The Ethereum Engine API uses a fork-choice rule to determine the canonical chain. The state flow in GCEI integration with Ethereum is as follows:

```plaintext
ForkchoiceState:
  struct {
    headBlockHash: H256,
    safeBlockHash: H256,
    finalizedBlockHash: H256,
  }

PayloadStatus:
  enum {
    VALID,
    INVALID,
    SYNCING,
    ACCEPTED,
    INVALID_BLOCK_HASH,
  }
```

- **ForkchoiceState**: This structure contains crucial information about the current head of the blockchain and the finalized block.
- **PayloadStatus**: After executing a block, the status of the payload is returned, indicating whether the block is valid or if further syncing is required.

---

## 3. Implementation Guidelines

The block processing lifecycle follows a well-defined sequence of transitions between different states. This ensures that blocks are processed efficiently and consistently across both Tendermint and Ethereum consensus mechanisms.

### 3.1 Block Processing Lifecycle

#### Pending to Ordered

1. **Consensus Layer**:
    - The consensus layer invokes `request_block_batch()` to retrieve a batch of transactions for processing.
    - The current blockchain state (`BlockHashState`) is provided as input, and unordered transactions are returned.

2. **Ordering Process**:
    - The transactions are ordered based on the consensus mechanism.
    - The ordered transactions are encapsulated in a `BlockBatch` structure and submitted to the execution layer via `send_ordered_block()`.

#### Ordered to Executed

1. **Execution Layer**:
    - The execution layer processes the ordered transactions, updating the state tree and generating the new state root.
    - Once execution is complete, the execution hash is returned via `recv_executed_block_hash()`.

2. **Validation**:
    - The consensus layer validates the execution results by verifying the state transitions and transaction receipts.
    - If validation passes, the block is ready for finalization.

#### Executed to Finalized

1. **Finalization**:
    - Consensus is reached on the execution results, and the finalized block hash is committed using `commit_block_hash()`.
    - The finalized block height is updated in the blockchain.

2. **State Persistence**:
    - The state changes are committed to persistent storage (e.g., a blockchain database).
    - Finalization events are emitted, ensuring that the block has been securely added to the immutable ledger.

