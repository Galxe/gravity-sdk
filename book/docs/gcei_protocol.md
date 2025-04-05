# GCEI (Gravity Consensus Execution Interface) Protocol Specification
## 1. Overview: Decoupling Consensus and Execution

In blockchain systems, the Consensus Layer (responsible for ordering transactions and agreeing on blocks) and the Execution Layer (responsible for processing transactions and updating state) must communicate effectively. Direct, synchronous communication can lead to tight coupling, blocking behavior, and reduced overall system throughput.

The **GCEI protocol**, implemented by the `BlockBufferManager`, addresses this by establishing a standardized, **asynchronous communication bridge** between these two layers. It acts as a shared, managed intermediary that:

* **Decouples** the layers, allowing them to operate more independently and concurrently.
* Provides **buffered queues** for transactions and block state information, smoothing out bursts of activity.
* Manages the **lifecycle state** of blocks rigorously, ensuring both layers have a consistent view of progress.
* Facilitates **synchronization** and state recovery.

This design promotes modularity, parallelism, and robustness in the system architecture.

## 2. Conceptual Architecture: A Shared State Machine & Buffers

The `BlockBufferManager` achieves decoupling through managed, shared state and defined interaction points:

1.  **`TxnBuffer` (Transaction Intake):** A simple, thread-safe queue (`Mutex<Vec<...>>`) acting as the entry point for verified transactions submitted by the Execution Layer (e.g., from a mempool). It provides basic buffering before transactions are consumed by the Consensus Layer.

2.  **`BlockStateMachine` (Core Logic & State):** This is the heart of the manager, tracking the intricate lifecycle of each block.
    * **State Storage (`blocks: HashMap<BlockId, BlockState>`):** A map holding the current status (`BlockState`) of every block actively managed by the buffer, keyed by the unique `BlockId`. This allows efficient lookup and updates.
    * **Block States (`BlockState` enum):** Defines the explicit stages a block progresses through:
        * `Ordered`: Confirmed by Consensus, awaiting execution. Contains full block data.
        * `Computed`: Executed by the Execution Layer, result (hash) available.
        * `Commited`: Confirmed by Consensus as part of the canonical chain, awaiting finalization/persistence by Execution.
    * **Asynchronous Notification (`sender: broadcast::Sender<()>`):** A broadcast channel used to wake up tasks that are waiting for specific data (e.g., Execution waiting for ordered blocks, Consensus waiting for computed results). This avoids inefficient polling.
    * **Synchronization Markers:** Tracks `latest_commit_block_number` and `latest_finalized_block_number` for initialization and state management.

3.  **Configuration & Readiness:**
    * `config: BlockBufferManagerConfig`: Defines timeouts for asynchronous waits, preventing indefinite blocking.
    * `buffer_state: AtomicU8`: Ensures the manager is explicitly initialized (`init`) before use, preventing operations on inconsistent state.

## 3. The Block Lifecycle: A Coordinated State Progression

The GCEI protocol defines a clear lifecycle for blocks, managed by `BlockBufferManager` and driven by interactions from both layers:

**(Initialization: `init`)** Before regular operation, the manager is initialized with the latest known state (committed block number, block mappings) to ensure consistency, especially after restarts.

1.  **Ordering (Consensus -> Manager):**
    * **Concept:** Consensus finalizes the order of transactions in a new block.
    * **Action:** Consensus calls `set_ordered_blocks()`, providing the `ExternalBlock` data.
    * **State Transition:** Block enters the `BlockState::Ordered` state in the `BlockStateMachine`.
    * **Effect:** The block is now available for the Execution Layer; waiting tasks are notified.

2.  **Retrieval for Execution (Manager -> Execution):**
    * **Concept:** Execution needs ordered blocks to process.
    * **Action:** Execution calls `get_ordered_blocks()`. The manager returns available `Ordered` blocks (up to `max_size`).
    * **Mechanism:** If no blocks are ready, the call waits *asynchronously* (using `wait_for_change`), respecting timeouts defined in `BlockBufferManagerConfig`.

3.  **Execution & Result Reporting (Execution -> Manager):**
    * **Concept:** Execution processes the block and determines its outcome (e.g., state changes resulting in a block hash).
    * **Action:** Execution calls `set_compute_res()` with the `BlockId`, block number, and the resulting hash (`ComputeRes`).
    * **State Transition:** Block moves from `Ordered` -> `BlockState::Computed`.
    * **Effect:** The execution result is now stored; waiting Consensus tasks are notified.

4.  **Result Retrieval for Consensus (Manager -> Consensus):**
    * **Concept:** Consensus needs the execution outcome to proceed (e.g., for voting or final confirmation).
    * **Action:** Consensus calls `get_executed_res()` for a specific `BlockId`.
    * **Mechanism:** If the block is still `Ordered`, the call waits *asynchronously*. If `Computed`, the `ComputeRes` is returned.

5.  **Commitment (Consensus -> Manager):**
    * **Concept:** Consensus reaches final agreement that the block is part of the canonical chain.
    * **Action:** Consensus calls `set_commit_blocks()`, providing the `BlockIdNumHash` details for committed blocks.
    * **State Transition:** Block moves from `Computed` -> `BlockState::Commited`.
    * **Effect:** The block is marked as ready for final persistence; waiting Execution tasks are notified.

