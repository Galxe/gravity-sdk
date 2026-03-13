# attack_reth_coordinator

- **Agent**: intention_attacker
- **Status**: success
- **Duration**: 60203ms
- **Steps**: 1

## Report

# Intention Attack Report: `RethCoordinator` Security Audit

## Critical Findings

### 1. **CRITICAL — Silent Task Death Causes Undetected Pipeline Split-Brain**

**Location:** `reth_coordinator/mod.rs:40-53`

```rust
pub async fn run(&self) {
    let reth_cli = self.reth_cli.clone();
    tokio::spawn(async move {
        reth_cli.start_execution().await.unwrap();
    });
    // ... two more identical spawns
}
```

**The Vulnerability:** The three spawned tasks (`start_execution`, `start_commit_vote`, `start_commit`) form a **sequential pipeline**: blocks are ordered → executed/voted → committed. If any single task panics or returns `Err` (triggering `.unwrap()`), the `JoinHandle` is **discarded** — no one observes the failure. The remaining two tasks continue running indefinitely.

**Concrete attack scenario — State Corruption:**
- `start_commit_vote` panics (e.g., via the consecutive error path returning `Err` at line 402–404).
- `start_execution` keeps pushing ordered blocks into the execution pipeline — blocks pile up, consuming unbounded memory.
- `start_commit` starves — it waits for committed blocks that will never arrive because votes are never cast.
- The node appears alive (main loop is waiting on shutdown signal at `main.rs:303`) but is **functionally dead** — a zombie node that consumes resources, accepts but never processes transactions, and may participate in consensus with stale state.

**Why this is fatal:** In a consensus network, a silently-dead node that remains "online" can cause liveness failures (missing votes) without triggering failover or alerting. The `main.rs:303` shutdown listener will **never fire** from this condition — the node hangs forever.

**Severity: CRITICAL**

---

### 2. **HIGH — Panic in `start_commit` Causes Silent Halt of Block Finalization**

**Location:** `reth_cli.rs:466-469`

```rust
let block_id = self.pipe_api.get_block_id(last_block.num).unwrap_or_else(|| {
    panic!("commit num {} not found block id", start_commit_num);
});
assert_eq!(ExternalBlockId::from_bytes(block_id.as_slice()), last_block.block_id);
```

