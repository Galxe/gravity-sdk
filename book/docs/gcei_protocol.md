# GCEI (Gravity Consensus Execution Interface) Protocol Specification

## 1. Overview

The **Gravity Consensus Execution Interface (GCEI)** protocol provides a standardized interface for communication between a blockchain's consensus and execution layers. Built on the **Aptos-BFT** consensus mechanism, GCEI ensures seamless coordination between transaction ordering and state execution, maintaining system integrity and supporting recovery operations.

## 2. Protocol Architecture

### 2.1 Core Components

1. **Interface Layers**  
   - **Consensus Layer**: Manages block ordering and consensus agreement based on the Aptos-BFT protocol.
   - **Execution Layer**: Responsible for processing transactions and updating the blockchain state.
   - **Recovery Layer**: Handles recovery operations to maintain consistency in the event of failures.

2. **Communication Flow**  
   GCEI facilitates communication between these layers as follows:

```text
Consensus Layer ←→ GCEI Protocol ←→ Execution Layer
        ↑                               ↑
        └──────── Recovery Layer ───────┘
```

### 2.2 State Management

1. **Block States**  
   GCEI operates with several key block states:

   - **Safe Block**: The last block that has been fully committed to the chain.
   - **Head Block**: The most recent block received by the execution engine.
   - **Ordered Block**: A block whose transactions have been ordered by the consensus layer.
   - **Executed Block**: A block whose transactions have been executed by the execution engine.

2. **State Transitions**  
   Blocks move through the following states:

```text
[Pending] → [Ordered] → [Executed] → [Committed]
```

## 3. API Specification – Design

### 3.1 Execution Layer API – Design

The **Execution Layer API** manages the interaction between the execution layer and the consensus layer. It defines the following core functions:

1. **`request_block_batch(state: BlockHashState) -> Result<BlockBatch>`**  
   - Description: Requests a batch of transactions from the execution engine based on the current blockchain state.
   - Inputs:  
     - `state`: The state of the block hash, including the safe block and the head block.
   - Output:  
     - `BlockBatch`: A batch of transactions to be processed.

2. **`send_ordered_block(block: BlockBatch) -> Result<()>`**  
   - Description: Sends an ordered block (a batch of transactions) from the consensus layer to the execution engine for processing.
   - Inputs:  
     - `block`: A batch of transactions that has been ordered by the consensus layer.

3. **`recv_executed_block_hash() -> Result<HashValue>`**  
   - Description: Receives the hash of the executed block from the execution engine after processing.
   - Output:  
     - `HashValue`: The hash of the executed block.

4. **`commit_block_hash(block_ids: Vec<BlockId>) -> Result<()>`**  
   - Description: Commits the block identified by the provided block IDs to finalize it in the blockchain.
   - Inputs:  
     - `block_ids`: A list of block IDs representing the blocks to be committed.

### 3.2 Consensus Layer API – Design

The **Consensus Layer API** enables the consensus layer to interact with the execution layer. The following functions are defined:

1. **`recv_batch() -> Result<BlockBatch>`**  
   - Description: Receives a batch of transactions prepared by the execution engine for consensus ordering.
   - Output:  
     - `BlockBatch`: A batch of transactions ready for ordering by the consensus layer.

2. **`recv_compute_res() -> Result<ExecutionResult>`**  
   - Description: Receives the results of the transaction execution from the execution engine.
   - Output:  
     - `ExecutionResult`: The results of the transaction execution, which the consensus layer uses for block finalization.

## 4. Protocol Operations

### 4.1 Transaction Processing Flow

1. **Batch Request**  
   The execution engine initiates a request for a transaction batch. The consensus layer validates the integrity of the batch and returns it for further processing.

2. **Block Ordering**  
   The consensus layer orders the transactions, creates an ordered block, and transmits it to the execution engine for processing.

3. **Execution**  
   The execution engine processes the transactions, generating execution results and the block hash, which is returned to the consensus layer.

4. **Commitment**  
   The consensus layer verifies the execution results, collects validator signatures, and commits the finalized block to the blockchain.

### 4.2 Recovery Mechanism

todo()

## 5. Error Handling

todo()


### 6.1 Core API Implementation

The GCEI protocol is implemented using a single primary trait, `ExecutionApi`, which handles all core interactions between the consensus and execution layers. The implementation uses Rust's async/await pattern for efficient non-blocking operations:

```rust
#[async_trait]
pub trait ExecutionApi: Send + Sync {
    // Request transactions from execution engine
    // safe block id is the last block id that has been committed in block tree
    // head block id is the last block id that received by the execution engine in block tree
    async fn request_block_batch(&self, state_block_hash: BlockHashState) -> BlockBatch;

    async fn send_ordered_block(&self, ordered_block: BlockBatch);

    // the block hash is the hash of the block that has been executed, which is passed by the send_ordered_block
    async fn recv_executed_block_hash(&self) -> HashValue;

    // this function is called by the execution layer commit the block hash
    async fn commit_block_hash(&self, block_ids: Vec<BlockId>);
}
```

