# GCEI (Gravity Consensus Execution Interface) Protocol Specification

## 1. Overview

The **Gravity Consensus Execution Interface (GCEI)** protocol provides a standardized interface for communication between a blockchain's consensus and execution layers. Built on the **Aptos-BFT** consensus mechanism, GCEI ensures seamless coordination between transaction ordering and state execution, maintaining system integrity and supporting recovery operations.

## 2. Protocol Architecture

### 2.1 Core Components

1. **Interface Layers**  
   - **ExecutionChannel Layer**: Responsible for processing transactions and updating the blockchain state.
   - **Recovery Layer**: Handles recovery operations to maintain consistency in the event of failures.

2. **Communication Flow**  
   GCEI facilitates communication between these layers as follows:

```text
Consensus Layer ←→ GCEI Protocol ←→ Execution Layer
        ↑                               ↑
        └──────── Recovery Layer ───────┘
```


## 3. API Specification – Design

### ExecutionChannel API

From the perspective of a transaction’s lifecycle, the `ExecutionChannel APIs` defines the following methods:

1. **`send_pending_txns`**

    - **Input**: None
    - **Output**: Returns `Result<(Vec<VerifiedTxnWithAccountSeqNum>)>`, which contains a verified
      transaction (`VerifiedTxn`) along with the committed nonce of the transaction sender’s account.
    - **Usage**: When called, this method retrieves all pending transactions from the transaction pool and then clears
      the pending queue.

2. **`recv_ordered_block`**

    - **Input**: A `BlockID` and an `OrderedBlock` (of type `ExternalBlock`), which contains ordered transactions and
      block metadata.
    - **Output**: Returns `Result<()>`, indicating whether the block is successfully received and accepted by the
      execution layer.
    - **Usage**: After the consensus engine proposes a block, it sends the block to the execution layer for transaction
      execution. This method allows the execution layer to receive and process the ordered block.

3. **`send_executed_block_hash`**

    - **Input**: A target block number (`BlockNumber`) and its corresponding block identifier (`BlockID`).
    - **Output**: Returns `Result<(ComputeRes)>`, which includes the computed `BlockHash` and the total number of
      transactions (`TxnNum`) processed so far.
    - **Usage**:
        - Once the execution layer computes the state commitment (i.e., the `BlockHash`), it sends this information to
          the consensus layer for finalization. The consensus layer attempts to reach a 2f+1 light consensus with other
          validators.
        - If the finalized state commitment deviates significantly from the originally proposed blocks, the pipeline
          controller may adjust the block proposing pace accordingly.

4. **`commit_block_info`**

    - **Input**: A vector of `BlockID` values, representing the blocks to be committed.
    - **Output**: Returns `Result<()>`, indicating the success or failure of the operation.
    - **Usage**: When the state commitment is finalized, the consensus layer notifies the execution layer to commit the
      block hash to the blockchain storage.

   ![GCEI Protocol](./book/assets/gcei_txn_lifecycle.png)


### 3.2 Recovery API

The `Recovery APIs` defines the following methods which help gravity node recover from an unexpected shutdown:

1. `latest_block_number()`: Retrieves the latest block height known to the Execution Layer.
2. `recover_ordered_block(parent_id, block)`: Replays the specified block from the Consensus Layer to the Execution
   Layer if the Execution Layer is missing it.
3. `register_execution_args(args)`: Collects initial data from the Consensus Layer at startup and sends it to the
   Execution Layer to facilitate recovery.
4. `finalized_block_number()`: Returns the Execution Layer’s highest fully persisted (finalized) block number.

For more details on the GCEI protocol APIs, please refer to the [Gravity SDK Architecture](./book/docs/architecture.md)
and [GCEI Protocol Specification](./book/docs/gcei_protocol.md) . These APIs define the standardized interfaces for
communication between the consensus and execution layers, ensuring seamless integration and efficient operation of the
Gravity SDK framework.

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



### 5.1 Core API Implementation

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

### 5.2 Implementation Details

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


## 6. Conclusion

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