**The Vulnerability:** Two hard `panic!` paths exist inside a detached `tokio::spawn` task. If block ID lookup fails (e.g., due to a race where the execution pipeline hasn't registered the block ID yet, or after a restart with partial state), the commit task panics **silently** inside the spawned task.

**Consequence:** Block finalization permanently stops. The execution task continues pushing blocks. The commit-vote task continues voting. But no blocks are ever committed to persistent storage. The node's on-disk state freezes while its in-memory state diverges — a **consistency split** that cannot self-heal. On restart, the node will attempt to replay from the last persisted block, but the `block_number_to_block_id` map (populated only at init) may now be stale or incomplete.

**Note on `start_commit_num` in the panic message:** The panic at line 467 prints `start_commit_num` but should print `last_block.num` — the wrong variable is logged, making post-mortem debugging misleading.

**Severity: HIGH**

---

### 3. **HIGH — Oneshot Channel Indefinite Hang Blocks Node Startup**

**Location:** `reth_coordinator/mod.rs:24-37`, `main.rs:299`

```rust
pub async fn send_execution_args(&self) {
    // ...
    let block_number_to_block_id = get_block_buffer_manager()
        .block_number_to_block_id()
        .await   // <-- blocks on Notify from init()
        // ...
    execution_args_tx.send(execution_args).unwrap();  // <-- panics if receiver dropped
}
```

**The Vulnerability:** `send_execution_args()` is called at `main.rs:299` **after** `ConsensusEngine::init()` (line 284-297). The method awaits `block_number_to_block_id()` which internally waits on a `Notify` from `BlockBufferManager::init()`. If `BlockBufferManager::init()` is never called (misconfiguration, consensus engine failure, or init ordering bug), the **entire node hangs permanently** at line 299 — `run()` is never called, no blocks are ever processed.

**There is no timeout.** There is no health check. There is no log indicating what the node is waiting for. The process will appear alive but produce zero work.

**Additionally:** The `.unwrap()` on `execution_args_tx.send()` (line 36) will panic if the reth execution layer has already dropped its receiver (e.g., due to an error during `new_pipe_exec_layer_api` initialization). Since the receiver is passed to `run_reth()` early at `main.rs:254`, a failure in the reth thread between receiver consumption and this send creates a **startup panic with no recovery path**.

**Severity: HIGH**

---

### 4. **HIGH — `shutdown` Receiver Cannot Be Shared Across Tasks (UB via `resubscribe`)**

**Location:** `reth_cli.rs:83` — `shutdown: broadcast::Receiver<()>` (not wrapped in any sync primitive for multi-task use), used in `start_execution:323`, `start_commit_vote:378`, `start_commit:449`.

```rust
pub struct RethCli<EthApi: RethEthCall> {
    // ...
    shutdown: broadcast::Receiver<()>,  // single receiver
}
```

**The Vulnerability:** `RethCli` holds a **single** `broadcast::Receiver`. All three tasks (spawned concurrently via `run()`) call `self.shutdown.resubscribe()` on each loop iteration. `broadcast::Receiver::resubscribe()` creates a new receiver from the **sender's current tail** — meaning any shutdown signal sent *before* a task calls `resubscribe()` on a given loop iteration is **silently dropped** for that task.

**Race condition:** If a shutdown signal arrives between the time a task finishes processing a block and calls `resubscribe()` at the top of the next loop iteration, that task **misses the shutdown signal entirely** and continues running indefinitely. The `tokio::select!` on the resubscribed receiver will never fire because the message was already sent.

**Severity: HIGH**

---

### 5. **MEDIUM — `block_number_to_block_id` Map is a Stale Snapshot with No Post-Init Updates**

**Location:** `reth_coordinator/mod.rs:28-33`

**The Vulnerability:** The `block_number_to_block_id` map sent to the execution layer is populated **only once during `init()`** from persisted storage. After initialization, new blocks flowing through the pipeline register their block IDs through a different mechanism (`pipe_api.get_block_id()`). 

If a node restarts after processing blocks that were not yet persisted to the `block_number_to_block_id_with_epoch` storage, those block IDs are **lost**. The execution layer receives an incomplete map, and the `start_commit` task's `get_block_id()` call at `reth_cli.rs:466` may fail — triggering the panic described in Finding #2.

This creates a **cascading failure**: incomplete persistence → restart → stale map → panic in commit task → silent halt.

**Severity: MEDIUM**

---

### 6. **MEDIUM — Incorrect Variable in Panic Diagnostic Message**

**Location:** `reth_cli.rs:466-468`

```rust
let block_id = self.pipe_api.get_block_id(last_block.num).unwrap_or_else(|| {
    panic!("commit num {} not found block id", start_commit_num);
});
```

The lookup is for `last_block.num` but the panic message prints `start_commit_num`. After processing multiple batches, `start_commit_num` will have advanced past `last_block.num`, so the error message points operators to the **wrong block number** during incident response. While not a logic flaw per se, this directly impairs the ability to diagnose the Critical/High issues above.

**Severity: MEDIUM**

---

## Summary

| # | Finding | Severity | Root Cause |
|---|---------|----------|------------|
| 1 | Silent task death → zombie node | **Critical** | Discarded `JoinHandle`, no supervision |
| 2 | Panic in commit task → permanent finalization halt | **High** | `panic!`/`assert_eq!` in detached task |
| 3 | Oneshot channel indefinite hang → startup deadlock | **High** | No timeout on `Notify` await, `.unwrap()` on send |
| 4 | Shutdown signal race → tasks ignore shutdown | **High** | `resubscribe()` on each iteration drops already-sent signals |
| 5 | Stale `block_number_to_block_id` after restart | **Medium** | One-time snapshot with no post-init sync |
| 6 | Wrong variable in panic message | **Medium** | `start_commit_num` vs `last_block.num` |

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Intention Attack Report: `RethCoordinator` Security Audit
 | 60203ms |