6.  **Retrieval for Finalization (Manager -> Execution):**
    * **Concept:** Execution needs to know which blocks are irrevocably committed to persist them permanently.
    * **Action:** Execution calls `get_commited_blocks()`.
    * **Mechanism:** Returns available `Commited` blocks, potentially waiting asynchronously.

7.  **Cleanup (Execution -> Manager):**
    * **Concept:** Execution confirms it has durably stored committed blocks, allowing the manager to reclaim resources.
    * **Action:** Execution calls `remove_commited_blocks()` with the latest *persisted* block number.
    * **Effect:** The `BlockStateMachine` removes entries for blocks at or below this number that are in the `Commited` state.

## 4. API Reference (Conceptual Grouping)

*(Provides the interface for layers to interact with the manager. Assumes shared access, e.g., via `Arc<BlockBufferManager>`)*

**Initialization & State**

* `new(config)`: Creates an uninitialized manager.
* `init(latest_commit_num, num_to_id_map)`: **Crucial first step.** Sets initial state, marks buffer as `Ready`. (Caller: Node init logic).
* `is_ready()`: Checks if `init` has been called. (Caller: Any).

**Transaction Flow**

* `push_txn(txn)`, `push_txns(txns)`: Adds transaction(s) to the buffer. (Caller: Execution/Mempool).
* `pop_txns(max_size)`: Retrieves transactions for block building. (Caller: Consensus).
* `recv_unbroadcasted_txn()`: *(Currently Unimplemented)*.

**Block Processing Flow**

* `set_ordered_blocks(parent_id, block)`: Consensus inputs an ordered block -> `Ordered` state.
* `get_ordered_blocks(start_num, max_size)`: Execution retrieves blocks for processing (waits asynchronously).
* `set_compute_res(block_id, hash, num)`: Execution inputs execution result -> `Computed` state.
* `get_executed_res(block_id, num)`: Consensus retrieves execution result (waits asynchronously).

**Block Commitment & Finalization Flow**

* `set_commit_blocks(block_ids)`: Consensus confirms block commitment -> `Commited` state.
* `get_commited_blocks(start_num, max_size)`: Execution retrieves blocks for persistence (waits asynchronously).
* `remove_commited_blocks(latest_persist_num)`: Execution confirms persistence, allowing cleanup.

**State Query (Primarily for Initialization/Recovery)**

* `latest_commit_block_number()`: Returns initial commit number from `init`.
* `block_number_to_block_id()`: Returns initial block map from `init`.

## 5. Configuration (`BlockBufferManagerConfig`)

Tunable parameters controlling asynchronous wait behavior:

* `wait_for_change_timeout` (Default: 100ms): Brief pause duration while waiting for internal notifications.
* `max_wait_timeout` (Default: 5s): Maximum time spent waiting in getter methods before returning a timeout error.

## 6. Usage Pattern Example (Simplified Pseudo-code)

Illustrates how Execution Layer tasks interact with the `BlockBufferManager` according to the defined lifecycle.

```pseudo
// Assume 'manager' is a shared reference to an initialized BlockBufferManager.

function mempool_forwarder_task(manager, txn_source) { // Lifecycle Step: Transaction Intake
  loop {
    raw_txn = txn_source.get_next_txn();
    verified_txn = prepare_verified_txn(raw_txn);
    manager.push_txn(verified_txn); // Push for Consensus
  }
}

function block_executor_task(manager) {
  last_processed = get_initial_block_num();
  loop {
    // Lifecycle Step 2: Retrieval for Execution
    blocks_batch = manager.get_ordered_blocks(last_processed + 1, BATCH_SIZE); // Waits if needed

    if (!blocks_batch.is_empty()) {
      for (block, parent_id) in blocks_batch {
        // Lifecycle Step 3: Execution & Result Reporting
        result_hash = internal_execute_block(block); // Execute
        manager.set_compute_res(block.id, result_hash, block.number); // Report result
        last_processed = block.number;
      }
    } else { sleep(short_pause); } // Pause if timed out
  }
}

function block_persister_task(manager) {
  last_persisted = get_last_persisted_num();
  loop {
    // Lifecycle Step 6: Retrieval for Finalization
    committed_batch = manager.get_commited_blocks(last_persisted + 1, BATCH_SIZE); // Waits if needed

    if (!committed_batch.is_empty()) {
      for block_info in committed_batch {
        internal_persist_block(block_info); // Persist to DB/Disk
        last_persisted = block_info.num;
      }
      // Lifecycle Step 7: Cleanup
      manager.remove_commited_blocks(last_persisted); // Notify manager
    } else { sleep(longer_pause); } // Pause if timed out
  }
}

// --- Initialization ---
// manager = BlockBufferManager::new(...);
// manager.init(...); // MUST be called first
// // Spawn tasks using an async runtime
// spawn(mempool_forwarder_task(manager, ...));
// spawn(block_executor_task(manager));
// spawn(block_persister_task(manager));
```