### 6.2 Implementation Details

The `ExecutionApi` trait provides four core functions that implement the operations described in the design section:

1. **`request_block_batch`**
   - Implements the batch request operation from Section 3.1.1
   - Takes a `BlockHashState` parameter containing safe and head block information
   - Returns a `BlockBatch` containing the requested transactions
   - Corresponds to the design's batch request flow in Section 4.1

2. **`send_ordered_block`**
   - Implements the ordered block transmission operation from Section 3.1.2
   - Accepts a `BlockBatch` containing ordered transactions
   - Facilitates the block ordering process described in Section 4.1

3. **`recv_executed_block_hash`**
   - Implements the execution result reception from Section 3.1.3
   - Returns a `HashValue` representing the executed block's hash
   - Supports the execution phase detailed in Section 4.1

4. **`commit_block_hash`**
   - Implements the block commitment operation from Section 3.1.4
   - Takes a vector of `BlockId`s for committing multiple blocks
   - Completes the commitment phase described in Section 4.1


## 7. Conclusion

The **GCEI protocol** is a key component of the **Gravity-SDK**, enabling efficient communication between the consensus and execution layers in a blockchain system. The design of the **Execution Layer API** and **Consensus Layer API** ensures that both layers can interact seamlessly. By separating the design from the implementation, the protocol provides flexibility for future improvements and optimizations. The use of Rust's asynchronous traits ensures that the protocol can handle high transaction volumes efficiently, making it a robust solution for modern blockchain systems.



## Detailed Stages

### 1. Pre-Consensus Stage

The **Pre-Consensus Stage** focuses on preparing transactions for the consensus process. This stage ensures that transactions are available and ready for consensus by collecting signatures that verify their availability.

Key Components:

- **Transaction Broadcasting:**

  Incoming transactions are broadcast to all peer nodes in the network. This process makes the network aware of new transactions that will eventually be proposed for consensus.

- **Quorum Store (Work in Progress):**

  The protocol aggregates quorum signatures from validators, generating a **Proof of Available (PoAv)**. The **PoAv** is a cryptographic proof that confirms the availability and accessibility of a transaction batch. This means that a sufficient number of validators have acknowledged that they can access the transactions, ensuring the batch is ready for consensus without needing to send the actual transactions again.


### 2. Consensus Stage

The **Consensus Stage** is where the network reaches agreement on the next block to be executed, ensuring the block complies with the 2-chain safety rules.

Key Components:

- **View Change:**

  If the current leader fails to propose a block within the expected time, a view change is triggered to elect a new leader. This process follows the **2-chain safety rules**, ensuring the new leader proposes a block consistent with the previous quorum certificate (QC) or timeout certificate (TC).

- **Block Proposal and Validation:**

  The leader selects transactions and packages them into a block for proposal. If the transactions have a **Proof of Available (PoAv)** in the **Quorum Store**, the leader can propose the **PoAv** instead of the full transactions. This optimizes the consensus process by reducing the data validators need to check.

  To ensure 2-chain safety:

   1. The new block can only be proposed if it follows a block with a QC or TC.
   2. A block is valid for ordering only if its child block has a QC or TC, ensuring chain continuity.
- **Quorum Certificate (QC) Formation:**

  Once the block (or **PoAv**) is validated, signatures from a sufficient number of validators are collected to form a **Quorum Certificate (QC)**, confirming the block's acceptance by the majority.

- **Execution Request:**

  After reaching consensus, the block or **PoAv** is sent to the execution engine for processing, adhering to the **2-chain safety rules**.


### 3. Post-Consensus Stage

The **Post-Consensus Stage** finalizes and commits the block to the blockchain, ensuring all nodes are synchronized and the block is safely recorded.

Key Components:

- **Execution Result Aggregation:**

  The execution engine processes the block's transactions and returns the results to the consensus layer. These results are aggregated across nodes and validated according to the **2-chain safety rules**, ensuring that blocks are processed in the correct sequence.

- **Signature Management:**

  Each node signs off on the execution results, verifying their agreement with the outcome.

- **Block Commitment:**

  Once the execution results are validated and sufficient signatures are collected, the block is committed to the blockchain.


## Recovery Mechanism

1. Supports local execution layer recovery. When restarting, if the height of the blocks that have achieved QC in the consensus layer is higher than the height of the blocks that have been executed in the execution layer, the blocks that have performed QC are directly played back.
2. If a new node is added or a node is restarted, receives the communication from the consensus layer, and finds that its current Round lags far behind other nodes, Block Sync is initiated to synchronize the missing blocks, and then playback is performed to achieve the status of the latest Round and participate in voting.