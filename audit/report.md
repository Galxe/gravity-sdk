# Code Review Report

## Overview

- **Tasks analyzed**: 25
- **Files explored**: 0

## Execution Summary

- ✅ **implement_block_buffer_manager_state_machine** (implementation_analyzer) — 100212ms, 1 steps
- ✅ **implement_node_entrypoint_and_lifecycle** (implementation_analyzer) — 105637ms, 1 steps
- ✅ **implement_mempool_transaction_pool** (implementation_analyzer) — 121679ms, 1 steps
- ✅ **implement_consensus_engine_initialization** (implementation_analyzer) — 97467ms, 1 steps
- ✅ **implement_reth_coordinator** (implementation_analyzer) — 123950ms, 1 steps
- ✅ **implement_reth_cli_block_execution** (implementation_analyzer) — 236490ms, 1 steps
- ✅ **implement_relayer_oracle_integration** (implementation_analyzer) — 228133ms, 1 steps
- ✅ **implement_https_api_security** (implementation_analyzer) — 154240ms, 1 steps
- ✅ **implement_mock_consensus** (implementation_analyzer) — 140493ms, 1 steps
- ✅ **implement_bootstrap_and_network** (implementation_analyzer) — 159024ms, 1 steps
- ✅ **implement_sentinel_monitoring** (implementation_analyzer) — 111866ms, 1 steps
- ✅ **implement_shared_crates_and_dependencies** (implementation_analyzer) — 119682ms, 1 steps
- ✅ **implement_gravity_cli_operations** (implementation_analyzer) — 195209ms, 1 steps
- ✅ **implement_concurrency_and_async_safety** (implementation_analyzer) — 208270ms, 1 steps
- ✅ **implement_error_handling_and_panic_safety** (implementation_analyzer) — 925318ms, 1 steps
- ✅ **implement_aptos_consensus_types_and_voting** (implementation_analyzer) — 124372ms, 1 steps
- ✅ **implement_aptos_executor_state_transitions** (implementation_analyzer) — 164890ms, 1 steps
- ✅ **implement_genesis_tool_key_material** (implementation_analyzer) — 202394ms, 1 steps
- ✅ **implement_aptos_consensus_safety_rules** (implementation_analyzer) — 244945ms, 1 steps
- ✅ **implement_txn_metrics_and_bench_binary** (implementation_analyzer) — 168173ms, 1 steps
- ✅ **implement_consensus_network_and_message_auth** (implementation_analyzer) — 238661ms, 1 steps
- ✅ **implement_qc_aggregator_and_vote_validation** (implementation_analyzer) — 280420ms, 1 steps
- ✅ **implement_liveness_and_leader_election** (implementation_analyzer) — 226633ms, 1 steps
- ✅ **implement_gravity_state_computer_and_epoch_manager** (implementation_analyzer) — 513896ms, 1 steps
- ✅ **implement_randomness_dkg_subsystem** (implementation_analyzer) — 547721ms, 1 steps
- ✅ **attack_node_entrypoint_and_lifecycle** (intention_attacker) — 71845ms, 1 steps
- ✅ **review_node_entrypoint_and_lifecycle** (ground_reviewer) — 74285ms, 1 steps
- ✅ **review_reth_cli_block_execution** (ground_reviewer) — 77511ms, 1 steps
- ✅ **attack_reth_cli_block_execution** (intention_attacker) — 77629ms, 1 steps
- ✅ **review_mempool_transaction_pool** (ground_reviewer) — 57435ms, 1 steps
- ✅ **review_block_buffer_manager_state_machine** (ground_reviewer) — 64856ms, 1 steps
- ✅ **attack_mempool_transaction_pool** (intention_attacker) — 73432ms, 1 steps
- ✅ **attack_block_buffer_manager_state_machine** (intention_attacker) — 71034ms, 1 steps
- ✅ **review_reth_coordinator** (ground_reviewer) — 51916ms, 1 steps
- ✅ **attack_reth_coordinator** (intention_attacker) — 60203ms, 1 steps
- ✅ **review_consensus_engine_initialization** (ground_reviewer) — 67313ms, 1 steps
- ✅ **review_relayer_oracle_integration** (ground_reviewer) — 65219ms, 1 steps
- ✅ **attack_consensus_engine_initialization** (intention_attacker) — 101382ms, 1 steps
- ✅ **attack_relayer_oracle_integration** (intention_attacker) — 69900ms, 1 steps
- ✅ **review_https_api_security** (ground_reviewer) — 78657ms, 1 steps
- ✅ **review_mock_consensus** (ground_reviewer) — 54820ms, 1 steps
- ✅ **attack_https_api_security** (intention_attacker) — 80958ms, 1 steps
- ✅ **review_bootstrap_and_network** (ground_reviewer) — 64445ms, 1 steps
- ✅ **attack_bootstrap_and_network** (intention_attacker) — 76420ms, 1 steps
- ✅ **attack_mock_consensus** (intention_attacker) — 141954ms, 1 steps
- ✅ **review_sentinel_monitoring** (ground_reviewer) — 159126ms, 1 steps
- ✅ **review_shared_crates_and_dependencies** (ground_reviewer) — 55441ms, 1 steps
- ✅ **attack_sentinel_monitoring** (intention_attacker) — 184690ms, 1 steps
- ✅ **attack_shared_crates_and_dependencies** (intention_attacker) — 137086ms, 1 steps
- ✅ **review_concurrency_and_async_safety** (ground_reviewer) — 204997ms, 1 steps
- ✅ **review_error_handling_and_panic_safety** (ground_reviewer) — 75786ms, 1 steps
- ✅ **attack_concurrency_and_async_safety** (intention_attacker) — 271673ms, 1 steps
- ✅ **attack_error_handling_and_panic_safety** (intention_attacker) — 224812ms, 1 steps
- ✅ **review_aptos_consensus_safety_rules** (ground_reviewer) — 154517ms, 1 steps
- ✅ **attack_aptos_consensus_safety_rules** (intention_attacker) — 116226ms, 1 steps
- ✅ **review_gravity_cli_operations** (ground_reviewer) — 872242ms, 1 steps
- ✅ **attack_gravity_cli_operations** (intention_attacker) — 910113ms, 1 steps
- ✅ **review_genesis_tool_key_material** (ground_reviewer) — 112435ms, 1 steps
- ✅ **review_aptos_executor_state_transitions** (ground_reviewer) — 55737ms, 1 steps
- ✅ **attack_genesis_tool_key_material** (intention_attacker) — 152510ms, 1 steps
- ✅ **review_txn_metrics_and_bench_binary** (ground_reviewer) — 62765ms, 1 steps
- ✅ **attack_txn_metrics_and_bench_binary** (intention_attacker) — 78714ms, 1 steps
- ✅ **attack_aptos_executor_state_transitions** (intention_attacker) — 243015ms, 1 steps
- ✅ **review_gravity_state_computer_and_epoch_manager** (ground_reviewer) — 78940ms, 1 steps
- ✅ **review_aptos_consensus_types_and_voting** (ground_reviewer) — 598978ms, 1 steps
- ✅ **attack_gravity_state_computer_and_epoch_manager** (intention_attacker) — 114508ms, 1 steps
- ✅ **review_consensus_network_and_message_auth** (ground_reviewer) — 72097ms, 1 steps
- ✅ **attack_qc_aggregator_and_vote_validation** (intention_attacker) — 156866ms, 1 steps
- ✅ **review_qc_aggregator_and_vote_validation** (ground_reviewer) — 170945ms, 1 steps
- ✅ **review_randomness_dkg_subsystem** (ground_reviewer) — 48052ms, 1 steps
- ✅ **attack_consensus_network_and_message_auth** (intention_attacker) — 60459ms, 1 steps
- ✅ **attack_aptos_consensus_types_and_voting** (intention_attacker) — 898637ms, 1 steps
- ✅ **review_liveness_and_leader_election** (ground_reviewer) — 161617ms, 1 steps
- ✅ **attack_liveness_and_leader_election** (intention_attacker) — 289944ms, 1 steps
- ✅ **attack_randomness_dkg_subsystem** (intention_attacker) — 490229ms, 1 steps

**Total**: 75 tasks, 14434146ms

---

## [node_entrypoint_and_lifecycle] Review the gravity_node main entrypoint (bin/gravity_node/src/main.rs) and CLI (

### 🎯 Intention

Review the gravity_node main entrypoint (bin/gravity_node/src/main.rs) and CLI (bin/gravity_node/src/cli.rs) for secure initialization, proper shutdown handling, signal handling race conditions, environment variable usage (RUST_BACKTRACE, ENABLE_PPROF, MOCK_CONSENSUS), and the unsafe use of std::env::set_var. Verify that the pprof profiler (setup_pprof_profiler) does not leak file handles or write to attacker-controllable paths. Audit the Ctrl+C/SIGTERM handler for graceful shutdown correctness and broadcast channel reliability.

### 🔍 Discovered Implementation

# Implementation Analysis: `gravity_node` Main Entrypoint & CLI

## Files Involved

| File | Description |
|------|-------------|
| `bin/gravity_node/src/main.rs` | Main entrypoint: initializes profiling, CLI, reth node, consensus, coordinator, and shutdown handler |
| `bin/gravity_node/src/cli.rs` | CLI definition via `clap::Parser`; dispatches subcommands to reth runner |
| `bin/gravity_node/src/reth_cli.rs` | `RethCli` struct: bridges consensus ↔ reth execution, handles block ordering/commit/vote loops |
| `bin/gravity_node/src/reth_coordinator/mod.rs` | `RethCoordinator`: spawns execution, commit-vote, and commit loops on `RethCli` |
| `bin/gravity_node/src/mempool.rs` | `Mempool`: wraps reth transaction pool with TTL-cached best-txns iterator and nonce ordering |
| `bin/gravity_node/src/relayer.rs` | `RelayerWrapper`: oracle data relayer with URI-based config, polling, and progress tracking |

---

## Execution Path (step-by-step)

### 1. `main()` (line 224)

1. **Pprof check**: reads `ENABLE_PPROF` env var. If set (any value), calls `setup_pprof_profiler()`.
2. **CLI parse**: `Cli::parse()` via clap. Extracts `relayer_config_path` and calls `check_bootstrap_config` with the node config path.
3. **Broadcast channel**: creates `broadcast::channel(1)` for shutdown signaling.
4. **Signal handler thread** (line 235): spawns a **new OS thread** with a dedicated single-threaded tokio runtime. Inside:
   - Registers `SIGTERM` via `tokio::signal::unix::signal(SignalKind::terminate())`.
   - Uses `tokio::select!` to await **either** `ctrl_c()` or `sigterm.recv()`.
   - On either signal, sends `()` on the cloned `shutdown_tx_clone` broadcast sender.
5. **Reth node launch**: calls `run_reth(cli, execution_args_rx, shutdown_tx.subscribe())` which:
   - Installs a sigsegv handler (`reth_cli_util::sigsegv_handler::install()`).
   - Conditionally sets `RUST_BACKTRACE=1` via `std::env::set_var` if not already set (line 72–74).
   - Creates a `std::sync::mpsc::sync_channel(1)` for passing `ConsensusArgs` back.
   - Spawns a **new OS thread** that runs `cli.run(...)` which boots the full reth Ethereum node.
   - Inside the reth launch closure: builds the node, extracts provider/pool/engine handles, sends `ConsensusArgs` and `block_number` back via the sync channel.
   - The spawned thread blocks on `tokio::select!` between `node_exit_future` and `shutdown.recv()`.
   - The **calling thread** blocks on `rx.recv().unwrap()` waiting for `ConsensusArgs`.
   - Returns `(ConsensusArgs, block_number, datadir_rx)`.
6. **Tokio runtime**: creates a new multi-threaded `tokio::runtime::Runtime`.
7. **Mempool**: creates `Mempool::new(pool, is_fullnode)`, extracts `tx_cache`.
8. **RethCli**: constructs `RethCli::new(consensus_args, txn_cache, shutdown_rx)`.
9. **Coordinator**: creates `RethCoordinator::new(client, latest_block_number, execution_args_tx)`.
10. **Consensus mode** (line 270):
    - If `MOCK_CONSENSUS` env var parses to `true`: creates `MockConsensus` and spawns `mock.run()`.
    - Otherwise: creates `RelayerWrapper`, sets it as `GLOBAL_RELAYER`, and initializes `ConsensusEngine`.
11. **Coordinator start**: calls `coordinator.send_execution_args()` then `coordinator.run()` which spawns three tasks: `start_execution`, `start_commit_vote`, `start_commit`.
12. **Shutdown wait** (line 302–304): subscribes to `shutdown_tx` and blocks on `shutdown_rx.recv()`.

---

## Key Functions

### `main.rs`

| Function | Signature | Description |
|----------|-----------|-------------|
| `main()` | `fn main()` | Entrypoint. Orchestrates profiling, CLI, reth, consensus, coordinator, shutdown. |
| `run_reth()` | `fn run_reth(cli, execution_args_rx, shutdown) -> (ConsensusArgs, u64, Receiver<PathBuf>)` | Spawns reth node on a dedicated OS thread, blocks calling thread until node is ready, returns consensus handles and latest block number. |
| `setup_pprof_profiler()` | `fn setup_pprof_profiler() -> Arc<Mutex<ProfilingState>>` | Spawns a background OS thread that cycles profiling sessions (3-min capture, 5-sec pause) for 30 minutes total. Writes protobuf files to CWD. |

### `cli.rs`

| Function | Signature | Description |
|----------|-----------|-------------|
| `Cli::run()` | `pub(crate) fn run(self, launcher) -> eyre::Result<()>` | Initializes tracing, creates `CliRunner`, dispatches subcommand (Node, Init, InitState, DumpGenesis, Db, P2P, Config, Prune). |
| `Cli::init_tracing()` | `pub fn init_tracing(&self) -> eyre::Result<Option<FileWorkerGuard>>` | Initializes tracing/logging. Returns guard that must be held alive. |
| `short_version()` / `long_version()` | `fn -> &'static str` | Lazily initialize build info from compile-time macros. |

### `reth_cli.rs`

| Function | Signature | Description |
|----------|-----------|-------------|
| `RethCli::new()` | `pub async fn new(args, txn_cache, shutdown) -> Self` | Extracts chain ID, initializes global crypto hasher, stores all handles. |
| `RethCli::start_execution()` | `pub async fn start_execution(&self) -> Result<(), String>` | Infinite loop: fetches ordered blocks from `block_buffer_manager`, pushes them to reth pipeline. Handles epoch changes. Exits on shutdown signal. |
| `RethCli::start_commit_vote()` | `pub async fn start_commit_vote(&self) -> Result<(), String>` | Infinite loop: receives execution results, converts to `TxnStatus`, sends compute results to `block_buffer_manager`. Tracks consecutive errors (max 5). Exits on shutdown. |
| `RethCli::start_commit()` | `pub async fn start_commit(&self) -> Result<(), String>` | Infinite loop: fetches committed blocks, sends committed block info to reth pipeline, waits for persistence. Exits on shutdown. |
| `RethCli::push_ordered_block()` | `pub async fn push_ordered_block(&self, block, parent_id) -> Result<(), String>` | Deserializes transactions (parallel via rayon for cache misses), resolves coinbase from proposer index, pushes `OrderedBlock` to pipe API. |
| `RethCli::get_coinbase_from_proposer_index()` | `fn(Option<u64>) -> Address` | Looks up proposer's reth address via `proposer_reth_map`. Falls back to `Address::ZERO` with warnings. |

### `reth_coordinator/mod.rs`

| Function | Signature | Description |
|----------|-----------|-------------|
| `RethCoordinator::run()` | `pub async fn run(&self)` | Spawns three independent tokio tasks: `start_execution`, `start_commit_vote`, `start_commit`. |
| `RethCoordinator::send_execution_args()` | `pub async fn send_execution_args(&self)` | Sends `ExecutionArgs` (block_number_to_block_id map) via oneshot channel to reth pipeline. Takes the sender so it can only fire once. |

---

## Environment Variables

| Variable | Read Location | Usage |
|----------|---------------|-------|
| `RUST_BACKTRACE` | `main.rs:72–74` | If not already set, force-set to `"1"` via `std::env::set_var`. |
| `ENABLE_PPROF` | `main.rs:226` | If set (any value), enables CPU profiling via `setup_pprof_profiler()`. |
| `MOCK_CONSENSUS` | `main.rs:270` | If parses to `true`, uses `MockConsensus` instead of real `ConsensusEngine`. |
| `MEMPOOL_CACHE_TTL_MS` | `mempool.rs:31–34` | Configures TTL for cached best-transactions iterator. Default: 1000ms. Read once via `OnceLock`. |

---

## `std::env::set_var` Usage (line 72–74)

```rust
if std::env::var_os("RUST_BACKTRACE").is_none() {
    std::env::set_var("RUST_BACKTRACE", "1");
}
```

- This is called inside `run_reth()`, which is invoked from `main()`.
- `std::env::set_var` is marked `unsafe` in Rust ≥1.66 (when using the 2024 edition) because it is not thread-safe — modifying environment variables while other threads read them is undefined behavior per POSIX.
- **At the point of invocation**: `setup_pprof_profiler()` may have already spawned a background thread (line 225–226 runs before `run_reth` at line 253–254). The signal handler thread is also already spawned (line 235). So `set_var` is called after at least 1–2 threads already exist.

---

## `setup_pprof_profiler()` Implementation (lines 161–222)

### Behavior
- Creates `ProfilingState { guard: None, profile_count: 0 }` wrapped in `Arc<Mutex>`.
- Spawns an OS thread that loops for **30 minutes**:
  1. Acquires lock, creates `ProfilerGuard::new(99)` (99 Hz sampling), stores in `state.guard`.
  2. Sleeps 3 minutes (profile duration).
  3. Acquires lock, takes the guard, generates report.
  4. Writes protobuf to `profile_{count}_proto_{formatted_time}.pb` in the **current working directory**.
  5. Sleeps 5 seconds, repeats.
- The `formatted_time` computation (line 194–199) extracts only the **millisecond** component of the current time (`time.millisecond()` formatted as `{:02}`), resulting in values `00`–`999`. This is used in the filename.
- File path construction (line 202): `format!("profile_{count}_proto_{formatted_time:?}.pb")` — note the `:?` debug format on `formatted_time` which is a `String`, so it produces `"profile_0_proto_\"042\".pb"` (with escaped quotes in the filename).

### File handle lifecycle
- `File::create(&proto_path)` opens a file. The `file` variable is bound inside a block scope within `if let Ok(mut file) = ...`. The `File` is dropped (and closed) at the end of that `if let` block (line 212). No file handles are held open across iterations.
- `ProfilerGuard` is taken via `.take()` and dropped after report generation within the same lock scope.

### Output path
- Files are written to **CWD** with a pattern-based name. The path is not user-configurable via environment variable or CLI argument — it is hardcoded relative to CWD.

---

## Shutdown Handler Implementation (lines 231–250)

### Architecture
```
broadcast::channel(1) → shutdown_tx
    ├── shutdown_tx.subscribe() → passed to run_reth()
    ├── shutdown_tx.subscribe() → shutdown_rx_cli (passed to RethCli)
    ├── shutdown_tx.subscribe() → shutdown_rx (main block_on await)
    └── shutdown_tx_clone.send(()) → signal handler thread
```

### Signal handler thread (lines 235–250)
- Spawns a **new OS thread** (not a tokio task).
- Creates its own single-threaded tokio runtime via `Builder::new_current_thread().enable_all().build()`.
- Registers `SIGTERM` listener **inside** this dedicated runtime.
- Uses `tokio::select!` between `ctrl_c()` and `sigterm.recv()`.
- On signal receipt, calls `shutdown_tx_clone.send(())`. The return value is discarded with `let _ =`.

### Broadcast channel capacity
- Channel capacity is **1** (`broadcast::channel(1)`).
- `broadcast::send(())` returns `Err` only if there are **zero** active receivers. Receivers are created via `.subscribe()` before the signal thread runs.
- Each `broadcast::Receiver` independently receives. If a receiver is lagging (hasn't consumed the previous message), it gets a `Lagged` error on next recv, but with capacity=1 and a single send, this is not an issue.

### Shutdown propagation in loops
- `start_execution`, `start_commit_vote`, and `start_commit` each call `self.shutdown.resubscribe()` on every loop iteration, creating a fresh receiver each time (line 323, 378, 449).
- `resubscribe()` creates a new receiver that will only see **future** messages. If the shutdown signal was sent before `resubscribe()` is called, that iteration's receiver **will not see it**. The loop would proceed to the next blocking call (`get_ordered_blocks`, `recv_compute_res`, or `get_committed_blocks`), which would block indefinitely.
- However, since the main `block_on` also awaits `shutdown_rx.recv()` at line 303, when the tokio runtime is dropped the spawned tasks are cancelled.

### `_shutdown_rx` (line 231)
- `let (shutdown_tx, _shutdown_rx) = broadcast::channel(1);` — the initial receiver `_shutdown_rx` is bound with an underscore prefix and **immediately dropped** at the end of `main()` scope (or earlier if the compiler determines it's unused). This is standard — broadcast receivers are created via `.subscribe()`.

---

## State Changes

| What | Where | Description |
|------|-------|-------------|
| Environment variables | `main.rs:73` | `RUST_BACKTRACE` set to `"1"` if unset |
| Filesystem (profile files) | `main.rs:203–210` | Protobuf profile files written to CWD when pprof enabled |
| Global relayer | `main.rs:278` | `GLOBAL_RELAYER` OnceLock set with `RelayerWrapper` instance |
| Global crypto hasher | `reth_cli.rs:108` | `GLOBAL_CRYPTO_TXN_HASHER` OnceLock initialized with keccak256 hasher |
| Transaction pool | `mempool.rs:224` | External transactions added to reth pool |
| Block buffer manager | `reth_cli.rs:433–436` | Compute results written via `set_compute_res` |
| Reth pipeline | `reth_cli.rs:260–274` | Ordered blocks pushed into execution pipeline |
| Reth pipeline | `reth_cli.rs:296` | Committed block info sent to pipeline |
| `TxnCache` (DashMap) | `mempool.rs:163`, `reth_cli.rs:198` | Tx hash → pool transaction cached on read, removed on block push |

---

## External Dependencies

| Dependency | Usage |
|------------|-------|
| `reth` / `greth` | Ethereum node framework (execution layer, transaction pool, provider, RPC) |
| `gaptos` / `api` | Gravity consensus types, config storage, relayer interface |
| `block_buffer_manager` | Global block ordering/commit buffer (accessed via `get_block_buffer_manager()` singleton) |
| `proposer_reth_map` | Maps validator index → reth address (via `get_reth_address_by_index`) |
| `pprof` | CPU profiling via sampling; generates protobuf reports |
| `tokio` | Async runtime, signal handling, broadcast/oneshot channels |
| `clap` | CLI argument parsing |
| `rayon` | Parallel transaction deserialization in `push_ordered_block` |
| `dashmap` | Concurrent hash map for `TxnCache` |
| `alloy_*` | Ethereum primitive types, EIP encoding/decoding |

### 🛠️ Ground Review

# Ground Review (Blue Team) — `gravity_node` Main Entrypoint & CLI

## Summary

Reviewed 6 files across the `gravity_node` binary crate: `main.rs`, `cli.rs`, `reth_cli.rs`, `reth_coordinator/mod.rs`, `mempool.rs`, and `relayer.rs`. Findings are organized by severity.

---

## Critical

### 1. `std::env::set_var` Called After Threads Are Spawned — Undefined Behavior
**File:** `main.rs:72–74`

```rust
if std::env::var_os("RUST_BACKTRACE").is_none() {
    std::env::set_var("RUST_BACKTRACE", "1");
}
```

`setup_pprof_profiler()` (line 225–226) spawns a background OS thread **before** `run_reth()` is called (line 253). The signal handler thread is also spawned (line 235). `std::env::set_var` is not thread-safe per POSIX — calling it with concurrent readers is **undefined behavior**. In Rust ≥1.66 / edition 2024 this is `unsafe` for exactly this reason. Even on older editions where the compiler doesn't enforce it, the UB is real.

**Recommendation:** Move the `set_var` call to the very first line of `main()`, before any threads are spawned, or use a process-wrapper/shell script to set the variable externally.

---

### 2. Shutdown Signal Race via `resubscribe()` in Hot Loops
**File:** `reth_cli.rs:323, 378, 449`

All three core loops (`start_execution`, `start_commit_vote`, `start_commit`) call `self.shutdown.resubscribe()` on **every iteration**, creating a fresh receiver. `resubscribe()` only receives **future** messages. If the shutdown signal is sent between the moment `resubscribe()` creates the new receiver and the moment `tokio::select!` begins awaiting it, the signal is **permanently lost** for that iteration. The loop then blocks indefinitely on the next `get_ordered_blocks` / `recv_compute_res` / `get_committed_blocks` call.

The system is partially saved because the tokio runtime drop (line 305, end of `main`) will cancel the spawned tasks — but this depends on an ungraceful cancellation rather than cooperative shutdown.

**Recommendation:** Create each `broadcast::Receiver` once at loop entry (outside the loop), and reuse it. Or switch to a `tokio::sync::watch` channel which always returns the latest value regardless of when the receiver was created.

---

### 3. Unwrap-on-Error Panics in Coordinator Task Spawns
**File:** `reth_coordinator/mod.rs:42–52`

```rust
tokio::spawn(async move {
    reth_cli.start_execution().await.unwrap();
});
```

All three spawned tasks (execution, commit_vote, commit) call `.unwrap()` on the `Result`. If any returns `Err`, the task panics silently (tokio swallows panics in spawned tasks by default). The other two tasks and the main loop continue running in a degraded, inconsistent state with no alerting.

**Recommendation:** Replace `.unwrap()` with explicit error handling that logs the error and triggers the shutdown broadcast, or use `tokio::spawn` with a `JoinHandle` that is awaited/monitored.

---

## Warning

### 4. Pprof Filename Contains Debug-Formatted Quotes
**File:** `main.rs:202`

```rust
let proto_path = format!("profile_{count}_proto_{formatted_time:?}.pb");
```

`formatted_time` is a `String`. Using `{:?}` (Debug format) wraps it in escaped quotes, producing filenames like `profile_0_proto_"042".pb`. This creates filenames containing literal quote characters, which can cause issues with shell tooling, log parsing, and some filesystems.

**Recommendation:** Use `{}` (Display) format instead of `{:?}`.

---

### 5. Profiler Writes to Uncontrolled CWD Path
**File:** `main.rs:203`

Profile files are written to the **current working directory** with no configurability. In containerized deployments CWD may be `/`, a read-only path, or a tmpfs that fills up. There is no protection against disk exhaustion (one file every ~3 minutes for 30 minutes = ~10 files).

**Recommendation:** Write profiles to a configurable directory (env var or CLI flag), defaulting to the node's data directory. Add a size/count guard.

---

### 6. `Mutex::lock().unwrap()` — Poison Propagation
**Files:** `main.rs:181, 188` (profiler), `mempool.rs:126` (cached_best)

`std::sync::Mutex::lock().unwrap()` will panic if the mutex is poisoned (i.e., a previous holder panicked). In the profiler, if `ProfilerGuard::new()` panics, subsequent lock acquisitions in the same thread loop will panic too. In the mempool, a panic during `best_txns` iteration will permanently poison the lock and crash all future `best_txns` calls.

**Recommendation:** Use `.lock().unwrap_or_else(|e| e.into_inner())` to recover from poisoned mutexes, or switch to `parking_lot::Mutex` which does not poison.

---

### 7. `rx.recv().unwrap()` — Opaque Panic on Reth Init Failure
**File:** `main.rs:157`

```rust
let (args, block_number) = rx.recv().unwrap();
```

If the reth node thread panics before sending on `tx`, the `sync_channel` sender is dropped and `rx.recv()` returns `Err(RecvError)`. The `.unwrap()` produces a generic panic with no context about what went wrong. The `_tx = tx.clone()` trick (line 80) only keeps the channel open until the thread exits — it doesn't prevent the panic path.

**Recommendation:** Replace with `.recv().expect("Reth node failed to initialize — check reth thread logs")` or propagate the error with context.

---

### 8. `oneshot::send().unwrap()` Without Guard
**File:** `reth_coordinator/mod.rs:36`

```rust
execution_args_tx.send(execution_args).unwrap();
```

If the `oneshot::Receiver` is dropped (e.g., reth thread exited early), this panics. Since `send_execution_args` is called from the main async block, this crashes the entire runtime.

**Recommendation:** Handle the `Err` case and trigger shutdown or log a fatal error.

---

### 9. Naming: `chian_info` Typo
**File:** `reth_cli.rs:103`

```rust
let chian_info = args.provider.chain_spec().chain;
```

Minor typo: `chian_info` → `chain_info`.

---

## Info

### 10. `_shutdown_rx` Immediately Dropped
**File:** `main.rs:231`

```rust
let (shutdown_tx, _shutdown_rx) = broadcast::channel(1);
```

The initial receiver `_shutdown_rx` is created and never used. This is idiomatic for `broadcast` (receivers are created via `.subscribe()`), but the underscore-prefixed name may confuse future readers. Consider `let (shutdown_tx, _) = ...` to make the intent explicit.

---

### 11. Profiler `formatted_time` Only Captures Milliseconds
**File:** `main.rs:194–199`

```rust
format!("{:02}", time.millisecond())
```

This only uses the millisecond component (0–999), not a full timestamp. Two profiles taken in different minutes but at the same millisecond offset will have name collisions (mitigated by the `count` prefix, but still confusing for debugging).

**Recommendation:** Use a full timestamp format (e.g., `%Y%m%d_%H%M%S`) for unambiguous file naming.

---

### 12. Duplicate `convert_account` Function
**Files:** `reth_cli.rs:86–90` and `mempool.rs:88–92`

Identical function defined in two places. Not a bug, but increases maintenance burden.

**Recommendation:** Extract to a shared utility module.

---

### 13. `MOCK_CONSENSUS` Env Var Parse Can Panic
**File:** `main.rs:270`

```rust
std::env::var("MOCK_CONSENSUS").unwrap_or("false".to_string()).parse::<bool>().unwrap()
```

While "false" always parses successfully, a user setting `MOCK_CONSENSUS=yes` or `MOCK_CONSENSUS=1` will cause a panic at startup. `bool::parse()` only accepts `"true"` and `"false"`.

**Recommendation:** Use `.parse::<bool>().unwrap_or(false)` or accept common truthy values like `"1"`, `"yes"`.

---

### 14. Mempool Creates a Separate Tokio Runtime
**File:** `mempool.rs:78`

```rust
runtime: tokio::runtime::Runtime::new().unwrap(),
```

The `Mempool` creates its own tokio runtime (in addition to the one created in `main.rs:255`). This is used only for `add_external_txn` spawns. Running two runtimes increases thread count and can cause subtle issues if tasks interact across runtime boundaries.

---

### 15. Relayer Config Logs RPC URLs at Info Level
**File:** `relayer.rs:97`

```rust
info!("relayer config: {:?}", config);
```

This logs the full `RelayerConfig` including all `uri_mappings` (RPC URLs). If any URL contains authentication tokens or API keys in query parameters, they will appear in logs.

**Recommendation:** Redact or omit URL query strings in log output.

---

## Summary Table

| # | Severity | Issue | Location |
|---|----------|-------|----------|
| 1 | **Critical** | `set_var` UB after thread spawn | `main.rs:72–74` |
| 2 | **Critical** | `resubscribe()` race loses shutdown signal | `reth_cli.rs:323,378,449` |
| 3 | **Critical** | Silent task panics in coordinator | `reth_coordinator/mod.rs:42–52` |
| 4 | Warning | Debug-format quotes in profile filename | `main.rs:202` |
| 5 | Warning | Profiles written to uncontrolled CWD | `main.rs:203` |
| 6 | Warning | Mutex poison panics | `main.rs:181,188`, `mempool.rs:126` |
| 7 | Warning | Opaque panic on reth init failure | `main.rs:157` |
| 8 | Warning | Unguarded `oneshot::send().unwrap()` | `reth_coordinator/mod.rs:36` |
| 9 | Warning | Typo: `chian_info` | `reth_cli.rs:103` |
| 10 | Info | Unused initial broadcast receiver | `main.rs:231` |
| 11 | Info | Incomplete timestamp in profile filename | `main.rs:194–199` |
| 12 | Info | Duplicate `convert_account` | `reth_cli.rs:86`, `mempool.rs:88` |
| 13 | Info | `MOCK_CONSENSUS` panics on non-bool values | `main.rs:270` |
| 14 | Info | Extra tokio runtime in Mempool | `mempool.rs:78` |
| 15 | Info | RPC URLs potentially logged with secrets | `relayer.rs:97` |

### ⚔️ Intention Attacker

# Intention Attack Report: `gravity_node` Security Audit

## Critical Findings

### 1. **CRITICAL — Shutdown Signal Loss via `resubscribe()` Creates Potential Infinite Hang**

**Files:** `reth_cli.rs:323`, `reth_cli.rs:378`, `reth_cli.rs:449`

All three core loops (`start_execution`, `start_commit_vote`, `start_commit`) call `self.shutdown.resubscribe()` **on every loop iteration**, creating a fresh receiver each time. A fresh receiver created via `resubscribe()` only sees **future** messages — it will not see a shutdown signal that was already sent.

**Attack scenario:** If the shutdown signal is sent during the window between the `resubscribe()` call and the `tokio::select!` branch registration, or while the loop is between iterations (e.g., processing a block), the new receiver **misses the signal entirely**. The loop then blocks indefinitely on `get_ordered_blocks()`, `recv_compute_res()`, or `get_committed_blocks()`.

The only backstop is that the tokio runtime is eventually dropped when `main()` exits (line 305), which cancels tasks. However, this relies on the main `shutdown_rx` at line 302-303 receiving the signal — creating a single point of failure. If for any reason the main `block_on` doesn't exit cleanly, the node hangs forever.

**Fix:** Clone the receiver once and reuse it, or use a `tokio::sync::watch` / `CancellationToken` instead of broadcast for shutdown signaling.

---

### 2. **CRITICAL — Spawned Task Panics Crash the Node Silently with No Recovery**

**File:** `reth_coordinator/mod.rs:42-52`

All three critical loops are spawned with `tokio::spawn` followed by `.unwrap()`:

```rust
tokio::spawn(async move {
    reth_cli.start_execution().await.unwrap();
});
```

If any of these tasks returns `Err(...)`, the `.unwrap()` panics **inside a spawned task**. Tokio's default behavior for spawned task panics is to silently abort that task (the JoinHandle is dropped, so the panic is swallowed). The other loops continue running, creating a **split-brain state**:

- If `start_execution` panics: blocks are ordered but never executed; the commit vote loop starves waiting for execution results.
- If `start_commit_vote` panics: blocks execute but compute results are never propagated to the block buffer manager; committed blocks stall.
- If `start_commit` panics: blocks execute and vote but are **never committed to persistent storage**. The node appears operational but **no state is being finalized**. On restart, all uncommitted blocks are lost.

**Severity amplifier:** There is no health check, watchdog, or cross-task liveness monitoring. The node can run in this degraded state indefinitely without any operator visibility.

---

### 3. **HIGH — `MOCK_CONSENSUS` Environment Variable Bypasses Real Consensus in Production**

**File:** `main.rs:270`

```rust
if std::env::var("MOCK_CONSENSUS").unwrap_or("false".to_string()).parse::<bool>().unwrap()
```

Setting `MOCK_CONSENSUS=true` replaces the entire consensus engine with `MockConsensus`. There is **no guard** against this being set in a production environment — no warning log, no config file cross-check, no compile-time feature gate.

**Attack scenario:** An attacker with access to the process environment (container escape, CI pipeline injection, shared hosting misconfiguration) sets `MOCK_CONSENSUS=true`. The node now runs with mock consensus, meaning it:
- Accepts blocks without real validator signatures
- Bypasses all BFT safety guarantees
- Potentially allows arbitrary block injection

This is a **single environment variable away from disabling all consensus safety**.

---

### 4. **HIGH — `std::env::set_var` After Thread Spawning = Undefined Behavior**

**File:** `main.rs:72-74`

```rust
if std::env::var_os("RUST_BACKTRACE").is_none() {
    std::env::set_var("RUST_BACKTRACE", "1");
}
```

At this point in execution, at least two threads have already been spawned:
1. The pprof profiler thread (line 225-226, runs before `run_reth` at line 253)
2. The signal handler thread (line 235)

`std::env::set_var` is **not thread-safe** per POSIX. Concurrent reads of the environment (which any thread can do implicitly, e.g., via libc locale lookups, DNS resolution, or tracing initialization) while `set_var` modifies it is **undefined behavior**. This is UB in Rust ≥1.66 editions and has caused real-world segfaults in production systems.

---

### 5. **HIGH — `Address::ZERO` Coinbase Fallback Silently Misdirects Block Rewards**

**File:** `reth_cli.rs:142-181`

When the proposer index is missing or not found in the map, the coinbase falls back to `Address::ZERO`:

```rust
return Address::ZERO;
```

This means block rewards (base fees, priority fees) are **sent to the zero address** — effectively burned. This happens silently with only a `warn!` log.

**Attack scenario:** If an attacker can influence the `proposer_reth_map` (e.g., by corrupting the epoch manager data or submitting a malformed validator set update), they can cause an entire epoch's worth of block rewards to be burned. Since the block still executes successfully, this corruption is **not caught by consensus** — the execution result hash includes the zero-address coinbase, and all honest nodes would agree on the same wrong coinbase.

**Impact:** Permanent, irrecoverable loss of block rewards with no on-chain indication of error.

---

### 6. **HIGH — TxnCache Unbounded Growth / Memory Exhaustion**

**Files:** `mempool.rs:163`, `reth_cli.rs:198`

The `TxnCache` (`DashMap<[u8; 32], Arc<ValidPoolTransaction>>`) grows on every call to `best_txns()` (line 163: `txn_cache.insert(tx_hash, pool_txn)`), but entries are only removed in `push_ordered_block` when a matching transaction is found in an ordered block (line 198: `self.txn_cache.remove(&key)`).

**Attack scenario:** If transactions enter the mempool cache via `best_txns()` but are never included in an ordered block (e.g., they expire, are replaced, or are filtered by consensus), their cache entries are **never evicted**. Over time, this causes unbounded memory growth. An attacker can flood the mempool with valid-but-low-priority transactions that are cached but never committed, eventually causing OOM.

There is no max-size limit, no TTL eviction, and no periodic cleanup on the `TxnCache` itself (the `CachedBest` TTL only controls the iterator, not the cache).

---

### 7. **MEDIUM — Profiler Writes to CWD with Debug-Formatted Filenames**

**File:** `main.rs:202`

```rust
let proto_path = format!("profile_{count}_proto_{formatted_time:?}.pb");
```

The `:?` (Debug) format on a `String` produces filenames with **escaped quotes**: `profile_0_proto_"042".pb`. This creates files with literal quote characters in the name, which:
- Can break shell scripts, monitoring tools, and log parsers
- May cause path traversal issues on certain filesystems
- Makes cleanup automation fragile

Additionally, `formatted_time` only captures milliseconds (`time.millisecond()`, range 0–999), not the full timestamp. Files from different profiling sessions at the same millisecond-of-second will **silently overwrite** each other, causing profile data loss.

---

### 8. **MEDIUM — Broadcast Channel Capacity of 1 with Fire-and-Forget Send**

**File:** `main.rs:231, 248`

```rust
let (shutdown_tx, _shutdown_rx) = broadcast::channel(1);
// ...
let _ = shutdown_tx_clone.send(());
```

The return value of `send()` is discarded with `let _ =`. If all receivers have been dropped (e.g., due to a panic in `run_reth` or the coordinator), the shutdown signal is silently lost. Combined with finding #1 (resubscribe races), this creates a scenario where a **second signal** (e.g., operator sends SIGTERM twice) cannot be delivered because the channel is already full and receivers have lagged.

---

### 9. **MEDIUM — `assert_eq!` in `start_commit` Panics the Commit Loop on Block ID Mismatch**

**File:** `reth_cli.rs:469`

```rust
assert_eq!(ExternalBlockId::from_bytes(block_id.as_slice()), last_block.block_id);
```

This assertion runs in the commit loop. If there is **any** block ID mismatch between the pipe API and the block buffer manager (e.g., due to a race condition during epoch change, or a corrupted block buffer), this `assert_eq!` panics, killing the commit task. Per finding #2, this panic is silently swallowed, and the node enters a state where blocks are executed but never committed.

A blockchain node should **never** use `assert_eq!` on data derived from external/consensus sources in a production loop. This should return an error that triggers a controlled shutdown or recovery.

---

## Summary

| # | Severity | Finding |
|---|----------|---------|
| 1 | **Critical** | Shutdown signal loss via `resubscribe()` — potential infinite hang |
| 2 | **Critical** | Spawned task panics silently swallowed — split-brain node state |
| 3 | **High** | `MOCK_CONSENSUS` env var disables all consensus safety with no guard |
| 4 | **High** | `std::env::set_var` after thread spawn — undefined behavior |
| 5 | **High** | `Address::ZERO` coinbase fallback burns block rewards irrecoverably |
| 6 | **High** | `TxnCache` unbounded growth — OOM via mempool flooding |
| 7 | **Medium** | Profiler writes with debug-formatted filenames + timestamp collision |
| 8 | **Medium** | Broadcast send result silently discarded — shutdown signal can be lost |
| 9 | **Medium** | `assert_eq!` in commit loop — consensus data mismatch kills commit silently |

---

## [reth_cli_block_execution] Security audit of bin/gravity_node/src/reth_cli.rs focusing on: (1) the push_ord

### 🎯 Intention

Security audit of bin/gravity_node/src/reth_cli.rs focusing on: (1) the push_ordered_block method for transaction deserialization safety, parallel processing with rayon, and the filtering of malformed transactions; (2) the start_execution loop for epoch transition handling, shutdown signal races, and error recovery; (3) the start_commit_vote loop for consecutive error tracking (GSDK-022) and potential stall scenarios; (4) the start_commit loop for block persistence correctness and panic conditions; (5) the txn_to_signed method for signer recovery and potential panic paths; (6) the get_coinbase_from_proposer_index for Address::ZERO fallback security implications.

### 🔍 Discovered Implementation

# Implementation Analysis: `bin/gravity_node/src/reth_cli.rs`

## Files/Contracts Involved

| File | Description |
|------|-------------|
| `bin/gravity_node/src/reth_cli.rs` | Core execution-layer bridge: deserializes transactions, feeds blocks to reth, receives execution results, commits finalized blocks |
| `bin/gravity_node/src/main.rs` | Entry point; constructs `RethCli`, wires shutdown broadcast, launches `RethCoordinator` |
| `bin/gravity_node/src/reth_coordinator/mod.rs` | Spawns three concurrent Tokio tasks calling `start_execution`, `start_commit_vote`, `start_commit` |
| `crates/block-buffer-manager/src/block_buffer_manager.rs` | Process-global `BlockBufferManager` singleton — state machine tracking blocks through Ordered → Computed → Committed lifecycle |
| `crates/proposer-reth-map/src/lib.rs` | Process-global `HashMap<u64, Vec<u8>>` mapping validator index → reth address, populated at epoch start |
| `greth` crate (`reth_pipe_exec_layer_ext_v2`) | `PipeExecLayerApi` — channel-based pipe connecting consensus-side block ordering to reth's execution engine |

---

## 1. `push_ordered_block` — Transaction Deserialization & Parallel Processing

### Signature
```rust
pub async fn push_ordered_block(&self, mut block: ExternalBlock, parent_id: B256) -> Result<(), String>
```

### Execution Path

**Step 1 — Cache lookup (sequential):**
- Pre-allocates `senders` and `transactions` as `Vec<Option<_>>` of length `block.txns.len()`.
- Iterates all transactions sequentially. For each, computes `txn.committed_hash()` and looks up the `DashMap<[u8;32], Arc<ValidPoolTransaction>>` cache.
- On cache hit: calls `.remove(&key)`, extracts the sender via `cached_txn.sender()` and clones the inner `TransactionSigned`. Populates `senders[idx]` and `transactions[idx]`.

**Step 2 — Parallel deserialization of cache misses (rayon):**
- Calls `block.txns.par_iter_mut()` using rayon's parallel iterator.
- Filters to only indices where `senders[idx].is_none()` (cache misses).
- For each, calls `Self::txn_to_signed(&mut txn.bytes, self.chain_id)`.
- On `Ok`: returns `Some((idx, sender, transaction))`.
- On `Err`: logs a warning with the index and error message, returns `None`.
- Collects results into `Vec<Option<...>>`, then flattens and assigns back into the `senders`/`transactions` vecs sequentially via `.for_each()`.

**Step 3 — Filtering malformed transactions:**
- Zips `senders` and `transactions` iterators. Only pairs where both are `Some` are kept.
- Missing pairs (failed decode or signer recovery) are logged with a warning and dropped.
- The block proceeds with only valid transactions — the `OrderedBlock` sent to reth may contain fewer transactions than the original `ExternalBlock`.

**Step 4 — Block assembly and push:**
- Extracts `randao`/`randomness` from `block.block_meta.randomness`, defaulting to zero.
- Calls `get_coinbase_from_proposer_index(block.block_meta.proposer_index)` for the coinbase address.
- Calls `pipe_api.push_ordered_block(OrderedBlock { ... })` synchronously (not async).
- Returns `Ok(())`.

### State Changes
- `self.txn_cache`: entries are **removed** via `DashMap::remove()` for every cache-hit transaction. Consumed entries are not re-inserted.
- `pipe_api`: an `OrderedBlock` is enqueued into reth's execution pipeline channel.

### Data Flow Observations
- The `filter` in step 2 reads `senders[*idx]` which was populated in step 1. Since step 1 is sequential and completes before the rayon block, there is no data race — the rayon threads only read indices where `senders[idx]` is `None`.
- The `.for_each()` after `.collect()` runs sequentially on the main thread (post-collect), so writes back to `senders[idx]`/`transactions[idx]` are also safe.
- The `par_iter_mut()` gives each rayon thread a `&mut` to a distinct `txn`, so `txn.bytes` mutation during `decode_2718` is safe.

---

## 2. `txn_to_signed` — Signer Recovery

### Signature
```rust
fn txn_to_signed(bytes: &mut [u8], _chain_id: u64) -> Result<(Address, TransactionSigned), String>
```

### Execution Path
1. Creates `slice = &bytes[..]` (immutable re-borrow of the mutable slice).
2. Calls `TransactionSigned::decode_2718(&mut slice)` — EIP-2718 typed transaction envelope decoding. On failure, returns `Err` with the decode error message.
3. Calls `txn.recover_signer()` — ECDSA signature recovery to derive the sender's public key → address. On failure, returns `Err`.
4. Returns `Ok((signer, txn))`.

### Observations
- `_chain_id` parameter is **unused** (prefixed with underscore). Chain ID validation, if any, happens inside reth's `decode_2718` or `recover_signer` logic, not here.
- Both failure modes return `Err(String)`, no panics. The caller (`push_ordered_block`) handles errors by logging and filtering.
- The `&mut [u8]` parameter allows `decode_2718` to advance the slice cursor, but the original bytes are not structurally mutated.

---

## 3. `get_coinbase_from_proposer_index` — Address Resolution

### Signature
```rust
fn get_coinbase_from_proposer_index(proposer_index: Option<u64>) -> Address
```

### Execution Path
1. If `proposer_index` is `None`: logs a warning with metric tag `coinbase_zero_address_fallback{reason=no_proposer_index}`, returns `Address::ZERO`.
2. Calls `get_reth_address_by_index(index)` — reads from the process-global `PROPOSER_RETH_ADDRESS_MAP` (populated by epoch manager at epoch start).
3. If `Some(bytes)` and `bytes.len() == 20`: constructs `Address::from_slice(&bytes)`.
4. If `Some(bytes)` but length ≠ 20: logs warning with `reason=invalid_address_length`, returns `Address::ZERO`.
5. If `None` (index not in map): logs warning with `reason=proposer_not_in_map`, returns `Address::ZERO`.

### State Changes
- None. Pure read from a global map.

### `Address::ZERO` Fallback Paths
Three distinct scenarios produce `Address::ZERO`:
1. No `proposer_index` in block metadata.
2. Proposer index exists but not found in the reth address map.
3. Address bytes have wrong length.

All three emit structured warning logs with metric-style tags for observability.

---

## 4. `start_execution` — Main Execution Loop

### Signature
```rust
pub async fn start_execution(&self) -> Result<(), String>
```

### Execution Path

**Initialization:**
1. Calls `self.provider.recover_block_number()` to get the last persisted block number. Sets `start_ordered_block = recovered + 1`.
2. Calls `get_block_buffer_manager().get_current_epoch()` to seed `self.current_epoch` via `AtomicU64::store(..., SeqCst)`.

**Main loop:**
3. Loads `current_epoch` from the atomic.
4. Resubscribes to `self.shutdown` broadcast channel (creates a fresh receiver each iteration to avoid lagged-receiver issues).
5. `tokio::select!` between:
   - `get_block_buffer_manager().get_ordered_blocks(start_ordered_block, None, current_epoch)` — blocks until a batch is ready (up to 5s internal timeout, then retries internally).
   - `shutdown.recv()` — breaks the loop, returns `Ok(())`.

**Error handling (epoch change):**
6. If `get_ordered_blocks` returns `Err`:
   - Checks if the error message contains `"Buffer is in epoch change"` **OR** if `current_epoch` differs from the buffer's current epoch.
   - If epoch change detected:
     - Calls `consume_epoch_change()` → returns the new epoch, transitions buffer state from `EpochChange` back to `Ready`.
     - Calls `latest_epoch_change_block_number()` → sets `start_ordered_block = value + 1`.
     - Atomically swaps `self.current_epoch` to the new epoch.
   - If not epoch-related: logs a warning.
   - In both cases: `continue` to next iteration.

**Success path:**
7. If `get_ordered_blocks` returns `Ok(vec)`:
   - If empty: logs info, continues.
   - Updates `start_ordered_block` to `last_block.block_meta.block_number + 1`.
   - Iterates each `(block, parent_id)`, converts `parent_id` to `B256`, calls `self.push_ordered_block(block, parent_id).await?`.

### State Changes
- `self.current_epoch`: written via `AtomicU64::store` at init, and `AtomicU64::swap` on epoch change.
- `pipe_api`: blocks pushed via `push_ordered_block`.
- `txn_cache`: entries consumed during `push_ordered_block`.

### Error Propagation
- `push_ordered_block` errors propagate up via `?` — any single malformed block (at the block level, not transaction level) terminates the loop with `Err`.
- `recover_block_number` failure is fatal (returns `Err` before loop starts).

---

## 5. `start_commit_vote` — Execution Result Processing Loop

### Signature
```rust
pub async fn start_commit_vote(&self) -> Result<(), String>
```

### Execution Path

**GSDK-022 consecutive error tracking:**
- Maintains `consecutive_errors: u32`, initialized to 0.
- Constant `MAX_CONSECUTIVE_ERRORS = 5`.

**Main loop:**
1. Resubscribes to shutdown channel.
2. `tokio::select!` between `self.recv_compute_res()` and `shutdown.recv()`.
3. On shutdown: breaks, returns `Ok(())`.

**On `recv_compute_res` success:**
4. Resets `consecutive_errors = 0`.
5. Copies `block_hash` into `[u8; 32]`.
6. Constructs `ExternalBlockId` from `execution_result.block_id`.
7. Maps `txs_info` into `Vec<TxnStatus>` — each with `txn_hash`, `sender` (converted via `convert_account`), `nonce`, `is_discarded`.
8. Wraps in `Arc<Some(Vec<TxnStatus>)>`.
9. Loads current epoch from `self.current_epoch` atomic.
10. Calls `get_block_buffer_manager().set_compute_res(block_id, block_hash, block_number, epoch, txn_status, events)`.
    - Inside `set_compute_res`: transitions the block from `Ordered` → `Computed`, detects `NewEpoch` events, sets `latest_epoch_change_block_number` and `next_epoch`.

**On `recv_compute_res` error:**
11. Increments `consecutive_errors`.
12. If `>= MAX_CONSECUTIVE_ERRORS` (5): logs error, returns `Err(...)` — this triggers a **panic** in the coordinator task (because `run()` calls `.unwrap()` on the result).
13. If below threshold: logs warning, sleeps 100ms, continues.

### State Changes
- `block_buffer_manager` block state: `Ordered` → `Computed`.
- `block_buffer_manager` epoch state: `next_epoch` and `latest_epoch_change_block_number` may be set if a `NewEpoch` event is present.

### Stall Scenarios
- If `recv_compute_res` returns `Ok` but `set_compute_res` returns `Err`, the error propagates via `?` and terminates the loop. The coordinator task panics.
- If `pull_executed_block_hash` blocks forever (reth stalls), this loop hangs indefinitely. The shutdown select arm is the only exit. No timeout is applied to the execution result pull itself.

---

## 6. `start_commit` — Block Commitment & Persistence Loop

### Signature
```rust
pub async fn start_commit(&self) -> Result<(), String>
```

### Execution Path

**Initialization:**
1. `start_commit_num = provider.recover_block_number() + 1`.

**Main loop:**
2. Loads `epoch` from `self.current_epoch`.
3. `tokio::select!` between `get_committed_blocks(start_commit_num, None, epoch)` and `shutdown.recv()`.
4. On `Err`: logs warning, continues (no epoch-change handling here, unlike `start_execution`).
5. On `Ok(empty)`: continues.

**On non-empty committed blocks:**
6. Gets `last_block` from the batch.
7. Calls `self.pipe_api.get_block_id(last_block.num)`:
   - If `None`: **panics** via `unwrap_or_else(|| panic!(...))`.
   - If `Some(block_id)`: asserts `ExternalBlockId::from_bytes(block_id) == last_block.block_id`. Mismatch **panics** via `assert_eq!`.
8. Updates `start_commit_num = last_block.num + 1`.
9. Iterates each `block_id_num_hash` in the batch:
   - Calls `self.send_committed_block_info(block_id, hash)` — errors propagate via `?`.
   - Collects `persist_notifier` if present (epoch-boundary blocks).
10. Calls `provider.recover_block_number()` to get `last_block_number`.
11. Calls `get_block_buffer_manager().set_state(start_commit_num - 1, last_block_number)` — updates watermarks.
12. For each `(block_number, persist_notifier)` pair:
    - Calls `self.wait_for_block_persistence(block_number).await` — blocks until reth confirms disk persistence.
    - Sends `()` on the `persist_notifier` channel. The `let _ =` discards send errors (receiver may have been dropped).

### Panic Conditions
Two explicit panic paths:
1. `pipe_api.get_block_id(last_block.num)` returns `None` — block number not found in reth's internal mapping.
2. `assert_eq!` fails — block ID from reth does not match block ID from buffer manager. This indicates a consistency violation between the two data paths.

### State Changes
- `pipe_api`: `commit_executed_block_hash` tells reth to canonicalize blocks.
- `block_buffer_manager`: `set_state` updates `latest_commit_block_number` and `latest_finalized_block_number`.
- `persist_notifier`: signals epoch-boundary persistence completion to consensus.

---

## Orchestration: `RethCoordinator::run()`

```rust
pub async fn run(&self) {
    let cli1 = self.reth_cli.clone();
    let cli2 = self.reth_cli.clone();
    let cli3 = self.reth_cli.clone();
    
    tokio::spawn(async move { cli1.start_execution().await.unwrap() });
    tokio::spawn(async move { cli2.start_commit_vote().await.unwrap() });
    tokio::spawn(async move { cli3.start_commit().await.unwrap() });
}
```

All three tasks share a single `Arc<RethCli>`. Each task's `.unwrap()` means any `Err` return causes that Tokio task to panic. The tasks do not monitor each other — if one panics, the other two continue running (until they themselves stall on a closed channel or the shutdown signal fires).

---

## External Dependencies Summary

| Dependency | Type | Usage |
|-----------|------|-------|
| `alloy_consensus::SignerRecoverable` | Trait | ECDSA signer recovery on `TransactionSigned` |
| `alloy_eips::Decodable2718` | Trait | EIP-2718 transaction envelope decoding |
| `rayon` | Crate | Parallel transaction deserialization for cache misses |
| `dashmap::DashMap` | Crate | Lock-free concurrent transaction cache |
| `block_buffer_manager` | Internal crate | Global block state machine (Ordered → Computed → Committed) |
| `proposer_reth_map` | Internal crate | Global validator-index → reth-address mapping |
| `greth::reth_pipe_exec_layer_ext_v2` | Internal crate | Channel pipe between consensus ordering and reth execution |
| `tokio::sync::broadcast` | Std | Shutdown signal propagation |
| `std::sync::atomic::AtomicU64` | Std | Lock-free epoch counter shared across three async tasks |

---

## Complete Data Flow Diagram

```
Consensus Layer
      │
      ▼
BlockBufferManager (Ordered blocks)
      │
      ▼ get_ordered_blocks()
start_execution ──── push_ordered_block() ──►  PipeExecLayerApi
      │                   │                         │
      │              [txn_cache read/remove]         │ (reth executes)
      │              [rayon parallel decode]          │
      │              [malformed txn filter]           ▼
      │                                        ExecutionResult
      │                                              │
      │                                    ◄──────────┘
      │                              pull_executed_block_hash()
      │                                              │
      ▼                                              ▼
  (epoch change) ◄──────────────────── start_commit_vote()
  consume_epoch_change()                     │
  reset start_ordered_block                  │ set_compute_res()
  swap current_epoch                         ▼
                                    BlockBufferManager (Computed)
                                             │
                                    (consensus commits)
                                             │
                                             ▼
                                    BlockBufferManager (Committed)
                                             │
                                   get_committed_blocks()
                                             │
                                             ▼
                                      start_commit()
                                             │
                              ┌──────────────┼──────────────┐
                              ▼              ▼              ▼
                      get_block_id()   commit_block()   set_state()
                      assert_eq!       to reth          watermarks
                                             │
                                             ▼
                                  wait_for_block_persistence()
                                             │
                                             ▼
                                    persist_notifier.send(())
                                             │
                                             ▼
                                    Consensus epoch transition
```

### 🛠️ Ground Review

# Ground Review (Blue Team) — Code Quality & Engineering Safety Audit

**Scope:** `bin/gravity_node/src/reth_cli.rs`, `reth_coordinator/mod.rs`, `main.rs`

---

## Critical

### C1. Unmonitored task panics — partial system failure without recovery
**File:** `reth_coordinator/mod.rs:42-52`

All three spawned tasks call `.unwrap()` on the `Result`. If any one task panics (e.g., `start_commit` hits its `assert_eq!` or `unwrap_or_else(|| panic!(...))`), the other two tasks continue running in a degraded, inconsistent state. There is no `JoinHandle` monitoring, no `tokio::select!` over the three futures, and no mechanism to propagate one task's failure to the others or to the shutdown broadcast.

```rust
tokio::spawn(async move { reth_cli.start_execution().await.unwrap() });
tokio::spawn(async move { reth_cli.start_commit_vote().await.unwrap() });
tokio::spawn(async move { reth_cli.start_commit().await.unwrap() });
```

**Impact:** A panic in the commit loop leaves the execution loop pushing blocks that will never be committed. A panic in the commit-vote loop causes execution results to back-pressure indefinitely. The node appears alive but is silently stalled — the worst kind of failure for a blockchain node.

**Recommendation:** Retain the `JoinHandle`s, `tokio::select!` over all three, and broadcast shutdown on the first failure. Alternatively, use `tokio::try_join!` or a task supervision pattern.

---

### C2. Explicit panic paths in `start_commit` — intentional crashes without graceful shutdown
**File:** `reth_cli.rs:466-469`

```rust
let block_id = self.pipe_api.get_block_id(last_block.num).unwrap_or_else(|| {
    panic!("commit num {} not found block id", start_commit_num);
});
assert_eq!(ExternalBlockId::from_bytes(block_id.as_slice()), last_block.block_id);
```

Two panic paths bypass the shutdown broadcast entirely. The process aborts the task without notifying other subsystems, leaving reth's execution engine and the consensus layer in an indeterminate state.

**Impact:** Unclean process termination mid-commit can corrupt block persistence state or leave dangling channel resources.

**Recommendation:** Return `Err(...)` instead of panicking. The coordinator should handle the error and trigger a clean shutdown.

---

### C3. `start_commit` silently swallows epoch-change errors
**File:** `reth_cli.rs:457-459`

```rust
if let Err(e) = block_ids {
    warn!("failed to get committed blocks: {}", e);
    continue;
}
```

Unlike `start_execution` (which detects and handles epoch changes), `start_commit` treats all errors — including `"Buffer is in epoch change"` — as transient warnings and retries with an unchanged `epoch` value. During an epoch transition, this loop will spin-warn indefinitely until `start_execution` happens to update `current_epoch` via `AtomicU64::swap`. There is a temporal race: if `start_commit`'s loop is faster, it will burn CPU logging warnings until the epoch atomic is updated by a different task.

**Impact:** Log flooding and wasted CPU during epoch transitions. If the epoch update in `start_execution` stalls or fails, `start_commit` enters an infinite warn loop.

**Recommendation:** Mirror the epoch-change detection logic from `start_execution`, or at minimum add a backoff/sleep on repeated errors.

---

## Warning

### W1. Broadcast receiver re-subscription on every loop iteration — missed shutdown signals
**File:** `reth_cli.rs:323, 378, 449`

```rust
let mut shutdown = self.shutdown.resubscribe();
```

Each loop iteration creates a fresh receiver. If a shutdown signal is sent while the loop is between iterations (e.g., during `push_ordered_block` processing), the signal is lost because the old receiver is dropped and the new one hasn't been created yet. The `resubscribe()` call only receives messages sent *after* it is created.

**Impact:** Shutdown may not be honored promptly. The loop must complete an entire `get_ordered_blocks` → `push_ordered_block` cycle before the next `select!` can observe shutdown. For `start_commit`, `wait_for_block_persistence` could block indefinitely with no shutdown check.

**Recommendation:** Create the receiver once outside the loop. Handle `RecvError::Lagged` instead of resubscribing.

---

### W2. `wait_for_block_persistence` has no timeout or shutdown check
**File:** `reth_cli.rs:301-307, 492-495`

```rust
pipe_api.wait_for_block_persistence(block_number).await;
```

This awaits indefinitely. If reth's storage engine stalls (disk full, I/O error, bug), the entire `start_commit` task hangs forever. There is no `tokio::select!` with the shutdown channel, no timeout, and no health check.

**Impact:** Node hangs on block persistence with no way to recover short of killing the process.

**Recommendation:** Wrap in `tokio::select!` with `shutdown.recv()` and/or add a timeout with configurable duration.

---

### W3. `recv_compute_res` can block indefinitely — stall propagation
**File:** `reth_cli.rs:278-286`

```rust
pipe_api.pull_executed_block_hash().await
    .ok_or_else(|| "failed to recv compute res: channel closed".to_string())?;
```

If reth's execution engine stalls (e.g., EVM execution loops on a pathological contract), this blocks forever. The `tokio::select!` in `start_commit_vote` covers the shutdown case, but the `recv_compute_res` future itself has no internal timeout. A stall here prevents the commit-vote loop from processing any subsequent blocks.

**Impact:** Single-block execution stall cascades to halt all block commitment.

---

### W4. Typo in variable name  
**File:** `reth_cli.rs:103`

```rust
let chian_info = args.provider.chain_spec().chain;
```

`chian_info` → `chain_info`. Minor readability issue but demonstrates a naming convention gap.

---

### W5. `calculate_txn_hash` uses `.unwrap()` on `try_into()`
**File:** `reth_cli.rs:93-95`

```rust
fn calculate_txn_hash(bytes: &Vec<u8>) -> [u8; 32] {
    alloy_primitives::utils::keccak256(bytes).as_slice().try_into().unwrap()
}

```

`keccak256` always returns 32 bytes so this is safe in practice, but the function is registered as a global callback (`GLOBAL_CRYPTO_TXN_HASHER`) — a panic here would crash any caller anywhere in the process. Also, `#[allow(clippy::ptr_arg)]` suppresses a valid lint; the parameter should be `&[u8]`.

---

### W6. `Address::ZERO` coinbase fallback has fee-burning side effects
**File:** `reth_cli.rs:142-181`

Three error paths silently fall back to `Address::ZERO` as the block coinbase. In EVM execution, priority fees and coinbase transfers go to this address. If a misconfiguration causes repeated fallbacks, fees accumulate at a burnable/unrecoverable address with no alerting beyond `warn!` logs.

**Impact:** Silent economic loss. Validators don't receive their block rewards.

**Recommendation:** Consider making this a hard error (return `Err`) for at least the `proposer_not_in_map` case, which likely indicates a real misconfiguration rather than a transient race.

---

### W7. `send_execution_args` panics on channel failure
**File:** `reth_coordinator/mod.rs:36`

```rust
execution_args_tx.send(execution_args).unwrap();
```

If the receiver has been dropped (e.g., reth node exited early), this panics.

---

### W8. Profiler writes to current working directory with predictable filenames
**File:** `main.rs:202-209`

```rust
let proto_path = format!("profile_{count}_proto_{formatted_time:?}.pb");
```

Profile files are written to CWD with predictable names. On a shared system, this could be a symlink-following attack vector. Also, `formatted_time` only captures milliseconds (not the full timestamp), so filenames may collide.

**Recommendation:** Write to a dedicated temp directory with restricted permissions. Use full timestamps.

---

## Info

### I1. Unused fields prefixed with `_`
**File:** `reth_cli.rs:74, 78, 81`

`_auth`, `_txn_listener`, `_txn_batch_size` are dead fields. They consume memory in every `Arc<RethCli>` clone (the `Mutex<Receiver>` is non-trivial). If they're kept for future use, document it; otherwise remove them.

---

### I2. `_chain_id` parameter unused in `txn_to_signed`
**File:** `reth_cli.rs:129`

The underscore-prefixed parameter suggests chain ID validation was planned but not implemented. If reth's `decode_2718` handles it internally, remove the parameter. If not, this is a missing validation.

---

### I3. Metric-style tags embedded in log strings
**File:** `reth_cli.rs:151, 165, 175`

```rust
"Metric: coinbase_zero_address_fallback{{reason=no_proposer_index}}"
```

Metrics embedded in log message strings are brittle and hard to query. These should use structured fields (e.g., tracing's `#[instrument]` or `info!(reason = "no_proposer_index", ...)`) or real metric counters.

---

### I4. `std::env::set_var` in multi-threaded context
**File:** `main.rs:73-74`

```rust
std::env::set_var("RUST_BACKTRACE", "1");
```

`set_var` is unsound in Rust ≥1.66 when called from a multi-threaded context (marked `unsafe` in nightly). At this call site the main thread has already spawned the profiler thread, creating a potential data race on the environment block.

---

### I5. `ENABLE_PPROF` env check uses `is_ok()` — any value enables profiling
**File:** `main.rs:226`

```rust
if std::env::var("ENABLE_PPROF").is_ok()
```

Setting `ENABLE_PPROF=false` or `ENABLE_PPROF=0` still enables profiling because `is_ok()` only checks existence. Use `.map(|v| v == "true" || v == "1")` or similar.

---

## Summary

| Severity | Count | Key Themes |
|----------|-------|-----------|
| **Critical** | 3 | Unmonitored task panics, explicit panic-instead-of-error in commit path, silent epoch-change mishandling in commit loop |
| **Warning** | 8 | Shutdown signal races, unbounded blocking waits, `Address::ZERO` fee burning, predictable profiler paths |
| **Info** | 5 | Dead code, missing structured metrics, `set_var` soundness, env-var parsing |

The most urgent items are **C1** (task supervision) and **C2** (panic-to-error conversion). Together they create a scenario where a single block consistency mismatch crashes one task and leaves the other two running in a zombified state with no external visibility.

### ⚔️ Intention Attacker

# Security Audit — Attack Findings: `reth_cli.rs` & Coordinator

## Critical Severity

### 1. Unsupervised Task Failure — Cascading Stall / Silent Half-Death
**File:** `reth_coordinator/mod.rs:40-53`

```rust
tokio::spawn(async move { reth_cli.start_execution().await.unwrap(); });
tokio::spawn(async move { reth_cli.start_commit_vote().await.unwrap(); });
tokio::spawn(async move { reth_cli.start_commit().await.unwrap(); });
```

The three critical pipeline tasks (`start_execution`, `start_commit_vote`, `start_commit`) are spawned independently with `.unwrap()`. If **one** task panics (which is by design — see `start_commit`'s `assert_eq!` and `unwrap_or_else(|| panic!())`), the other two tasks **continue running indefinitely** in a degraded state. There is no cross-task health monitoring, no `JoinHandle` collection, and no propagation of a single task's failure to the others.

**Attack scenario:** If `start_commit` panics due to a block ID mismatch (line 469), `start_execution` and `start_commit_vote` continue pushing and computing blocks that will **never be committed or persisted**. The node appears alive (accepts transactions, produces execution results) but is actually a black hole — blocks enter the pipeline and are never finalized. The `BlockBufferManager` fills up with `Computed` blocks that never transition to `Committed`, eventually exhausting memory.

**Impact:** Node liveness failure, potential memory exhaustion, silent chain stall without operator notification beyond a Tokio panic log line that is easy to miss.

---

### 2. `Address::ZERO` Coinbase — Block Reward / Fee Theft via Proposer Map Manipulation
**File:** `reth_cli.rs:142-181`

Three separate fallback paths silently set `coinbase = Address::ZERO`. When `Address::ZERO` is used as the coinbase:
- All transaction priority fees (tips) for that block are credited to `Address::ZERO`
- Any block reward goes to `Address::ZERO`
- Any actor who controls the `Address::ZERO` private key (or can craft a transaction from it) collects the fees

**Attack scenario:** An attacker who can influence the `PROPOSER_RETH_ADDRESS_MAP` (e.g., by injecting a validator with a malformed 19-byte or 21-byte reth address) causes all blocks proposed by that validator to use `Address::ZERO` as coinbase. The fees accumulate at a known address. The attack is particularly dangerous because:
1. The fallback is **silent at the consensus level** — the block is still produced and committed.
2. The warning log is easily drowned in production log volume.
3. There is no circuit breaker — this can happen for every block in an epoch if the map is corrupted.

**Impact:** Theft of transaction fees for affected blocks; economic attack on validators.

---

## High Severity

### 3. Epoch Desynchronization Between `start_commit` and `start_execution`
**File:** `reth_cli.rs:441-499` vs `reth_cli.rs:309-370`

`start_commit` reads `self.current_epoch` (line 448) but **does not handle epoch transitions**. When it receives an error from `get_committed_blocks`, it simply logs a warning and continues (line 458-459). Compare this with `start_execution` which has explicit epoch change handling (lines 333-343).

**Attack scenario:** During an epoch transition:
1. `start_execution` detects the epoch change, consumes it, and atomically swaps `current_epoch`.
2. `start_commit` was already mid-iteration with the **old** epoch value.
3. `get_committed_blocks(start_commit_num, None, old_epoch)` may return stale data or errors indefinitely.
4. Since there's no epoch-change recovery logic, `start_commit` enters an infinite retry loop of `warn! → continue` with no backoff, burning CPU.

**Impact:** Commit pipeline stall at epoch boundaries; blocks execute but never commit; potential infinite warn-log spam.

---

### 4. `start_commit` Block ID Assertion — Panic-Based DoS on Consistency Violation
**File:** `reth_cli.rs:466-469`

```rust
let block_id = self.pipe_api.get_block_id(last_block.num).unwrap_or_else(|| {
    panic!("commit num {} not found block id", start_commit_num);
});
assert_eq!(ExternalBlockId::from_bytes(block_id.as_slice()), last_block.block_id);
```

Two hard panics with no recovery path. While the `assert_eq!` is arguably correct (a consistency violation should be fatal), the **panic only kills the commit task** while execution and vote tasks continue (see Finding #1). This creates the worst possible failure mode: silent data divergence without a full node shutdown.

**Impact:** If any transient state inconsistency occurs (e.g., race between `pipe_api` internal state and `BlockBufferManager`), the commit loop dies permanently while the node continues operating in a zombie state.

---

### 5. Silent Transaction Dropping — Consensus/Execution Divergence
**File:** `reth_cli.rs:225-242`

Malformed transactions are silently filtered out with only a `warn!` log. The `OrderedBlock` sent to reth contains **fewer transactions** than the original `ExternalBlock` from consensus.

```rust
_ => {
    warn!("Filtering out transaction at index {} with missing sender or body", idx);
}
```

**Attack scenario:** If consensus has ordered a block with N transactions and expects execution results for N transactions, but reth only executes N-k (after filtering), the execution result from reth will contain a different transaction count. This creates a divergence between what consensus expects and what the execution layer produces.

If the `TxnStatus` vector returned in `start_commit_vote` (line 420-430) has a different length than consensus expects, downstream systems that index by position into the transaction list will be operating on misaligned data.

**Impact:** State divergence between consensus and execution layer; potential for incorrect transaction status reporting.

---

## Medium Severity

### 6. Unbounded `recv_compute_res` Blocking — No Execution Timeout
**File:** `reth_cli.rs:278-286`

```rust
pub async fn recv_compute_res(&self) -> Result<ExecutionResult, String> {
    let result = pipe_api.pull_executed_block_hash().await
        .ok_or_else(|| "failed to recv compute res: channel closed".to_string())?;
```

`pull_executed_block_hash()` has **no timeout**. If reth's execution engine hangs (e.g., on a pathological EVM transaction, infinite loop in a smart contract, or disk I/O stall), the `start_commit_vote` loop blocks forever. The shutdown `select!` arm cannot fire because it was already resolved in the previous iteration — a new `resubscribe()` happens **before** the blocking call, but the `tokio::select!` correctly covers this. However, the inner `pull_executed_block_hash().await` inside `recv_compute_res` is **not** wrapped in the select — if this call itself blocks, the select has already resolved to this branch and will never check shutdown again.

Wait — re-reading lines 379-385: the `select!` does wrap `self.recv_compute_res()` vs `shutdown.recv()`. So shutdown **will** fire during the await. This finding is **downgraded** but remains relevant: there is no execution **timeout** per block, meaning a single slow block can stall the vote pipeline indefinitely (until external shutdown). An operator has no way to skip a stuck block.

**Impact:** Pipeline stall if reth execution hangs on a single block; requires manual intervention.

---

### 7. Transaction Cache Asymmetric Consumption — Memory Leak Vector
**File:** `reth_cli.rs:196-202`

```rust
if let Some((_, cached_txn)) = self.txn_cache.remove(&key) {
    senders[idx] = Some(cached_txn.sender());
    transactions[idx] = Some(cached_txn.transaction.transaction().inner().clone());
}
```

Cache entries are **removed** on hit but there is no eviction policy visible for entries that are **never** hit. If transactions are added to the `DashMap` (by the mempool listener) but never included in an ordered block (e.g., dropped by consensus, replaced by higher-nonce transaction), they remain in the cache forever.

The `_txn_batch_size: 2000` field is unused (prefixed with `_`), suggesting a planned-but-unimplemented batch eviction mechanism.

**Impact:** Unbounded memory growth over time if transaction turnover is high and inclusion rate is low.

---

### 8. `persist_notifier` Send Failure Silently Discarded
**File:** `reth_cli.rs:495`

```rust
let _ = persist_notifier.send(()).await;
```

The `let _ =` pattern discards the `Result` from sending the persistence notification. If the receiver has been dropped (e.g., consensus epoch manager timed out waiting), the epoch transition signal is lost. The consensus layer may hang waiting for persistence confirmation that was sent but never delivered.

**Impact:** Potential epoch transition stall if the notification receiver is dropped before the persistence completes.

---

### 9. `start_commit` Uses Wrong Block Number in Panic Message
**File:** `reth_cli.rs:466-468`

```rust
let block_id = self.pipe_api.get_block_id(last_block.num).unwrap_or_else(|| {
    panic!("commit num {} not found block id", start_commit_num);
});
```

The panic message prints `start_commit_num` but the actual lookup is for `last_block.num`. At line 470, `start_commit_num` has already been updated to `last_block.num + 1` — wait, no, line 470 is **after** this check. But `start_commit_num` may still differ from `last_block.num` if the batch contains multiple blocks. The panic message will mislead operators during incident response.

**Impact:** Operational — misleading panic messages during critical failures hamper debugging.

---

## Summary

| # | Finding | Severity | Category |
|---|---------|----------|----------|
| 1 | Unsupervised task failure → zombie node | **Critical** | Availability / State Machine |
| 2 | `Address::ZERO` coinbase fee theft | **Critical** | Economic / Authorization |
| 3 | `start_commit` epoch desync | **High** | State Machine / Liveness |
| 4 | Panic-only consistency check in commit loop | **High** | Availability |
| 5 | Silent transaction filtering → consensus divergence | **High** | State Consistency |
| 6 | No per-block execution timeout | **Medium** | Availability |
| 7 | Transaction cache has no eviction | **Medium** | Resource Exhaustion |
| 8 | Persist notifier failure silently dropped | **Medium** | Liveness |
| 9 | Wrong variable in panic message | **Medium** | Operational |

---

## [mempool_transaction_pool] Review bin/gravity_node/src/mempool.rs for: (1) the TxPool trait implementation 

### 🎯 Intention

Review bin/gravity_node/src/mempool.rs for: (1) the TxPool trait implementation correctness including best_txns nonce ordering enforcement and the CachedBest TTL mechanism; (2) potential lock contention with std::sync::Mutex around cached_best in an async context; (3) the txn_cache (DashMap) unbounded growth and potential memory exhaustion; (4) the add_external_txn method for signer recovery safety and the fire-and-forget tokio::spawn pattern; (5) the remove_txns method for proper transaction cleanup; (6) the get_broadcast_txns method for potential information leakage; (7) environment variable parsing for MEMPOOL_CACHE_TTL_MS.

### 🔍 Discovered Implementation

## Implementation Analysis: `bin/gravity_node/src/mempool.rs`

---

### Files/Contracts Involved

| File | Description |
|------|-------------|
| `bin/gravity_node/src/mempool.rs` | `Mempool` struct and `TxPool` trait implementation |
| `bin/gravity_node/src/reth_cli.rs` | Defines `TxnCache` type alias (`Arc<DashMap<[u8;32], Arc<ValidPoolTransaction<EthPooledTransaction>>>>`) and `RethTransactionPool` type alias |
| `bin/gravity_node/src/main.rs` | Constructs `Mempool`, passes it to consensus engine or mock consensus |
| `crates/block-buffer-manager/src/block_buffer_manager.rs` | Defines `TxPool` trait with `best_txns`, `get_broadcast_txns`, `add_external_txn`, `remove_txns` |

---

### Key Types

- **`TxnCache`**: `Arc<DashMap<[u8; 32], Arc<ValidPoolTransaction<EthPooledTransaction>>>>` — concurrent hash map keyed by raw 32-byte tx hash, values are arc-wrapped validated pool transactions.
- **`RethTransactionPool`**: Fully parameterized `reth_transaction_pool::Pool` using `EthTransactionValidator`, `CoinbaseTipOrdering`, and `DiskFileBlobStore`.
- **`CachedBest`**: Internal struct holding a boxed `BestTransactions` iterator, a creation timestamp, and a `HashMap<Address, u64>` tracking last-yielded nonce per sender.

---

### Execution Path & Key Functions

#### 1. `cache_ttl() -> Duration` (lines 28–37)

- Uses `std::sync::OnceLock` for one-time initialization.
- Reads `MEMPOOL_CACHE_TTL_MS` environment variable.
- Parses as `u64`; on any failure (missing var, non-numeric string, negative value represented as string), silently falls back to `1000` ms.
- The `OnceLock` means the env var is read exactly once per process lifetime. Changes to the env var after first access have no effect.

#### 2. `Mempool::new(pool, enable_broadcast) -> Self` (lines 73–81)

- Wraps the reth `RethTransactionPool`.
- Creates a fresh `TxnCache` (`Arc<DashMap::new()>`).
- Wraps a new `CachedBest` (initialized as expired) inside `Arc<std::sync::Mutex>`.
- Creates a **new dedicated `tokio::runtime::Runtime`** (separate from the main runtime) stored as a field.
- `enable_broadcast` flag controls whether `get_broadcast_txns` returns transactions or an empty iterator.

#### 3. `TxPool::best_txns(&self, filter, limit) -> Box<dyn Iterator<Item = VerifiedTxn>>` (lines 121–178)

**Lock acquisition**: Calls `self.cached_best.lock().unwrap()` — this is a `std::sync::Mutex` lock. The lock is held for the entire duration of iterator consumption (the `.filter_map().take(limit).collect()` chain runs while the lock is held).

**Cache refresh logic** (lines 127–133):
- If the cached iterator is expired (elapsed > TTL) **or** is `None`, replaces the entire `CachedBest` with a fresh iterator from `self.pool.best_transactions()`, resets `last_nonces` to empty.
- Otherwise reuses the existing iterator and its position.

**Nonce ordering enforcement** (lines 141–151):
- For each transaction yielded by the underlying `BestTransactions` iterator:
  - Looks up the sender's last-seen nonce in `last_nonces`.
  - If a previous nonce exists for this sender and `nonce != last + 1`, the transaction is **skipped** (`return None`).
  - If no previous nonce exists for this sender, the transaction is accepted regardless of its nonce value (the first transaction from any sender is always accepted).
  - On acceptance, inserts/updates `sender -> nonce` in `last_nonces`.

**Filter application** (lines 154–159):
- If a filter closure is provided, it is called with `(ExternalAccountAddress, nonce, TxnHash)`.
- Transactions rejected by the filter return `None` from `filter_map` — but their nonce **has already been recorded** in `last_nonces` (line 151 executes before the filter check at line 154). This means a filtered-out transaction still advances the nonce tracking, so subsequent transactions from the same sender must have `nonce == filtered_nonce + 1` to pass.

**TxnCache insertion** (lines 162–163):
- Every accepted transaction is inserted into `self.txn_cache` using the raw 32-byte hash as key. The value is the `Arc<ValidPoolTransaction>` cloned from the iterator.
- Insertions happen unconditionally — there is no eviction, TTL, or size bound on this map.

**Empty result handling** (lines 170–176):
- If the collected result is empty, the `CachedBest` is reset: `best_txns` set to `None`, `last_nonces` cleared. This forces a fresh iterator on next call.

**Return**: The collected `Vec<VerifiedTxn>` is returned as a boxed iterator. The mutex guard is dropped when the function returns.

**`last_nonces` lifecycle** (lines 136, 169):
- `std::mem::take` extracts `last_nonces` from the `CachedBest` before iteration (replacing it with an empty `HashMap`).
- After iteration, `last_nonces` is placed back into `best_txns.last_nonces`.
- `last_nonces` persists across multiple calls to `best_txns` as long as the cache has not expired. On cache expiry, it is reset to empty.

#### 4. `TxPool::get_broadcast_txns(&self, filter) -> Box<dyn Iterator<Item = VerifiedTxn>>` (lines 180–204)

- If `self.enable_broadcast` is `false`, returns `std::iter::empty()` immediately.
- If enabled, calls `self.pool.all_transactions()` which returns all transactions currently in the pool (pending + queued).
- Iterates with `.all()`, applies the optional filter, converts each `Recovered<TransactionSigned>` to `VerifiedTxn` via `to_verified_txn_from_recovered_txn`.
- Collects into a `Vec` and returns as boxed iterator.
- No nonce ordering enforcement is applied.
- No caching is applied — every call fetches the full transaction set.
- No limit parameter exists on this method.

#### 5. `TxPool::add_external_txn(&self, txn: VerifiedTxn) -> bool` (lines 206–241)

**Decode** (line 207): Decodes `txn.bytes` using `TransactionSigned::decode_2718`.

**Signer recovery** (lines 210–216): Calls `txn.recover_signer()` which performs ECDSA recovery from the transaction signature. On failure, logs error and returns `false`.

**Pool submission** (lines 218–233):
- Wraps the decoded transaction in `Recovered::new_unchecked(txn, signer)` — this bypasses re-verification of the signer since it was just recovered above.
- Creates `EthPooledTransaction::new(recovered, len)`.
- Extracts `sender` and `to` addresses for error logging.
- Spawns an async task on `self.runtime` (the dedicated Tokio runtime) that calls `pool.add_external_transaction(pool_txn).await`.
- **Returns `true` immediately** before the async pool insertion completes. The actual insertion result is only logged on error; success/failure does not propagate to the caller.

**Error handling**: Decode failures and signer recovery failures return `false`. Pool insertion failures are logged but the function has already returned `true`.

#### 6. `TxPool::remove_txns(&self, txns: Vec<VerifiedTxn>)` (lines 243–259)

- Short-circuits on empty input.
- Decodes each `VerifiedTxn` from EIP-2718 bytes to extract the transaction hash.
- Decode failures are logged and the transaction is skipped.
- Calls `self.pool.remove_transactions(eth_txn_hashes)` with all successfully decoded hashes.
- **Does NOT remove entries from `self.txn_cache`** (the `DashMap`). Transactions removed from the reth pool remain in the cache indefinitely.

#### 7. `convert_account(acc: Address) -> ExternalAccountAddress` (lines 88–92)

- Takes a 20-byte Ethereum address.
- Zero-pads it into bytes `[12..32]` of a 32-byte array.
- Wraps in `ExternalAccountAddress::new`.

#### 8. `to_verified_txn(pool_txn)` (lines 94–105) and `to_verified_txn_from_recovered_txn(pool_txn)` (lines 107–118)

- Both produce `VerifiedTxn` with: EIP-2718 encoded bytes, converted sender address, nonce as `sequence_number`, hardcoded `chain_id: 0`, and tx hash.
- The first variant takes `Arc<ValidPoolTransaction<EthPooledTransaction>>` (from pool iterator).
- The second takes `Recovered<TransactionSigned>` (from `all_transactions()`).

---

### State Changes

| Operation | State Modified | Details |
|-----------|----------------|---------|
| `best_txns` | `cached_best` (Mutex-guarded) | Replaces iterator on TTL expiry; updates `last_nonces` per call; resets entirely if result is empty |
| `best_txns` | `txn_cache` (DashMap) | Inserts every yielded transaction keyed by 32-byte hash; never removes entries |
| `add_external_txn` | reth pool (async) | Adds transaction to underlying reth pool via `add_external_transaction` on a spawned task |
| `remove_txns` | reth pool (sync) | Removes transactions by hash from underlying reth pool |
| `remove_txns` | `txn_cache` | **Not modified** — removed transactions remain in the DashMap |

---

### External Dependencies

| Dependency | Usage |
|------------|-------|
| `dashmap::DashMap` | Concurrent hash map for `TxnCache`; lock-free reads/writes |
| `greth::reth_transaction_pool` | `TransactionPool` trait (`best_transactions`, `all_transactions`, `add_external_transaction`, `remove_transactions`), `BestTransactions` iterator, `EthPooledTransaction`, `ValidPoolTransaction` |
| `greth::reth_primitives` | `Recovered`, `TransactionSigned` |
| `alloy_consensus` | `SignerRecoverable` trait for `recover_signer()` |
| `alloy_eips` | `Decodable2718`, `Encodable2718` for EIP-2718 encoding/decoding |
| `alloy_primitives::Address` | 20-byte Ethereum address type |
| `block_buffer_manager::TxPool` | Trait being implemented |
| `gaptos::api_types` | `ExternalAccountAddress`, `ExternalChainId`, `TxnHash`, `VerifiedTxn` |
| `tokio::runtime::Runtime` | Dedicated runtime for fire-and-forget pool insertion in `add_external_txn` |

---

### Data Flow Diagram

```
Caller (Consensus/BlockBufferManager)
  │
  ├─ best_txns(filter, limit)
  │    │
  │    ├─ lock cached_best (std::sync::Mutex)
  │    ├─ if expired → pool.best_transactions() → new CachedBest
  │    ├─ iterate: nonce check → filter → to_verified_txn → txn_cache.insert
  │    ├─ if empty → reset CachedBest to None
  │    └─ return Vec<VerifiedTxn> as boxed iterator
  │
  ├─ get_broadcast_txns(filter)
  │    │
  │    ├─ if !enable_broadcast → empty iterator
  │    └─ pool.all_transactions().all() → filter → to_verified_txn_from_recovered_txn → Vec
  │
  ├─ add_external_txn(txn)
  │    │
  │    ├─ decode_2718 → recover_signer
  │    ├─ Recovered::new_unchecked
  │    ├─ runtime.spawn(pool.add_external_transaction) ← fire-and-forget
  │    └─ return true (before async completes)
  │
  └─ remove_txns(txns)
       │
       ├─ decode_2718 → extract hash
       ├─ pool.remove_transactions(hashes)
       └─ txn_cache is NOT cleaned
```

---

### Construction in `main.rs` (lines 256–260)

```rust
let pool = Box::new(Mempool::new(
    consensus_args.pool.clone(),
    gcei_config.base.role == RoleType::FullNode,  // broadcast enabled only for FullNodes
));
let txn_cache = pool.tx_cache();
```

- `enable_broadcast` is `true` only when the node role is `FullNode`.
- The `TxnCache` reference is extracted and passed separately to `RethCli::new()` (line 264), giving the RPC layer access to the same DashMap that `best_txns` populates.
- The `Mempool` (as `Box<dyn TxPool>`) is passed to either `MockConsensus::new(pool)` (line 272) or `ConsensusEngine::init(..., pool)` (line 294), depending on `MOCK_CONSENSUS` env var.

### 🛠️ Ground Review

# Code Quality & Security Review: `bin/gravity_node/src/mempool.rs`

## Critical

### 1. Unbounded `TxnCache` Growth — Memory Exhaustion
**File:** `mempool.rs:163` | `reth_cli.rs:71`

The `TxnCache` (`DashMap<[u8;32], Arc<ValidPoolTransaction>>`) grows without bound. `best_txns` inserts every yielded transaction (line 163), but `remove_txns` (line 243–259) only removes from the reth pool — **never from `txn_cache`**. The only removal path is in `reth_cli.rs:198` during `push_ordered_block`, which removes entries as blocks are processed. However, if transactions are evicted from the reth pool without being included in a block (e.g., replaced-by-fee, pool eviction, TTL expiry), their cache entries persist forever.

- **Severity:** Critical
- **Risk:** Unbounded memory growth over time, eventual OOM in long-running nodes.
- **Recommendation:** Add periodic cache eviction (e.g., on `remove_txns`, or a background sweep keyed on age or pool membership).

---

### 2. `std::sync::Mutex` in Async Context — Potential Deadlock
**File:** `mempool.rs:67, 126`

`cached_best` uses `std::sync::Mutex`, which blocks the OS thread when contended. If `best_txns` is called from an async task on a Tokio runtime, holding this lock while iterating (lines 126–177) blocks the entire runtime thread. Under contention, this can starve the async executor.

- **Severity:** Critical
- **Risk:** Thread starvation and effective deadlock under concurrent `best_txns` calls from async contexts.
- **Recommendation:** Use `tokio::sync::Mutex` if the lock must be held across `.await` points, or ensure `best_txns` is only ever called from a blocking/dedicated thread. Alternatively, minimize the lock scope — currently the lock is held during the entire `filter_map().take().collect()` chain.

---

### 3. `lock().unwrap()` — Panic on Poisoned Mutex
**File:** `mempool.rs:126`

If any thread panics while holding the `cached_best` lock, the mutex becomes poisoned. Subsequent calls to `best_txns` will panic unconditionally via `.unwrap()`, crashing the node.

- **Severity:** Critical
- **Risk:** Single panic cascades into permanent node failure.
- **Recommendation:** Handle the poisoned case gracefully, e.g., `lock().unwrap_or_else(|e| e.into_inner())` to recover the inner state, or use a `parking_lot::Mutex` which does not poison.

---

### 4. `Runtime::new().unwrap()` — Panic on Resource Exhaustion
**File:** `mempool.rs:78`

Creates a **new dedicated Tokio runtime** inside `Mempool::new`. This spawns additional OS threads and adds a second runtime to the process (the main runtime is created at `main.rs:255`). `Runtime::new().unwrap()` will panic if the OS cannot allocate threads.

- **Severity:** Critical
- **Risk:** Silent panic during construction if system resources are constrained; additionally, two Tokio runtimes add unnecessary complexity and resource usage.
- **Recommendation:** Accept a `tokio::runtime::Handle` from the caller instead of creating a second runtime. If a dedicated runtime is intentional, document why and handle the `Result`.

---

## Warning

### 5. `add_external_txn` Returns `true` Before Async Insertion Completes
**File:** `mempool.rs:223–234`

The fire-and-forget `runtime.spawn` pattern means the caller receives `true` even if the pool rejects the transaction (duplicate, invalid nonce, pool full, etc.). The caller has no way to distinguish success from deferred failure.

- **Severity:** Warning
- **Risk:** Silent transaction drops; callers may assume a transaction is in the pool when it is not.
- **Recommendation:** If the API contract permits, make `add_external_txn` async and `.await` the result. If fire-and-forget is intentional, document the contract and consider a metric/counter for failed insertions.

---

### 6. Nonce Tracking Advances on Filtered-Out Transactions
**File:** `mempool.rs:151–158`

`last_nonces.insert(sender, nonce)` executes at line 151 **before** the filter check at line 154. If the filter rejects a transaction, its nonce is still recorded, which means subsequent transactions from the same sender must have `nonce == rejected_nonce + 1`. A filtered-out transaction creates a nonce gap from the perspective of the caller.

- **Severity:** Warning
- **Risk:** Legitimate transactions from a sender may be silently skipped if an earlier transaction was filtered out. This could cause transaction starvation for specific accounts.
- **Recommendation:** Move the `last_nonces.insert` after the filter check (after line 158, inside the `Some(verified_txn)` branch).

---

### 7. `get_broadcast_txns` Has No Limit — Full Pool Dump
**File:** `mempool.rs:180–204`

Unlike `best_txns` which accepts a `limit`, `get_broadcast_txns` calls `pool.all_transactions().all()` and collects everything. On a busy network, this could yield thousands of transactions, causing a large memory allocation and slow iteration.

- **Severity:** Warning
- **Risk:** Memory spikes and latency during broadcast in nodes with large mempools.
- **Recommendation:** Add a `limit` parameter or an internal cap.

---

### 8. Duplicate `convert_account` Definition
**File:** `mempool.rs:88` | `reth_cli.rs:86`

The same `convert_account` function (identical bodies) is defined in both `mempool.rs` and `reth_cli.rs`. This is a maintenance hazard — a bug fix in one won't propagate to the other.

- **Severity:** Warning
- **Recommendation:** Extract to a shared module or re-export from one location.

---

## Info

### 9. `chain_id` Hardcoded to `0`
**File:** `mempool.rs:102, 115`

Both `to_verified_txn` and `to_verified_txn_from_recovered_txn` set `chain_id: ExternalChainId::new(0)`. If downstream consumers rely on chain ID for replay protection or routing, this hardcoded zero may cause issues.

- **Severity:** Info
- **Recommendation:** Thread the actual chain ID from the node configuration into the conversion functions.

---

### 10. Silent Env Var Parse Failure in `cache_ttl()`
**File:** `mempool.rs:31–34`

`MEMPOOL_CACHE_TTL_MS` parse failures (e.g., `"abc"`, `"-1"`, `""`) are silently swallowed and fall back to 1000ms. An operator setting a value and getting the default has no indication of misconfiguration.

- **Severity:** Info
- **Recommendation:** Log a warning when the env var is present but unparseable.

---

### 11. `CachedBest::new()` Relies on Time Arithmetic for Expiry
**File:** `mempool.rs:54`

`Instant::now() - cache_ttl() - Duration::from_millis(1)` — this subtraction could theoretically underflow on platforms where `Instant` starts near zero. In practice, Rust panics on `Instant` underflow in debug and saturates to zero in release. The intent is clear but fragile.

- **Severity:** Info
- **Recommendation:** Use a dedicated `expired` boolean flag or wrap `created_at` in an `Option<Instant>` to express "never initialized."

---

## Summary

| Severity | Count | Key Themes |
|----------|-------|------------|
| **Critical** | 4 | Unbounded cache growth, mutex misuse in async, panic propagation, extra runtime |
| **Warning** | 4 | Fire-and-forget semantics, nonce tracking bug, no broadcast limit, code duplication |
| **Info** | 3 | Hardcoded chain ID, silent config failure, fragile time arithmetic |

The highest-priority items are the **unbounded `TxnCache`** (item 1) and the **nonce-tracking-before-filter bug** (item 6), as these directly affect node stability and transaction ordering correctness under production load.

### ⚔️ Intention Attacker

# Security Audit Report: `bin/gravity_node/src/mempool.rs`

## Attack Surface Analysis — Fatal Logic Vulnerabilities

---

### CRITICAL-01: Unbounded `TxnCache` Memory Growth — Denial of Service

**Severity: Critical**
**Location:** `mempool.rs:162-163`, `mempool.rs:243-259`

**Finding:** The `txn_cache` (`DashMap<[u8;32], Arc<ValidPoolTransaction>>`) grows without bound. Every transaction yielded by `best_txns` is inserted (line 163), but `remove_txns` (line 243-259) **never removes entries from `txn_cache`** — it only removes from the underlying reth pool.

The only place `txn_cache` entries are removed is in `reth_cli.rs:198` during `push_ordered_block`, where entries are consumed via `self.txn_cache.remove(&key)`. However, this only removes transactions that are actually included in committed blocks. Transactions that are:
- Yielded by `best_txns` but never included in a block (e.g., dropped due to gas price, replaced by newer txns)
- Superseded by replacement transactions (same sender, same nonce, higher gas)
- From senders whose accounts are drained

...will **remain in the DashMap forever**, leaking memory proportional to total transaction throughput. On a high-throughput chain, this is a guaranteed OOM vector.

**Attack scenario:** An attacker submits thousands of transactions, then replaces them all with higher-gas-price replacements. The originals remain cached indefinitely. Repeated cycles exhaust node memory.

---

### CRITICAL-02: `add_external_txn` Returns `true` Before Pool Insertion — State Desynchronization

**Severity: Critical**
**Location:** `mempool.rs:206-241`

**Finding:** `add_external_txn` decodes and recovers the signer synchronously, then spawns an async task (line 223) to actually insert the transaction into the reth pool, and immediately returns `true` (line 234) **before the insertion completes or is even attempted**.

```rust
self.runtime.spawn(async move {
    let res = pool.add_external_transaction(pool_txn).await;
    if let Err(e) = res { tracing::error!(...); }
});
true  // returned before spawn completes
```

**Consequences:**
1. The caller (consensus/VFN forwarding) believes the transaction was accepted and may propagate this confirmation upstream, but the reth pool could reject it (duplicate nonce, insufficient balance, gas limit exceeded, pool full).
2. There is no backpressure mechanism — if the reth pool is congested or full, the fire-and-forget pattern silently drops transactions while reporting success.
3. In a Validator Full Node (VFN) forwarding scenario (confirmed by `e2e` test in recent commits), this means the forwarding node reports success to the user while the transaction may never enter the mempool.

---

### HIGH-01: Filtered Transactions Poison Nonce Tracking — Transaction Censorship

**Severity: High**
**Location:** `mempool.rs:141-158`

**Finding:** In `best_txns`, the nonce is recorded in `last_nonces` at line 151 **before** the filter is applied at lines 154-158. If a transaction passes the nonce-ordering check but is rejected by the filter, its nonce is still recorded:

```rust
// Line 151: nonce recorded BEFORE filter
last_nonces.insert(sender, nonce);

// Lines 154-158: filter applied AFTER nonce recording
if let Some(ref f) = filter {
    if !f((sender.clone(), nonce, hash)) {
        return None;  // filtered out, but nonce already advanced
    }
}
```

This means for sender `A` with nonces `[5, 6, 7]`: if nonce 6 is filtered out, `last_nonces[A] = 6`. Nonce 7 passes the check (`7 == 6 + 1`). But the block will contain nonces 5 and 7 **without 6**, which will fail execution on-chain since EVM enforces strict nonce sequencing.

**Impact:** The filter (used by consensus for deduplication) can create "impossible" transaction sequences that pass mempool validation but fail at execution, wasting block space. Worse: because `last_nonces` persists across calls within the TTL window, a single filtered transaction can permanently block all subsequent transactions from that sender until the cache expires.

---

### HIGH-02: `std::sync::Mutex` in Async Context — Potential Deadlock Under Load

**Severity: High**
**Location:** `mempool.rs:67`, `mempool.rs:126`

**Finding:** `cached_best` uses `std::sync::Mutex` (not `tokio::sync::Mutex`). The lock is acquired at line 126 and held during the entire iterator consumption chain including `.filter_map().take(limit).collect()` (lines 137-167).

The `filter_map` closure on each transaction performs:
- Hash computation (`pool_txn.hash()`)
- Address conversion
- Filter closure invocation
- EIP-2718 encoding
- DashMap insertion

If the reth `BestTransactions` iterator blocks or yields slowly (which it can under pool contention), this `std::sync::Mutex` blocks the OS thread. In an async context (tokio runtime), this blocks the entire tokio worker thread, potentially starving other async tasks including the consensus engine.

While `std::sync::Mutex` is acceptable for very short critical sections, this lock is held for an iteration of up to `limit` items with non-trivial per-item work, making it a scalability bottleneck and potential contributor to consensus timeouts under high transaction volumes.

---

### HIGH-03: `CachedBest` TTL Allows Stale/Duplicate Transaction Delivery

**Severity: High**
**Location:** `mempool.rs:59-61`, `mempool.rs:127-133`, `mempool.rs:136-169`

**Finding:** The `CachedBest` iterator is reused across multiple calls within the TTL window (default 1 second). The `last_nonces` map persists across these calls. This creates two problems:

1. **Stale iterator state:** Within the TTL window, the underlying reth pool may have changed (new transactions added, old ones removed, replacements). The cached `BestTransactions` iterator does not reflect these changes, potentially yielding transactions that have already been removed from the pool or missing higher-priority replacements.

2. **Cross-call nonce coupling:** If call 1 yields sender A's nonce 5, and call 2 (within TTL) checks sender A, it requires nonce 6. If nonce 6 was added between calls but the cached iterator was already past that position, nonce 6 is skipped. This can cause valid transactions to be systematically excluded until cache expiry.

3. **Environment variable manipulation:** `MEMPOOL_CACHE_TTL_MS` is read via `OnceLock` (line 29-37). Setting it to `0` makes the cache expire every call (performance issue), and setting it to `u64::MAX` effectively freezes the iterator permanently (liveness issue). No validation bounds.

---

### MEDIUM-01: `get_broadcast_txns` Has No Transaction Limit — Amplification Attack

**Severity: Medium**
**Location:** `mempool.rs:180-204`

**Finding:** Unlike `best_txns` which has a `limit` parameter, `get_broadcast_txns` calls `self.pool.all_transactions().all()` with no limit, collects everything into a `Vec`, and returns it. On a FullNode (`enable_broadcast == true`), this dumps the entire mempool contents.

If the reth pool contains thousands of transactions, each call:
- Allocates a `Vec` of all transactions
- EIP-2718 encodes every single one
- Performs address conversion for each

This can be triggered repeatedly by the caller (presumably the broadcast/gossip subsystem), creating a CPU and memory amplification vector. An attacker flooding the mempool with cheap transactions forces expensive serialization work on every broadcast cycle.

---

### MEDIUM-02: Hardcoded `chain_id: 0` in `VerifiedTxn` — Chain Replay Risk

**Severity: Medium**
**Location:** `mempool.rs:102`, `mempool.rs:115`

**Finding:** Both `to_verified_txn` (line 102) and `to_verified_txn_from_recovered_txn` (line 115) hardcode `chain_id: ExternalChainId::new(0)` regardless of the actual chain ID. The real chain ID is known and used elsewhere (e.g., `reth_cli.rs:104-107`).

If downstream consumers of `VerifiedTxn` rely on `chain_id` for replay protection validation or cross-chain checks, the hardcoded zero defeats that protection. Transactions signed for one chain could be treated as valid on another if only the `VerifiedTxn.chain_id` field is checked.

---

### MEDIUM-03: `tokio::runtime::Runtime::new().unwrap()` — Unguarded Panic in Constructor

**Severity: Medium**
**Location:** `mempool.rs:78`

**Finding:** `Mempool::new` creates a dedicated tokio runtime with `Runtime::new().unwrap()`. If runtime creation fails (e.g., OS thread/fd limits exhausted), this panics and crashes the entire node. This is constructed in the main startup path (main.rs:256), so a transient resource shortage at startup causes a hard crash with no recovery.

Additionally, creating a **separate** tokio runtime means `add_external_txn` spawns tasks on a different runtime than the main application. If the main runtime shuts down, the Mempool's runtime may still have in-flight tasks, creating an unclean shutdown path. Conversely, the Mempool runtime is never explicitly shut down.

---

### Summary

| ID | Severity | Finding | Impact |
|----|----------|---------|--------|
| CRITICAL-01 | Critical | `txn_cache` grows unbounded, never cleaned for non-included transactions | OOM / node crash |
| CRITICAL-02 | Critical | `add_external_txn` returns `true` before async pool insertion | Silent transaction loss, false confirmations |
| HIGH-01 | High | Filtered transactions poison nonce tracking | Invalid transaction sequences, sender censorship |
| HIGH-02 | High | `std::sync::Mutex` held during full iteration in async context | Thread starvation, consensus timeouts |
| HIGH-03 | High | Stale cached iterator + unvalidated TTL env var | Missed transactions, potential liveness freeze |
| MEDIUM-01 | Medium | `get_broadcast_txns` has no limit on returned transactions | CPU/memory amplification |
| MEDIUM-02 | Medium | Hardcoded `chain_id: 0` in all `VerifiedTxn` output | Bypassed replay protection |
| MEDIUM-03 | Medium | Unguarded panic on runtime creation + orphaned runtime on shutdown | Hard crash, unclean shutdown |

---

## [block_buffer_manager_state_machine] Deep security audit of crates/block-buffer-manager/src/block_buffer_manager.rs f

### 🎯 Intention

Deep security audit of crates/block-buffer-manager/src/block_buffer_manager.rs focusing on: (1) the BlockStateMachine state transitions (Ordered->Computed->Committed) for correctness and race conditions; (2) epoch transition handling including the next_epoch/current_epoch split, release_inflight_blocks, and consume_epoch_change for TOCTOU races; (3) the AtomicU8 buffer_state transitions between Uninitialized/Ready/EpochChange; (4) potential deadlocks in nested lock acquisitions; (5) the set_ordered_blocks duplicate/old-epoch block handling; (6) panic conditions in set_commit_blocks and get_ordered_blocks; (7) the remove_committed_blocks garbage collection for correctness; (8) the pop_txns off-by-one fix (GSDK-024); (9) unbounded HashMap growth in blocks and profile maps.

### 🔍 Discovered Implementation

# Implementation Analysis: `block_buffer_manager.rs`

## Files/Contracts Involved

| File | Description |
|------|-------------|
| `crates/block-buffer-manager/src/block_buffer_manager.rs` | Core file — manages block lifecycle through the GCEI pipeline (Ordered → Computed → Committed) |
| `crates/block-buffer-manager/src/lib.rs` | Crate root / re-exports |

---

## Data Structures

### `BlockKey` (L102–112)
- Composite key: `(epoch: u64, block_number: u64)`. Derived `Hash`, `Eq`. Used as HashMap key for all block and profile lookups.

### `BlockState` enum (L114–134)
Four variants representing the block lifecycle:
- **`Ordered { block: ExternalBlock, parent_id: BlockId }`** — block received from consensus ordering
- **`Computed { id: BlockId, compute_result: StateComputeResult }`** — execution result attached
- **`Committed { hash, compute_result, id, persist_notifier }`** — commit decision made; optional `Sender<()>` for epoch-change persistence signaling
- **`Historical { id: BlockId }`** — recovered from storage at init, only has block ID

### `BufferState` enum (L147–153)
`repr(u8)` for atomic storage: `Uninitialized` (0), `Ready` (1), `EpochChange` (2).

### `BlockStateMachine` (L165–176)
All mutable state behind a single `tokio::sync::Mutex`:
- `blocks: HashMap<BlockKey, BlockState>` — the block state map
- `profile: HashMap<BlockKey, BlockProfile>` — timing metadata per block
- `latest_commit_block_number: u64` — highest committed block number
- `latest_finalized_block_number: u64` — highest finalized (persisted) block number
- `block_number_to_block_id: HashMap<u64, BlockId>` — reverse lookup
- `current_epoch: u64` / `next_epoch: Option<u64>` — epoch tracking with split for transitions
- `latest_epoch_change_block_number: u64` — block number where last epoch change occurred
- `sender: broadcast::Sender<()>` — notification channel for waiters

### `BlockBufferManager` (L211–218)
Top-level manager:
- `txn_buffer: TxnBuffer` — separate `Mutex<Vec<TxnItem>>` for transaction buffering
- `block_state_machine: Mutex<BlockStateMachine>` — the single lock protecting all block state
- `buffer_state: AtomicU8` — lock-free readiness check
- `ready_notifier: Arc<Notify>` — one-shot notification for initialization
- `config: BlockBufferManagerConfig` — timeouts and limits

---

## Execution Path — Key Functions

### 1. `new(config)` → `Arc<Self>` (L221–250)
- Constructs all fields; `buffer_state` starts as `Uninitialized`.
- Spawns a background tokio task that loops forever, calling `remove_committed_blocks()` on each interval (`remove_committed_blocks_interval`, default 1s).
- The spawned task calls `.unwrap()` on the result of `remove_committed_blocks()` (L246).

### 2. `init(latest_commit_block_number, block_number_to_block_id_with_epoch, initial_epoch)` (L269–303)
- Acquires `block_state_machine` lock.
- Sets `latest_commit_block_number`, `latest_finalized_block_number` (both to same value).
- Populates `block_number_to_block_id` from input (stripping epoch).
- Sets `current_epoch = initial_epoch`.
- If map is non-empty: looks up `latest_commit_block_number` in the map (`.unwrap()` — panics if key missing), inserts a `Historical` block, sets `latest_epoch_change_block_number`.
- Stores `BufferState::Ready` via `AtomicU8` with `SeqCst`.
- Calls `ready_notifier.notify_waiters()`.

### 3. `init_epoch(epoch)` (L307–311)
- Acquires lock, sets `current_epoch` directly. No interaction with `next_epoch` or `buffer_state`.

### 4. `set_ordered_blocks(parent_id, block)` (L398–497)
**Readiness gate**: If `!is_ready()`, awaits `ready_notifier`.

**Epoch validation (L414–436)**:
- Reads `current_epoch` from locked BSM.
- `block.epoch < current_epoch` → logs warning, returns `Ok(())` (silently dropped).
- `block.epoch > current_epoch` → returns `Err(...)`.
- `block.epoch == current_epoch` → proceeds.

**Duplicate detection (L441–457)**:
- Checks if `BlockKey(epoch, block_number)` already exists in `blocks`.
- If exists with same ID → warns and returns `Ok(())`.
- If exists with different ID → warns and returns `Ok(())` (does NOT error or panic).

**Parent resolution (L459–484)**:
- Looks up parent as `BlockKey(epoch, block_number - 1)`. **Note**: If `block_number == 0`, this wraps to `u64::MAX` due to unsigned subtraction.
- Falls back to `BlockKey(epoch - 1, block_number - 1)` when epoch > 0.
- If neither found, uses the provided `parent_id`.
- If found parent's ID differs from provided `parent_id`, logs info and uses the found parent's ID.

**Insertion (L486–496)**:
- Inserts `BlockState::Ordered { block, parent_id }` into `blocks`.
- Records `set_ordered_block_time` in profile.
- Broadcasts notification.

### 5. `get_ordered_blocks(start_num, max_size, expected_epoch)` (L499–572)
**Gates**: readiness check, then `is_epoch_change()` check (returns error if in epoch change).

**Polling loop** with `max_wait_timeout` (default 5s):
- Acquires lock, iterates from `start_num` upward looking for `BlockState::Ordered` entries with key `(expected_epoch, current_num)`.
- **Panic condition (L544–546)**: If a block exists at the key but is NOT `Ordered` (i.e., it's `Computed`, `Committed`, or `Historical`), the function **panics**.
- `None` → breaks inner loop (no more contiguous blocks).
- If `result` is empty: drops lock, calls `wait_for_change()`, retries.
- Returns collected `Vec<(ExternalBlock, BlockId)>`.

### 6. `set_compute_res(block_id, block_hash, block_num, epoch, txn_status, events)` (L703–756)
- Acquires lock.
- Looks up `BlockKey(epoch, block_num)`. Must be `Ordered`; asserts `block.block_meta.block_id == block_id`.
- Calls `calculate_new_epoch_state(&events, block_num, &mut bsm)` which may set `next_epoch` and `latest_epoch_change_block_number`.
- Constructs `StateComputeResult`, replaces state with `BlockState::Computed`.
- **Panic (L755)**: If lookup fails or state is not `Ordered`, panics with message.

### 7. `calculate_new_epoch_state(events, block_num, bsm)` (L652–701)
- Scans events for `GravityEvent::NewEpoch`.
- If found: deserializes `ValidatorSet` from BCS bytes, sets `bsm.latest_epoch_change_block_number = block_num`, sets `bsm.next_epoch = Some(new_epoch)`.
- Does **not** update `current_epoch` — that is deferred to `release_inflight_blocks`.

### 8. `set_commit_blocks(block_ids, epoch)` (L758–839)
- Acquires lock, iterates over `block_ids`.
- For each:
  - **`Computed` with matching ID** → transitions to `Committed`. If `compute_result.epoch_state()` is `Some`, creates an `mpsc::channel(1)` persist notifier.
  - **`Computed` with mismatched ID** → **panics** (L802–805).
  - **`Committed` with mismatched ID** → **panics** (L809–811).
  - **`Committed` with matching ID** → no-op (idempotent re-commit).
  - **`Ordered`** → **panics** (L813–817) — block not yet computed.
  - **`Historical` with mismatched ID** → **panics** (L822–826).
  - **`Historical` with matching ID** → no-op.
  - **`None` (key missing)** → **panics** (L831–834).
- Broadcasts notification. Returns `Vec<Receiver<()>>` for persist notifiers.

### 9. `get_committed_blocks(start_num, max_size, epoch)` (L841–914)
- Polling loop similar to `get_ordered_blocks`.
- Iterates contiguously from `start_num`, collecting `Committed` blocks.
- **`persist_notifier.take()`** (L879): moves the `Sender` out of the `Committed` state — subsequent calls for the same block will get `None`.
- On success, updates `latest_finalized_block_number` to `max(current, last_result.num)`.

### 10. `release_inflight_blocks()` (L954–983)
- Acquires lock.
- Reads `latest_epoch_change_block_number`.
- If `next_epoch` is `Some`, takes it and updates `current_epoch`.
- **Retains only blocks with `block_number <= latest_epoch_change_block_number`** — removes all inflight blocks after the epoch boundary.
- Sets `buffer_state` to `EpochChange` via `AtomicU8` `SeqCst` store.
- Also retains profiles with same filter.
- Broadcasts notification.

### 11. `consume_epoch_change()` → `u64` (L348–357)
- Acquires lock, reads `current_epoch`.
- Sets `buffer_state` back to `Ready` via `AtomicU8` `SeqCst` store.
- Returns the epoch value.

### 12. `remove_committed_blocks()` (L252–267)
- Acquires lock.
- **Early return** if `blocks.len() < max_block_size` (default 256).
- Reads `latest_finalized_block_number`, then does a no-op `max()` update (L259–261: `max(x, x)` since both sides are the same field).
- Retains blocks and profiles where `block_number >= latest_finalized_block_number`.
- Broadcasts notification.

### 13. `pop_txns(max_size, gas_limit)` (L365–396)
- Acquires `txn_buffer.txns` lock (separate from `block_state_machine`).
- Iterates `TxnItem`s, tracking cumulative `gas_limit` and `count`.
- **GSDK-024 fix**: Uses `.position()` to find split point — the first item where adding it would exceed `gas_limit` OR `count >= max_size`.
- Drains `0..split_point` from the buffer, flattens txns into result vec.
- The check `count >= max_size` uses `>=` meaning it caps at exactly `max_size` items.

### 14. `push_txns(txns, gas_limit)` (L330–338)
- Acquires `txn_buffer.txns` lock, pushes a `TxnItem` with `std::mem::take(txns)`.

---

## State Changes Summary

| Function | State Modified |
|----------|---------------|
| `init` | `latest_commit_block_number`, `latest_finalized_block_number`, `block_number_to_block_id`, `current_epoch`, `blocks` (Historical), `latest_epoch_change_block_number`, `buffer_state` → Ready |
| `init_epoch` | `current_epoch` |
| `set_ordered_blocks` | `blocks` (insert Ordered), `profile` |
| `get_ordered_blocks` | `profile` (timing only) |
| `set_compute_res` | `blocks` (Ordered → Computed), `next_epoch` (maybe), `latest_epoch_change_block_number` (maybe), `profile` |
| `set_commit_blocks` | `blocks` (Computed → Committed), `profile` |
| `get_committed_blocks` | `persist_notifier` (taken), `latest_finalized_block_number`, `profile` |
| `release_inflight_blocks` | `current_epoch` (from `next_epoch`), `blocks` (retain ≤ epoch change), `profile` (retain), `buffer_state` → EpochChange |
| `consume_epoch_change` | `buffer_state` → Ready |
| `remove_committed_blocks` | `blocks` (retain ≥ finalized), `profile` (retain) |
| `push_txns` | `txn_buffer.txns` (push) |
| `pop_txns` | `txn_buffer.txns` (drain) |

---

## Lock Acquisition Map

- **`block_state_machine: Mutex<BlockStateMachine>`** — acquired by: `init`, `init_epoch`, `wait_for_change`, `set_ordered_blocks`, `get_ordered_blocks`, `set_compute_res`, `set_commit_blocks`, `get_committed_blocks`, `set_state`, `latest_commit_block_number`, `block_number_to_block_id`, `get_current_epoch`, `release_inflight_blocks`, `consume_epoch_change`, `remove_committed_blocks`, `latest_epoch_change_block_number`.
- **`txn_buffer.txns: Mutex<Vec<TxnItem>>`** — acquired by: `push_txns`, `pop_txns`.
- No function acquires both locks. The two mutexes are independent.

## AtomicU8 `buffer_state` Transitions

| From | To | Where |
|------|----|-------|
| `Uninitialized` | `Ready` | `init()` L300 |
| `Ready` | `EpochChange` | `release_inflight_blocks()` L978 |
| `EpochChange` | `Ready` | `consume_epoch_change()` L355 |

All transitions use `Ordering::SeqCst`. Reads (`is_ready`, `is_epoch_change`) also use `SeqCst`.

## Epoch Transition Sequence

1. `set_compute_res` detects `NewEpoch` event → calls `calculate_new_epoch_state` → sets `next_epoch = Some(new_epoch)` and `latest_epoch_change_block_number = block_num`. Does NOT change `current_epoch`.
2. `release_inflight_blocks` → acquires lock, takes `next_epoch` into `current_epoch`, retains only blocks ≤ epoch change block, sets `buffer_state` = `EpochChange`.
3. `consume_epoch_change` → acquires lock, reads `current_epoch` (now updated), sets `buffer_state` = `Ready`.

The split between `next_epoch` and `current_epoch` ensures that `current_epoch` is only updated atomically during `release_inflight_blocks` while the lock is held, not during computation.

## External Dependencies

- `anyhow` — error handling
- `aptos_executor_types::StateComputeResult` — execution result type
- `gaptos` — API types (`ExternalBlock`, `VerifiedTxn`, `BlockId`, `GravityEvent`, `EpochState`, etc.)
- `tokio` — async runtime (`Mutex`, `Notify`, `mpsc`, `broadcast`, timers)
- `bcs` — binary canonical serialization for `ValidatorSet`
- `tracing` — structured logging

## Panic Inventory

| Location | Condition |
|----------|-----------|
| L292 | `init()`: `latest_commit_block_number` not found in `block_number_to_block_id_with_epoch` map |
| L544–546 | `get_ordered_blocks()`: block exists at key but is not `Ordered` |
| L600 | `get_executed_res()`: block_id mismatch on `Computed` state (assert_eq) |
| L630 | `get_executed_res()`: block_id mismatch on `Committed` state (assert_eq) |
| L721 | `set_compute_res()`: block_id mismatch on `Ordered` state (assert_eq) |
| L755 | `set_compute_res()`: block not found or not `Ordered` |
| L802–805 | `set_commit_blocks()`: `Computed` block id mismatch |
| L810–811 | `set_commit_blocks()`: `Committed` block id mismatch |
| L814–817 | `set_commit_blocks()`: block is still `Ordered` (not computed) |
| L822–826 | `set_commit_blocks()`: `Historical` block id mismatch |
| L831–834 | `set_commit_blocks()`: block key not found at all |

## Notification Mechanism

- `broadcast::Sender<()>` with capacity 1024 in the BSM — receivers subscribe in `wait_for_change()`.
- `Arc<Notify>` (`ready_notifier`) — one-shot: `init()` calls `notify_waiters()`, all functions with readiness gate await `notified()`.
- `mpsc::channel(1)` persist notifiers — created per epoch-change-bearing committed block, receiver returned to caller of `set_commit_blocks`, sender stored in `Committed` state and taken by `get_committed_blocks`.

### 🛠️ Ground Review

# Ground Review (Blue Team) — `block_buffer_manager.rs`

## Critical

### C1. Unsigned integer underflow in parent key computation (L460–461)
```rust
let parent_key_current =
    BlockKey::new(block.block_meta.epoch, block.block_meta.block_number - 1);
```
When `block_number == 0`, the subtraction wraps to `u64::MAX`. This creates a bogus `BlockKey` used for a HashMap lookup. While in practice it will likely return `None` (falling through to the provided `parent_id`), it is undefined-intent behavior that silently masks a logic error. If any block with `block_number == u64::MAX` ever exists in the map, this would silently bind a wrong parent.

**Severity: Critical**
**Recommendation:** Guard with a checked subtraction or explicit `block_number == 0` branch.

---

### C2. Background GC task panics on error — kills the tokio runtime (L243–247)
```rust
tokio::spawn(async move {
    loop {
        tokio::time::sleep(clone.config.remove_committed_blocks_interval).await;
        clone.remove_committed_blocks().await.unwrap();
    }
});
```
`.unwrap()` on the result of `remove_committed_blocks` means any error (currently unlikely since it returns `Ok`, but any future modification could trigger it) panics inside the spawned task. In tokio, an unwinding panic inside `spawn` aborts only that task by default, but depending on panic hooks or runtime config it could crash the node.

**Severity: Critical**
**Recommendation:** Replace `.unwrap()` with `.inspect_err(|e| tracing::error!(...))` or `if let Err(e) = ... { ... }`.

---

### C3. No-op `max()` in `remove_committed_blocks` (L257–262)
```rust
let latest_persist_block_num = block_state_machine.latest_finalized_block_number;
block_state_machine.latest_finalized_block_number = std::cmp::max(
    block_state_machine.latest_finalized_block_number,
    latest_persist_block_num,
);
```
`latest_persist_block_num` is read from the same field on L257, so the `max()` on L259–261 is `max(x, x)` — a dead operation. This is likely a bug from a refactor where `latest_persist_block_num` was previously read from a different source. Depending on the original intent, this may mean the GC watermark is never actually advancing here, leading to unbounded growth in the `blocks` HashMap (see W1).

**Severity: Critical**
**Recommendation:** Determine the original design intent. If GC was supposed to use a different watermark (e.g., a separately-tracked persistence cursor), restore the correct source.

---

### C4. TOCTOU race on readiness gate (L403–405, L505–507, etc.)
```rust
if !self.is_ready() {
    self.ready_notifier.notified().await;
}
```
`is_ready()` reads `buffer_state` atomically, but between the check returning `true` and the subsequent lock acquisition, the state could change (e.g., to `EpochChange`). The function then proceeds as if `Ready`. This pattern appears in `set_ordered_blocks`, `get_ordered_blocks`, `set_compute_res`, `set_commit_blocks`, `get_committed_blocks`, `block_number_to_block_id`, and `get_current_epoch`.

Additionally, there is a **missed-notification race**: if `init()` calls `notify_waiters()` between the `is_ready()` check returning `false` and the `.notified().await` registration, the notification is lost and the caller hangs forever.

**Severity: Critical**
**Recommendation:** Register the `Notified` future *before* checking the condition:
```rust
let notified = self.ready_notifier.notified();
if !self.is_ready() {
    notified.await;
}
```

---

## Warning

### W1. Unbounded HashMap growth in `blocks` and `profile` maps
The GC in `remove_committed_blocks` only fires when `blocks.len() >= max_block_size` (256) and only retains blocks `>= latest_finalized_block_number`. However:
- `block_number_to_block_id` (L171) is populated in `init()` and never cleaned up — it grows monotonically.
- `profile` is cleaned alongside `blocks` in GC, but also independently grows during epoch transitions before `release_inflight_blocks` prunes it.

Under sustained load or long-running nodes, `block_number_to_block_id` becomes a slow memory leak.

**Severity: Warning**
**Recommendation:** Add GC for `block_number_to_block_id` in `remove_committed_blocks`, retaining only entries `>= latest_finalized_block_number`.

---

### W2. `persist_notifier.take()` is destructive and non-idempotent (L879)
```rust
persist_notifier: persist_notifier.take(),
```
`get_committed_blocks` calls `.take()` on the `Sender`, meaning only the *first* caller to retrieve a committed block gets the persist notifier. Any subsequent call (e.g., a retry after a transient downstream failure) will get `None`, and the epoch-change persistence signal may never fire.

**Severity: Warning**
**Recommendation:** Document this single-consumer contract explicitly, or use a broadcast-style mechanism if multiple consumers are possible.

---

### W3. Global singleton with hidden default config (lib.rs L8–12)
```rust
pub fn get_block_buffer_manager() -> &'static Arc<BlockBufferManager> {
    GLOBAL_BLOCK_BUFFER_MANAGER.get_or_init(|| {
        BlockBufferManager::new(block_buffer_manager::BlockBufferManagerConfig::default())
    })
}
```
A `OnceLock` global with hardcoded default config means the first caller wins. If any path calls `get_block_buffer_manager()` before the intended initialization with custom config, the system silently runs with defaults. This is a testability and operability hazard.

**Severity: Warning**
**Recommendation:** Consider a pattern where the config must be explicitly provided at startup, or panic if accessed before initialization.

---

### W4. Duplicate block with different ID is silently accepted (L450–456)
```rust
} else {
    warn!(
        "set_ordered_blocks: block {} with epoch {} already exists with different id ...",
        ...
    );
}
return Ok(());
```
When a block arrives at the same `(epoch, block_number)` but with a **different** `block_id`, it is logged as a warning and silently dropped. In a Byzantine environment, this means the first block to arrive for a given slot wins. If this is by design (consensus guarantees uniqueness), the warn-level log is misleading. If it's not by design, silently accepting this is a correctness risk.

**Severity: Warning**
**Recommendation:** Either upgrade to an error return (signaling a protocol violation) or downgrade the log to info with a comment explaining why it's safe.

---

### W5. Excessive use of `panic!` for recoverable states (11 panic sites)
The implementation report documents 11 distinct `panic!` sites across `get_ordered_blocks`, `set_compute_res`, `set_commit_blocks`, and `get_executed_res`. Many of these represent state-machine invariant violations that could arise from reordering, delayed messages, or duplicate calls. In a long-running node, panicking on these conditions means a transient protocol glitch crashes the entire process.

**Severity: Warning**
**Recommendation:** Convert at least the non-assertion panics to `anyhow::Error` returns. Reserve `panic!` only for truly unrecoverable corruption.

---

## Info

### I1. `recv_unbroadcasted_txn` is `unimplemented!()` (L326–328)
Public function that will panic at runtime if called. Dead code or incomplete feature.

**Recommendation:** Either implement or remove from the public interface.

### I2. `block.clone()` on insertion (L488)
```rust
BlockState::Ordered { block: block.clone(), parent_id }
```
`block` is consumed by value in the function signature — this clone is unnecessary. The function takes `block: ExternalBlock` (owned), so it could be moved directly.

**Recommendation:** Replace `block.clone()` with `block` to avoid a potentially expensive clone of the full block payload.

### I3. Typo in panic message (L810)
```
"Commited Block id..."
```
Should be "Committed".

### I4. `wait_for_change` error path is a no-op (L564–567)
```rust
Ok(_) => continue,
Err(_) => continue,
```
Both branches do the same thing, making the `match` redundant. Could be replaced with `let _ = self.wait_for_change(...).await;`.

### I5. `BlockProfile` uses `SystemTime` instead of `Instant`
`SystemTime` is subject to clock adjustments (NTP jumps). For measuring elapsed durations between pipeline stages (L745–748), `Instant` would be monotonically correct.

---

## Summary

| Severity | Count | Key Themes |
|----------|-------|------------|
| **Critical** | 4 | Integer underflow, background task panic, no-op GC watermark, TOCTOU race on init |
| **Warning** | 5 | Memory leak, non-idempotent take, global singleton, silent duplicate acceptance, excessive panics |
| **Info** | 5 | Dead code, unnecessary clone, typo, redundant match, clock source |

The most impactful items to address are **C3** (the no-op GC watermark, which may be the root cause of long-running memory growth) and **C4** (the missed-notification race, which can cause indefinite hangs on startup). **C1** is a latent bug that becomes exploitable only at `block_number == 0`, but the fix is trivial.

### ⚔️ Intention Attacker

# Security Audit: `block_buffer_manager.rs` — Attack Findings

---

## CRITICAL Severity

### 1. Unsigned Integer Underflow in Parent Block Lookup (L460–461)

```rust
let parent_key_current =
    BlockKey::new(block.block_meta.epoch, block.block_meta.block_number - 1);
```

When `block_number == 0`, the expression `block.block_meta.block_number - 1` wraps to `u64::MAX` in Rust release builds (debug builds panic). This creates a `BlockKey` with `block_number = 18446744073709551615`, which will silently miss the lookup and fall through to using the caller-provided `parent_id` — potentially an attacker-controlled or stale value. There is **no guard** checking `block_number > 0` before the subtraction.

**Impact:** A block 0 at the start of any epoch will perform a nonsensical parent lookup. If consensus ever delivers a block with `block_number == 0`, this could associate it with an incorrect parent, breaking the block chain's integrity.

---

### 2. `latest_commit_block_number` Never Updated After Init (L169, L281, L926)

The field `latest_commit_block_number` is set during `init()` and `set_state()`, but **`set_commit_blocks()` never advances it**. When blocks transition `Computed → Committed`, the latest commit block number is never incremented. Only `latest_finalized_block_number` is updated (in `get_committed_blocks`).

This means any component reading `latest_commit_block_number` (exposed via L932–935) receives a **stale value** representing the initial state, not the actual latest committed block. If any downstream logic uses this to make safety decisions (e.g., pruning, fork choice), it operates on dangerously outdated information.

**Impact:** Stale commit tracking can lead to incorrect state assumptions by consumers of this API.

---

### 3. Duplicate Block with Different ID Silently Accepted (L450–456)

```rust
} else {
    warn!(
        "set_ordered_blocks: block {} with epoch {} already exists with different id ...",
        ...
    );
}
return Ok(());
```

When a block arrives at the same `(epoch, block_number)` slot but with a **different `block_id`**, the function logs a warning and **returns `Ok(())`** — silently dropping the conflicting block. This is an **equivocation** signal (two different blocks proposed at the same height), which in a BFT system is a critical safety violation that should be escalated, not silently swallowed.

**Impact:** An equivocating validator's conflicting block is silently discarded. No slashing evidence is produced, no error is returned to the caller. This weakens the BFT accountability guarantees.

---

## HIGH Severity

### 4. TOCTOU Race: `is_ready()` / `is_epoch_change()` Checks vs Lock Acquisition

Multiple functions follow this pattern:

```rust
if !self.is_ready() {           // atomic read (no lock)
    self.ready_notifier.notified().await;
}
// ... later ...
let mut bsm = self.block_state_machine.lock().await;  // lock acquired
```

The `buffer_state` is an `AtomicU8` read **outside** the mutex. Between the atomic check and lock acquisition, another task can call `release_inflight_blocks()` which sets `buffer_state = EpochChange`. The function proceeds as if the buffer is `Ready` when it is actually in `EpochChange`.

`get_ordered_blocks` (L509) does check `is_epoch_change()` after the readiness gate, but this second check is **also outside the lock**, creating a double TOCTOU window. `set_compute_res`, `set_commit_blocks`, and `get_committed_blocks` have **no epoch-change check at all** after the readiness gate.

**Impact:** Operations can execute during an epoch transition when the block map is being pruned by `release_inflight_blocks`, potentially leading to panics (block not found) or operating on stale data.

---

### 5. Background GC Task Panics Crash the Node (L243–247)

```rust
tokio::spawn(async move {
    loop {
        tokio::time::sleep(clone.config.remove_committed_blocks_interval).await;
        clone.remove_committed_blocks().await.unwrap();
    }
});
```

The `.unwrap()` means any error from `remove_committed_blocks()` will **panic inside the spawned task**. While the current implementation of `remove_committed_blocks` always returns `Ok(())`, any future modification that adds an error path will crash the entire node via an unhandled panic in a background task. This is a latent defect with an extremely high blast radius.

**Impact:** A single error in block cleanup kills the node process.

---

### 6. `remove_committed_blocks` No-Op Max Calculation (L259–262)

```rust
let latest_persist_block_num = block_state_machine.latest_finalized_block_number;
block_state_machine.latest_finalized_block_number = std::cmp::max(
    block_state_machine.latest_finalized_block_number,
    latest_persist_block_num,  // same value!
);
```

This computes `max(x, x)` — a guaranteed no-op. This is almost certainly a **copy-paste bug** where the intent was to compare against a *different* value (e.g., `latest_commit_block_number` or a value derived from the retained blocks). As-is, the GC function never advances `latest_finalized_block_number`, meaning it relies entirely on `get_committed_blocks` to do so. If `get_committed_blocks` is not called frequently enough, finalization tracking falls behind.

**Impact:** Dead code that suggests a missing correctness invariant. The GC retention boundary may not advance correctly under all call patterns.

---

### 7. `persist_notifier.take()` Makes Block Persistence One-Shot-Only (L879)

```rust
persist_notifier: persist_notifier.take(),
```

`get_committed_blocks` uses `.take()` to move the `Sender` out of the `Committed` state. If `get_committed_blocks` is called **twice** for the same block range (e.g., due to a retry after a downstream failure), the second call returns blocks with `persist_notifier: None`. The downstream consumer that needs to signal persistence completion will **never receive the sender**, meaning the epoch-change persistence signal is lost.

**Impact:** If the first consumer of the committed block fails and the block is re-fetched, the epoch-change persistence notification channel is permanently broken. This can cause the node to stall indefinitely waiting for a persistence signal that can never arrive.

---

### 8. Unbounded HashMap Growth (L167–168, L171)

The `blocks`, `profile`, and `block_number_to_block_id` HashMaps have no capacity limits. The GC (`remove_committed_blocks`) only runs when `blocks.len() >= max_block_size` (256) and only prunes blocks below `latest_finalized_block_number`. If `latest_finalized_block_number` is not advancing (see finding #6), or if ordered blocks arrive faster than they are committed and finalized, these maps grow without bound.

The `block_number_to_block_id` map is **never pruned** — it only grows in `init()` but is a latent risk if populated with a large historical range.

**Impact:** Unbounded memory growth under sustained load or stalled finalization, eventually leading to OOM and node crash.

---

## MEDIUM Severity

### 9. `release_inflight_blocks` Sets `EpochChange` **After** Pruning Blocks (L974–978)

```rust
block_state_machine
    .blocks
    .retain(|key, _| key.block_number <= latest_epoch_change_block_number);

self.buffer_state.store(BufferState::EpochChange as u8, Ordering::SeqCst);
```

The `buffer_state` is set to `EpochChange` **after** the blocks have already been pruned. Since `buffer_state` is an `AtomicU8` outside the mutex, another task that checks `is_epoch_change()` *between* the retain and the store will see `Ready` but operate on a **pruned block map**. Combined with the TOCTOU in finding #4, this widens the race window.

**Impact:** Brief window where the block map is pruned but the system still reports `Ready` state.

---

### 10. `init_epoch` Bypasses Epoch Transition Protocol (L307–311)

```rust
pub async fn init_epoch(&self, epoch: u64) {
    let mut block_state_machine = self.block_state_machine.lock().await;
    block_state_machine.current_epoch = epoch;
}
```

This directly overwrites `current_epoch` without interacting with `next_epoch`, `buffer_state`, or `latest_epoch_change_block_number`. If called concurrently with or after `calculate_new_epoch_state` has set `next_epoch`, the epoch state becomes inconsistent: `current_epoch` is overwritten, but `next_epoch` still holds the old pending value, which will be applied again by `release_inflight_blocks` — potentially rolling `current_epoch` backward or forward incorrectly.

**Impact:** Epoch state corruption if `init_epoch` races with the normal epoch transition path.

---

### 11. Global Singleton Initialization Without Configuration (lib.rs L9–11)

```rust
pub fn get_block_buffer_manager() -> &'static Arc<BlockBufferManager> {
    GLOBAL_BLOCK_BUFFER_MANAGER.get_or_init(|| {
        BlockBufferManager::new(block_buffer_manager::BlockBufferManagerConfig::default())
    })
}
```

The `OnceLock` global always uses `default()` config. If any caller uses `get_block_buffer_manager()` before the "real" initialization with custom config, the default config is locked in permanently with no way to reconfigure. There is also no indication to the caller that they received a default-configured instance.

**Impact:** Silent misconfiguration if initialization ordering is not carefully controlled.

---

## Summary

| # | Finding | Severity | Type |
|---|---------|----------|------|
| 1 | u64 underflow on `block_number - 1` when `block_number == 0` | **Critical** | Integer Overflow |
| 2 | `latest_commit_block_number` never updated after commit | **Critical** | State Tracking Bug |
| 3 | Equivocating blocks silently dropped without error | **Critical** | Missing Safety Check |
| 4 | TOCTOU between atomic `buffer_state` checks and mutex acquisition | **High** | Race Condition |
| 5 | Background GC task `.unwrap()` crashes node on any error | **High** | Unhandled Panic |
| 6 | `remove_committed_blocks` computes `max(x, x)` — no-op bug | **High** | Logic Error |
| 7 | `persist_notifier.take()` breaks epoch persistence on retry | **High** | One-Shot Resource |
| 8 | Unbounded HashMap growth (blocks, profile) | **High** | Resource Exhaustion |
| 9 | `EpochChange` flag set after block pruning — widens TOCTOU | **Medium** | Race Condition |
| 10 | `init_epoch` bypasses epoch transition protocol | **Medium** | State Corruption |
| 11 | Global singleton locks in default config on first access | **Medium** | Misconfiguration |

---

## [reth_coordinator] Review bin/gravity_node/src/reth_coordinator/mod.rs for: (1) the run() method sp

### 🎯 Intention

Review bin/gravity_node/src/reth_coordinator/mod.rs for: (1) the run() method spawning three independent tokio tasks (start_execution, start_commit_vote, start_commit) with unwrap() — if any task panics the others continue silently; (2) the send_execution_args oneshot channel usage and potential for the receiver to never get data; (3) the block_number_to_block_id mapping correctness during initialization.

### 🔍 Discovered Implementation

## Implementation Analysis: `bin/gravity_node/src/reth_coordinator/mod.rs`

---

### Files/Contracts Involved

| File | Role |
|---|---|
| `bin/gravity_node/src/reth_coordinator/mod.rs` | Defines `RethCoordinator` — spawns three long-running tasks and sends initialization data over a oneshot channel |
| `bin/gravity_node/src/main.rs` | Constructs `RethCoordinator`, creates the oneshot channel, calls `send_execution_args()` then `run()` |
| `bin/gravity_node/src/reth_cli.rs` | Defines `RethCli` with `start_execution()`, `start_commit_vote()`, `start_commit()` — all `async fn -> Result<(), String>` infinite loops |
| `crates/block-buffer-manager/src/lib.rs` | Global singleton `get_block_buffer_manager()` via `OnceLock` |
| `crates/block-buffer-manager/src/block_buffer_manager.rs` | `BlockBufferManager` implementation including `block_number_to_block_id()` and `init()` |
| `greth` (external crate) | Defines `ExecutionArgs` struct and `new_pipe_exec_layer_api()` which consumes the oneshot receiver |

---

### Execution Path

**Startup sequence in `main.rs`:**

1. **Line 252** — `let (execution_args_tx, execution_args_rx) = oneshot::channel();`
2. **Line ~64–130** — `execution_args_rx` is passed into `run_reth()`, which forwards it to `greth::reth_pipe_exec_layer_ext_v2::new_pipe_exec_layer_api()`. This means the receiver is handed off to the reth execution layer **before** the sender fires.
3. **Lines 267–268** — `RethCoordinator::new(client.clone(), latest_block_number, execution_args_tx)` wraps the sender in `Arc<Mutex<Option<oneshot::Sender<ExecutionArgs>>>>`.
4. **Line 299** — `coordinator.send_execution_args().await` is called.
5. **Line 300** — `coordinator.run().await` is called, which spawns three detached tasks.

---

### Key Functions

#### `RethCoordinator::new(reth_cli, _latest_block_number, execution_args_tx) -> Self`
- Wraps `execution_args_tx` in `Arc<Mutex<Option<...>>>`.
- Note: `_latest_block_number` (prefixed with `_`) is **unused** — it is accepted but discarded.

#### `send_execution_args(&self)` (lines 24–38)
1. Locks the mutex guarding the oneshot sender.
2. Calls `.take()` to extract the sender from the `Option` (consuming it; subsequent calls become no-ops).
3. If `Some`, awaits `get_block_buffer_manager().block_number_to_block_id()` — this **blocks until `BlockBufferManager::init()` has been called** (waits on a `Notify`).
4. Converts each `(u64, BlockId)` entry to `(u64, B256)` via `B256::new(block_id.bytes())`.
5. Sends `ExecutionArgs { block_number_to_block_id }` on the oneshot channel, calling `.unwrap()` on the send result.

#### `run(&self)` (lines 40–53)
Spawns three **independent, detached** `tokio::spawn` tasks:
- `reth_cli.start_execution().await.unwrap()`
- `reth_cli.start_commit_vote().await.unwrap()`
- `reth_cli.start_commit().await.unwrap()`

Each task clones the `Arc<RethCli>` and runs in its own spawned future.

#### `RethCli::start_execution(&self) -> Result<(), String>` (reth_cli.rs:309–370)
- Infinite loop reading ordered blocks from `BlockBufferManager`, pushing them to the execution pipeline.
- Handles epoch changes by calling `consume_epoch_change()`.
- Exits cleanly on shutdown signal. Returns `Err` from `recover_block_number()` or `push_ordered_block()`.

#### `RethCli::start_commit_vote(&self) -> Result<(), String>` (reth_cli.rs:372–439)
- Infinite loop receiving compute results via `recv_compute_res()`.
- Tracks consecutive errors; returns `Err` after 5 consecutive `recv_compute_res()` failures.
- Exits cleanly on shutdown signal.

#### `RethCli::start_commit(&self) -> Result<(), String>` (reth_cli.rs:441–499)
- Infinite loop reading committed blocks from `BlockBufferManager`.
- Contains a `panic!` path: `self.pipe_api.get_block_id(last_block.num).unwrap_or_else(|| panic!(...))` — hard crash if a committed block number has no corresponding block ID.
- Contains `assert_eq!` on block ID match — another potential panic.
- Exits cleanly on shutdown signal.

---

### Findings on the Three Specific Areas

#### (1) `run()` — Three Independent Tasks with `unwrap()`

**What happens:** Each of the three `tokio::spawn` blocks calls `.unwrap()` on the `Result<(), String>` returned by the respective method. All three methods are infinite loops that return `Result<(), String>`.

**Behavior on failure:**
- If any method returns `Err(String)`, the `.unwrap()` inside the spawned task causes that **task** to panic.
- A panicked `tokio::spawn` task does **not** propagate the panic to the spawning runtime or to other tasks. The `JoinHandle` returned by `tokio::spawn` is **discarded** (not awaited, not stored), so the panic is entirely unobserved.
- The remaining two tasks continue running with no awareness that a sibling has died.
- `run()` itself returns immediately after spawning (it does not await the `JoinHandle`s), so the caller has no mechanism to detect task failure.

**Additional panic paths within the tasks (beyond `Err` → `unwrap()`):**
- `start_commit` line 466: `self.pipe_api.get_block_id(...).unwrap_or_else(|| panic!(...))` — panics if a committed block has no block ID.
- `start_commit` contains `assert_eq!` — panics on block ID mismatch.
- These panics are also isolated within the spawned task.

#### (2) `send_execution_args` Oneshot Channel

**Channel lifecycle:**
1. Channel created at `main.rs:252`.
2. **Receiver** passed to `run_reth()` → `new_pipe_exec_layer_api()` early in startup.
3. **Sender** passed to `RethCoordinator::new()`, wrapped in `Arc<Mutex<Option<...>>>`.
4. `send_execution_args()` called at `main.rs:299`.

**Potential for receiver to never get data:**
- `send_execution_args()` awaits `block_number_to_block_id()`, which blocks on `BlockBufferManager::init()` completing. If `init()` is never called, this method hangs indefinitely — the receiver never receives.
- The `.take()` pattern means the sender is consumed on first call. If `send_execution_args()` were called before the `BlockBufferManager` is ready and some other failure interrupted it between the `.take()` and the `.send()`, the sender would be dropped without sending, and the receiver would get a `RecvError`.
- The `.unwrap()` on `execution_args_tx.send(execution_args)` at line 36: `oneshot::Sender::send()` returns `Err` if the receiver has been dropped. If the reth execution layer dropped the receiver before this point, this `.unwrap()` panics.

**In normal flow:** `send_execution_args()` is called **before** `run()` at `main.rs:299–300`, so the data is sent before the execution tasks begin. The sequencing is: send init data → spawn workers.

#### (3) `block_number_to_block_id` Mapping Correctness During Initialization

**Data flow:**
1. `BlockBufferManager::init()` receives `block_number_to_block_id_with_epoch: HashMap<u64, (u64, BlockId)>` from persisted storage.
2. `init()` strips the epoch, storing `HashMap<u64, BlockId>` in `block_state_machine.block_number_to_block_id`.
3. `block_number_to_block_id()` waits for `init()` to complete, then **clones** the entire map.
4. `send_execution_args()` converts `BlockId` → `B256` via `B256::new(block_id.bytes())` — a direct byte-level copy of the 32-byte identifier.

**Key observations about the mapping:**
- The `block_number_to_block_id` map is populated **only** during `init()`. It is never updated after initialization — new blocks flowing through the Ordered → Computed → Committed pipeline are tracked in a separate `blocks: HashMap<BlockKey, BlockState>` structure.
- The map represents a **snapshot of historical state at startup**. It contains all block numbers and their consensus-assigned block IDs that were persisted before the node restarted.
- The conversion from `BlockId` (consensus layer, `[u8; 32]`) to `B256` (alloy/reth layer, also `[u8; 32]`) is a direct 1:1 byte copy — no transformation, hashing, or reinterpretation occurs.
- If `init()` is called with an empty `block_number_to_block_id_with_epoch`, the map sent to the execution layer will be empty — this is handled (the `if !block_number_to_block_id_with_epoch.is_empty()` guard in `init()` only applies to inserting into `blocks`, not to the map itself).

---

### State Changes

| Operation | State Modified |
|---|---|
| `RethCoordinator::new()` | Wraps oneshot sender in `Arc<Mutex<Option<...>>>` |
| `send_execution_args()` | `.take()` consumes the sender from the `Option` (sets it to `None`), sends `ExecutionArgs` over the oneshot channel |
| `run()` | Spawns 3 detached tokio tasks; no direct state mutation |
| `start_execution()` | Pushes ordered blocks into the reth execution pipeline via `push_ordered_block()` |
| `start_commit_vote()` | Writes compute results to `BlockBufferManager` via `set_compute_res()` |
| `start_commit()` | Reads committed blocks, calls `send_committed_block_info()`, updates `BlockBufferManager` state via `set_state()`, waits for block persistence |

### External Dependencies

| Dependency | Usage |
|---|---|
| `greth::reth_pipe_exec_layer_ext_v2` | `ExecutionArgs` struct definition; `new_pipe_exec_layer_api()` consumes the oneshot receiver |
| `block_buffer_manager` (internal crate) | Global singleton providing block lifecycle state machine |
| `tokio::sync::oneshot` | One-shot channel for passing `ExecutionArgs` from coordinator to execution layer |
| `tokio::sync::Mutex` | Async mutex guarding the `Option<Sender>` |
| `alloy_primitives::B256` | 32-byte hash type for block ID representation in the reth layer |

### 🛠️ Ground Review

# Ground Review: `reth_coordinator/mod.rs` & Related Files

## Code Quality & Engineering Safety Audit

---

### Critical

#### 1. Unobserved Task Panics — Silent Half-Dead Node
**File:** `reth_coordinator/mod.rs:40-53`

```rust
tokio::spawn(async move {
    reth_cli.start_execution().await.unwrap();
});
```

All three `JoinHandle`s from `tokio::spawn` are **discarded**. If any task panics (via the `.unwrap()` or the explicit `panic!`/`assert_eq!` paths in `start_commit`), the failure is entirely unobserved. The remaining two tasks continue operating on a node that is now partially broken — e.g., blocks keep executing but never commit, or commit votes succeed but commits don't land.

This is the most dangerous pattern in the file: **a node can silently enter a degraded state with no alerting, no restart, and no log output indicating which subsystem died.**

**Recommendation:** Store the `JoinHandle`s, `tokio::select!` over them, and trigger a coordinated shutdown (or at minimum log + abort) when any task exits.

---

#### 2. Explicit `panic!` and `assert_eq!` in Production Path
**File:** `reth_cli.rs:466-469`

```rust
let block_id = self.pipe_api.get_block_id(last_block.num).unwrap_or_else(|| {
    panic!("commit num {} not found block id", start_commit_num);
});
assert_eq!(ExternalBlockId::from_bytes(block_id.as_slice()), last_block.block_id);
```

These are **hard panics in an infinite-loop production path**, not test code. Combined with finding #1, a panic here silently kills the commit task while execution and voting continue — causing state divergence. Panics should be converted to `Err(...)` returns that propagate through the `Result` return type already declared on `start_commit`.

---

#### 3. `unwrap()` on Oneshot Send — Unguarded Crash
**File:** `reth_coordinator/mod.rs:36`

```rust
execution_args_tx.send(execution_args).unwrap();
```

`oneshot::Sender::send()` returns `Err` if the receiver has been dropped. If the reth execution layer thread panics or exits before this point (e.g., a bad chain spec, DB open failure), this `.unwrap()` crashes the entire coordinator thread with no diagnostic context. Should be handled with a meaningful error message or graceful shutdown.

---

### Warning

#### 4. Potential Indefinite Hang in `send_execution_args`
**File:** `reth_coordinator/mod.rs:28-30`

```rust
let block_number_to_block_id = get_block_buffer_manager()
    .block_number_to_block_id()
    .await
```

This `.await` blocks until `BlockBufferManager::init()` completes (internally waits on a `Notify`). There is **no timeout**. If `init()` is never called (e.g., consensus engine fails to start at `main.rs:284-297`), the node hangs forever at line 299 with no log output explaining why. A `tokio::time::timeout` with a diagnostic message would make debugging significantly easier.

---

#### 5. `shutdown` Receiver Is Not `Clone` — Resubscribe on Every Loop Iteration
**File:** `reth_cli.rs:323, 378, 449`

```rust
let mut shutdown = self.shutdown.resubscribe();
```

Each loop iteration calls `.resubscribe()`, creating a new receiver. This works because `broadcast::Receiver` is not `Clone`, but it means any shutdown signal sent *between* the previous `recv` and the next `resubscribe` is lost (lagged). In practice the loop body is fast enough that this is unlikely to cause issues, but it is a subtle correctness concern. A cleaner pattern would be to hold a single receiver across iterations and only resubscribe on `RecvError::Lagged`.

---

#### 6. Unused Constructor Parameter
**File:** `reth_coordinator/mod.rs:18`

```rust
_latest_block_number: u64,
```

Accepted and silently discarded. The caller (`main.rs:268`) computes and passes this value. If it's not needed, remove it from the API to avoid confusion. If it will be needed later, that's speculative code.

---

#### 7. Typo in Variable Name
**File:** `reth_cli.rs:103`

```rust
let chian_info = args.provider.chain_spec().chain;
```

`chian_info` → `chain_info`. Minor, but this is in a constructor that every reader will encounter.

---

### Info

#### 8. `Arc<Mutex<Option<oneshot::Sender>>>` Is Over-Engineered
**File:** `reth_coordinator/mod.rs:12`

The `Arc<Mutex<Option<...>>>` wrapper around a oneshot sender that is used exactly once, from a single call site (`send_execution_args`), with no concurrent access, is unnecessary complexity. Since `send_execution_args` is called once before `run()` (sequentially at `main.rs:299-300`), the sender could be consumed via an `Option` field with a `&mut self` method, or simply passed as an argument.

#### 9. Logging the Entire `block_number_to_block_id` Map
**File:** `reth_coordinator/mod.rs:34`

```rust
info!("send_execution_args block_number_to_block_id: {:?}", block_number_to_block_id);
```

On a node recovering from a long history, this map could contain thousands of entries. This will produce a multi-megabyte log line at `info` level. Should be `debug!` or log only the count at `info`.

#### 10. `_pool` and `_txn_listener` and `_auth` Fields on `RethCli`
**File:** `reth_cli.rs:74, 78, 79, 81`

Four fields prefixed with `_` indicate they exist solely to hold ownership (prevent drops). This is fine for resource-holding, but worth a comment explaining why, as it's not obvious whether these are TODO placeholders or intentional RAII anchors.

---

## Summary

| Severity | Count | Key Themes |
|----------|-------|------------|
| **Critical** | 3 | Silent task death, production panics, unguarded oneshot send |
| **Warning** | 4 | Indefinite hang, broadcast resubscribe semantics, dead parameter, typo |
| **Info** | 3 | Over-wrapped types, verbose logging, undocumented ownership fields |

The most impactful fix is **Critical #1 + #2 together**: store `JoinHandle`s, select over them, and convert the `panic!`/`assert_eq!` in `start_commit` to error returns. This eliminates the primary failure mode where a node silently degrades into a half-functioning state.

### ⚔️ Intention Attacker

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

---

## [consensus_engine_initialization] Review crates/api/src/consensus_api.rs for: (1) the ConsensusEngine::init method

### 🎯 Intention

Review crates/api/src/consensus_api.rs for: (1) the ConsensusEngine::init method's complex multi-runtime initialization sequence for ordering dependencies and potential partial initialization failures; (2) the GLOBAL_CONFIG_STORAGE OnceLock initialization and panic on double-set; (3) the NetworkBuilder creation with role type handling (FIXME for VFN); (4) the HTTPS server gating behind #[cfg(debug_assertions)] — verify this is intentional and no sensitive endpoints leak in release; (5) the event_subscription_service initialization ordering; (6) the jemalloc global allocator configuration.

### 🔍 Discovered Implementation

## Implementation Analysis: `crates/api/src/consensus_api.rs`

---

### 1. Global Allocator Configuration (Line 37–39)

```rust
#[cfg(unix)]
#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;
```

- **Scope**: Unix platforms only (`#[cfg(unix)]`).
- **Effect**: Replaces the system allocator with jemalloc for the entire process. This is a static declaration — no runtime configuration (arena count, dirty page decay, etc.) is applied. Default jemalloc tuning parameters are used.

---

### 2. `ConsensusEngine::init` — Full Initialization Sequence (Lines 107–363)

The method is `pub async fn init(args: ConsensusEngineArgs, pool: Box<dyn TxPool>) -> Arc<Self>`. It performs a long, strictly sequential initialization with no rollback on partial failure. Each step that fails will panic, leaving prior runtimes running in the `runtimes` vec with no cleanup.

#### Step-by-step execution path:

| Step | Lines | Operation | Failure Mode |
|------|-------|-----------|-------------|
| 1 | 111 | Install panic handler via `setup_panic_handler()` | — |
| 2 | 113 | `fail_point_check(&node_config)` — if failpoints feature is compiled in and config has entries, calls `fail::cfg()` which **panics** on error | `panic!` |
| 3 | 114–115 | Create `ConsensusDB` on disk at `node_config.storage.dir()` | Panics on DB open failure (inside `ConsensusDB::new`) |
| 4 | 116 | `init_peers_and_metadata` — extracts network IDs, creates `PeersAndMetadata` | — |
| 5 | 117–118 | Create logger with optional file output | — |
| 6 | 120–128 | `start_telemetry_service` — if it returns a runtime, push to `runtimes` | — |
| 7 | 129 | Wrap `consensus_db` as `DbReaderWriter` | — |
| 8 | 130–133 | Create `EventSubscriptionService` wrapping the DB in `RwLock` | — |
| 9 | 135–142 | **GLOBAL_CONFIG_STORAGE OnceLock set** (see §3 below) | `panic!` on double-set |
| 10 | 143 | Convert `chain_id: u64` → `ChainId` | — |
| 11 | 144 | `extract_network_configs` — collects fullnode networks + validator network; **panics** if validator network has `mutual_authentication == false` (`network.rs:42–44`) | `panic!` |
| 12 | 151–226 | **Network loop** — for each network config: create runtime, create `NetworkBuilder` (with VFN role override — see §4), register JWK/DKG/consensus/mempool handles, build and start network | `panic!` if >1 validator network detected (line 178) |
| 13 | 229–253 | Transform network handles → `ApplicationNetworkInterfaces` | — |
| 14 | 259–262 | Create consensus notifier/listener pair | — |
| 15 | 265 | Start node inspection service (spawns thread) | — |
| 16 | 266–267 | Create mpsc channels (capacity 1 each) for consensus→mempool and notification | — |
| 17 | 274–285 | `init_mempool` — subscribes to reconfigurations via `event_subscription_service`, creates `CoreMempool`, bootstraps mempool runtime | — |
| 18 | 288 | Create default `VTxnPoolState` | — |
| 19 | 291–299 | `create_dkg_runtime` — only for validators; subscribes to reconfig + DKG start events, starts DKG runtime | — |
| 20 | 300–308 | `init_jwk_consensus` — only if JWK interfaces exist; subscribes to reconfig + JWK updated events | — |
| 21 | 309 | `init_block_buffer_manager` — async, reads block history from DB for last 256 blocks, populates block buffer manager | `.unwrap()` on DB range query (bootstrap.rs:292) |
| 22 | 310–321 | `start_consensus` — subscribes to reconfigurations, starts consensus provider runtime | — |
| 23 | 324–337 | Create `ConsensusToMempoolHandler`, spawn on dedicated "Con2Mempool" runtime | — |
| 24 | 341–357 | **HTTPS server** — gated behind `#[cfg(debug_assertions)]` (see §5) | — |
| 25 | 358 | Wrap all runtimes in `Arc<ConsensusEngine>` | — |
| 26 | 361 | **`notify_initial_configs(latest_block_number)`** — triggers initial config notification to all event subscribers. This is the final step; all subscriptions must be registered before this call | Result is discarded with `let _` |

---

### 3. GLOBAL_CONFIG_STORAGE OnceLock (Lines 135–142)

```rust
if let Some(config) = config_storage {
    match GLOBAL_CONFIG_STORAGE.set(config) {
        Ok(_) => {}
        Err(_) => {
            panic!("Failed to set config storage")
        }
    }
}
```

- `GLOBAL_CONFIG_STORAGE` is a `OnceLock<Arc<dyn ConfigStorage>>` (imported from `gaptos::api_types::config_storage`).
- If `config_storage` is `Some`, it attempts to set the global. If the `OnceLock` was already initialized (e.g., from a prior call), `set()` returns `Err` and the code **panics unconditionally**.
- If `config_storage` is `None`, the global is never set — any downstream code that calls `GLOBAL_CONFIG_STORAGE.get()` will receive `None`.
- The comment on line 134 reads `"It seems stupid, refactor when debugging finished"`, indicating this is acknowledged as provisional code.
- There is no guard or check before calling `set()` — calling `ConsensusEngine::init` twice with `Some(config_storage)` in the same process will panic on the second call.

---

### 4. NetworkBuilder VFN Role Override (Lines 161–174)

```rust
let mut network_builder = NetworkBuilder::create(
    chain_id,
    if network_id.is_vfn_network() {
        // FIXME(nekomoto): This is a temporary solution to support block sync for
        // validator node which is not the current epoch validator.
        RoleType::FullNode
    } else {
        node_config.base.role
    },
    ...
);
```

- For VFN (Validator Full Node) networks, the role is **hardcoded** to `RoleType::FullNode` regardless of `node_config.base.role`.
- For all other networks (validator, public fullnode), the configured `node_config.base.role` is used.
- The FIXME comment states this is a temporary workaround: when a validator node is not in the current epoch's validator set, it needs to sync blocks via the VFN network as a full node.
- This means a single node process can present different roles on different networks simultaneously.

---

### 5. HTTPS Server `#[cfg(debug_assertions)]` Gating (Lines 341–357)

```rust
#[cfg(debug_assertions)]
{
    let https_config = prepare_https_server_config(&node_config, consensus_db.clone());
    if !https_config.address.is_empty() {
        let runtime = gaptos::aptos_runtimes::spawn_named_runtime("Http".into(), None);
        runtime.spawn(async move {
            https_server(
                https_config.address,
                https_config.cert_pem,
                https_config.key_pem,
                https_config.consensus_db,
            ).await
        });
        runtimes.push(runtime);
    }
}
```

**The entire HTTPS server block is compile-time excluded from release builds.** The `#[cfg(debug_assertions)]` attribute means this code only exists when compiling in debug mode (i.e., without `--release`).

The comment on lines 338–340 documents the intent:
> *"The HTTP/HTTPS API server exposes consensus/DKG endpoints, failpoint injection, and heap profiling. None of these are needed in production. Gate the entire server behind debug_assertions so it is not started in release builds."*

**Endpoints exposed in debug builds** (from `https/mod.rs`):

| Route | Method | Handler | TLS-Required |
|-------|--------|---------|-------------|
| `/tx/submit_tx` | POST | `submit_tx` | Yes (HTTPS-only via `ensure_https` middleware) |
| `/tx/get_tx_by_hash/:hash_value` | GET | `get_tx_by_hash` | Yes |
| `/dkg/status` | GET | `get_dkg_status` | No |
| `/dkg/randomness/:block_number` | GET | `get_randomness` | No |
| `/consensus/latest_ledger_info` | GET | `get_latest_ledger_info` | No |
| `/consensus/ledger_info/:epoch` | GET | `get_ledger_info_by_epoch` | No |
| `/consensus/block/:epoch/:round` | GET | `get_block` | No |
| `/consensus/qc/:epoch/:round` | GET | `get_qc` | No |
| `/consensus/validator_count/:epoch` | GET | `get_validator_count_by_epoch` | No |
| `/set_failpoint` | POST | `set_failpoint` | No |
| `/mem_prof` | POST | `control_profiler` | No |

The HTTPS-only enforcement (via `ensure_https` middleware at `mod.rs:106`) only applies to `/tx/*` routes. All other routes including `/set_failpoint` and `/mem_prof` are served over plain HTTP.

Additionally, per `GSDK-013` (line 118–124): the `/tx/*` routes are **only registered when TLS certs are configured**. Without TLS certs, only `http_routes` are served.

---

### 6. `event_subscription_service` Initialization Ordering (Lines 130–361)

The `EventSubscriptionService` is created at line 130 and passed by `&mut` reference through the initialization chain. Subscriptions are registered **before** `notify_initial_configs` is called:

**Subscription registration order:**
1. **Mempool** (line 245 in `bootstrap.rs`) — `subscribe_to_reconfigurations()`
2. **DKG** (line 109–114 in `bootstrap.rs`) — `subscribe_to_reconfigurations()` + `subscribe_to_events(DKGStartEvent)`
3. **JWK Consensus** (line 219–224 in `bootstrap.rs`) — `subscribe_to_reconfigurations()` + `subscribe_to_events(ObservedJWKsUpdated)`
4. **Consensus** (line 157–158 in `bootstrap.rs`) — `subscribe_to_reconfigurations()`
5. **NetworkBuilder** also receives `Some(&mut event_subscription_service)` during network creation (line 172) for internal subscriptions.

**Trigger point (line 361):**
```rust
let _ = event_subscription_service.lock().await.notify_initial_configs(latest_block_number);
```

- The service is wrapped in `Arc<Mutex<_>>` at line 327 — after all `&mut` borrows are complete.
- `notify_initial_configs` fires the initial configuration to all registered subscribers.
- The `let _` discards the `Result`, meaning errors from this notification are silently ignored.

---

### 7. Partial Initialization Failure Characteristics

The `runtimes` vector accumulates tokio `Runtime` instances as initialization progresses. If a panic occurs mid-initialization:

- **Runtimes already pushed** to the vec are not explicitly shut down — they are dropped when the vec goes out of scope during unwinding, which triggers `Runtime::drop` (blocking shutdown).
- **Network builders** that have called `.build()` and `.start()` (line 223–224) have already spawned background tasks on their runtimes.
- **Event subscriptions** registered before the failure point remain subscribed but will never receive `notify_initial_configs`.
- **ConsensusDB** remains open on disk.
- **GLOBAL_CONFIG_STORAGE** once set cannot be unset — subsequent retry in the same process will panic.

---

### Files Involved

| File | Role |
|------|------|
| `crates/api/src/consensus_api.rs` | Main `ConsensusEngine::init` orchestrator |
| `crates/api/src/bootstrap.rs` | Subsystem initialization functions (mempool, DKG, JWK, consensus, block buffer manager, peers) |
| `crates/api/src/network.rs` | Network config extraction, runtime creation, client/service registration, interface construction |
| `crates/api/src/https/mod.rs` | HTTP/HTTPS server with route definitions, TLS configuration, `ensure_https` middleware |
| `crates/api/src/https/set_failpoints.rs` | Failpoint injection endpoint |
| `crates/api/src/https/heap_profiler.rs` | Heap profiling control endpoint |
| `crates/api/src/https/tx.rs` | Transaction submission and query endpoints |
| `crates/api/src/https/consensus.rs` | Consensus state query endpoints |
| `crates/api/src/https/dkg.rs` | DKG status and randomness query endpoints |

### State Changes

- **Disk**: `ConsensusDB` opened/created at `node_config.storage.dir()`
- **Global static**: `GLOBAL_CONFIG_STORAGE` OnceLock set (irreversible within process)
- **Global static**: jemalloc installed as global allocator (compile-time, Unix only)
- **Process-wide**: Panic handler installed via `setup_panic_handler`
- **In-memory**: Block buffer manager initialized with recent 256 blocks of history
- **Network**: Multiple tokio runtimes spawned, network listeners bound to configured addresses

### 🛠️ Ground Review

# Ground Review (Blue Team) — Code Quality & Engineering Safety Audit

**Scope**: `crates/api/src/consensus_api.rs`, `crates/api/src/bootstrap.rs`, `crates/api/src/network.rs`, `crates/api/src/https/mod.rs`

---

## Critical

### C1. No Cleanup on Partial Initialization Failure
**File**: `consensus_api.rs:107–363`

`ConsensusEngine::init` accumulates tokio `Runtime` instances into a `Vec<Runtime>` across ~25 sequential steps. Any panic mid-sequence leaves already-started runtimes (with active network listeners, spawned tasks, and open DB handles) to unwind via `Drop` in an undefined order. Critically:

- Network builders that have called `.build()` + `.start()` (line 223–224) have already bound ports and spawned background tasks.
- `GLOBAL_CONFIG_STORAGE` (a `OnceLock`) is irreversible — a retry in the same process will panic unconditionally (line 139).
- `ConsensusDB` remains open on disk with no explicit close path.

**Recommendation**: Introduce a builder/guard pattern that tracks initialization stages and implements `Drop` for orderly teardown, or at minimum wrap the entire init in a catch-unwind boundary that shuts down accumulated runtimes.

---

### C2. Silently Discarded Error from `notify_initial_configs`
**File**: `consensus_api.rs:361`

```rust
let _ = event_subscription_service.lock().await.notify_initial_configs(latest_block_number);
```

The `Result` from notifying all event subscribers of initial configuration is silently discarded with `let _`. If this fails, subsystems (mempool, DKG, JWK consensus, consensus itself) will never receive their initial configuration and will sit idle without any error signal.

**Recommendation**: At minimum, log the error. Ideally, propagate it or panic, since a failed initial notification means the node is non-functional.

---

### C3. `.unwrap()` on DB Range Query During Bootstrap
**File**: `bootstrap.rs:292`

```rust
.get_range_with_filter::<BlockNumberSchema, _>(&start_key, &end_key, ...)
.unwrap()
```

A corrupted or partially-written database will panic the node during `init_block_buffer_manager`. This is called from the main init path (consensus_api.rs:309) and has no recovery mechanism.

**Recommendation**: Return a `Result` and handle the error gracefully (e.g., log and attempt recovery from an earlier epoch, or surface the error to the caller).

---

## Warning

### W1. `GLOBAL_CONFIG_STORAGE` OnceLock with Panic-on-Double-Set
**File**: `consensus_api.rs:134–142`

```rust
// It seems stupid, refactor when debugging finished
```

The author's own comment acknowledges this is provisional. Calling `ConsensusEngine::init` twice with `Some(config_storage)` in the same process (e.g., in tests or after a restart attempt) unconditionally panics. The `OnceLock` cannot be unset.

**Recommendation**: Use `GLOBAL_CONFIG_STORAGE.get_or_init(|| config)` if idempotency is acceptable, or document the single-init invariant with a compile-time or runtime guard that provides a clear error message. Remove the debug comment.

---

### W2. `ensure_https` Middleware Does Not Actually Enforce HTTPS
**File**: `https/mod.rs:32–36`

```rust
async fn ensure_https(req: Request<Body>, next: Next) -> Response {
    if req.uri().scheme_str() != Some("https") {
        return Response::builder().status(400).body("HTTPS required".into()).unwrap();
    }
    next.run(req).await
}
```

In typical reverse-proxy or direct axum deployments, `req.uri().scheme_str()` returns `None` (not `Some("http")`) because the scheme is not populated for incoming requests at the application layer. This means the middleware will reject **all** requests, including legitimate HTTPS ones handled by the TLS termination layer below. The test at line 229 succeeds only because `reqwest` sends a full-form URI over HTTPS.

**Recommendation**: Verify this middleware works correctly in production. A more reliable approach is to check the connection info (e.g., `ConnectInfo`) or rely on the fact that when `axum_server::bind_rustls` is used, all connections are TLS by definition — making the middleware unnecessary.

---

### W3. `mpsc` Channels with Capacity 1 — Back-Pressure Risk
**File**: `consensus_api.rs:266–267`

```rust
let (consensus_to_mempool_sender, consensus_to_mempool_receiver) = mpsc::channel(1);
let (notification_sender, notification_receiver) = mpsc::channel(1);
```

Both channels have a buffer of 1. Under load, the consensus-to-mempool path will block if the mempool cannot consume fast enough, potentially stalling consensus. Similarly, the notification channel can back-pressure the notifier.

**Recommendation**: Evaluate whether capacity 1 is intentional for flow control or an oversight. Document the reasoning. Consider increasing the buffer or using an unbounded channel if blocking consensus is unacceptable.

---

### W4. VFN Role Override — Undocumented Dual-Role Behavior
**File**: `consensus_api.rs:161–174`

A single node process presents `RoleType::FullNode` on VFN networks and `node_config.base.role` (typically `Validator`) on other networks simultaneously. The `FIXME` comment confirms this is a "temporary solution." This creates a split-brain role identity that may confuse peer authentication, metrics, or role-gated logic downstream.

**Recommendation**: Track this as tech debt. Add a TODO issue reference. At minimum, add a log line when the role override activates so operators can observe the behavior.

---

### W5. `init_block_buffer_manager` Scans All Epochs Unconditionally
**File**: `bootstrap.rs:279–312`

```rust
for epoch_i in (1..=max_epoch).rev() {
```

This iterates over **every epoch** in the database in reverse order, performing a range scan on each. For a long-running chain with many epochs, this is an unbounded O(epochs × blocks-per-epoch) operation at startup. The commented-out early break (`has_large`) suggests an optimization was planned but disabled.

**Recommendation**: Re-enable the early break or limit iteration to epochs that could contain blocks in the `[start_block_number, latest_block_number]` range. The current code scans epoch 1 even if `start_block_number` is millions of blocks higher.

---

## Info

### I1. Debug Comment Left in Production Code
**File**: `consensus_api.rs:134`

```rust
// It seems stupid, refactor when debugging finished
```

Unprofessional comment that should be cleaned up regardless of refactoring plans.

---

### I2. Commented-Out Code in `init_block_buffer_manager`
**File**: `bootstrap.rs:280–281, 295, 309–311`

Multiple commented-out lines (`has_large`, early break) indicate incomplete refactoring. Dead commented code adds noise and confusion about intended behavior.

---

### I3. Typo in Comment
**File**: `consensus_api.rs:256`

```rust
// The consensus_listener would listenes the request...
```

"listenes" → "listen to"

---

### I4. Unused Variable Naming Convention
**File**: `consensus_api.rs:273`

```rust
let (_mempool_client_sender, _mempool_client_receiver) = mpsc::channel(1);
```

`_mempool_client_receiver` is immediately passed to `init_mempool` at line 279, so the `_` prefix is misleading — it suggests the variable is unused when it is actually consumed. `_mempool_client_sender` is truly unused and silently dropped, closing the send side of the channel.

---

### I5. Test Creates Files Without Cleanup on Failure
**File**: `https/mod.rs:198–200`

```rust
fs::create_dir(dir.clone() + "/src/https/test").unwrap();
fs::write(dir.clone() + "/src/https/test/cert.pem", cert_pem).unwrap();
```

The test creates files under the source tree and has no cleanup (`Drop` guard or `defer`). If the test panics after directory creation but before completion, stale cert files remain in the source tree.

---

### I6. `node_config.clone()` Passed to `create_dkg_runtime` as Mutable
**File**: `consensus_api.rs:292`

```rust
let dkg_runtime = create_dkg_runtime(
    &mut node_config.clone(),
    ...
);
```

A full clone of `NodeConfig` is created just to pass as `&mut`. Any mutations made by `create_dkg_runtime` are discarded. If mutation is unnecessary, change the signature to `&NodeConfig`. If mutation is needed, the clone-and-discard pattern hides the intent.

---

## Summary

| Severity | Count | Key Themes |
|----------|-------|-----------|
| **Critical** | 3 | No rollback on partial init failure, silenced errors, unwrap on DB reads |
| **Warning** | 5 | OnceLock double-set panic, HTTPS enforcement ineffective, channel back-pressure, dual-role identity, unbounded epoch scan |
| **Info** | 6 | Dead comments, naming, test hygiene, unnecessary clone |

### ⚔️ Intention Attacker

# Intention Attack Report: `crates/api/src/consensus_api.rs` and Related Subsystems

---

## CRITICAL Severity

### 1. `submit_tx` is a `todo!()` — Unimplemented Endpoint Panics the Node Process

**File:** `crates/api/src/https/tx.rs:28`

```rust
pub async fn submit_tx(_request: TxRequest) -> Result<JsonResponse<SubmitResponse>, StatusCode> {
    todo!()
}
```

**Attack:** In debug builds, the `/tx/submit_tx` endpoint is registered and reachable. Any POST to this endpoint will execute `todo!()`, which **panics the async task**. Depending on how the runtime is configured, this could crash the entire "Http" tokio runtime or propagate via the `JoinHandle`. Since the HTTP runtime is shared, a single crafted request kills the debug server.

More critically, the `TxRequest` struct accepts **arbitrary `Vec<u8>` with no authentication**:

```rust
pub struct TxRequest {
    tx: Vec<u8>,
    //    Public key and signature to authenticate
    //    authenticator: (),
}
```

The commented-out authenticator field means there is **zero authentication on transaction submission** even when implemented. This is a placeholder with no access control design.

**Impact:** DoS on debug nodes (immediate). When eventually implemented without the commented authenticator, unauthenticated transaction injection into the consensus pipeline.

---

### 2. `notify_mempool_of_committed_transactions` Error Path Calls `todo!()` — Unrecoverable Crash on Mempool Notification Failure

**File:** `crates/api/src/consensus_mempool_handler.rs:40-41`

```rust
if let Err(_error) = result {
    todo!()
}
```

**Attack:** If the mempool notification sender ever returns an error (network backpressure, channel closed, resource exhaustion), the `ConsensusToMempoolHandler` task **panics**. This is on the critical consensus-to-mempool commit notification path. Once this task dies:

- **Committed transactions are never communicated to mempool** for garbage collection.
- The mempool fills with already-committed transactions, causing memory exhaustion.
- New blocks continue to be proposed with already-committed transactions (duplication).
- The `consensus_notification_listener` is dropped, breaking the consensus commit acknowledgement channel — consensus may stall waiting for commit ACKs.

**Impact:** A single transient mempool notification error permanently kills the consensus→mempool pipeline, leading to mempool corruption, consensus stall, and potential chain halt.

---

### 3. `SyncForDuration` Handler Calls `todo!()` — Consensus Sync Path Crashes the Node

**File:** `crates/api/src/consensus_mempool_handler.rs:111-113`

```rust
ConsensusNotification::SyncForDuration(_consensus_sync_duration_notification) => {
    todo!()
}
```

**Attack:** If consensus ever sends a `SyncForDuration` notification (e.g., during epoch transitions or network partitioning recovery), the handler panics and kills the `ConsensusToMempoolHandler` task. This has the same cascading effects as finding #2 above — total consensus pipeline breakdown.

**Impact:** Any legitimate `SyncForDuration` event permanently crashes the consensus→mempool handler.

---

### 4. Silently Discarded Error on `notify_initial_configs` — Subscribers May Never Receive Initial State

**File:** `crates/api/src/consensus_api.rs:361`

```rust
let _ = event_subscription_service.lock().await.notify_initial_configs(latest_block_number);
```

**Attack:** All event subscribers (mempool, DKG, JWK consensus, consensus itself) depend on `notify_initial_configs` to bootstrap their initial configuration state. If this call fails, the error is silently discarded with `let _`. Subscribers start in an **uninitialized state** — they have subscriptions registered but never received their initial configuration.

**Consequences:**
- **DKG** may not know the current validator set, failing to participate in randomness generation.
- **Mempool** may not know current gas parameters or validator membership.
- **Consensus** may not know the current epoch's validator set.
- The node appears healthy (no panic, no error log) but is **functionally broken**.

**Impact:** Silent node malfunction. The node joins the network but cannot correctly participate in consensus, potentially causing liveness failures if enough validators are affected.

---

## HIGH Severity

### 5. `GLOBAL_CONFIG_STORAGE` OnceLock Makes Node Non-Restartable Within Process

**File:** `crates/api/src/consensus_api.rs:135-142`

```rust
if let Some(config) = config_storage {
    match GLOBAL_CONFIG_STORAGE.set(config) {
        Ok(_) => {}
        Err(_) => {
            panic!("Failed to set config storage")
        }
    }
}
```

**Attack:** `OnceLock::set` is irreversible. If `ConsensusEngine::init` is called twice in the same process (e.g., during graceful restart, hot-reload, or testing), the second call **panics unconditionally**. This is acknowledged by the comment `"It seems stupid, refactor when debugging finished"` — meaning this known-broken code shipped to the codebase.

Additionally, if `config_storage` is `None`, downstream code calling `GLOBAL_CONFIG_STORAGE.get()` receives `None` with **no indication** that config was never set. Any code path that assumes config is present will silently fail or produce incorrect behavior.

**Impact:** Process-level restart impossible. Stale global config cannot be updated after epoch transitions or configuration changes.

---

### 6. No Rollback on Partial Initialization — Zombie Runtimes and Leaked Resources

**File:** `crates/api/src/consensus_api.rs:107-363`

**Attack:** The `init` method is a ~250-line sequential initialization with no `Drop` guard or rollback mechanism. If any step panics:

- **Network runtimes already `.start()`ed** (line 224) have bound to ports and are actively accepting connections, but no subsystem will process their messages.
- **`ConsensusDB`** remains open and locked on disk — a subsequent restart attempt may fail.
- **`GLOBAL_CONFIG_STORAGE`** (if already set) prevents retry.
- **Event subscriptions** are registered but will never be triggered.

A partially-initialized node occupies network ports, holds DB locks, and corrupts global state — yet appears dead. An operator must kill the process and manually verify DB integrity.

**Impact:** Partial failure leaves the system in an irrecoverable state without process restart and potential manual intervention.

---

### 7. `ensure_https` Middleware Is Ineffective — HTTPS Check Always Fails on Server-Side

**File:** `crates/api/src/https/mod.rs:32-37`

```rust
async fn ensure_https(req: Request<Body>, next: Next) -> Response {
    if req.uri().scheme_str() != Some("https") {
        return Response::builder().status(400).body("HTTPS required".into()).unwrap();
    }
    next.run(req).await
}
```

**Attack:** In axum behind `axum_server`, the TLS termination happens at the server layer. The `Request` object received by axum middleware **does not have a scheme** set — `req.uri().scheme_str()` returns `None`, not `Some("https")`, because the URI in the HTTP request line is path-only (e.g., `POST /tx/submit_tx`), not absolute. This means:

- On an HTTPS server: The check incorrectly returns 400 because scheme is `None`, **blocking all legitimate HTTPS requests** to `/tx/*` routes.
- On an HTTP server: The check correctly rejects (but for the wrong reason — scheme is `None`, not `"http"`).

The test at `mod.rs:229` appears to pass only because it uses `reqwest` which sends requests with full URIs to HTTPS endpoints, but real-world HTTP clients behind proxies or making standard requests will see `None` for the scheme. **The GSDK-013 security gate is either non-functional or accidentally blocks all traffic.** Given that `submit_tx` is `todo!()`, this hasn't been caught in production.

**Impact:** The intended HTTPS-only enforcement for transaction endpoints is fundamentally broken. When `submit_tx` is implemented, it will either be inaccessible (scheme is `None`) or the check will be bypassed/removed, exposing transactions over plaintext.

---

### 8. Unauthenticated Failpoint Injection Over HTTP — Remote Code Behavior Modification

**File:** `crates/api/src/https/mod.rs:115`, `crates/api/src/https/set_failpoints.rs:26-37`

```rust
// mod.rs:115 — no TLS, no auth
.route("/set_failpoint", post(set_fail_point_lambda))
```

```rust
// set_failpoints.rs:27 — arbitrary failpoint name and actions
match fail::cfg(&request.name, &request.actions) {
```

**Attack:** In debug builds with the `failpoints` feature, the `/set_failpoint` endpoint accepts **arbitrary failpoint names and actions** over **plain HTTP** with **zero authentication**. An attacker with network access can:

1. Inject `return` or `panic` actions on any compiled-in failpoint.
2. Cause targeted crashes at specific code paths (e.g., consensus voting, block proposal, DKG).
3. Inject `sleep(N)` actions to create artificial latency, causing timeouts and liveness failures.
4. Inject `print` actions to leak internal state via logs.

The test at `mod.rs:224-227` confirms this works over plain HTTP:
```rust
client.post("http://127.0.0.1:5425/set_failpoint").json(&map).send().await.unwrap();
```

**Impact:** Complete remote control of node behavior in debug builds. While gated behind `debug_assertions`, if debug builds are ever deployed to staging/testnet (common practice), this is a full node compromise vector.

---

## MEDIUM Severity

### 9. `notify_events` Error Causes Panic — Event Delivery Failure Crashes the Commit Handler

**File:** `crates/api/src/consensus_mempool_handler.rs:85`

```rust
event_subscription_service.notify_events(block_number, events).unwrap();
```

**Attack:** If `notify_events` returns an error (e.g., subscriber channel full, subscriber dropped), the `.unwrap()` panics, killing the `ConsensusToMempoolHandler`. Every subsequent commit notification is lost. Unlike an explicit error handler, this is a **hidden panic** inside a `select!` loop that silently terminates the critical commit pipeline.

**Impact:** Any event delivery failure becomes a permanent consensus→mempool pipeline failure.

---

### 10. Block Timestamp Uses Local `SystemTime::now()` Instead of Consensus Timestamp

**File:** `crates/api/src/consensus_mempool_handler.rs:79`

```rust
self.mempool_notification_handler
    .notify_mempool_of_committed_transactions(
        committed_transactions,
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
    )
```

**Attack:** The block timestamp passed to mempool for transaction expiry/GC uses the **local wall clock** rather than the consensus-agreed block timestamp. This creates:

1. **Clock skew vulnerability:** If the local clock is ahead, transactions may be prematurely evicted from mempool. If behind, expired transactions linger.
2. **Non-determinism:** Different validators use different local timestamps for the same committed block, causing divergent mempool states.
3. The code itself acknowledges this: `// TODO(gravity_byteyue): the block timestamp usecs should be modified`

Additionally, the parameter name says `_usecs` but `as_secs()` provides **seconds**, not microseconds — a 10^6 magnitude error if downstream code interprets this as microseconds.

**Impact:** Mempool transaction expiry/GC based on inconsistent timestamps across validators. Potential off-by-10^6 if downstream interprets as microseconds.

---

### 11. `init_block_buffer_manager` Scans All Epochs Unconditionally — Quadratic Startup Cost

**File:** `crates/api/src/bootstrap.rs:279-312`

```rust
for epoch_i in (1..=max_epoch).rev() {
    consensus_db
        .get_range_with_filter::<BlockNumberSchema, _>(...)
        .unwrap()
```

**Attack:** The block buffer manager initialization iterates over **every epoch from 1 to max_epoch**, performing a full range scan per epoch, even though only blocks in `[latest - 256, latest]` are needed. The commented-out early-exit optimization (`has_large`/`break`) confirms this was recognized but not implemented.

For long-running chains with many epochs, this is an **O(epochs × blocks_per_epoch)** startup penalty. An attacker who triggers frequent epoch changes (e.g., through validator set churn) can significantly slow node restarts, effectively creating a liveness issue.

**Impact:** Node restart time grows linearly with epoch count. Potential denial-of-service through artificial epoch inflation.

---

## Summary Table

| # | Finding | Severity | Type |
|---|---------|----------|------|
| 1 | `submit_tx` is `todo!()` — panics on call, no auth design | Critical | DoS / Missing AuthZ |
| 2 | Mempool notification error path is `todo!()` — kills commit pipeline | Critical | Unrecoverable State |
| 3 | `SyncForDuration` handler is `todo!()` — kills commit pipeline | Critical | Unrecoverable State |
| 4 | `notify_initial_configs` error silently discarded | Critical | Silent Malfunction |
| 5 | `GLOBAL_CONFIG_STORAGE` OnceLock prevents restart | High | Irrecoverable State |
| 6 | No rollback on partial init — zombie runtimes, leaked resources | High | Resource Leak |
| 7 | `ensure_https` middleware checks wrong field — always `None` | High | Broken Security Gate |
| 8 | Unauthenticated failpoint injection over HTTP | High | Remote Code Control |
| 9 | `notify_events` `.unwrap()` panics on delivery failure | Medium | Hidden Panic |
| 10 | Local clock used for block timestamp + seconds/microseconds mismatch | Medium | Non-determinism |
| 11 | All-epoch scan in block buffer init — no early exit | Medium | Performance DoS |

---

## [relayer_oracle_integration] Security audit of bin/gravity_node/src/relayer.rs focusing on: (1) the RelayerCo

### 🎯 Intention

Security audit of bin/gravity_node/src/relayer.rs focusing on: (1) the RelayerConfig file loading for path traversal and config injection; (2) the URI parsing in parse_source_from_uri for input validation and potential integer overflow; (3) the should_block_poll logic for race conditions between fetched_nonce and onchain_nonce; (4) the poll_and_update_state caching mechanism for stale data issues; (5) the GLOBAL_CONFIG_STORAGE.get().unwrap() in get_oracle_source_states for panic safety; (6) the add_uri method's error handling when oracle state is not found; (7) the overall flow control between on-chain state and off-chain polling for consistency.

### 🔍 Discovered Implementation

# Implementation Analysis: `bin/gravity_node/src/relayer.rs`

## Files/Contracts Involved

| File | Description |
|---|---|
| `bin/gravity_node/src/relayer.rs` | Core relayer module — `RelayerWrapper`, config loading, URI parsing, poll gating, trait impl |
| `bin/gravity_node/src/main.rs` | Entry point — constructs `RelayerWrapper`, sets `GLOBAL_RELAYER` |
| `crates/api/src/lib.rs` | CLI arg definition for `--relayer_config` (`GravityNodeArgs`) |
| `crates/api/src/config_storage.rs` | `ConfigStorageWrapper` — mediates `GLOBAL_CONFIG_STORAGE.fetch_config_bytes()` |
| `crates/block-buffer-manager/src/lib.rs` | `get_block_buffer_manager()` — returns `OnceLock<Arc<BlockBufferManager>>` singleton |
| `crates/block-buffer-manager/src/block_buffer_manager.rs` | `BlockBufferManager::latest_commit_block_number()` — reads committed block height |
| `gaptos` (external compiled dep) | Defines `Relayer` trait, `PollResult`, `ExecError`, `OracleSourceState`, `GLOBAL_RELAYER`, `GLOBAL_CONFIG_STORAGE`, `OnChainConfig` |
| `greth` (external compiled dep) | Defines `OracleRelayerManager` — the actual external-chain polling engine |

---

## Execution Path

### 1. Initialization (`main.rs:224–298`)

1. CLI is parsed via `clap`. `relayer_config_path` comes from `--relayer_config` flag (`Option<PathBuf>`).
2. `run_reth()` starts the Ethereum execution layer; returns consensus args + `datadir`.
3. If `MOCK_CONSENSUS` env var is **not** set:
   - `RelayerWrapper::new(relayer_config_path, datadir)` is called (line 277).
   - The wrapper is wrapped in `Arc` and set into `GLOBAL_RELAYER` via `OnceLock::set()`. A failure to set **panics** (line 281).
4. Consensus engine is initialized and begins calling `Relayer` trait methods on the global relayer.

### 2. `RelayerWrapper::new(config_path, datadir)` — relayer.rs:84–102

1. If `config_path` is `Some`, calls `RelayerConfig::from_file(&path)`.
2. `from_file` reads the file at the given path via `std::fs::read_to_string(path)`, then deserializes with `serde_json::from_str`.
3. If loading fails, logs a warning and falls back to `RelayerConfig::default()` (empty `HashMap`).
4. Constructs `OracleRelayerManager::new(datadir)` — the underlying polling engine.
5. Returns `RelayerWrapper { manager, tracker: ProviderProgressTracker::new(), config }`.

### 3. `Relayer::add_uri(uri, _rpc_url)` — relayer.rs:215–247

1. **Config lookup**: Looks up `uri` in `self.config.uri_mappings`. If not found, returns `Err(ExecError::Other(...))`. The provided `_rpc_url` parameter is **ignored entirely**.
2. **Fetch on-chain state**: Calls `get_oracle_source_states()` (see §5 below).
3. **Match oracle state**: Calls `find_oracle_state_for_uri(uri, &oracle_states)` — parses `source_type`/`source_id` from the URI and searches the on-chain state vector. Returns error if not found.
4. **Extract warm-start values**: `onchain_nonce = oracle_state.latest_nonce`, `onchain_block_number = oracle_state.latest_record.map(|r| r.block_number).unwrap_or(0)`.
5. **Delegate to manager**: Calls `self.manager.add_uri(uri, actual_url, onchain_nonce, onchain_block_number)`.

### 4. `Relayer::get_last_state(uri)` — relayer.rs:250–286

1. **Fetch on-chain state**: Calls `get_oracle_source_states()`.
2. **Match oracle state**: Calls `find_oracle_state_for_uri(uri, &oracle_states)`. If not found, `onchain_nonce` and `onchain_block_number` are both `None` (no error — silently proceeds).
3. **Read tracker state**: `self.tracker.get_state(uri).await` — returns cached `ProviderState` or `Default` (nonce=`None`, last_had_update=`false`, last_result=`None`).
4. **Check poll gate**: Calls `should_block_poll(&state, onchain_nonce)`.
   - If blocked and cached result exists → returns cached `PollResult` clone.
   - If blocked but no cached result → returns `Err`.
5. **Poll**: If not blocked, calls `poll_and_update_state(uri, onchain_nonce, onchain_block_number, &state)`.

### 5. `get_oracle_source_states()` — relayer.rs:105–139

1. Calls `get_block_buffer_manager().latest_commit_block_number().await` — reads `block_state_machine.latest_commit_block_number` behind a tokio `Mutex`.
2. Calls `GLOBAL_CONFIG_STORAGE.get().unwrap()` to obtain the config storage singleton.
3. Calls `.fetch_config_bytes(OnChainConfig::OracleState, block_number.into())`.
4. If bytes are returned, converts to `Bytes` and BCS-deserializes into `Vec<OracleSourceState>`.
5. On any failure (no bytes, conversion error, deserialization error), logs a warning and returns `vec![]`.

### 6. `parse_source_from_uri(uri)` — relayer.rs:143–160

1. Checks prefix `"gravity://"`. Returns `None` if absent.
2. Slices at byte offset 10 (`&uri[10..]`).
3. Splits remainder by `'/'`, expects at least 2 parts.
4. Parses `parts[0]` as `u32` (source_type) and `parts[1]` (with query string stripped at `'?'`) as `u64` (source_id).
5. Returns `Some((source_type, source_id))`.

### 7. `should_block_poll(state, onchain_nonce)` — relayer.rs:172–180

1. If `state.fetched_nonce` is `Some(fetched)` AND `onchain_nonce` is `Some(onchain)`:
   - Returns `true` if `state.last_had_update && fetched > onchain`.
2. Otherwise returns `false`.

### 8. `poll_and_update_state(uri, onchain_nonce, onchain_block_number, state)` — relayer.rs:182–210

1. Delegates to `self.manager.poll_uri(uri, onchain_nonce, onchain_block_number)`.
2. Maps the error to `ExecError::Other`.
3. Calls `self.tracker.update_state(uri, &result)` — stores `fetched_nonce`, `last_had_update`, and cached `last_result`.
4. Returns the `PollResult`.

---

## Key Functions

| Function | Signature | What It Does |
|---|---|---|
| `RelayerConfig::from_file` | `(path: &PathBuf) -> Result<Self, String>` | Reads JSON file, deserializes into `RelayerConfig` |
| `RelayerConfig::get_url` | `(&self, uri: &str) -> Option<&str>` | HashMap lookup of URI → RPC URL |
| `RelayerWrapper::new` | `(config_path: Option<PathBuf>, datadir: PathBuf) -> Self` | Loads config, constructs `OracleRelayerManager` |
| `get_oracle_source_states` | `() -> Vec<OracleSourceState>` | Fetches BCS-encoded oracle states from on-chain config storage |
| `parse_source_from_uri` | `(uri: &str) -> Option<(u32, u64)>` | Parses `gravity://<u32>/<u64>/...` into `(source_type, source_id)` |
| `find_oracle_state_for_uri` | `(uri, states) -> Option<&OracleSourceState>` | Finds matching oracle state by source_type + source_id |
| `should_block_poll` | `(state, onchain_nonce) -> bool` | Gates polling when fetched data hasn't been consumed on-chain |
| `poll_and_update_state` | `(uri, onchain_nonce, onchain_block_number, state) -> Result<PollResult, ExecError>` | Delegates to `manager.poll_uri`, caches result |
| `ProviderProgressTracker::get_state` | `(&self, name: &str) -> ProviderState` | Returns cloned state or default |
| `ProviderProgressTracker::update_state` | `(&self, name: &str, result: &PollResult)` | Inserts/overwrites state for a provider |

---

## State Changes

| What Changes | Where | Trigger |
|---|---|---|
| `ProviderProgressTracker.states` (HashMap) | `relayer.rs:66` | `update_state()` called after every successful `poll_uri` |
| `OracleRelayerManager` internal state | Inside `greth` crate (opaque) | `add_uri()` and `poll_uri()` modify internal provider registrations and polling state |
| `GLOBAL_RELAYER` (OnceLock) | `main.rs:278` | Set exactly once during startup; panics if set twice |
| `GLOBAL_CONFIG_STORAGE` (OnceLock) | External crate init | Set during consensus engine initialization |
| `BlockBufferManager.block_state_machine.latest_commit_block_number` | `block_buffer_manager.rs:932` | Updated by block commit pipeline |

---

## External Dependencies

| Dependency | Import Path | Usage |
|---|---|---|
| `gaptos` | `gaptos::api_types::*` | `Relayer` trait, `PollResult`, `ExecError`, `OracleSourceState`, `GLOBAL_RELAYER`, `GLOBAL_CONFIG_STORAGE`, `OnChainConfig` |
| `greth` | `greth::reth_pipe_exec_layer_relayer` | `OracleRelayerManager` — the actual external-chain polling engine |
| `block_buffer_manager` | `block_buffer_manager::get_block_buffer_manager` | Global singleton for committed block number tracking |
| `bcs` | `bcs::from_bytes` | BCS deserialization of `Vec<OracleSourceState>` |
| `serde_json` | `serde_json::from_str` | JSON deserialization of `RelayerConfig` |
| `tokio::sync::Mutex` | `tokio::sync::Mutex` | Protects `ProviderProgressTracker.states` |
| `async_trait` | `async_trait::async_trait` | Enables async methods in `Relayer` trait impl |

---

## Detailed Observations by Audit Focus Area

### (1) RelayerConfig File Loading — `from_file` (lines 26–32)

- The `path` parameter is an `Option<PathBuf>` sourced from the CLI `--relayer_config` flag.
- `std::fs::read_to_string(path)` is called directly on the user-supplied path with no canonicalization, no path validation, and no restriction to a specific directory.
- On failure, the code falls back to `RelayerConfig::default()` (empty `uri_mappings`), meaning a missing or malformed config silently results in all `add_uri` calls failing with "not found in local config."
- The config content (`uri_mappings: HashMap<String, String>`) maps arbitrary string keys to arbitrary string RPC URLs. There is no URL scheme validation or allowlist on the RPC URL values.
- The config is logged at info level on line 97: `info!("relayer config: {:?}", config)` — this prints the full config including all RPC URLs.

### (2) URI Parsing — `parse_source_from_uri` (lines 143–160)

- The function slices at byte offset 10 (`&uri[10..]`). The prefix `"gravity://"` is exactly 10 ASCII bytes, so this is correct for ASCII input.
- `parts[0].parse::<u32>()` and `source_id_str.parse::<u64>()` — both use Rust's standard `str::parse()`, which returns `Err` on overflow. The `?` operator propagates this as `None`, so integer overflow results in a failed match rather than wrapping.
- The query string is stripped from `parts[1]` only (via `split('?').next()`). If the URI has more path segments (e.g., `gravity://1/2/task_type`), they are ignored — only `parts[0]` and `parts[1]` are used.
- If `uri` contains non-ASCII characters in the scheme prefix, the byte-offset slice at `&uri[10..]` could panic on a non-UTF-8 boundary. However, since `starts_with("gravity://")` only matches ASCII, and Rust strings are always valid UTF-8, the input must begin with 10 ASCII bytes to reach the slice.

### (3) `should_block_poll` Logic (lines 172–180)

- The function reads `state.fetched_nonce` (set by `update_state` under the tracker `Mutex`) and `onchain_nonce` (fetched fresh from on-chain config in the same `get_last_state` call).
- The lock on `self.tracker.states` is acquired and released in `get_state()` (line 264) **before** `should_block_poll` is called (line 271). Between these two calls, another concurrent `get_last_state` or `poll_and_update_state` could modify the tracker state via `update_state`.
- The function returns `false` when either nonce is `None`, meaning: (a) first poll always proceeds (no fetched nonce yet), and (b) if oracle state is not found on-chain (`onchain_nonce = None`), polling is never blocked.
- The blocking condition `fetched > onchain` uses unsigned comparison on `u128`. There is no subtraction, so no underflow concern.

### (4) `poll_and_update_state` Caching (lines 182–210)

- Every successful `poll_uri` result is cached via `self.tracker.update_state(uri, &result)` regardless of whether `result.updated` is true or false.
- The cached `last_result` is only served when `should_block_poll` returns `true` (in `get_last_state`). This means stale cached data only surfaces when the on-chain nonce hasn't caught up.
- The `state` parameter passed into `poll_and_update_state` is a snapshot from before the lock was released. It is logged but not used for any decision-making within this function — only `onchain_nonce` and `onchain_block_number` are forwarded to `manager.poll_uri`.
- There is no TTL or cache invalidation mechanism. The cached result persists until overwritten by a new poll.

### (5) `GLOBAL_CONFIG_STORAGE.get().unwrap()` — Panic Safety (line 111)

- `GLOBAL_CONFIG_STORAGE` is a `OnceLock`. `.get()` returns `Option<&T>`. `.unwrap()` panics if the `OnceLock` has not been initialized.
- Initialization of `GLOBAL_CONFIG_STORAGE` occurs during consensus engine init (`ConsensusEngine::init`), which is called in `main.rs:284–297`.
- `get_oracle_source_states()` is called from `add_uri` and `get_last_state`, which are trait methods called by the consensus engine. Since the relayer is set into `GLOBAL_RELAYER` (line 278) **before** the consensus engine is initialized (line 284), but the trait methods are only invoked once the engine is running, the `OnceLock` should be initialized by the time `get()` is called.
- However, there is no compile-time or runtime guard ensuring this ordering; it depends entirely on the consensus engine not calling `Relayer` methods before `GLOBAL_CONFIG_STORAGE` is set.

### (6) `add_uri` Error Handling When Oracle State Not Found (lines 215–247)

- `add_uri` returns `Err(ExecError::Other(...))` in two cases:
  1. URI not found in local config (`self.config.get_url(uri)` returns `None`) — line 220.
  2. Oracle state not found for the URI (`find_oracle_state_for_uri` returns `None`) — lines 226–230.
- The `_rpc_url` parameter (the on-chain-provided URL) is completely ignored. Only the local config file's URL is used.
- If `get_oracle_source_states()` returns an empty vec (due to `GLOBAL_CONFIG_STORAGE` returning `None`, conversion failure, or BCS deserialization failure), `find_oracle_state_for_uri` will always return `None`, causing `add_uri` to always fail.
- The full `oracle_states` vector is logged at info level (line 224), which may contain all oracle metadata.

### (7) Flow Control Between On-Chain and Off-Chain State

- **`add_uri` flow**: Synchronous gate — requires both local config entry AND on-chain oracle state to succeed. On-chain state provides the warm-start nonce/block for the polling engine.
- **`get_last_state` flow**: Fetches on-chain state fresh on every call. If on-chain state is unavailable, both `onchain_nonce` and `onchain_block_number` are `None`, causing `should_block_poll` to return `false`, so polling always proceeds.
- **Consistency gap**: The on-chain state is read at the `latest_commit_block_number` from the block buffer manager. Between reading the on-chain state and performing the poll, new blocks may have committed, meaning the on-chain nonce used for gating may be stale relative to the actual chain state.
- **No retry/backoff**: If `poll_uri` fails, the error propagates immediately to the caller. There is no retry logic, exponential backoff, or circuit breaker within this module.
- **Asymmetric error handling**: `add_uri` fails hard when oracle state is missing (returns `Err`), while `get_last_state` degrades gracefully (proceeds with `None` nonces). This means a provider can be successfully polled via `get_last_state` even if it could never have been registered via `add_uri` due to missing oracle state.

### 🛠️ Ground Review

# Ground Review (Blue Team) — `bin/gravity_node/src/relayer.rs`

## Critical

### C1. `GLOBAL_CONFIG_STORAGE.get().unwrap()` — Unguarded Panic
**File:** `relayer.rs:110-111`
```rust
GLOBAL_CONFIG_STORAGE
    .get()
    .unwrap()
```
The `OnceLock::get()` returns `Option`, and `.unwrap()` will **panic and crash the node** if the lock hasn't been initialized yet. Initialization happens in `ConsensusEngine::init` (`main.rs:284-297`), which runs *after* `GLOBAL_RELAYER` is set (`main.rs:278`). The ordering is correct at startup today, but there is **no compile-time or runtime guard** — any refactor that changes call ordering, or any code path that invokes `get_oracle_source_states()` before consensus engine init, will cause an unrecoverable crash.

**Recommendation:** Replace with `.get().ok_or_else(|| ...)` and propagate the error, or use `expect()` with a descriptive message at minimum.

---

### C2. RPC URLs Logged at `info` Level — Credential Leakage
**File:** `relayer.rs:97`
```rust
info!("relayer config: {:?}", config);
```
**File:** `relayer.rs:238-239`
```rust
info!("Adding URI: {}, RPC URL: {}, ...", uri, actual_url, ...);
```
`RelayerConfig` contains `uri_mappings: HashMap<String, String>` where values are RPC URLs. These URLs commonly include **API keys or authentication tokens** (e.g., `https://mainnet.infura.io/v3/YOUR_API_KEY`). Logging the full config and individual URLs at `info` level exposes credentials in log aggregation systems, stdout captures, and log files.

**Recommendation:** Redact or mask URL values before logging. Log only the URI keys or a sanitized form of URLs (e.g., scheme + host only).

---

### C3. Oracle State Vector Logged in Error Path — Data Exposure
**File:** `relayer.rs:224`
```rust
info!("Oracle states: {:?}", oracle_states);
```
**File:** `relayer.rs:228`
```rust
"Oracle state not found for URI: {uri}. Available states: {oracle_states:?}"
```
The full `Vec<OracleSourceState>` is logged on every `add_uri` call and embedded in error messages. This may contain all oracle metadata for every configured source. At scale, this creates excessive log volume and potential data exposure.

**Recommendation:** Log only `oracle_states.len()` in the info path, and include only the target `(source_type, source_id)` in error messages rather than the entire state vector.

---

## Warning

### W1. TOCTOU Race in `get_last_state` — Stale Tracker State
**File:** `relayer.rs:264-285`
```rust
let state = self.tracker.get_state(uri).await;  // lock acquired and released
// ... gap where other tasks can mutate tracker ...
if Self::should_block_poll(&state, onchain_nonce) {
    // decision based on potentially stale state
}
```
The `Mutex` lock on `ProviderProgressTracker.states` is acquired in `get_state()` (line 264), the `ProviderState` is **cloned**, and the lock is released. Between this point and the `should_block_poll` check (line 271), a concurrent `poll_and_update_state` call can overwrite the state via `update_state`. This is a classic **time-of-check-to-time-of-use** gap that could result in:
- A stale cached result being served when a fresh one is available
- A poll proceeding when it should have been blocked

**Recommendation:** Hold the lock across the entire check-then-act sequence, or accept and document the race as benign (it appears non-catastrophic since worst case is one extra poll or one stale response).

---

### W2. `_rpc_url` Parameter Silently Ignored
**File:** `relayer.rs:215`
```rust
async fn add_uri(&self, uri: &str, _rpc_url: &str) -> Result<(), ExecError> {
```
The `_rpc_url` parameter from the `Relayer` trait is **completely discarded**. Only the local config file's URL is used. The leading underscore silences the compiler warning, but this creates a misleading API contract — callers believe they're providing an RPC URL, but it's never used. If the local config is missing an entry, the call fails with a confusing "not found in local config" error even though a valid URL was provided.

**Recommendation:** Either use `_rpc_url` as a fallback when the local config has no entry, or document explicitly why it's rejected.

---

### W3. Silent Fallback to Empty Config on File Load Failure
**File:** `relayer.rs:84-96`
```rust
let config = config_path
    .and_then(|path| match RelayerConfig::from_file(&path) {
        Ok(cfg) => { ... Some(cfg) }
        Err(e) => {
            warn!("Failed to load relayer config: {}. Using empty config.", e);
            None
        }
    })
    .unwrap_or_default();
```
If the user provides a `--relayer_config` path and it fails to load (typo, permission error, malformed JSON), the node **silently starts with an empty config**. Every subsequent `add_uri` call will fail with "not found in local config." The node appears healthy but the relayer is non-functional.

**Recommendation:** If a config path was explicitly provided and fails to load, this should be a **fatal error**, not a silent fallback.

---

### W4. No Cache TTL or Eviction on `ProviderProgressTracker`
**File:** `relayer.rs:50-74`
The `states: Mutex<HashMap<String, ProviderState>>` map grows monotonically. There is no TTL, eviction policy, or size limit. If URIs are dynamic or numerous, this map can grow unbounded. Each entry also holds a cloned `PollResult` (which includes `jwk_structs: Vec<...>`), potentially retaining significant memory.

**Recommendation:** Add a bounded size or TTL-based eviction, or document that URIs are expected to be a small, static set.

---

### W5. No Path Validation on Config File
**File:** `relayer.rs:26-28`
```rust
pub fn from_file(path: &PathBuf) -> Result<Self, String> {
    let content = std::fs::read_to_string(path)
```
The path comes directly from CLI args with no canonicalization or restriction. While this is a node operator-supplied path (not external user input), there's no validation that it points to a regular file within an expected directory. Symlink traversal could read arbitrary files.

**Severity context:** Low in practice because only the node operator can supply this path, but noted for defense-in-depth.

---

### W6. `println!` in Production Code
**File:** `config_storage.rs:23`
```rust
println!("fetch_config_bytes: {config_name:?}, block_number: {block_number:?}");
```
This function is called on every `get_oracle_source_states()` invocation (which happens on every `add_uri` and `get_last_state` call). Using `println!` instead of `tracing::debug!` bypasses structured logging, cannot be filtered by log level, and pollutes stdout in production.

**Recommendation:** Replace with `tracing::debug!` or remove.

---

## Info

### I1. Idiomatic Rust: `&PathBuf` → `&Path`
**File:** `relayer.rs:26`
```rust
pub fn from_file(path: &PathBuf) -> Result<Self, String> {
```
Idiomatic Rust prefers `&Path` over `&PathBuf` in function signatures — `PathBuf` auto-derefs to `Path`, and `&Path` is more general.

---

### I2. Unnecessary `Vec` Collection in URI Parsing
**File:** `relayer.rs:149`
```rust
let parts: Vec<&str> = rest.split('/').collect();
```
This allocates a `Vec` only to index into positions 0 and 1. Using `split('/').next()` and an iterator would avoid the allocation, though the performance impact is negligible.

---

### I3. Asymmetric Error Handling Between `add_uri` and `get_last_state`
**File:** `relayer.rs:226-230` vs `relayer.rs:253-262`

`add_uri` treats a missing oracle state as a hard error (`Err`), while `get_last_state` degrades gracefully (sets nonces to `None` and proceeds). This means a provider can be polled via `get_last_state` even if it could never have been registered via `add_uri`. This may be intentional, but the asymmetry is worth documenting.

---

### I4. Magic Number in URI Parsing
**File:** `relayer.rs:148`
```rust
let rest = &uri[10..]; // len("gravity://") = 10
```
The comment is helpful, but using `"gravity://".len()` or `strip_prefix("gravity://")` would be self-documenting and resilient to prefix changes.

---

## Summary

| Severity | Count | Key Themes |
|----------|-------|------------|
| **Critical** | 3 | Unguarded `.unwrap()` on `OnceLock`, credential leakage in logs, excessive data in error messages |
| **Warning** | 6 | TOCTOU race, ignored API parameter, silent config fallback, unbounded cache, no path validation, `println!` in prod |
| **Info** | 4 | Idiomatic Rust nits, asymmetric error handling, magic number |

### ⚔️ Intention Attacker

# Intention Attack Report: `bin/gravity_node/src/relayer.rs`

## Findings

---

### CRITICAL-1: `_rpc_url` Parameter Silently Ignored in `add_uri` — On-Chain RPC URL Bypassed

**File:** `relayer.rs:215-220`

```rust
async fn add_uri(&self, uri: &str, _rpc_url: &str) -> Result<(), ExecError> {
    let actual_url = self
        .config
        .get_url(uri)
        .ok_or_else(|| ExecError::Other(format!("Provider {uri} not found in local config")))?;
```

**Attack:** The `Relayer` trait provides an `_rpc_url` parameter — presumably the on-chain-authorized RPC endpoint that the consensus protocol has agreed upon. This parameter is **completely discarded** via the `_` prefix. Instead, the node operator's local JSON config file dictates which RPC endpoint is used.

**Impact:** A malicious or misconfigured node operator can point the relayer at an arbitrary RPC endpoint (e.g., one they control) by editing their local `relayer_config.json`. The oracle polling engine will then fetch data from this attacker-controlled endpoint instead of the consensus-agreed source. Since the polled data feeds into on-chain oracle state, this enables **oracle data poisoning** — the node will propose fraudulent oracle updates sourced from a spoofed RPC. This completely undermines the trust model if the consensus layer intended the on-chain URL to be authoritative.

**Severity: CRITICAL**

---

### CRITICAL-2: RPC URLs Logged at `info` Level — Credential Leakage

**File:** `relayer.rs:97` and `relayer.rs:238-239`

```rust
info!("relayer config: {:?}", config);                    // line 97 — dumps all RPC URLs
info!("Adding URI: {}, RPC URL: {}, ...", uri, actual_url, ...); // line 238 — per-URI
```

**Attack:** RPC URLs for services like Infura, Alchemy, and QuickNode typically contain **API keys embedded in the URL** (e.g., `https://mainnet.infura.io/v3/YOUR_API_KEY`). Both the full config dump and per-URI log statements emit these at `info` level, which is the default production log level.

**Impact:** API keys leak into log aggregation systems (ELK, Datadog, CloudWatch), container stdout, and log files. Anyone with log access can steal the operator's RPC provider credentials, potentially incurring costs or hitting rate limits against the operator's account. This is an **OWASP Top 10 Sensitive Data Exposure** vulnerability.

**Severity: CRITICAL**

---

### HIGH-1: TOCTOU Race Condition in `get_last_state` — Poll Gate Bypass

**File:** `relayer.rs:250-286`

```rust
let state = self.tracker.get_state(uri).await;    // line 264 — lock acquired and RELEASED
// ... logging ...
if Self::should_block_poll(&state, onchain_nonce) { // line 271 — decides on stale snapshot
    // ...
}
self.poll_and_update_state(uri, ..., &state).await   // line 285 — polls with stale state
```

**Attack:** `get_state()` acquires the mutex, clones the state, and **releases the lock** (line 60-61). Between this release and the `should_block_poll` check on line 271, concurrent calls to `get_last_state` for the same URI can also read the same snapshot. Both callers see `should_block_poll = false`, and both proceed to poll simultaneously. After both polls complete, `update_state` is called twice, each overwriting the other's result.

**Impact:** The poll gating mechanism (`should_block_poll`) exists to prevent the relayer from flooding the chain with duplicate data when the on-chain nonce hasn't caught up. This race condition allows **duplicate oracle submissions** — two concurrent callers both poll and both return fresh data to the consensus engine before the on-chain nonce advances. This can cause wasted gas, nonce conflicts in the oracle contract, or consensus disagreements between validators that have different views of which data was already submitted.

**Severity: HIGH**

---

### HIGH-2: `GLOBAL_CONFIG_STORAGE.get().unwrap()` — Unguarded Panic in Production

**File:** `relayer.rs:109-111`

```rust
let config_bytes = match GLOBAL_CONFIG_STORAGE
    .get()
    .unwrap()    // <-- panics if OnceLock not yet set
```

**Attack:** `GLOBAL_RELAYER` is set on `main.rs:278` **before** `ConsensusEngine::init` on `main.rs:284-296`, which is where `GLOBAL_CONFIG_STORAGE` gets initialized (via the `config_storage: Some(...)` argument). The code relies on an **implicit temporal ordering assumption**: that the consensus engine will not call `add_uri` or `get_last_state` until after `GLOBAL_CONFIG_STORAGE` is set.

If the consensus engine's initialization invokes any relayer method eagerly (e.g., to pre-register oracle sources during init), or if any background task races against the init sequence, this `.unwrap()` will **crash the entire node process**.

**Impact:** Node crash (denial of service). Since this is a validator node, repeated crashes during startup could prevent the validator from participating in consensus, potentially triggering slashing or liveness penalties depending on the protocol design.

**Severity: HIGH**

---

### HIGH-3: Silent Config Fallback Creates Invisible Failure Mode

**File:** `relayer.rs:84-96`

```rust
let config = config_path
    .and_then(|path| match RelayerConfig::from_file(&path) {
        Ok(cfg) => { ... Some(cfg) }
        Err(e) => {
            warn!("Failed to load relayer config: {}. Using empty config.", e);
            None   // <-- falls back to empty HashMap
        }
    })
    .unwrap_or_default();
```

**Attack:** If the config file is missing, corrupted, has wrong permissions, or contains a JSON syntax error, the relayer silently initializes with an **empty `uri_mappings`**. Every subsequent `add_uri` call will fail with "not found in local config" — but because this happens at runtime when the consensus engine requests URI registration, the failure surfaces as individual `ExecError`s rather than a startup abort.

**Impact:** The validator node boots successfully and joins consensus but **silently fails to register any oracle data sources**. The node cannot fulfill its oracle obligations, potentially causing the operator to miss oracle duties, lose rewards, or be penalized — all without any obvious startup error. The `warn!` log is easy to miss in high-volume production logging.

**Severity: HIGH**

---

### MEDIUM-1: Oracle State Debug Dump in Error Messages — Information Disclosure

**File:** `relayer.rs:226-230`

```rust
ExecError::Other(format!(
    "Oracle state not found for URI: {uri}. Available states: {oracle_states:?}"
))
```

And line 224:
```rust
info!("Oracle states: {:?}", oracle_states);
```

**Attack:** The full `Vec<OracleSourceState>` — containing **all registered oracle sources, their nonces, block numbers, and metadata** — is dumped both into error messages and `info` logs. If error messages propagate to RPC responses or external monitoring, this exposes the complete oracle topology to external observers.

**Impact:** An attacker can enumerate all oracle sources, their current nonces, and their progress. This metadata enables targeted attacks against specific oracle providers (knowing their source type, ID, and how far behind they are).

**Severity: MEDIUM**

---

### MEDIUM-2: `get_last_state` Asymmetry Allows Polling Unregistered Providers

**File:** `relayer.rs:250-262` vs `relayer.rs:215-247`

**Attack:** `add_uri` requires both a local config entry AND on-chain oracle state to succeed (hard fail). However, `get_last_state` degrades gracefully when oracle state is missing — it sets `onchain_nonce = None` and `onchain_block_number = None`, which causes `should_block_poll` to return `false`, and proceeds to poll via `manager.poll_uri`.

If the consensus engine calls `get_last_state` for a URI that was previously registered (via `add_uri`) but whose on-chain oracle state has since been removed or invalidated, polling continues **without any nonce-based gating**. The poll gate is completely disabled because `onchain_nonce` is `None`.

**Impact:** A provider that should be blocked (because on-chain hasn't consumed its data) continues to be polled unboundedly. This can cause resource exhaustion on the polling engine and potentially allow stale or duplicate data to be submitted when the on-chain oracle state is in a transitional or invalid state.

**Severity: MEDIUM**

---

### MEDIUM-3: No Cache Invalidation or TTL on Cached Poll Results

**File:** `relayer.rs:64-74` and `relayer.rs:271-278`

```rust
if Self::should_block_poll(&state, onchain_nonce) {
    if let Some(cached) = &state.last_result {
        return Ok(cached.clone());  // Could be arbitrarily old
    }
}
```

**Attack:** When `should_block_poll` returns `true`, the code returns a cached `PollResult` that was stored during the last successful poll. There is no TTL, no staleness check, and no cache invalidation. If the on-chain nonce gets stuck (e.g., due to a bug in the oracle contract or a consensus stall), the relayer will serve the same cached result **indefinitely** on every call to `get_last_state`.

**Impact:** The consensus layer receives infinitely stale oracle data. If the cached data contains price feeds or other time-sensitive information, this could lead to incorrect on-chain state that persists until the on-chain nonce advances — which may never happen if the nonce is stuck due to the very same stale data being resubmitted.

**Severity: MEDIUM**

---

## Summary

| # | Finding | Severity | Category |
|---|---------|----------|----------|
| C-1 | On-chain RPC URL discarded; local config enables oracle data poisoning | **Critical** | Authorization Bypass |
| C-2 | RPC URLs (containing API keys) logged at `info` level | **Critical** | Sensitive Data Exposure |
| H-1 | TOCTOU race between `get_state` and `should_block_poll` | **High** | Race Condition |
| H-2 | `.unwrap()` on `GLOBAL_CONFIG_STORAGE` with no init ordering guarantee | **High** | Panic / DoS |
| H-3 | Silent fallback to empty config makes oracle failure invisible | **High** | Fail-Open Design |
| M-1 | Full oracle state vector dumped in errors and logs | **Medium** | Information Disclosure |
| M-2 | `get_last_state` polls without nonce gating when oracle state is missing | **Medium** | Logic Asymmetry |
| M-3 | No TTL on cached poll results; can serve infinitely stale data | **Medium** | Stale State |

---

## [https_api_security] Security audit of the HTTP/HTTPS API server in crates/api/src/https/ including: 

### 🎯 Intention

Security audit of the HTTP/HTTPS API server in crates/api/src/https/ including: (1) mod.rs for the ensure_https middleware bypass (checks scheme_str which may not be set for direct connections), TLS configuration, and route exposure; (2) tx.rs for the submit_tx todo!() panic in production and get_tx_by_hash returning empty data; (3) set_failpoints.rs for arbitrary failpoint injection via HTTP (even with feature gate); (4) heap_profiler.rs for potential DoS via profiling endpoints; (5) consensus.rs and dkg.rs for sensitive consensus/DKG data exposure; (6) the 1MB body limit (GSDK-011) adequacy; (7) missing authentication/authorization on all endpoints; (8) the test code writing files to the source directory.

### 🔍 Discovered Implementation

# HTTP/HTTPS API Server — Implementation Analysis

## Files Involved

| File | Description |
|------|-------------|
| `crates/api/src/https/mod.rs` | Server setup, routing, TLS config, `ensure_https` middleware |
| `crates/api/src/https/tx.rs` | Transaction submission and lookup stubs |
| `crates/api/src/https/set_failpoints.rs` | Failpoint injection endpoint (feature-gated) |
| `crates/api/src/https/heap_profiler.rs` | jemalloc profiling control endpoint (feature-gated) |
| `crates/api/src/https/consensus.rs` | Consensus data query endpoints |
| `crates/api/src/https/dkg.rs` | DKG status and randomness query endpoints |

---

## Execution Path

### Server Startup (`HttpsServer::serve`)

1. Installs `rustls::crypto::ring::default_provider().install_default().unwrap()`
2. Constructs `DkgState::new(consensus_db)` from optional `ConsensusDB`
3. Builds two route groups: `https_routes` (TLS-guarded) and `http_routes` (public)
4. If `has_tls` (both cert + key present): merges both route groups. Otherwise: **only `http_routes` are registered** — TX endpoints are entirely excluded (GSDK-013)
5. Applies `DefaultBodyLimit::max(1_048_576)` (1 MB) globally (GSDK-011)
6. Parses bind address with `panic!` on failure (GSDK-014)
7. Binds with `axum_server::bind_rustls` (TLS) or `axum_server::bind` (plain HTTP)

### Route Map

**HTTPS-only routes** (guarded by `ensure_https` middleware, only registered when TLS is configured):

| Method | Path | Handler |
|--------|------|---------|
| `POST` | `/tx/submit_tx` | `tx::submit_tx` |
| `GET` | `/tx/get_tx_by_hash/:hash_value` | `tx::get_tx_by_hash` |

**HTTP routes** (always registered, no middleware guard):

| Method | Path | Handler |
|--------|------|---------|
| `GET` | `/dkg/status` | `dkg::get_dkg_status` |
| `GET` | `/dkg/randomness/:block_number` | `dkg::get_randomness` |
| `GET` | `/consensus/latest_ledger_info` | `consensus::get_latest_ledger_info` |
| `GET` | `/consensus/ledger_info/:epoch` | `consensus::get_ledger_info_by_epoch` |
| `GET` | `/consensus/block/:epoch/:round` | `consensus::get_block` |
| `GET` | `/consensus/qc/:epoch/:round` | `consensus::get_qc` |
| `GET` | `/consensus/validator_count/:epoch` | `consensus::get_validator_count_by_epoch` |
| `POST` | `/set_failpoint` | `set_failpoints::set_failpoint` |
| `POST` | `/mem_prof` | `heap_profiler::control_profiler` |

---

## Key Functions

### `ensure_https` middleware (`mod.rs`)

```rust
async fn ensure_https(req: Request<Body>, next: Next) -> Response
```
- Checks `req.uri().scheme_str() != Some("https")`
- Returns HTTP 400 `"HTTPS required"` if scheme is not `https`
- **Behavior note**: For direct TCP connections to the server, `scheme_str()` returns `None` (the scheme is not populated in the URI by axum for incoming requests). This means the middleware would reject direct connections even over TLS, since the scheme field is not set by the server — it only works correctly when an upstream proxy or the client explicitly sets the full URI with scheme. In practice, the `https_routes` are only registered when TLS is configured, providing the actual protection at the routing level (GSDK-013).

### `submit_tx` (`tx.rs`)

```rust
pub async fn submit_tx(_request: TxRequest) -> Result<Json<SubmitResponse>, StatusCode>
```
- **Body is `todo!()`** — panics unconditionally at runtime
- Parameter prefixed with `_` (intentionally unused)
- `TxRequest` has a commented-out `authenticator` field
- Any call to this endpoint will crash the server thread

### `get_tx_by_hash` (`tx.rs`)

```rust
pub async fn get_tx_by_hash(request: HashValue) -> Result<Json<TxResponse>, StatusCode>
```
- Logs the hash via `info!`
- **Always returns `Ok(Json(TxResponse { tx: vec![] }))`** — empty data regardless of input
- Non-panicking stub, but returns no useful data

### `set_failpoint` (`set_failpoints.rs`)

```rust
// With feature = "failpoints":
pub async fn set_failpoint(request: FailpointConf) -> impl IntoResponse
```
- Calls `fail::cfg(&request.name, &request.actions)` — sets an arbitrary named failpoint with arbitrary actions
- Returns 200 with confirmation on success, 500 on error
- **Without the feature**: returns 400 `"Failpoints are not enabled at a feature level"`

**Field name discrepancy**: `FailpointConf` declares field `actions` (plural), but the integration test sends `"action"` (singular). This would cause a serde deserialization error at runtime unless there's a rename attribute not visible in the source.

### `control_profiler` (`heap_profiler.rs`)

```rust
pub async fn control_profiler(_request: ControlProfileRequest) -> impl IntoResponse
```
- With `jemalloc-profiling` feature: calls `PROFILER.set_prof_active(enable)` which uses `unsafe { raw::write(...) }` to toggle jemalloc profiling via CTL interface
- Both success and error paths return **HTTP 200** with JSON body (errors not surfaced as non-200)
- Without feature: returns 200 `"jemalloc profiling is not enabled"`
- `HeapProfiler` uses a `Mutex` that panics on poison (`lock().unwrap()`)

---

## State Changes

| Endpoint | What Changes |
|----------|-------------|
| `POST /set_failpoint` | Mutates global `fail::cfg` failpoint state — can alter behavior of any code path using `fail_point!` macros throughout the process |
| `POST /mem_prof` | Writes to jemalloc `prof.active` and `prof.thread_active_init` globals via unsafe FFI |
| `POST /tx/submit_tx` | None (panics before any state change) |
| All `GET` endpoints | None (read-only) |

---

## Access Control Summary

| Aspect | Status |
|--------|--------|
| **Authentication** | None on any endpoint |
| **Authorization** | None on any endpoint |
| **TLS enforcement** | TX routes excluded from router when TLS not configured; `ensure_https` middleware as secondary check (but scheme_str is not populated for direct connections) |
| **Feature gating** | `set_failpoint` gated on `failpoints` feature; `control_profiler` gated on `jemalloc-profiling` feature |
| **Body size limit** | 1 MB global limit (GSDK-011) |
| **Rate limiting** | None |

---

## External Dependencies

- **`axum` / `axum_server`**: HTTP framework and TLS server
- **`rustls` (via `axum_server::tls_rustls`)**: TLS termination with ring crypto provider
- **`fail` crate**: Failpoint injection framework
- **`tikv_jemalloc_ctl::raw`**: Unsafe jemalloc profiler control
- **`aptos_consensus::consensusdb::ConsensusDB`**: Consensus database for ledger queries
- **`gaptos::aptos_crypto::HashValue`**: 32-byte hash type used in URL path extraction

---

## Test Code (`mod.rs`, `#[cfg(test)]`)

The `work` test:
1. Generates a self-signed TLS certificate via `rcgen`
2. **Writes cert/key PEM files to `$CARGO_MANIFEST_DIR/src/https/test/`** — writes into the source tree
3. Spawns server on `127.0.0.1:5425`
4. Uses `reqwest` client with `danger_accept_invalid_hostnames(true)` and `danger_accept_invalid_certs(true)`
5. Tests failpoint activation over plain HTTP, then TX endpoints over HTTPS
6. Confirms `submit_tx` returns 2xx (but this would actually panic the server — the test may pass due to the response being sent before the panic propagates, or the test may be broken)

### 🛠️ Ground Review

# Ground Review (Blue Team) — HTTP/HTTPS API Server

## Critical

### CR-01: `todo!()` panic in production handler — `tx.rs:28`
**File:** `crates/api/src/https/tx.rs:28`

`submit_tx` calls `todo!()`, which panics unconditionally. In a tokio multi-threaded runtime, this aborts the task (and potentially the entire server if panic=abort is configured). Any HTTP POST to `/tx/submit_tx` will crash the serving thread.

**Impact:** Denial of service. Any authenticated or unauthenticated caller can crash the server task.
**Fix:** Return a proper `501 Not Implemented` status code, or remove the route until the handler is implemented.

---

### CR-02: `ensure_https` middleware is a no-op for direct connections — `mod.rs:32-37`
**File:** `crates/api/src/https/mod.rs:32-37`

The middleware checks `req.uri().scheme_str() != Some("https")`, but axum does **not** populate `scheme_str()` on incoming requests from direct TCP connections — it returns `None`. This means:
- Over TLS: `None != Some("https")` → **rejects** the request (false negative)
- Over plain HTTP: `None != Some("https")` → **rejects** the request

The middleware rejects *all* direct connections regardless of protocol. The actual protection comes from the route not being registered when TLS is absent (line 119-123), making this middleware dead/misleading code rather than a security layer.

**Impact:** Misleading security control. If anyone relies on this middleware as a security boundary (e.g., registers https_routes without the `has_tls` guard), it will still reject legitimate TLS traffic.
**Fix:** Remove the middleware or replace it with a check that inspects the connection's TLS state via axum `ConnectInfo` or a custom layer that examines the `rustls` session.

---

### CR-03: Unauthenticated failpoint injection — `set_failpoints.rs:26-38`, route at `mod.rs:115`
**File:** `crates/api/src/https/set_failpoints.rs:26-38`

When the `failpoints` feature is enabled, *any* network-reachable caller can POST arbitrary failpoint names and actions via `/set_failpoint` over **plain HTTP** (no TLS required — it's on `http_routes`). The `fail::cfg()` call can:
- Inject `panic` actions into any `fail_point!` macro in the process
- Inject `sleep` actions to cause arbitrary delays (DoS)
- Alter control flow (`return`, `yield`) in critical consensus paths

Feature gating is a compile-time control, not a runtime access control.

**Impact:** Remote code behavior manipulation. An attacker with network access to a failpoints-enabled build can destabilize or crash the node at will.
**Fix:** At minimum, bind this endpoint to localhost only or add authentication. Better: ensure failpoints builds are never deployed to production (enforce via CI).

---

### CR-04: Unauthenticated profiler control — `heap_profiler.rs:36-44`, route at `mod.rs:116`
**File:** `crates/api/src/https/heap_profiler.rs:36-44`

The `/mem_prof` endpoint is on `http_routes` (no TLS, no auth). When `jemalloc-profiling` is enabled, any caller can toggle heap profiling on/off. Enabling profiling imposes significant performance overhead (CPU + I/O for profile dumps).

**Impact:** Denial of service via performance degradation.
**Fix:** Restrict to localhost or add authentication.

---

### CR-05: Mutex poison panic — `heap_profiler.rs:53`
**File:** `crates/api/src/https/heap_profiler.rs:53`

```rust
let _guard = self.mutex.lock().unwrap();
```

If the mutex is poisoned (a prior holder panicked), this `unwrap()` will panic, cascading the failure to every subsequent caller. In a server context, one bad request permanently bricks the profiler endpoint.

**Impact:** Cascading failure. A single panic (e.g., from a jemalloc error) permanently disables the profiler control path.
**Fix:** Use `lock().unwrap_or_else(|e| e.into_inner())` to recover from poison, or use `parking_lot::Mutex` which doesn't poison.

---

## Warning

### WR-01: Test writes files into source tree — `mod.rs:198-200`
**File:** `crates/api/src/https/mod.rs:198-200`

The test creates `src/https/test/cert.pem` and `src/https/test/key.pem` inside `$CARGO_MANIFEST_DIR`. These are **never cleaned up** — no `Drop` guard, no `defer`, no `finally`. Repeated test runs will fail on `fs::create_dir` (directory already exists). This also risks accidentally committing TLS key material to version control.

**Impact:** Test pollution, potential secret leakage if committed.
**Fix:** Use `tempfile::tempdir()` for ephemeral test artifacts instead of writing into the source tree. Add `src/https/test/` to `.gitignore` as a safety net.

---

### WR-02: `danger_accept_invalid_certs(true)` in test — `mod.rs:214`
**File:** `crates/api/src/https/mod.rs:214`

The test disables both hostname and certificate validation. While this is common for self-signed test certs, the `danger_accept_invalid_hostnames(true)` + `danger_accept_invalid_certs(true)` combo means the test doesn't actually validate *any* TLS behavior. The test proves the server accepts connections, but not that TLS is correctly configured.

**Impact:** Low — test-only code, but provides false confidence in TLS correctness.

---

### WR-03: Profiler errors returned as HTTP 200 — `heap_profiler.rs:39-40`
**File:** `crates/api/src/https/heap_profiler.rs:39-40`

Both success and failure paths return `Json(ControlProfileResponse { ... })` which maps to HTTP 200. Callers cannot distinguish success from failure without parsing the response body string.

**Impact:** Silent failures. Monitoring/alerting that checks HTTP status codes will miss profiler errors.
**Fix:** Return a 500 status code on the error path.

---

### WR-04: Log message misrepresents behavior — `mod.rs:122`
**File:** `crates/api/src/https/mod.rs:122`

```rust
info!("WARNING: TLS not configured. Consensus/DKG sensitive endpoints are disabled. Only serving public HTTP routes.");
```

This claims "Consensus/DKG sensitive endpoints are disabled," but looking at the `http_routes` router (lines 107-116), **all consensus and DKG endpoints are still registered and served over plain HTTP**. Only the TX endpoints (`/tx/submit_tx`, `/tx/get_tx_by_hash`) are excluded. The log message is factually wrong.

**Impact:** Operator confusion. An admin reading logs may believe consensus data is protected when it is not.
**Fix:** Correct the log message to reflect what is actually disabled (TX endpoints only).

---

### WR-05: Test sends `"action"` (singular), struct expects `"actions"` (plural) — `mod.rs:223` vs `set_failpoints.rs:16`
**File:** `crates/api/src/https/mod.rs:223` / `crates/api/src/https/set_failpoints.rs:16`

The test builds `map.insert("action", "return")` but `FailpointConf` declares `actions: String`. Without a `#[serde(alias = "action")]` attribute, the deserialization should fail with a 422 error. If the test passes, it may be because the failpoint is being set by the `fail_point!` macro's own mechanism or the test isn't actually reaching that assertion.

**Impact:** Either a latent bug in the API contract or a broken test that gives false confidence.
**Fix:** Align the field name between test and struct, or add `#[serde(alias = "action")]`.

---

### WR-06: Full epoch scan on every query — `consensus.rs:117`, `consensus.rs:263`
**File:** `crates/api/src/https/consensus.rs:117`, `consensus.rs:263`

Both `get_ledger_info_by_epoch` and `get_validator_count_by_epoch` call `consensus_db.get_all::<EpochByBlockNumberSchema>()` and then do a linear `.find()`. Similarly, `get_block_by_round` and `get_qc_by_round` fetch all blocks/QCs in an epoch via `get_range` and do a linear scan for the matching round.

**Impact:** Performance degradation as chain grows. These are unauthenticated endpoints, enabling a trivial resource exhaustion attack by issuing many queries.
**Fix:** Add indexed lookups or caching. At minimum, add rate limiting to these endpoints.

---

### WR-07: Consensus/DKG data exposed over plain HTTP without authentication — `mod.rs:107-114`
**File:** `crates/api/src/https/mod.rs:107-114`

All consensus and DKG endpoints (ledger info, block data, QC data, validator counts, DKG status, randomness values) are served over plain HTTP with zero authentication. This exposes:
- Current epoch/round (reveals chain tip position)
- Block hashes and parent relationships (reveals fork structure)
- Validator counts and DKG session metadata
- **Randomness values** — if these are used for leader election or other protocol decisions, exposing them over an unauthenticated channel could have protocol security implications

**Impact:** Information disclosure of consensus-internal state to any network observer.

---

## Info

### IN-01: `#[allow(dead_code)]` scattered across `heap_profiler.rs` — lines 11, 17, 18, 21, 51
**File:** `crates/api/src/https/heap_profiler.rs`

Multiple `#[allow(dead_code)]` annotations suggest the code is conditionally used but the feature gating isn't applied consistently. The `#[cfg(feature = "jemalloc-profiling")]` is applied inside the function body rather than on the struct/const definitions, leading to warnings that are suppressed rather than addressed.

**Fix:** Apply `#[cfg(feature = "jemalloc-profiling")]` to the struct and constants directly.

---

### IN-02: `env` import unused in `heap_profiler.rs:6`
**File:** `crates/api/src/https/heap_profiler.rs:6`

`use std::env` is imported but never used.

---

### IN-03: Hardcoded port in test — `mod.rs:202`
**File:** `crates/api/src/https/mod.rs:202`

The test binds to `127.0.0.1:5425`. If another process (or parallel test run) holds that port, the test will panic. Use port `0` and extract the bound address to avoid flaky tests.

---

### IN-04: Duplicate `ErrorResponse` struct — `consensus.rs:48` and `dkg.rs:67`
**File:** `crates/api/src/https/consensus.rs:48`, `crates/api/src/https/dkg.rs:67`

`ErrorResponse` is defined identically in both modules. This is a minor DRY violation.

---

## Summary

| Severity | Count | Key Themes |
|----------|-------|------------|
| **Critical** | 5 | Production `todo!()` panic, dead security middleware, unauthenticated admin endpoints, mutex poison cascade |
| **Warning** | 7 | Test hygiene, misleading logs, silent error swallowing, linear-scan performance, data exposure |
| **Info** | 4 | Dead code, unused imports, hardcoded test ports, minor DRY violations |

**Top 3 actions by risk-reduction ROI:**
1. Remove or gate the `submit_tx` route until the handler is implemented (CR-01)
2. Add authentication or localhost-binding to `/set_failpoint` and `/mem_prof` (CR-03, CR-04)
3. Correct the misleading log message and remove the non-functional `ensure_https` middleware (WR-04, CR-02)

### ⚔️ Intention Attacker

# Intention Attack Report — HTTP/HTTPS API Server Security Audit

## Findings Summary

| # | Finding | Severity | File |
|---|---------|----------|------|
| 1 | `submit_tx` is a live remote crash oracle | **Critical** | `tx.rs:28` |
| 2 | Unauthenticated failpoint injection grants arbitrary code-path control | **Critical** | `set_failpoints.rs:27` |
| 3 | `ensure_https` middleware is a dead guard — never rejects real traffic | **High** | `mod.rs:32-37` |
| 4 | Consensus/DKG data exposed without authentication on plain HTTP | **High** | `mod.rs:107-116` |
| 5 | Unauthenticated heap profiler toggle enables remote DoS | **High** | `heap_profiler.rs:36-44` |
| 6 | Misleading log message inverts the security posture | **Medium** | `mod.rs:122` |
| 7 | `get_all` full-table scan enables resource exhaustion | **Medium** | `consensus.rs:117`, `consensus.rs:263` |
| 8 | Profiler error masked as HTTP 200 | **Medium** | `heap_profiler.rs:39-40` |
| 9 | Test writes TLS material into source tree | **Medium** | `mod.rs:198-200` |

---

## Detailed Analysis

### 1. CRITICAL — `submit_tx` is a Remote Crash Oracle

**File:** `tx.rs:27-29`

```rust
pub async fn submit_tx(_request: TxRequest) -> Result<Json<SubmitResponse>, StatusCode> {
    todo!()
}
```

**Attack:** Any authenticated or unauthenticated client (when TLS is configured) that sends a `POST /tx/submit_tx` with a valid JSON body will trigger `todo!()` which calls `panic!()`. In a multi-threaded tokio runtime, this panics the task — and depending on the panic handler configuration, **can crash the entire node process**.

**Business logic flaw:** The system advertises a transaction submission endpoint and registers it in the router. An attacker who discovers this endpoint (trivial via path enumeration) gains a **deterministic, single-request denial-of-service** against any node with TLS enabled. Even if tokio catches the panic at the task level, the connection is dropped and the thread's stack is unwound, which under load becomes a resource exhaustion vector.

**The test at `mod.rs:242-243` appears to verify success**, but in practice the `todo!()` panic occurs *after* axum has already begun processing, meaning the test may either be silently broken or only passing due to timing/panic-catch behavior.

---

### 2. CRITICAL — Unauthenticated Failpoint Injection = Arbitrary Behavior Control

**File:** `set_failpoints.rs:26-38`, `mod.rs:115`

```rust
// Registered on http_routes — no TLS, no auth, no middleware
.route("/set_failpoint", post(set_fail_point_lambda))
```

```rust
pub async fn set_failpoint(request: FailpointConf) -> impl IntoResponse {
    match fail::cfg(&request.name, &request.actions) { ... }
}
```

**Attack:** The `/set_failpoint` endpoint is on the `http_routes` group — served on plain HTTP with **zero authentication**. The `fail::cfg()` call accepts **arbitrary failpoint names and arbitrary action strings**. The `fail` crate supports actions like `panic`, `sleep(N)`, `return(value)`, `off`, and even `delay(N)`.

An attacker with network access can:
1. **Inject `panic` actions** into any `fail_point!()` macro across the entire codebase → crash consensus, crash mempool, crash any critical path
2. **Inject `sleep(999999)` actions** → freeze consensus rounds, halt block production
3. **Inject `return` actions** → alter validation results, skip signature checks, or bypass any logic guarded by a failpoint

**This is the single most dangerous finding.** Even though it is feature-gated on `failpoints`, if a production binary is accidentally compiled with this feature (or if the feature is intentionally enabled for debugging in staging/testnet), any network-adjacent attacker gains **full control over the node's execution behavior**.

The feature gate at compile time is the **only** protection. There is no runtime guard, no auth check, no IP allowlist.

---

### 3. HIGH — `ensure_https` Middleware is Inert

**File:** `mod.rs:32-37`

```rust
async fn ensure_https(req: Request<Body>, next: Next) -> Response {
    if req.uri().scheme_str() != Some("https") {
        return Response::builder().status(400).body("HTTPS required".into()).unwrap();
    }
    next.run(req).await
}
```

**Flaw:** For incoming TCP connections handled by axum, `req.uri()` contains only the path (e.g., `/tx/submit_tx`), **not the full URI with scheme**. The `scheme_str()` method returns `None`, not `Some("http")`. Since `None != Some("https")`, the middleware **rejects ALL direct requests — including legitimate TLS connections**.

This means the `ensure_https` layer either:
- **Blocks all TX requests** even over valid TLS (making TX endpoints unusable), or
- **Is never reached** because the routing structure means axum doesn't apply it as expected

In either case, the middleware provides **no meaningful security**. The actual protection comes solely from the routing logic at lines 119-124 that excludes `https_routes` when TLS is not configured. The middleware creates a false sense of defense-in-depth.

---

### 4. HIGH — Consensus & DKG Data Unauthenticated on Plain HTTP

**File:** `mod.rs:107-116`

All of these endpoints are registered on `http_routes` with no authentication:

| Endpoint | Sensitive Data Exposed |
|----------|----------------------|
| `/dkg/status` | Current epoch, round, block number, participating node count |
| `/dkg/randomness/:block_number` | On-chain randomness values (hex encoded) |
| `/consensus/latest_ledger_info` | Latest epoch, round, block number, block hash |
| `/consensus/ledger_info/:epoch` | Historical ledger info per epoch |
| `/consensus/block/:epoch/:round` | Block IDs, parent IDs, internal structure |
| `/consensus/qc/:epoch/:round` | Quorum certificate data, commit info |
| `/consensus/validator_count/:epoch` | Validator set size per epoch |

**Attack:** An adversary with network access can enumerate the entire consensus history, learn the validator set size (useful for calculating BFT thresholds), extract randomness values (potentially useful for predicting or replaying randomness-dependent logic), and monitor real-time consensus progress — all without any authentication.

**The log message at line 122 is also misleading** — it says "Consensus/DKG sensitive endpoints are disabled" when TLS is not configured, but these endpoints are on `http_routes` which are **always served**. Only the TX endpoints are disabled. This inverts the operator's understanding of the security posture.

---

### 5. HIGH — Unauthenticated Heap Profiler Toggle → DoS

**File:** `heap_profiler.rs:36-44`, `mod.rs:116`

```rust
.route("/mem_prof", post(control_profiler_lambda))  // http_routes, no auth
```

**Attack:** An attacker can repeatedly toggle `POST /mem_prof {"enable": true}` to force jemalloc heap profiling on a production node. Heap profiling imposes **significant memory and CPU overhead** (typically 2-5x slowdown). Rapidly toggling profiling on/off can also cause:
- Memory pressure from accumulated profiling metadata
- Increased allocation latency affecting consensus timing
- Potential OOM if profiling data accumulates

**Compounding factor:** The error path returns HTTP 200 (`heap_profiler.rs:40`), so monitoring systems that check for non-200 responses will not detect failures or abuse.

---

### 6. MEDIUM — Misleading Security Log Creates Operator Blind Spot

**File:** `mod.rs:122`

```rust
info!("WARNING: TLS not configured. Consensus/DKG sensitive endpoints are disabled. Only serving public HTTP routes.");
```

**Flaw:** This log message is factually incorrect. When TLS is not configured:
- **Disabled:** only `/tx/submit_tx` and `/tx/get_tx_by_hash` (the `https_routes`)
- **Still served:** ALL consensus, DKG, failpoint, and profiler endpoints (the `http_routes`)

The log says "Consensus/DKG sensitive endpoints are disabled" but they are on `http_routes` and **remain fully active**. An operator reading this log would believe the node is safe to expose to the network without TLS, when in reality all sensitive read endpoints and dangerous write endpoints (failpoints, profiler) are wide open.

---

### 7. MEDIUM — Full Table Scan Enables Resource Exhaustion

**File:** `consensus.rs:117`, `consensus.rs:263`

```rust
let all_epoch_blocks = match consensus_db.get_all::<EpochByBlockNumberSchema>() {
```

**Attack:** `get_ledger_info_by_epoch` and `get_validator_count_by_epoch` both call `get_all::<EpochByBlockNumberSchema>()` which loads **every epoch-to-block-number mapping** into memory, then performs a linear scan. On a long-running chain with many epochs, this becomes an O(n) memory + CPU operation per request.

An attacker can issue concurrent requests to these endpoints to amplify memory pressure, since each request independently allocates the full result set. With no rate limiting and no authentication, this is a straightforward amplification DoS.

---

### 8. MEDIUM — Profiler Errors Masked as Success

**File:** `heap_profiler.rs:39-40`

```rust
Ok(_) => Json(ControlProfileResponse { response: "success".to_string() }),
Err(e) => Json(ControlProfileResponse { response: e }),  // Still HTTP 200!
```

Both success and failure return HTTP 200. Any monitoring, alerting, or client-side error handling that relies on HTTP status codes will be blind to profiler failures. An operator cannot distinguish between "profiling activated" and "profiling failed due to jemalloc not being compiled with profiling support" without parsing the response body.

---

### 9. MEDIUM — Test Code Writes Secrets into Source Tree

**File:** `mod.rs:198-200`

```rust
fs::create_dir(dir.clone() + "/src/https/test").unwrap();
fs::write(dir.clone() + "/src/https/test/cert.pem", cert_pem).unwrap();
fs::write(dir.clone() + "/src/https/test/key.pem", key_pem).unwrap();
```

**Flaw:** The test writes TLS private key material into `$CARGO_MANIFEST_DIR/src/https/test/`. If `.gitignore` does not exclude this path, private keys can be committed to version control. Even with `.gitignore`, the test does not clean up after itself (no `Drop` guard, no `finally` cleanup), so repeated test runs leave key material on disk in the source tree.

Additionally, the test uses a hardcoded port `5425` without checking for availability, causing flaky failures in CI environments with parallel test execution.

---

## Recommendations (Priority Order)

1. **Remove `submit_tx` endpoint entirely or replace `todo!()` with a proper `501 Not Implemented` response.** A `todo!()` in a network handler is a remote kill switch.

2. **Add authentication middleware to ALL endpoints.** At minimum, a shared secret / API key for operator endpoints. Consensus data should require node identity verification.

3. **Restrict `/set_failpoint` and `/mem_prof` to localhost-only binding** or behind an admin-authenticated route group, regardless of feature gating.

4. **Fix or remove the `ensure_https` middleware** — it doesn't work as designed and creates false confidence. If scheme enforcement is needed, check the `x-forwarded-proto` header or the connection's TLS state instead of `uri().scheme_str()`.

5. **Fix the misleading log message** at `mod.rs:122` to accurately reflect which endpoints remain exposed.

6. **Add pagination or caching** to the `get_all` queries in `consensus.rs` to prevent full-table-scan DoS.

7. **Return proper HTTP error codes** from `control_profiler` (use 500 for failures, not 200).

8. **Fix the test** to use `tempdir` and cleanup guards instead of writing into the source tree.

---

## [mock_consensus] Review bin/gravity_node/src/consensus/mock_consensus/ for: (1) mock.rs MockConse

### 🎯 Intention

Review bin/gravity_node/src/consensus/mock_consensus/ for: (1) mock.rs MockConsensus for block ID generation using DefaultHasher (non-cryptographic, predictable); (2) environment variable-driven configuration (MOCK_SET_ORDERED_INTERVAL_MS, MOCK_MAX_BLOCK_SIZE, MAX_EXECUTED_GAP) without validation; (3) the Condvar-based flow control between ordering and execution for potential deadlocks; (4) the process_epoch_change assert that could panic in production; (5) mempool.rs for transaction tracking correctness; (6) ensuring mock consensus cannot be accidentally enabled in production via MOCK_CONSENSUS env var.

### 🔍 Discovered Implementation

# Mock Consensus Implementation Analysis

## Files/Contracts Involved

| File | Description |
|------|-------------|
| `bin/gravity_node/src/consensus/mock_consensus/mod.rs` | Module declaration — exports `mempool` and `mock` submodules |
| `bin/gravity_node/src/consensus/mock_consensus/mock.rs` | Core `MockConsensus` struct: block production loop, execution tracking, epoch handling (277 lines) |
| `bin/gravity_node/src/consensus/mock_consensus/mempool.rs` | `Mempool` wrapper over `TxPool` with nonce-based transaction filtering (49 lines) |

---

## Execution Path

```
TxPool (external trait object)
    │
    ▼
Mempool::get_txns()              ── nonce-filtered transaction selection
    │
    ▼
MockConsensus::construct_block() ── DefaultHasher-based BlockId, proposer_index=0
    │
    ▼
BlockBufferManager::set_ordered_blocks()
    │
    ▼
[std::sync::Condvar backpressure: MAX_EXECUTED_GAP]
    │
    ▼
BlockBufferManager::get_executed_res()  ── waits for execution engine
    │
    ▼
BlockBufferManager::set_commit_blocks() ── finalizes block commit
    │
    ▼
process_epoch_change()           ── handles NewEpoch events
    │
    ▼
commit_txns (unbounded mpsc)     ── forwards committed TxnIds to mempool
```

---

## Key Functions

### `mock.rs` — `MockConsensus`

| Function | Signature | Description |
|----------|-----------|-------------|
| `new` | `async fn new(pool: Box<dyn TxPool>) -> Self` | Initializes with a hardcoded genesis `BlockId` (32-byte constant), calls `get_block_buffer_manager().init(0, block_number_to_block_id, 1)`, starts at epoch 1 |
| `construct_block` | `fn(block_number: u64, txns: Vec<VerifiedTxn>, attr: ExternalPayloadAttr, epoch: u64) -> ExternalBlock` | Hashes `txns` + `attr` via `DefaultHasher`, writes the resulting `u64` into bytes `[0..8]` of a 32-byte array (bytes `[8..32]` are zeroed), returns `ExternalBlock` with `proposer_index: Some(0)` |
| `check_and_construct_block` | `async fn(&Mutex<Mempool>, u64, ExternalPayloadAttr, u64) -> ExternalBlock` | Polls mempool in a loop (10ms sleep between polls), returns block if: (a) time gap > 1 second, (b) no new txns but buffer non-empty, or (c) txn count exceeds `max_txn_num` |
| `run` | `async fn run(mut self)` | Main event loop — spawns ordering task, commit-txn forwarding task, then loops receiving executed block results and committing them |
| `process_epoch_change` | `fn(&mut self, events: &[GravityEvent], block_number: u64)` | Iterates events; on `GravityEvent::NewEpoch(epoch, _)`, asserts sequential epoch via `assert_eq!`, then atomically stores new epoch and epoch start block number |

### `mempool.rs` — `Mempool`

| Function | Signature | Description |
|----------|-----------|-------------|
| `new` | `fn(pool_txns: Box<dyn TxPool>) -> Self` | Wraps a `TxPool` with an empty `next_sequence_numbers` map |
| `reset_epoch` | `fn(&mut self)` | Clears `next_sequence_numbers` HashMap |
| `get_txns` | `fn(&mut self, block_txns: &mut Vec<VerifiedTxn>, max_block_size: usize) -> bool` | Clones `next_sequence_numbers`, creates a filter closure that accepts txns with `seq_num >= next_nonce`, calls `pool_txns.best_txns(Some(filter), max_block_size)`, inserts each returned txn's `(sender, nonce+1)` into tracking map, breaks when `block_txns.len() >= max_block_size` |
| `commit_txns` | `fn(&mut self, _txns: &[TxnId])` | **Empty method body** — committed transactions are accepted but no state is modified |

---

## Environment Variable Configuration

| Variable | Location | Cached? | Default | Purpose |
|----------|----------|---------|---------|---------|
| `MOCK_SET_ORDERED_INTERVAL_MS` | `mock.rs:28` | Yes (`OnceLock`) | `200` | Sleep duration (ms) between ordering successive blocks |
| `MOCK_MAX_BLOCK_SIZE` | `mock.rs:36` | **No** — re-read on every call | `7000` | Max transactions per block |
| `MAX_EXECUTED_GAP` | `mock.rs:45` | Yes (`OnceLock`) | `16` | Max allowed lag between ordered and executed block numbers before backpressure engages |

All three use the pattern: `std::env::var(...).unwrap_or_else(|_| default).parse().unwrap_or(default)`. Invalid values silently fall back to defaults.

---

## State Changes

| Operation | What Changes |
|-----------|-------------|
| `MockConsensus::new` | Initializes `BlockBufferManager` global with genesis block at position 0, epoch 1 |
| `Mempool::get_txns` | Mutates `next_sequence_numbers`: inserts `(sender, nonce+1)` for each txn yielded |
| `Mempool::reset_epoch` | Clears entire `next_sequence_numbers` map |
| `set_ordered_blocks` | Writes ordered block into `BlockBufferManager` (external) |
| `set_commit_blocks` | Writes committed block with execution hash into `BlockBufferManager` (external) |
| Condvar consumer (`mock.rs:232-236`) | Updates `executed_jam_wait` mutex value to latest `block_number`, calls `notify_all()` |
| `process_epoch_change` | Atomically stores new `epoch` and `epoch_start_block_number` |
| `Mempool::commit_txns` | **No-op** — method body is empty |

---

## Concurrency Architecture

### Channels
- **`block_meta_tx/rx`**: bounded `tokio::sync::mpsc::channel(8)` — ordering task sends `ExternalBlockMeta`, main loop receives
- **`commit_txns_tx/rx`**: unbounded `tokio::sync::mpsc::unbounded_channel` — main loop sends committed `Vec<TxnId>`, dedicated task forwards to `Mempool::commit_txns` (which is a no-op)

### Backpressure (Condvar)
- **Field**: `executed_jam_wait: Arc<(Mutex<u64>, Condvar)>`
- **Producer** (ordering task, `mock.rs:174-188`): After ordering a block, acquires mutex, enters `while (block_number - *executed_number) > MAX_EXECUTED_GAP` spin-wait on `cvar.wait()`
- **Consumer** (main loop, `mock.rs:232-236`): After receiving execution result, acquires mutex, sets `*executed_number = block_number`, calls `cvar.notify_all()`
- The `std::sync::Mutex` is held across a `Condvar::wait()` call inside a `tokio::spawn`'d async task — this blocks the OS thread

### Epoch Transitions
- Ordering task checks `epoch` atomic on each iteration; on change, calls `release_inflight_blocks()`, resets mempool, reloads `epoch_start_block_number`
- Main loop calls `process_epoch_change` after each committed block

---

## External Dependencies

| Dependency | Usage |
|------------|-------|
| `block_buffer_manager::BlockBufferManager` (via `get_block_buffer_manager()`) | Global singleton for block ordering, execution result retrieval, and commit finalization |
| `block_buffer_manager::TxPool` (trait) | Transaction pool interface injected into `Mempool` |
| `gaptos::api_types` | Type definitions: `BlockId`, `ExternalBlock`, `ExternalBlockMeta`, `ExternalPayloadAttr`, `VerifiedTxn`, `GravityEvent`, `ExternalAccountAddress`, `TxnHash` |
| `std::hash::DefaultHasher` | Non-cryptographic `SipHash`-based hasher used for `BlockId` generation |
| `tokio` | Async runtime: `tokio::sync::Mutex`, `tokio::sync::mpsc`, `tokio::time::sleep`, `tokio::spawn` |

---

## Assertions and Panics

| Location | Type | Condition | Effect |
|----------|------|-----------|--------|
| `mock.rs:270` | `assert_eq!` | `current_epoch != new_epoch - 1` | Panics if epoch transition is non-sequential |
| `mock.rs:225` | `panic!` | `get_executed_res` returns error not containing `"get_executed_res timeout"` | Panics on non-timeout execution errors; timeouts are retried in a loop |

---

## Block ID Generation Detail

`construct_block` (`mock.rs:76-88`) produces a `BlockId` as follows:
1. Creates `DefaultHasher::new()` (Rust's `SipHash-1-3`, deterministic within a process but seed may vary across builds/versions)
2. Hashes the transaction list and payload attributes
3. Calls `hasher.finish()` → `u64`
4. Copies 8 bytes (big-endian) into `bytes[0..8]` of a 32-byte array
5. `bytes[8..32]` remain `0x00`
6. Wraps as `BlockId(bytes)`

### 🛠️ Ground Review

# Ground Review: `mock_consensus` Module

## Critical

### 1. `std::sync::Mutex` + `Condvar` blocks tokio worker thread (`mock.rs:173-188`)

```rust
let mut executed_number = lock.lock().unwrap();
while (block_number - *executed_number) > get_max_executed_gap() {
    executed_number = cvar.wait(executed_number).unwrap();
}
```

A `std::sync::Condvar::wait()` call inside a `tokio::spawn`'d async task blocks the OS thread backing the tokio runtime. Under high backpressure (large gap between ordered and executed blocks), this starves the tokio thread pool, potentially deadlocking the entire runtime if all worker threads are blocked simultaneously. The consumer side (lines 231-236) is on the main `run()` future, which is also async — if the runtime has no free threads to poll it, the `notify_all()` never fires.

**Recommendation:** Replace `std::sync::{Mutex, Condvar}` with `tokio::sync::Notify` or a `tokio::sync::Semaphore` for async-compatible backpressure.

---

### 2. `assert_eq!` in production path crashes the node (`mock.rs:270`)

```rust
assert_eq!(self.epoch.load(std::sync::atomic::Ordering::SeqCst), *epoch - 1);
```

If a non-sequential epoch event arrives (e.g., due to a reorg, a bug in the execution engine, or duplicate events), the entire node panics. `assert_eq!` is a debugging invariant, not a production guard.

**Recommendation:** Replace with an `if` check that logs an error and either skips the event or triggers a graceful shutdown.

---

### 3. Integer underflow on epoch assertion (`mock.rs:270`)

If `*epoch` is `0` (a zero-valued epoch event from a malformed or adversarial source), `*epoch - 1` underflows to `u64::MAX`, causing the assertion to fail with a confusing message. This is a subset of the assert issue above but worth noting as a separate vector.

---

## Warning

### 4. Non-cryptographic `BlockId` generation (`mock.rs:76-81`)

```rust
let mut hasher = DefaultHasher::new();
txns.hash(&mut hasher);
attr.hash(&mut hasher);
let block_id = hasher.finish();
let mut bytes = [0u8; 32];
bytes[0..8].copy_from_slice(&block_id.to_be_bytes());
```

`DefaultHasher` is `SipHash-1-3`, which is not collision-resistant. Only 8 of 32 bytes are populated — yielding 64 bits of entropy, not 256. Across a 32-byte `BlockId` space, this makes collisions feasible at ~2³² blocks. For a mock/test consensus this is acceptable, but there must be a hard gate preventing this code from running in production (see finding #8).

---

### 5. `MOCK_MAX_BLOCK_SIZE` re-read on every block (`mock.rs:35-40`)

```rust
fn get_max_txn_num() -> usize {
    std::env::var("MOCK_MAX_BLOCK_SIZE")
        .unwrap_or_else(|_| "7000".to_string())
        .parse()
        .unwrap_or(7000)
}
```

Unlike `ORDERED_INTERVAL_MS` and `MAX_EXECUTED_GAP` (which use `OnceLock`), this is called on every block construction. `std::env::var` acquires a global lock on the environment. At high block rates this is needless contention. The inconsistency with the other two env-var helpers also suggests this was unintentional.

**Recommendation:** Wrap in `OnceLock` to match the other two, or document why dynamic reload is intentional.

---

### 6. Unbounded channel for committed transactions (`mock.rs:197-198`)

```rust
let (commit_txns_tx, mut commit_txns_rx) =
    tokio::sync::mpsc::unbounded_channel::<Vec<TxnId>>();
```

The receiver calls `Mempool::commit_txns`, which is a **no-op** (line 47). This means:
- Memory is allocated for every `Vec<TxnId>` and never acted upon.
- The unbounded channel has no backpressure — under sustained load, this is an unbounded memory leak of `TxnId` vectors queuing faster than they're drained (they are drained, but into a no-op, so the only bound is receive speed).

**Recommendation:** Either implement `commit_txns` to clean up `next_sequence_numbers` (removing committed nonces), or remove the channel and the spawn entirely to eliminate dead code.

---

### 7. `.unwrap()` on `txn_status` without defensive check (`mock.rs:254-256`)

```rust
.txn_status
.as_ref()
.as_ref()
.unwrap()
```

If the execution engine returns `None` for `txn_status`, this panics. Even in a mock context, execution errors or empty blocks could produce this. A double `.as_ref()` chain followed by `.unwrap()` is fragile.

---

### 8. No compile-time or runtime guard against production use

There is no `#[cfg(test)]`, `#[cfg(feature = "mock")]`, or runtime environment check that prevents `MockConsensus` from being instantiated in a production binary. The module is unconditionally compiled and exported. If a production node is accidentally started with the mock consensus path active, the weak `BlockId` generation (finding #4) and the no-op `commit_txns` (finding #6) would silently degrade security and correctness.

**Recommendation:** Gate behind a cargo feature flag (e.g., `feature = "mock-consensus"`) that is excluded from release profiles.

---

## Info

### 9. `next_sequence_numbers` cloned on every `get_txns` call (`mempool.rs:28`)

```rust
let next_txns = self.next_sequence_numbers.clone();
```

The `HashMap` is cloned to move into the filter closure. For a large number of unique senders, this is an O(n) allocation on every block. An `Rc`/`Arc` wrapper or a reference-based filter design would avoid this.

---

### 10. Timestamp uses second-level granularity (`mock.rs:155-158`)

```rust
ts: SystemTime::now()
    .duration_since(SystemTime::UNIX_EPOCH)
    .unwrap()
    .as_secs(),
```

The `> 1` second time-gap check on line 112 (`time_gap > 1`) combined with second-precision timestamps means blocks can include up to ~2 seconds of wall-clock drift before triggering early emission. This is coarse but likely acceptable for a mock.

---

### 11. Silent fallback on invalid environment variables

All three env-var helpers silently fall back to defaults on parse failure. A misconfigured `MOCK_MAX_BLOCK_SIZE=abc` produces no log warning — the operator has no indication their configuration was ignored.

---

## Summary

| Severity | Count | Key Themes |
|----------|-------|------------|
| **Critical** | 3 | Blocking std Mutex in async runtime; production panics via `assert_eq!`; integer underflow |
| **Warning** | 5 | Weak BlockId; inconsistent env-var caching; unbounded channel to no-op; unwrap on execution result; no production gate |
| **Info** | 3 | HashMap clone per block; coarse timestamps; silent env-var fallback |

The most actionable items are (1) replacing the `std::sync::Condvar` with an async-compatible primitive, (2) removing or guarding the `assert_eq!` in `process_epoch_change`, and (8) gating the entire module behind a feature flag to prevent accidental production use.

### ⚔️ Intention Attacker

# Mock Consensus Security Audit — Attack Findings

---

## CRITICAL

### 1. Mock Consensus Activatable in Production via Environment Variable
**Location:** `bin/gravity_node/src/main.rs:270`

```rust
if std::env::var("MOCK_CONSENSUS").unwrap_or("false".to_string()).parse::<bool>().unwrap() {
```

**Vulnerability:** Any deployed production binary can be silently switched to mock consensus by setting `MOCK_CONSENSUS=true`. There is:
- No compile-time gate (`#[cfg(feature = "mock")]` or `#[cfg(debug_assertions)]`)
- No log warning emitted when mock mode activates
- No environment allowlist (e.g., rejecting activation when `ENV=production`)

**Impact:** An attacker with access to environment variables (container orchestration, CI/CD, compromised deploy config) can downgrade a production validator from BFT consensus to single-node mock consensus. This **eliminates all Byzantine fault tolerance** — the attacker becomes the sole block proposer with `proposer_index: 0`, can censor transactions, reorder them arbitrarily, and produce blocks at will. The rest of the network would not detect this if the node is a full node rather than a validator, but if it's a validator, it breaks the consensus quorum assumptions.

---

### 2. `commit_txns` Is a No-Op — Committed Transactions Are Never Removed from the Pool
**Location:** `mempool.rs:47`

```rust
pub fn commit_txns(&mut self, _txns: &[TxnId]) {}
```

**Vulnerability:** After a block is committed, the committed transactions are sent to the mempool via an unbounded channel (`mock.rs:263`), but `commit_txns` does nothing. The underlying `TxPool` is never notified that these transactions were committed. This means:

1. **Transaction replay in future blocks:** `best_txns()` will continue returning already-committed transactions. The only guard is the `next_sequence_numbers` nonce filter in `get_txns`, but this is **reset on every epoch change** (`reset_epoch` clears the map at `mempool.rs:23`). After an epoch boundary, all previously committed transactions become eligible for re-inclusion.

2. **Unbounded memory growth:** The `TxPool` accumulates transactions indefinitely since `remove_txns()` is never called. Over time, this causes memory exhaustion on long-running nodes.

3. **Unbounded channel as amplifier:** The `commit_txns_tx` channel (`mock.rs:197`) is unbounded. Even though the consumer is a no-op, the unbounded channel itself means the producer (main loop) can enqueue `Vec<TxnId>` faster than the consumer drains them under load, adding further memory pressure.

---

### 3. Non-Cryptographic Block ID with 75% Zero Bytes — Collision and Predictability
**Location:** `mock.rs:76-81`

```rust
let mut hasher = DefaultHasher::new();   // SipHash-1-3
txns.hash(&mut hasher);
attr.hash(&mut hasher);
let block_id = hasher.finish();          // 64-bit output
let mut bytes = [0u8; 32];
bytes[0..8].copy_from_slice(&block_id.to_be_bytes());
// bytes[8..32] = 0x00 — 24 zero bytes
```

**Vulnerability:** Three compounding issues:

- **64-bit collision space:** `BlockId` is a 256-bit type, but only 64 bits carry entropy. The birthday-bound collision probability reaches 50% at ~2³² blocks (~4 billion). More critically, `get_executed_res` (`block_buffer_manager.rs:574`) uses `assert_eq!(id, &block_id)` — a collision causes a **node panic**.
- **Deterministic within a process:** `DefaultHasher` (SipHash) uses keys that are fixed per compilation/process. An attacker who knows the binary can precompute block IDs for any given transaction set.
- **`bytes[8..32]` are always zero:** This is a distinguishing fingerprint. Any system inspecting `BlockId` values can trivially identify mock-consensus-produced blocks, and the zero-padding reduces effective entropy for any downstream use.

---

## HIGH

### 4. `std::sync::Mutex` Held Across `Condvar::wait()` Inside `tokio::spawn` — Thread Starvation
**Location:** `mock.rs:173-188`

```rust
// Inside a tokio::spawn'd async task:
let (lock, cvar) = executed_jam_wait.as_ref();
let mut executed_number = lock.lock().unwrap();
while (block_number - *executed_number) > get_max_executed_gap() {
    executed_number = cvar.wait(executed_number).unwrap();  // BLOCKS OS THREAD
}
```

**Vulnerability:** This uses `std::sync::Mutex` + `std::sync::Condvar` (blocking primitives) inside an async task running on the tokio runtime. When the execution engine falls behind by `MAX_EXECUTED_GAP` blocks, this **blocks the entire OS thread** that the tokio worker is using. Under the default tokio multi-threaded runtime, this occupies one of a limited number of worker threads. If multiple tasks hit this path or if the thread pool is small, this leads to:

- **Thread pool exhaustion:** Other async tasks (including the execution pipeline that would *resolve* the backpressure) can starve, creating a **deadlock-like livelock**.
- **Cascading latency:** All other tasks scheduled on the blocked worker thread experience unbounded delays.

The consumer side (`mock.rs:232-236`) also uses `std::sync::Mutex`, but only briefly, so it's less dangerous — however, if the consumer task happens to be scheduled on the same blocked thread, a true deadlock occurs.

---

### 5. `process_epoch_change` Uses `assert_eq!` — Production Panic on Non-Sequential Epoch
**Location:** `mock.rs:270`

```rust
assert_eq!(self.epoch.load(std::sync::atomic::Ordering::SeqCst), *epoch - 1);
```

**Vulnerability:** If a `GravityEvent::NewEpoch` event arrives with a non-sequential epoch number (e.g., due to a bug in the execution engine, an event replay, or epoch skipping during recovery), the node **panics unconditionally**. This is:

- **Unrecoverable:** The node crashes and, on restart, may hit the same state and crash again (crash loop).
- **Externally triggerable:** If the execution engine or event source can be influenced to emit an out-of-sequence epoch event, this becomes a remote denial-of-service vector against the consensus node.
- **Missing in `#[cfg(debug_assertions)]`:** `assert_eq!` compiles into release builds. This is not a debug-only check.

---

### 6. TOCTOU Race on Epoch Check in Ordering Task
**Location:** `mock.rs:142-152`

```rust
let mut current_epoch = epoch.load(std::sync::atomic::Ordering::SeqCst);
loop {
    if current_epoch != epoch.load(std::sync::atomic::Ordering::SeqCst) {
        current_epoch = epoch.load(std::sync::atomic::Ordering::SeqCst);
        // ... reset mempool, reload block_number ...
    }
    block_number += 1;
    // ... construct block with current_epoch ...
```

**Vulnerability:** The epoch is checked via two separate atomic loads (lines 144–145). Between the comparison and the reload, the epoch could change again (e.g., two rapid epoch changes). More critically, the epoch is checked at the **top of the loop**, but `check_and_construct_block` (which can take >1 second due to polling) runs with the **stale** `current_epoch`. If an epoch change occurs during block construction:

- The block is constructed with the old epoch number
- `set_ordered_blocks` may silently drop it (old-epoch block) or the execution engine processes it under the wrong epoch
- The `block_number` may be wrong because `epoch_start_block_number` was updated by the main loop concurrently

This is a classic TOCTOU: the epoch is checked, then used non-atomically across an `await` boundary.

---

## MEDIUM

### 7. `Mempool::reset_epoch` Discards Nonce Tracking — Post-Epoch Transaction Replay
**Location:** `mempool.rs:22-24`, called from `mock.rs:148`

```rust
pub fn reset_epoch(&mut self) {
    self.next_sequence_numbers.clear();
}
```

**Vulnerability:** On epoch change, the entire nonce tracking map is cleared. Since `commit_txns` is a no-op (finding #2), the underlying `TxPool` still contains all previously seen transactions. After the reset, `get_txns` will accept transactions starting from nonce 0 for every account, meaning **every uncommitted transaction in the pool becomes eligible for re-inclusion**, including transactions that were already committed in the previous epoch. This can lead to duplicate transaction execution if the execution engine doesn't independently enforce nonce ordering.

### 8. `MOCK_MAX_BLOCK_SIZE` Re-Read on Every Block — Runtime Manipulation
**Location:** `mock.rs:35-40`

```rust
fn get_max_txn_num() -> usize {
    std::env::var("MOCK_MAX_BLOCK_SIZE")
        .unwrap_or_else(|_| "7000".to_string())
        .parse()
        .unwrap_or(7000)
}
```

**Vulnerability:** Unlike `MOCK_SET_ORDERED_INTERVAL_MS` and `MAX_EXECUTED_GAP` (which use `OnceLock`), `MOCK_MAX_BLOCK_SIZE` is re-read from the environment on **every block construction**. An attacker who can modify the process environment at runtime (e.g., via `/proc/<pid>/environ` manipulation on Linux, or a co-located process) can:
- Set it to `0` → blocks are produced with zero transactions, halting chain progress
- Set it to `usize::MAX` → a single block attempts to include all pool transactions, causing memory spikes and execution timeouts
- Oscillate the value to create unpredictable block sizes

### 9. Arithmetic Underflow in Backpressure Check
**Location:** `mock.rs:176`

```rust
let large_gap = block_number - *executed_number;
```

**Vulnerability:** If `executed_number` somehow exceeds `block_number` (e.g., due to the epoch TOCTOU race where `block_number` is reset but `executed_number` retains the old value), this unsigned subtraction **wraps to `u64::MAX`**, causing the backpressure `while` loop to spin indefinitely (since `u64::MAX > MAX_EXECUTED_GAP`). The `Condvar::wait` would never be satisfied because no execution result will produce a block number large enough to close the gap. This results in a **permanent thread hang**.

---

## Summary

| # | Severity | Finding | Root Cause |
|---|----------|---------|------------|
| 1 | **Critical** | Mock consensus activatable in production | No compile-time or runtime guard on `MOCK_CONSENSUS` env var |
| 2 | **Critical** | `commit_txns` is a no-op — txns never removed from pool | Empty method body; `TxPool::remove_txns` never called |
| 3 | **Critical** | Non-cryptographic 64-bit Block ID in 256-bit field | `DefaultHasher` (SipHash) + 24 zero-padded bytes |
| 4 | **High** | `std::sync::Mutex` blocks tokio worker threads | Blocking sync primitives in async context |
| 5 | **High** | `assert_eq!` panics node on non-sequential epoch | Debug assertion in release code path |
| 6 | **High** | TOCTOU race on epoch transitions | Non-atomic epoch check + async block construction |
| 7 | **Medium** | Nonce tracking reset enables post-epoch tx replay | `reset_epoch` clears map while pool retains txns |
| 8 | **Medium** | `MOCK_MAX_BLOCK_SIZE` mutable at runtime | No `OnceLock` caching; re-read per block |
| 9 | **Medium** | Unsigned underflow in backpressure arithmetic | `block_number - executed_number` with no underflow guard |

---

## [bootstrap_and_network] Review crates/api/src/bootstrap.rs and crates/api/src/network.rs for: (1) config

### 🎯 Intention

Review crates/api/src/bootstrap.rs and crates/api/src/network.rs for: (1) config loading with panic on missing file; (2) the init_block_buffer_manager historical block reconstruction logic and the commented-out has_large optimization; (3) DKG and JWK consensus runtime initialization with proper error handling; (4) the mutual_authentication enforcement for validator networks; (5) the network builder role type assignment; (6) the mock_mempool_client_sender test helper that generates transactions with random keys; (7) channel capacity choices (mpsc::channel(1)) that could cause backpressure issues.

### 🔍 Discovered Implementation

# Implementation Analysis: `bootstrap.rs` and `network.rs`

## Files Involved

| File | Description |
|------|-------------|
| `crates/api/src/bootstrap.rs` | Node bootstrap: config loading, DKG/JWK consensus init, mempool init, block buffer reconstruction |
| `crates/api/src/network.rs` | Network configuration builders, mutual auth enforcement, mock mempool helper, network interface wiring |
| `crates/api/src/consensus_api.rs` | Channel creation site for the `mpsc::channel(1)` instances |
| `aptos-core/consensus/src/consensusdb/mod.rs` | `get_range_with_filter` and `get_max_epoch` on `ConsensusDB` |
| `aptos-core/consensus/src/consensusdb/schema/block/mod.rs` | `BlockNumberSchema` definition |
| `crates/block-buffer-manager/src/block_buffer_manager.rs` | `BlockBufferManager::init()` — consumes the reconstructed block map |

---

## 1. Config Loading with Panic on Missing File

**Function:** `check_bootstrap_config` (bootstrap.rs:53–71)

```rust
pub fn check_bootstrap_config(node_config_path: Option<PathBuf>) -> NodeConfig
```

**Execution path:**
1. Calls `.expect()` on the `Option<PathBuf>` — **panics** if `None` with message `"Config is required to launch node"`.
2. Checks `config_path.exists()` — **panics** if the file doesn't exist on disk.
3. Calls `NodeConfig::load_from_path(config_path.clone())` — **panics** via `.unwrap_or_else()` if parsing fails.

**State changes:** None. Pure config loader — returns `NodeConfig` or panics the process.

**Observations:** All three failure modes use `panic!()` / `.expect()`, making this a fail-fast startup gate. There is no `Result` return — callers cannot handle errors gracefully.

---

## 2. `init_block_buffer_manager` — Historical Block Reconstruction

**Function:** `init_block_buffer_manager` (bootstrap.rs:272–321)

```rust
pub async fn init_block_buffer_manager(consensus_db: &Arc<ConsensusDB>, latest_block_number: u64)
```

**Execution path:**

1. Computes `start_block_number = latest_block_number.saturating_sub(256)` (the `RECENT_BLOCKS_RANGE` constant at line 46).
2. Calls `consensus_db.get_max_epoch()` — this does a reverse iterator seek on the `BlockSchema` column family, returning the highest epoch or defaulting to `1` if empty.
3. Iterates **all epochs from `max_epoch` down to 1** (`(1..=max_epoch).rev()`).
4. For each epoch, constructs a full-range scan key: `(epoch_i, HashValue::zero())` to `(epoch_i, HashValue::new([0xFF; 32]))`.
5. Calls `consensus_db.get_range_with_filter::<BlockNumberSchema, _>(...)` which:
   - Sets RocksDB lower/upper bound on the encoded key
   - Collects **all** key-value pairs in that range first
   - Then applies the filter closure: `*block_number >= start_block_number && *block_number <= latest_block_number`
6. For each result `((epoch, block_id), block_number)`:
   - If `block_number` is not yet in the map, inserts `block_number -> (epoch, BlockId)`.
   - If `block_number` already exists, **only replaces** if the new `epoch` is strictly greater (keeps the highest-epoch mapping for a given block number).
7. If `start_block_number == 0`, manually inserts genesis: `0 -> (0, GENESIS_BLOCK_ID)`.
8. Calls `get_block_buffer_manager().init(latest_block_number, block_number_to_block_id, max_epoch).await`.

**The commented-out `has_large` optimization (lines 281, 295, 309–311):**

```rust
// let mut has_large = false;
...
// has_large = true;
...
// if !has_large {
//     break;
// }
```

This was an early-exit optimization: if a given epoch produced zero results matching the filter, the loop would `break` and stop scanning older epochs. With it commented out, **every epoch from `max_epoch` down to 1 is scanned unconditionally**, regardless of whether results are found. The TODO at line 280 reads `"TODO(graivity_lightman): Fix this"`.

**Performance implication of the commented-out code:** The `get_range_with_filter` method collects **all** entries per epoch into a Vec before filtering. With many epochs and a narrow 256-block window, this performs full scans of every epoch's BlockNumber column family entries. The early-exit would have skipped epochs that have no blocks in the target range.

---

## 3. DKG and JWK Consensus Runtime Initialization

### DKG Runtime: `create_dkg_runtime` (bootstrap.rs:102–144)

```rust
pub fn create_dkg_runtime(
    node_config: &mut NodeConfig,
    event_subscription_service: &mut EventSubscriptionService,
    dkg_network_interfaces: Option<ApplicationNetworkInterfaces<DKGMessage>>,
    vtxn_pool: &VTxnPoolState,
) -> Option<Runtime>
```

**Execution path:**
1. If `node_config.base.role.is_validator()`:
   - Subscribes to reconfigurations — `.expect()` on failure (**panics**).
   - Subscribes to `"0x1::dkg::DKGStartEvent"` — `.expect()` on failure (**panics**).
   - Wraps both in `Some(...)`.
2. If not a validator: `dkg_subscriptions = None`.
3. If `dkg_network_interfaces` is `Some`:
   - Unwraps `dkg_subscriptions` via `.expect()` — **panics** if subscriptions are `None` (would happen if network interfaces are provided for a non-validator).
   - Unwraps `node_config.validator_network.as_ref().unwrap()` — **panics** if no validator network is configured.
   - Calls `start_dkg_runtime(...)` and returns `Some(runtime)`.
4. If `dkg_network_interfaces` is `None`: returns `None`.

**Invariant assumed:** DKG network interfaces are only provided for validator nodes. If a non-validator somehow receives `Some(interfaces)`, the `.expect()` at line 123–124 will panic.

### JWK Consensus: `init_jwk_consensus` (bootstrap.rs:210–231) → `start_jwk_consensus_runtime` (bootstrap.rs:175–208)

```rust
pub fn init_jwk_consensus(...) -> Runtime
```

**Execution path:**
1. `init_jwk_consensus` subscribes to reconfigurations (`.expect()` — **panics** on failure).
2. Subscribes to `"0x1::jwks::ObservedJWKsUpdated"` events (`.expect()` — **panics** on failure).
   - Note: the error message at line 224 says `"must subscribe to DKG events"` — this is a **copy-paste error** in the message string; it's actually subscribing to JWK events.
3. Calls `start_jwk_consensus_runtime(...)` with `Some(...)` for both parameters.

**Inside `start_jwk_consensus_runtime`:**
1. If `jwk_consensus_network_interfaces` is `Some`:
   - Unwraps `jwk_consensus_subscriptions` via `.expect()` — **panics** if `None`.
   - Unwraps `node_config.validator_network.as_ref().unwrap()` — **panics** if no validator network.
   - Calls `aptos_jwk_consensus::start_jwk_consensus_runtime(...)`.
2. If `None`: `jwk_consensus_runtime = None`.
3. **Line 207:** `jwk_consensus_runtime.expect("JWK consensus runtime must be started")` — **panics** if the runtime was not created.

**Observation:** `init_jwk_consensus` always passes `Some(...)` for the network interfaces, so the `None` branch at line 205 is only reachable via direct calls to `start_jwk_consensus_runtime` with `None`. The TODO at line 218 notes validators should be the only ones subscribing to reconfig events, but this is not currently enforced.

---

## 4. Mutual Authentication Enforcement

**Function:** `extract_network_configs` (network.rs:38–48)

```rust
pub fn extract_network_configs(node_config: &NodeConfig) -> Vec<NetworkConfig>
```

**Execution path:**
1. Collects all `full_node_networks` into a `Vec<NetworkConfig>`.
2. If `node_config.validator_network` is `Some`:
   - Checks `network_config.mutual_authentication` — if `false`, **panics** with `"Validator networks must always have mutual_authentication enabled!"`.
   - Pushes the validator network config to the list.
3. Returns the combined list.

**Observations:**
- Mutual authentication is **only enforced for validator networks**. Full node networks in `full_node_networks` are **not checked** — they can have `mutual_authentication` set to `false` without triggering a panic.
- This is a startup-time check only. There is no runtime re-validation.

---

## 5. Network Builder Role Type Assignment

**Function:** `register_client_and_service_with_network` (network.rs:177–192)

```rust
pub fn register_client_and_service_with_network<T>(
    network_builder: &mut NetworkBuilder,
    network_id: NetworkId,
    network_config: &NetworkConfig,
    application_config: NetworkApplicationConfig,
    allow_out_of_order_delivery: bool,
) -> ApplicationNetworkHandle<T>
```

**Execution path:**
1. Calls `network_builder.add_client_and_service(...)` which returns `(NetworkSender<T>, NetworkEvents<T>)`.
2. Wraps these into an `ApplicationNetworkHandle<T>` with the `network_id`.

**Role assignment:** The `NetworkBuilder` type itself is imported from `gaptos::aptos_network_builder::builder::NetworkBuilder` — it is **not defined in this codebase**. The role (validator vs. full node) is determined by the `NetworkId` passed in (`NetworkId::Validator`, `NetworkId::Vfn`, `NetworkId::Public`), which is derived from `NetworkConfig::network_id`. The actual `NetworkBuilder` construction happens upstream in the dependency.

The `network_id` flows from `extract_network_configs` → `NetworkConfig::network_id` → used in `create_network_interfaces` and `register_client_and_service_with_network`.

---

## 6. `mock_mempool_client_sender` Test Helper

**Function:** `mock_mempool_client_sender` (network.rs:112–134)

```rust
pub async fn mock_mempool_client_sender(mut mc_sender: aptos_mempool::MempoolClientSender)
```

**Execution path (infinite loop):**
1. Generates a **random** `AccountAddress` once via `AccountAddress::random()` (line 113).
2. Loops forever:
   - Creates a `SignedTransaction` with:
     - `RawTransaction::new_script(...)` using the random address, incrementing `seq_num`, empty `Script`, zero gas, zero max gas, expiration = `now + 60 seconds`, `ChainId::test()`.
     - Public key: `Ed25519PrivateKey::generate_for_testing().public_key()` — a **new random key each iteration**.
     - Signature: `Ed25519Signature::try_from(&[1u8; 64][..]).unwrap()` — a **hardcoded constant** (64 bytes of `0x01`).
   - Creates a `oneshot::channel()` and **drops the receiver** immediately (`_receiver`).
   - Sends `MempoolClientRequest::SubmitTransaction(txn, sender)` via the mpsc sender. Uses `let _ =` so send errors are silently ignored.
   - Sleeps 1 second.

**Observations:**
- The signature is **not cryptographically valid** — it's a fixed byte pattern, not derived from the private key. This is test-only code (`#[allow(dead_code)]`, comment says "used for UT").
- A new random private key is generated per iteration but the public key doesn't match the account address (which is set once).
- The oneshot receiver is dropped, meaning whatever processes the transaction will get a `Canceled` error if it tries to send a response.

---

## 7. Channel Capacity Choices — `mpsc::channel(1)`

All capacity-1 channels are created in `crates/api/src/consensus_api.rs`:

| Line | Channel | Usage |
|------|---------|-------|
| **266** | `(consensus_to_mempool_sender, consensus_to_mempool_receiver)` | Carries `QuorumStoreRequest` from consensus to mempool. Sender goes to `start_consensus()`, receiver goes to `init_mempool()`. |
| **267** | `(notification_sender, notification_receiver)` | Creates `MempoolNotificationListener` from the receiver. Sender wrapped in `ConsensusNotifier`. Used for commit notifications from consensus to mempool. |
| **273** | `(_mempool_client_sender, _mempool_client_receiver)` | Both prefixed with `_` — the sender is unused, the receiver is passed to `init_mempool()` as `_mempool_client_receiver`. This channel is effectively a placeholder/dead channel. |

**Additional capacity-1 channels elsewhere:**

| File | Line | Usage |
|------|------|-------|
| `crates/block-buffer-manager/src/block_buffer_manager.rs` | 784 | `(tx, rx)` — used as a one-shot persist notification signal within `push_commit_blocks`. Capacity-1 is intentional as a binary signal. |
| `aptos-core/consensus/src/epoch_manager.rs` | 249 | `(sync_info_tx, sync_info_rx)` — sync info channel within `EpochManager` construction. |

**Data flow for the critical channels (consensus_api.rs:266–267):**

```
consensus runtime
    └─ consensus_to_mempool_sender ──[capacity 1]──► consensus_to_mempool_receiver ──► init_mempool()
    └─ ConsensusNotifier(notification_sender) ──[capacity 1]──► MempoolNotificationListener(notification_receiver) ──► init_mempool()
```

Both active channels have a buffer of exactly 1 message. The `consensus_to_mempool_sender` carries `QuorumStoreRequest` messages (batch transaction pulls from quorum store), and the `notification_sender` carries consensus commit notifications to mempool. A capacity of 1 means the sender blocks as soon as one message is in-flight and unprocessed.

---

## External Dependencies Summary

| Import Source | What's Used |
|---|---|
| `aptos_consensus` | `ConsensusDB`, `BlockNumberSchema`, `ConsensusMsg`, `StorageWriteProxy`, `QuorumStoreDB`, `consensus_provider::start_consensus` |
| `aptos_mempool` | `CoreMempool`, `MempoolClientRequest`, `MempoolSyncMsg`, `QuorumStoreRequest`, `bootstrap` |
| `gaptos::aptos_dkg_runtime` | `start_dkg_runtime`, `DKGMessage`, network interface protocols |
| `gaptos::aptos_jwk_consensus` | `start_jwk_consensus_runtime`, `JWKConsensusMsg`, network interface protocols |
| `gaptos::aptos_network_builder` | `NetworkBuilder` (external — not in this repo) |
| `gaptos::aptos_network` | `ProtocolId`, `NetworkApplicationConfig`, `NetworkClient`, `NetworkServiceEvents`, `PeersAndMetadata` |
| `block_buffer_manager` | `get_block_buffer_manager()`, `TxPool`, `BlockBufferManager::init()` |

### 🛠️ Ground Review

# Ground Review (Blue Team) — Code Quality & Security Audit

**Scope:** `crates/api/src/bootstrap.rs`, `crates/api/src/network.rs`, `crates/api/src/consensus_api.rs`

---

## Critical

### 1. Unbounded Full-DB Scan in `init_block_buffer_manager` — Performance / DoS Risk
**File:** `bootstrap.rs:279–312`

The commented-out early-exit optimization (`has_large`) means every epoch from `max_epoch` down to `1` is scanned unconditionally. `get_range_with_filter` collects **all** entries per epoch into a `Vec` before applying the 256-block window filter. On a long-running node with thousands of epochs, this performs a full RocksDB scan at startup for each epoch — unbounded memory allocation and CPU time.

```rust
// TODO(graivity_lightman): Fix this
// let mut has_large = false;
...
// if !has_large {
//     break;
// }
```

- **Impact:** Startup time degrades linearly with epoch count. Memory spike from materializing all entries before filtering. Could OOM on resource-constrained validators.
- **Recommendation:** Restore the early-exit optimization or, better, push the filter predicate into the RocksDB iterator bounds so entries outside the block range are never materialized.

### 2. `mpsc::channel(1)` Backpressure on Critical Consensus→Mempool Path
**File:** `consensus_api.rs:266–267`

```rust
let (consensus_to_mempool_sender, consensus_to_mempool_receiver) = mpsc::channel(1);
let (notification_sender, notification_receiver) = mpsc::channel(1);
```

Both channels carry consensus-critical traffic (`QuorumStoreRequest` batch pulls and commit notifications). A buffer of 1 means the sender blocks the moment one message is in-flight and unprocessed. If the mempool consumer stalls even briefly, consensus is back-pressured. This is a liveness hazard under load.

- **Impact:** Under burst conditions, consensus runtime blocks waiting on mempool, potentially causing proposal timeouts or missed rounds.
- **Recommendation:** Increase buffer capacity with documented rationale (e.g., `mpsc::channel(64)` or a configurable value). Profile under peak load to determine the right bound.

---

## Warning

### 3. Pervasive `panic!`/`.expect()` — No Graceful Error Propagation
**Files:** `bootstrap.rs:55,57,64,111,114,123–125,159,190–193,207,246–247` | `network.rs:43` | `consensus_api.rs:139`

Every initialization function uses `panic!` or `.expect()` on failure rather than returning `Result`. This is acceptable for a fail-fast startup gate **if and only if** the process is expected to crash and be restarted by a supervisor. However:

- The panic messages are inconsistent in quality (some are descriptive, some are not).
- Line 224 in `bootstrap.rs` has a **copy-paste error**: the panic message says `"must subscribe to DKG events"` but the code is subscribing to JWK events (`ObservedJWKsUpdated`).
- Line 207 in `bootstrap.rs`: `jwk_consensus_runtime.expect("JWK consensus runtime must be started")` panics if `None`, but the `None` branch exists only for a code path that `init_jwk_consensus` **never takes** (it always passes `Some`). This is dead-code-panic — confusing to a future maintainer.

- **Recommendation:** Fix the misleading panic message at line 224. Consider annotating the dead `None` branch at line 205 with `unreachable!()` and a comment explaining the invariant. For production hardening, convert to `Result`-based error handling so the process can log and exit cleanly rather than unwinding.

### 4. Missing Mutual Authentication Check on Full Node Networks
**File:** `network.rs:38–48`

```rust
pub fn extract_network_configs(node_config: &NodeConfig) -> Vec<NetworkConfig> {
    let mut network_configs: Vec<NetworkConfig> = node_config.full_node_networks.to_vec();
    if let Some(network_config) = node_config.validator_network.as_ref() {
        if !network_config.mutual_authentication {
            panic!("Validator networks must always have mutual_authentication enabled!");
        }
        network_configs.push(network_config.clone());
    }
    network_configs
}
```

Mutual auth is enforced for validator networks but **not checked for full node networks**. A misconfigured full node network with `mutual_authentication: false` would silently accept unauthenticated peers. Depending on what data full nodes relay, this could allow unauthorized nodes to join the gossip layer.

- **Impact:** An adversary could connect as an unauthenticated full node peer to observe or inject mempool transactions.
- **Recommendation:** At minimum, log a warning when a full node network has mutual auth disabled. Consider enforcing it by default and requiring an explicit opt-out flag.

### 5. Dead Channel — Unused `_mempool_client_sender`
**File:** `consensus_api.rs:273`

```rust
let (_mempool_client_sender, _mempool_client_receiver) = mpsc::channel(1);
```

Both sides are prefixed with `_`. The sender is immediately dropped. The receiver is passed to `init_mempool` as `_mempool_client_receiver` (also `_`-prefixed in the function signature at `bootstrap.rs:239`). This channel is a placeholder that never carries data — the receiver will return `None` immediately on poll since the sender is dropped.

- **Impact:** Not a bug per se, but dead allocations and confusing naming. A future developer may try to use `_mempool_client_sender` and be confused when the receiver never gets messages.
- **Recommendation:** Add a comment explaining why this is a placeholder, or remove the channel and pass a stub receiver if the downstream API allows it.

### 6. `.unwrap()` on RocksDB Range Query Without Context
**File:** `bootstrap.rs:292`

```rust
.get_range_with_filter::<BlockNumberSchema, _>(...)
.unwrap()
```

If the RocksDB read fails (disk I/O error, corruption), this panics with no context about which epoch was being scanned. Combined with the unbounded loop, a transient disk error crashes the node with an unhelpful backtrace.

- **Recommendation:** Use `.unwrap_or_else(|e| panic!("Failed to scan epoch {epoch_i}: {e}"))` or propagate the error.

---

## Info

### 7. `mock_mempool_client_sender` — Test Helper with Invalid Signatures
**File:** `network.rs:112–134`

This function generates transactions with:
- A hardcoded constant signature (`[1u8; 64]`) that is not derived from the private key.
- A new random private key per iteration whose public key doesn't match the account address.
- The oneshot receiver is dropped immediately, so any response callback will get `Canceled`.

This is marked `#[allow(dead_code)]` and commented as "used for UT", so it's not a production risk. However:

- **Recommendation:** Move this to a `#[cfg(test)]` module or a `tests/` helper crate so it cannot accidentally be called from production code. The `#[allow(dead_code)]` annotation suppresses the compiler's dead code warning, hiding the fact that it's genuinely unused in production paths.

### 8. `node_config.clone()` Passed into DKG Runtime Creation
**File:** `consensus_api.rs:292`

```rust
let dkg_runtime = create_dkg_runtime(
    &mut node_config.clone(),
    ...
);
```

A full clone of `NodeConfig` is created just to pass a `&mut` reference. The mutation (if any) is lost since it's applied to the clone. This is wasteful if `NodeConfig` is large.

- **Recommendation:** If mutation isn't needed, change the signature to `&NodeConfig`. If it is needed, investigate why the result is discarded.

### 9. TODO Comments Indicating Known Technical Debt
Several TODO comments indicate unresolved work:

| Location | TODO |
|---|---|
| `bootstrap.rs:280` | `TODO(graivity_lightman): Fix this` — the epoch scan optimization |
| `bootstrap.rs:218` | `TODO(gravity): only validator should subscribe the reconf events` — role enforcement |
| `network.rs:57` | `TODO: make this configurable (e.g., for compression)` |
| `network.rs:87` | `TODO: why is this not FIFO?` — mempool queue style |
| `consensus_api.rs:134` | `"It seems stupid, refactor when debugging finished"` |

- **Recommendation:** Track these in a backlog. The epoch scan fix (item 1 above) and the role enforcement are the most impactful.

---

## Summary

| Severity | Count | Key Items |
|----------|-------|-----------|
| **Critical** | 2 | Unbounded DB scan at startup; capacity-1 channels on consensus-critical path |
| **Warning** | 4 | Pervasive panics with misleading messages; missing full-node mutual auth check; dead channel; context-free `.unwrap()` |
| **Info** | 3 | Test helper in production module; unnecessary config clone; unresolved TODOs |

### ⚔️ Intention Attacker

# Security Audit Report: `bootstrap.rs` and `network.rs`

## Attack Surface Analysis

---

### 1. **[Critical] Capacity-1 Channels Create Consensus-to-Mempool Deadlock Vector**

**File:** `consensus_api.rs:266-267`

```rust
let (consensus_to_mempool_sender, consensus_to_mempool_receiver) = mpsc::channel(1);
let (notification_sender, notification_receiver) = mpsc::channel(1);
```

**The problem:** Both the `QuorumStoreRequest` channel (used by consensus to pull transaction batches from mempool) and the commit notification channel (used to tell mempool which transactions were committed) have a buffer of exactly 1. These two channels form a **bidirectional dependency loop**:

- Consensus sends a `QuorumStoreRequest` to mempool via `consensus_to_mempool_sender` → blocks if mempool hasn't consumed the previous message.
- Consensus sends a commit notification via `notification_sender` → blocks if mempool hasn't consumed the previous notification.
- Mempool must process **both** channels to make progress.

If mempool stalls on processing one channel (e.g., it's busy executing a commit GC triggered by a notification), the other channel's sender in consensus blocks. Since consensus is single-threaded per round, this can create a **circular wait**: consensus can't send the commit notification because the channel is full, and mempool can't drain the QuorumStoreRequest channel because it's waiting for the commit to finalize.

**Impact:** Under load, this can cause consensus to stall entirely — blocks stop being produced. This is a liveness failure, not just a performance issue. A malicious validator could deliberately trigger this by flooding transactions to maximize mempool processing time.

**Severity:** **Critical** — Production liveness risk under sustained load.

---

### 2. **[Critical] `init_block_buffer_manager` Full-Table-Scan DoS on Startup**

**File:** `bootstrap.rs:272-321`

```rust
for epoch_i in (1..=max_epoch).rev() {
    // TODO(graivity_lightman): Fix this
    // let mut has_large = false;
    ...
    consensus_db.get_range_with_filter::<BlockNumberSchema, _>(...)
    ...
    // if !has_large { break; }
}
```

**The problem:** The commented-out early-exit optimization means that on every node restart, **every epoch from `max_epoch` down to 1 is scanned unconditionally**, even though only 256 blocks are needed. The `get_range_with_filter` method collects **all** key-value pairs per epoch into a `Vec` before applying the filter.

For a node that has been running for thousands of epochs, this means:
- Startup reads the **entire** `BlockNumberSchema` column family across all epochs
- Memory usage spikes proportionally to total historical blocks
- Startup time degrades linearly (or worse) with chain age

**Attack scenario:** An attacker who can cause frequent epoch changes (e.g., through validator churn or reconfiguration proposals) inflates the epoch count. Every subsequent node restart becomes progressively slower, eventually making the node unable to rejoin the network within a reasonable timeframe.

**Impact:** Denial of service through startup exhaustion. On a long-running chain, this is an unbounded O(total_blocks) scan for what should be an O(256) operation.

**Severity:** **Critical** — Worsens over time; eventually prevents node restarts.

---

### 3. **[High] Missing Mutual Authentication Enforcement on Full Node Networks**

**File:** `network.rs:38-48`

```rust
pub fn extract_network_configs(node_config: &NodeConfig) -> Vec<NetworkConfig> {
    let mut network_configs: Vec<NetworkConfig> = node_config.full_node_networks.to_vec();
    if let Some(network_config) = node_config.validator_network.as_ref() {
        if !network_config.mutual_authentication {
            panic!("Validator networks must always have mutual_authentication enabled!");
        }
        network_configs.push(network_config.clone());
    }
    network_configs
}
```

**The problem:** Mutual authentication is enforced **only** for `validator_network`. All networks in `full_node_networks` pass through with **no authentication check whatsoever**. This includes VFN (Validator Full Node) networks, which carry consensus messages (as confirmed in `consensus_api.rs:202-210` where consensus is registered on **every** network).

Looking at `consensus_api.rs:163-166`, there's even a special case where VFN networks get `RoleType::FullNode` to "support block sync for validator node which is not the current epoch validator" — meaning consensus data flows over this unauthenticated channel.

**Attack scenario:** A misconfigured (or intentionally weakened) VFN network config with `mutual_authentication: false` allows any peer to connect, receive consensus messages, and inject forged messages into the consensus pipeline.

**Impact:** Man-in-the-middle attacks on the VFN network. Since consensus messages are registered on all networks (including full node networks), an unauthenticated VFN network becomes an attack vector for consensus message injection.

**Severity:** **High** — Configuration-dependent but silently allows insecure setups.

---

### 4. **[High] Non-Validator Node Receiving DKG Interfaces Causes Unrecoverable Panic**

**File:** `bootstrap.rs:119-124`

```rust
let dkg_runtime = match dkg_network_interfaces {
    Some(interfaces) => {
        ...
        let (reconfig_events, dkg_start_events) = dkg_subscriptions
            .expect("DKG needs to listen to NewEpochEvents events and DKGStartEvents");
        let my_addr = node_config.validator_network.as_ref().unwrap().peer_id();
```

**The problem:** The code assumes an invariant that `dkg_network_interfaces` is `Some` **only** when the node is a validator. But this invariant is not enforced at the type level — it depends on the caller wiring things correctly in `consensus_api.rs`. If a full node is misconfigured with a network that has `is_validator_network() == true` (e.g., wrong `NetworkId`), the DKG handle gets created (line 192-200 of consensus_api.rs), but `dkg_subscriptions` is `None` (because `is_validator()` returned false at line 108).

This causes:
1. `.expect()` panic at line 123-124 — process crash
2. If somehow that's bypassed, `.unwrap()` at line 125 on `validator_network` — second crash point

**Similarly for JWK at line 193:** `node_config.validator_network.as_ref().unwrap()` crashes if the validator network config is absent.

**Impact:** A single misconfigured `network_id` field in a full node's config crashes the node on startup with no recovery path. There is no validation that catches this before the panic.

**Severity:** **High** — Configuration error causes unrecoverable crash with misleading error messages.

---

### 5. **[Medium] Copy-Paste Error in JWK Event Subscription Masks Debugging**

**File:** `bootstrap.rs:222-224`

```rust
let jwk_updated_events = event_subscription_service
    .subscribe_to_events(vec![], vec!["0x1::jwks::ObservedJWKsUpdated".to_string()])
    .expect("JWK consensus must subscribe to DKG events");  // <-- WRONG
```

**The problem:** The error message says "DKG events" but the code is subscribing to JWK events (`ObservedJWKsUpdated`). If this `.expect()` ever fires, the operator sees "JWK consensus must subscribe to DKG events" and investigates DKG configuration — which is completely unrelated to the actual failure.

**Impact:** Misdirects incident response. In a production outage where the event subscription service is degraded, this sends operators down the wrong debugging path.

**Severity:** **Medium** — Operational confusion during incidents.

---

### 6. **[Medium] Dead Mempool Client Channel Creates Silent Message Black Hole**

**File:** `consensus_api.rs:273`

```rust
let (_mempool_client_sender, _mempool_client_receiver) = mpsc::channel(1);
```

Both sides are prefixed with `_`, and the sender is never used. The receiver is passed to `init_mempool()` which passes it to `aptos_mempool::bootstrap`. Inside the mempool runtime, this channel is presumably monitored for `MempoolClientRequest::SubmitTransaction` messages — but **nothing ever sends to it**.

**The problem:** If any code path inside the mempool runtime expects to receive external transaction submissions through this channel (as the `mock_mempool_client_sender` test helper suggests is the intended pattern), those submissions will never arrive. The mempool will only receive transactions via the P2P network gossip, not from local RPC submission.

**Impact:** If local transaction submission is ever enabled (e.g., for an RPC endpoint), the plumbing is broken. The channel exists but the sender is immediately dropped, meaning any future `.send()` returns `Err` immediately.

**Severity:** **Medium** — Currently latent, becomes critical if local TX submission is enabled.

---

### 7. **[Medium] `init_block_buffer_manager` Silently Uses Stale Block Mapping on DB Error**

**File:** `bootstrap.rs:284-292`

```rust
consensus_db
    .get_range_with_filter::<BlockNumberSchema, _>(...)
    .unwrap()  // <-- panics on DB error
```

The `.unwrap()` at line 292 will panic if the RocksDB read fails for any epoch. However, the real issue is subtler: if a **partial** failure occurs (e.g., corruption in one epoch's data), the loop panics mid-way through building `block_number_to_block_id`. Since the HashMap is constructed incrementally and favors higher epochs (line 301: `if *cur_epoch < epoch`), a panic during an early epoch scan means the map already contains entries from later epochs. But these are lost — the process crashes rather than using the partial (but potentially valid) data.

**Alternative concern:** If this `.unwrap()` were changed to `.unwrap_or_default()` (a common "fix"), it would silently skip corrupted epochs, potentially mapping block numbers to wrong block IDs from older epochs. This would cause the block buffer manager to serve incorrect block data.

**Severity:** **Medium** — Data integrity risk if error handling is loosened without proper validation.

---

## Summary

| # | Finding | Severity | Type |
|---|---------|----------|------|
| 1 | Capacity-1 channels create consensus↔mempool deadlock | **Critical** | Liveness DoS |
| 2 | Unbounded epoch scan on startup (commented-out optimization) | **Critical** | Startup DoS |
| 3 | No mutual auth enforcement on full node / VFN networks | **High** | Authentication bypass |
| 4 | DKG/JWK invariant violation causes unrecoverable crash | **High** | Crash via misconfiguration |
| 5 | Copy-paste error in JWK error message references DKG | **Medium** | Operational misdirection |
| 6 | Dead mempool client channel (sender immediately dropped) | **Medium** | Silent functionality loss |
| 7 | Partial DB corruption causes full startup crash | **Medium** | Data integrity / availability |

**Recommended priority:** Fix #1 and #2 immediately — both are production-impacting. #3 should be addressed by extending the `mutual_authentication` check to all networks that carry consensus messages.

---

## [sentinel_monitoring] Security review of the sentinel binary (bin/sentinel/src/) for: (1) main.rs for 

### 🎯 Intention

Security review of the sentinel binary (bin/sentinel/src/) for: (1) main.rs for command-line argument handling without proper validation; (2) config.rs for TOML deserialization safety and webhook URL validation; (3) notifier.rs for webhook URL SSRF potential, rate limiting bypass, and credential exposure in webhook URLs; (4) watcher.rs for glob pattern injection and symlink following; (5) whitelist.rs for regex DoS (ReDoS) in whitelist patterns; (6) analyzer.rs for error pattern regex safety; (7) probe.rs for health check URL SSRF; (8) reader.rs for file reading safety with linemux.

### 🔍 Discovered Implementation

# Sentinel Binary — Implementation Analysis

## Overview

The sentinel is a log-monitoring daemon that tails log files via glob patterns, matches error lines against regex patterns, checks a whitelist for frequency-based suppression, and dispatches alerts to Feishu/Slack webhooks with priority-level routing (P0/P1/P2). An optional HTTP health probe runs in a background task.

---

## Files Involved

| File | Purpose |
|------|---------|
| `main.rs` | Entry point, CLI arg parsing, orchestration loop |
| `config.rs` | TOML config deserialization, priority-level webhook resolution |
| `notifier.rs` | Webhook HTTP dispatch (Feishu + Slack), per-priority rate limiting |
| `watcher.rs` | Glob-based file discovery with recency filtering |
| `whitelist.rs` | CSV-loaded regex rules with frequency-based alert suppression |
| `analyzer.rs` | Single regex filter for identifying error lines |
| `probe.rs` | Periodic HTTP health check with failure-threshold alerting |
| `reader.rs` | Thin wrapper around `linemux::MuxedLines` for tailing files |

---

## Execution Path

### 1. Startup (`main.rs:23-60`)

1. `env_logger::init()` — initializes logging from `RUST_LOG` env var.
2. `env::args().collect()` — collects CLI args into `Vec<String>`. If `args.len() < 2`, prints usage and exits with code 1. No further validation of `args[1]` beyond passing it to `Config::load()`.
3. `Config::load(config_path)` — reads the TOML file from disk (`fs::read_to_string`), deserializes via `toml::from_str`.
4. If `config.monitoring.whitelist_path` is `Some`, loads `Whitelist::load(path)`. Otherwise uses `Whitelist::default()` (empty rules).
5. Constructs `Watcher`, `Reader`, `Analyzer`, `Notifier`.
6. `notifier.verify_webhooks()` — sends a startup message to every configured webhook URL. Fails hard (via `?` + `context()`) if any webhook is unreachable.
7. If `config.probe` is `Some`, spawns `Probe::run()` in a detached `tokio::spawn`.
8. Calls `watcher.discover()` for initial file set, then `reader.add_file()` for each.

### 2. Main Loop (`main.rs:75-125`)

Uses `tokio::select!` with two branches:

- **Event-driven branch**: `reader.next_line()` returns a `linemux::Line`. Calls `analyzer.is_error(line)` — if false, skips. If true, calls `whitelist.check(line)` which returns one of:
  - `Skip` — below threshold, no alert.
  - `Alert { count, priority }` — above threshold, sends alert at the rule's priority.
  - `AlwaysAlert` — no whitelist rule matched, sends alert at `Priority::P0`.

- **Periodic branch**: `interval.tick()` triggers `watcher.discover()` to find new files matching glob patterns and adds them to the reader.

---

## Key Functions

### `config.rs`

| Function | Signature | Behavior |
|----------|-----------|----------|
| `Config::load` | `fn load<P: AsRef<Path>>(path: P) -> Result<Self>` | `fs::read_to_string(path)` then `toml::from_str(&content)`. No schema validation beyond serde derive. No URL format validation on webhook fields. |
| `AlertingConfig::get_webhooks` | `fn get_webhooks(&self, priority: Priority) -> (Option<&str>, Option<&str>)` | Looks up `self.priorities[priority]`; if found, uses its feishu/slack URLs with fallback to top-level defaults. Returns `(feishu, slack)` tuple. |
| `AlertingConfig::all_webhooks` | `fn all_webhooks(&self) -> Vec<(&str, &str)>` | Collects all unique non-empty webhook URLs across default + per-priority configs. Uses `HashSet` for dedup. |
| `default_min_alert_interval` | `fn default_min_alert_interval() -> u64` | Returns `5` (seconds). Used as `#[serde(default)]` for `min_alert_interval`. |

**Config structs deserialized from TOML:**
- `GeneralConfig`: `check_interval_ms: u64`
- `MonitoringConfig`: `file_patterns: Vec<String>`, `recent_file_threshold_seconds: u64`, `error_pattern: String`, `whitelist_path: Option<String>`
- `AlertingConfig`: `feishu_webhook: Option<String>`, `slack_webhook: Option<String>`, `min_alert_interval: u64`, `priorities: HashMap<Priority, PriorityAlertConfig>`
- `ProbeConfig`: `url: String`, `check_interval_seconds: u64`, `failure_threshold: u32`
- `Priority` enum: `P0`, `P1`, `P2` with case-insensitive serde aliases (`p0`/`P0`)

### `notifier.rs`

| Function | Signature | Behavior |
|----------|-----------|----------|
| `Notifier::new` | `fn new(config: AlertingConfig) -> Self` | Creates `reqwest::Client::new()` (no TLS config, no redirect policy, no timeout). Initializes empty `HashMap<Priority, Instant>` behind `Arc<Mutex>`. |
| `Notifier::send` | `async fn send(&self, text: &str, priority: Priority) -> Result<()>` | Gets webhooks via `config.get_webhooks(priority)`. POSTs JSON to each non-empty URL. Feishu payload: `{"msg_type":"text","content":{"text":...}}`. Slack payload: `{"text":...,"channel":"#alerts-devops","username":"System-Monitor"}`. Channel is hardcoded. |
| `Notifier::verify_webhooks` | `async fn verify_webhooks(&self) -> Result<()>` | Iterates `config.all_webhooks()`, POSTs a startup message to each. Bails if list is empty or any POST fails. |
| `Notifier::alert` | `async fn alert(&self, message: &str, file: &str, priority: Priority) -> Result<()>` | Rate-limits per priority: acquires `Mutex`, checks if `now - last_alert_time[priority] < min_alert_interval`. If within window, returns `Ok(())` silently. Otherwise records timestamp and calls `self.send()`. Errors from `send()` are logged to stderr but swallowed (`Ok(())` always returned). |

**Rate limiting details**: Uses `std::sync::Mutex` (not `tokio::Mutex`) in async context. The lock is held only briefly (check + insert) before being dropped, then `send()` is called outside the lock. Rate limiting is per-priority-level globally, not per-message-content — a P0 alert for one error suppresses all other P0 alerts within the interval.

### `watcher.rs`

| Function | Signature | Behavior |
|----------|-----------|----------|
| `Watcher::new` | `fn new(config: MonitoringConfig) -> Self` | Stores config and initializes empty `HashSet<PathBuf>` for known files. |
| `Watcher::discover` | `fn discover(&mut self) -> Result<Vec<PathBuf>>` | Iterates `config.file_patterns`, calls `glob::glob(pattern)` for each. For each matched path, calls `should_monitor()` and checks `known_files.insert()` for dedup. |
| `Watcher::should_monitor` | `fn should_monitor(&self, path: &Path, now: u64) -> bool` | Calls `fs::metadata(path)` — this follows symlinks (not `fs::symlink_metadata`). Checks if the file's `modified` time is within `recent_file_threshold_seconds` of `now`. |

**Glob patterns** come directly from the TOML config `file_patterns: Vec<String>` and are passed to `glob::glob()` without sanitization.

### `whitelist.rs`

| Function | Signature | Behavior |
|----------|-----------|----------|
| `WhitelistRule::new` | `fn new(pattern_str: &str, threshold: i32, priority: Priority) -> Result<Self>` | Attempts `Regex::new(pattern_str)`. On failure, falls back to `Regex::new(&regex::escape(pattern_str))` (literal match). Initializes empty `VecDeque<Instant>` for timestamp tracking. |
| `WhitelistRule::matches` | `fn matches(&self, line: &str) -> bool` | Calls `self.pattern.is_match(line)`. |
| `Whitelist::load` | `fn load<P: AsRef<Path>>(path: P) -> Result<Self>` | Opens CSV file with `has_headers(false)`, `comment(Some(b'#'))`, `flexible(true)`. Parses columns: `pattern, threshold [, priority]`. Skips records with `< 2` fields or empty pattern. Priority defaults to `P0` if missing or unrecognized. |
| `Whitelist::check` | `fn check(&mut self, line: &str) -> CheckResult` | Iterates rules in order, returns on first match. If `threshold == -1`, returns `Skip` (always suppress). Otherwise maintains a sliding window (`VecDeque`) of timestamps within `WINDOW_SECONDS` (300s = 5 min). If count > threshold, returns `Alert { count, priority }`. If count <= threshold, returns `Skip`. If no rule matches, returns `AlwaysAlert`. |

**Threshold type**: `threshold` is `i32` but is compared as `u32` on line 148: `count > rule.threshold as u32`. A negative threshold other than `-1` (e.g., `-2`) would cast to a large `u32` value (4294967294), effectively making the rule never trigger an alert. Only `-1` is explicitly handled.

### `analyzer.rs`

| Function | Signature | Behavior |
|----------|-----------|----------|
| `Analyzer::new` | `fn new(error_pattern: &str) -> Result<Self>` | Compiles `error_pattern` from config via `Regex::new()`. Returns error if pattern is invalid regex. |
| `Analyzer::is_error` | `fn is_error(&self, line: &str) -> bool` | Returns `self.error_regex.is_match(line)`. |

### `probe.rs`

| Function | Signature | Behavior |
|----------|-----------|----------|
| `Probe::new` | `fn new(config: ProbeConfig, notifier: Notifier) -> Self` | Creates `reqwest::Client` with 10-second timeout. Falls back to `Client::new()` (no timeout) if builder fails. |
| `Probe::run` | `async fn run(self)` | Infinite loop: sends HTTP GET to `config.url` every `check_interval_seconds`. Increments `failures` counter on non-2xx or network error. When `failures >= failure_threshold`, sends P0 alert via `notifier.alert()` and resets `failures = 0`. On recovery (success after failures), resets counter and logs. |

**Probe URL** comes from `config.probe.url` (a `String`), passed directly to `self.client.get(&self.config.url)`. No URL validation or scheme restriction.

### `reader.rs`

| Function | Signature | Behavior |
|----------|-----------|----------|
| `Reader::new` | `fn new() -> Result<Self>` | Creates `linemux::MuxedLines::new()`. |
| `Reader::add_file` | `async fn add_file(&mut self, path: impl Into<PathBuf>) -> Result<()>` | Delegates to `self.lines.add_file(path)`. `linemux` uses inotify/kqueue to watch for file changes and streams new lines. |
| `Reader::next_line` | `async fn next_line(&mut self) -> Option<Line>` | Returns `self.lines.next_line().await.ok().flatten()`. Errors are silently dropped (converted to `None` via `.ok()`). |

---

## State Changes

| Component | State | Mutability |
|-----------|-------|------------|
| `Watcher::known_files` | `HashSet<PathBuf>` — grows as new files are discovered, never shrinks | `&mut self` |
| `WhitelistRule::timestamps` | `VecDeque<Instant>` — sliding window, entries added on match and expired entries removed from front | `&mut self` via `Whitelist::check` |
| `Notifier::last_alert_times` | `Arc<Mutex<HashMap<Priority, Instant>>>` — stores last alert time per priority level, updated on each non-rate-limited alert | Shared via `Arc<Mutex>` |
| `Probe::failures` | Local `u32` counter — incremented on failure, reset on recovery or threshold trigger | Local variable in `run()` |
| `Reader::lines` | `MuxedLines` — internal file watch state, files added but never removed | `&mut self` |

---

## External Dependencies

| Crate | Version | Usage |
|-------|---------|-------|
| `tokio` | 1.0 (full) | Async runtime, timers, select, spawn |
| `reqwest` | 0.11 (json) | HTTP client for webhook POSTs and probe GETs |
| `serde` / `serde_json` | workspace | JSON payload construction, config deserialization |
| `toml` | 0.8 | TOML config file parsing |
| `regex` | 1.10 | Error pattern matching, whitelist pattern matching |
| `glob` | 0.3 | File discovery by glob pattern |
| `linemux` | 0.3 | File tailing (inotify/kqueue-based line-by-line streaming) |
| `csv` | workspace | Whitelist CSV file parsing |
| `sha2` / `hex` | — | Listed in Cargo.toml but **not used** in any source file |
| `chrono` | — | Listed in Cargo.toml but **not used** in any source file |
| `anyhow` | — | Error handling throughout |
| `log` / `env_logger` | — | Logging initialization; actual output uses `println!`/`eprintln!` |

---

## Data Flow Summary

```
TOML Config File
      │
      ▼
  Config::load() ──→ MonitoringConfig ──→ Watcher (glob patterns)
      │                                        │
      │                                        ▼
      │                                  glob::glob() ──→ PathBuf list
      │                                        │
      │                                        ▼
      │                                  Reader::add_file() ──→ linemux tailing
      │
      ├──→ AlertingConfig ──→ Notifier (webhook URLs, rate limit config)
      │
      ├──→ error_pattern ──→ Analyzer (compiled Regex)
      │
      ├──→ whitelist_path ──→ Whitelist::load() (CSV → Vec<WhitelistRule>)
      │
      └──→ ProbeConfig ──→ Probe (health check URL, threshold)

Main Loop:
  linemux line event
      │
      ▼
  Analyzer::is_error() ──[false]──→ skip
      │ [true]
      ▼
  Whitelist::check() ──→ Skip | Alert{count,priority} | AlwaysAlert
      │                    │              │
      │                [skip]    [send alert with rule priority]
      ▼
  Notifier::alert(msg, file, priority)
      │
      ▼
  Rate limit check (per-priority, min_alert_interval seconds)
      │ [pass]
      ▼
  Notifier::send() ──→ HTTP POST to Feishu/Slack webhook URLs
```

### 🛠️ Ground Review

# Sentinel Binary — Ground Review (Blue Team)

## Code Quality & Engineering Safety Audit

---

### Critical

#### 1. `std::sync::Mutex` used in async context — potential deadlock
**File:** `notifier.rs:13` / `notifier.rs:89`

`std::sync::Mutex` is used inside `async fn alert()`. While the lock is held briefly and dropped before the `.await` on `send()`, this is a latent hazard. If future refactoring moves the `.await` inside the lock scope, the task will block the entire tokio worker thread. The `unwrap()` on `.lock()` is also a panic risk — a poisoned mutex (from a prior panic) will crash the sentinel.

```rust
let mut times = self.last_alert_times.lock().unwrap(); // panics on poison
```

**Recommendation:** Use `tokio::sync::Mutex` or replace `.unwrap()` with `.lock().unwrap_or_else(|e| e.into_inner())` to recover from poisoning.

---

#### 2. No timeout on HTTP client for webhook requests
**File:** `notifier.rs:20`

```rust
client: Client::new(), // no timeout configured
```

The `Notifier`'s `reqwest::Client` has no timeout. A hung webhook endpoint will block the calling task indefinitely. Since `send()` is called from the main event loop, this can stall all alert processing. The `Probe` correctly sets a 10s timeout, but the `Notifier` — which is on the critical path — does not.

**Recommendation:** Build the client with `.timeout(Duration::from_secs(10))` or similar.

---

#### 3. Unsafe `i32` to `u32` cast in threshold comparison
**File:** `whitelist.rs:148`

```rust
if count > rule.threshold as u32 {
```

A negative threshold other than `-1` (e.g., `-2`, `-100`) is cast to `u32` via wrapping, producing a value like `4294967294`. This silently disables the rule rather than erroring. Only `-1` is explicitly handled as "always skip."

**Recommendation:** Validate at load time that `threshold` is either `-1` or `>= 0`. Reject all other negative values with an error.

---

### Warning

#### 4. Webhook URLs not validated — SSRF vector
**Files:** `config.rs` (all webhook fields), `probe.rs:22`

Webhook URLs from the TOML config and probe URL are passed directly to `reqwest::Client::post()` / `::get()` with no validation of scheme, host, or format. A misconfigured or malicious config could target internal services (e.g., `http://169.254.169.254/latest/meta-data/` on cloud instances, or `file:///etc/passwd` if reqwest ever supported it).

**Recommendation:** Validate URLs at config load time: enforce `https://` scheme, reject private/link-local IP ranges, and optionally allowlist expected domains.

---

#### 5. Unsanitized glob patterns from config — path traversal
**File:** `watcher.rs:27`

```rust
for pattern in &self.config.file_patterns {
    for entry in glob(pattern)? {
```

Glob patterns are read directly from the TOML config with no sanitization. Patterns like `../../**/*.log` or `/etc/**/*` could cause the sentinel to monitor arbitrary files on the filesystem. While this is a config file (typically operator-controlled), defense-in-depth is appropriate.

**Recommendation:** Validate that resolved paths remain under an expected base directory, or reject patterns containing `..`.

---

#### 6. ReDoS risk in user-supplied regex patterns
**Files:** `whitelist.rs:36`, `analyzer.rs:8`

Both `WhitelistRule::new()` and `Analyzer::new()` compile regex patterns from external input (CSV file and TOML config). While Rust's `regex` crate guarantees linear-time matching (no catastrophic backtracking), compilation of very large or complex patterns can still consume significant time and memory.

**Recommendation:** Add a `regex::RegexBuilder` with `.size_limit()` to bound compilation cost. This provides an additional safety margin.

---

#### 7. Symlink following in file discovery
**File:** `watcher.rs:39`

```rust
if let Ok(metadata) = fs::metadata(path) {
```

`fs::metadata()` follows symlinks. An attacker who can create symlinks in a monitored directory could point to sensitive files (e.g., `/etc/shadow`), causing their contents to be read and potentially forwarded as alert text to webhook endpoints.

**Recommendation:** Use `fs::symlink_metadata()` and either reject symlinks or resolve and validate the target path against an allowlist.

---

#### 8. `known_files` grows unboundedly — memory leak
**File:** `watcher.rs:7`

```rust
known_files: HashSet<PathBuf>,
```

Files are added to `known_files` on discovery but never removed, even after deletion or rotation. In long-running deployments with frequent log rotation, this `HashSet` grows without bound. Similarly, `MuxedLines` in `reader.rs` accumulates file watches that are never cleaned up.

**Recommendation:** Periodically prune `known_files` entries whose paths no longer exist on disk. Consider removing stale watches from the reader as well.

---

#### 9. Hardcoded Slack channel and username
**File:** `notifier.rs:42-43`

```rust
"channel": "#alerts-devops",
"username": "System-Monitor"
```

The Slack channel `#alerts-devops` and username `System-Monitor` are hardcoded in the payload. These should be configurable via the TOML config to allow deployment flexibility without code changes.

---

#### 10. Silent error swallowing in `reader.rs`
**File:** `reader.rs:19`

```rust
self.lines.next_line().await.ok().flatten()
```

`.ok()` converts all `Err` variants to `None`, silently discarding I/O errors, permission failures, or inotify/kqueue issues. The caller in `main.rs` has no way to distinguish "no line available" from "fatal read error."

**Recommendation:** At minimum, log errors before discarding them. Consider propagating fatal errors to allow the main loop to handle them.

---

### Info

#### 11. Unused dependencies in `Cargo.toml`
**File:** `Cargo.toml`

`sha2`, `hex`, and `chrono` are declared as dependencies but are not imported or used in any source file. This bloats compile time and binary size unnecessarily.

**Recommendation:** Remove unused dependencies.

---

#### 12. Mixed logging: `println!`/`eprintln!` vs `log` crate
**Files:** All files

`env_logger` is initialized in `main.rs`, but all actual output uses `println!` / `eprintln!` instead of `log::info!` / `log::error!`. This means `RUST_LOG` has no effect on output verbosity — all messages are always printed.

**Recommendation:** Replace `println!`/`eprintln!` calls with appropriate `log` macros (`info!`, `warn!`, `error!`) so that log levels are actually controlled by `RUST_LOG`.

---

#### 13. `Probe` client fallback removes timeout silently
**File:** `probe.rs:18-20`

```rust
.build()
.unwrap_or_else(|_| Client::new()),
```

If the builder fails (unlikely but possible), the fallback `Client::new()` has **no timeout**, silently degrading the safety guarantee. The error is also discarded.

**Recommendation:** Log the builder error and propagate it rather than falling back to an unconfigured client.

---

#### 14. No graceful shutdown handling
**File:** `main.rs`

The main loop runs forever with no signal handling (`SIGTERM`, `SIGINT`). On process termination, in-flight HTTP requests are abruptly dropped. In containerized deployments, this prevents clean shutdown and potential alert loss.

**Recommendation:** Add a `tokio::signal::ctrl_c()` branch to the `select!` loop for graceful shutdown.

---

## Summary

| Severity | Count | Key Themes |
|----------|-------|------------|
| **Critical** | 3 | Mutex poisoning panic, no HTTP timeout on critical path, unsafe integer cast |
| **Warning** | 7 | SSRF vectors, symlink following, memory leak, silent error swallowing, hardcoded config |
| **Info** | 4 | Unused deps, inconsistent logging, missing graceful shutdown |

The codebase is compact and well-structured, but the critical items — especially the missing HTTP timeout on the notifier and the unsafe threshold cast — should be addressed before production use. The SSRF surface (webhook URLs and probe URL from config) warrants validation if the config file is not strictly operator-controlled.

### ⚔️ Intention Attacker

# Sentinel Binary — Security Audit (Red Team Attack Report)

---

## Critical Severity

### 1. SSRF via Unconstrained Webhook and Probe URLs

**Files:** `notifier.rs:37,53,84` / `probe.rs:35` / `config.rs:42,68-72`

No URL validation exists anywhere. Webhook URLs (`feishu_webhook`, `slack_webhook`) and probe URL (`probe.url`) are read from TOML config and passed directly to `reqwest::Client::post()` / `::get()`. An attacker who can modify the config file (or an operator who misconfigures it) can:

- Target internal services: `http://169.254.169.254/latest/meta-data/` (cloud metadata), `http://localhost:8500/v1/agent/self` (Consul), etc.
- Exfiltrate log content to arbitrary endpoints — every alert body contains raw log lines, which may include secrets, tokens, or PII.
- Port-scan internal networks by observing timing/error differences.

Additionally, `reqwest::Client::new()` in `notifier.rs:22` is created with **no timeout, no redirect policy, and no TLS verification config**. This means:
- Infinite hangs on unresponsive internal targets (DoS of the sentinel itself).
- Open redirects can chain SSRF to arbitrary destinations.

**Attack:** Set `feishu_webhook = "http://169.254.169.254/latest/meta-data/iam/security-credentials/"` in TOML. Sentinel will POST log content to the cloud metadata endpoint on every alert cycle.

---

### 2. Log Content Injection into Webhook Payloads (Data Exfiltration / Injection)

**File:** `notifier.rs:109-110`

```rust
let text = format!(
    "🚨 **Log Sentinel Alert** [{priority}] 🚨\nFile: `{file}`\nError:\n```\n{message}\n```"
);
```

Raw, unsanitized log line content is interpolated directly into the webhook payload. This has two consequences:

- **Data exfiltration:** Any secrets accidentally logged (API keys, tokens, database credentials, session IDs) are forwarded verbatim to external webhook services (Feishu/Slack). This violates least-privilege and data minimization principles.
- **Slack/Feishu payload injection:** Crafted log lines containing JSON control characters or Slack mrkdwn formatting (`@here`, `@channel`, `<@U12345>`) can manipulate the alert message, tag arbitrary users, or break JSON payload structure.

---

### 3. Negative Threshold Bypass — Silent Alert Suppression

**File:** `whitelist.rs:125-146`

```rust
if rule.threshold == -1 {
    return CheckResult::Skip;  // Only -1 is guarded
}
// ...
if count > rule.threshold as u32 {  // line 146
```

Only `threshold == -1` is explicitly handled as "always suppress." Any other negative value (e.g., `-2`, `-100`) is cast to `u32`, wrapping to an astronomically large number (e.g., `-2` becomes `4294967294`). This means:

- A rule with `threshold = -2` will **never** trigger an alert because the count can never exceed ~4.3 billion within a 5-minute window.
- An attacker who can edit the whitelist CSV can silently suppress all alerts for a specific error pattern by setting a negative threshold other than `-1`, which looks like a legitimate configuration value to a casual reviewer.

**Attack:** Add line `critical_error_pattern,-2,p0` to whitelist CSV. All matching critical errors are silently swallowed forever. No warning is logged.

---

## High Severity

### 4. Per-Priority Rate Limiting Causes Cross-Error Alert Suppression

**File:** `notifier.rs:97-107`

Rate limiting is keyed **per priority level globally**, not per error message or per file:

```rust
if let Some(last) = times.get(&priority) {
    if now.duration_since(*last) < Duration::from_secs(self.config.min_alert_interval) {
        return Ok(());  // Silently dropped
    }
}
```

**Attack scenario:** An attacker who can write to a monitored log file causes a flood of low-severity P0 errors (e.g., benign-looking "connection reset" messages). Each one resets the P0 rate-limit window. Genuine critical P0 alerts arriving within `min_alert_interval` seconds (default: 5s) of the flood are **silently dropped** — `Ok(())` is returned, no log is emitted, no error propagated. The operator has no visibility that alerts were suppressed.

This is especially dangerous because the default `min_alert_interval` of 5 seconds is short enough to seem safe but long enough for a sustained attacker to block every real alert.

---

### 5. Symlink Following in File Discovery — Arbitrary File Monitoring

**File:** `watcher.rs:41`

```rust
if let Ok(metadata) = fs::metadata(path) {  // follows symlinks
```

`fs::metadata()` follows symlinks (unlike `fs::symlink_metadata()`). Combined with unconstrained glob patterns from config, an attacker who can create symlinks in a monitored directory can:

- Cause sentinel to tail arbitrary files outside the intended log directory (e.g., `/etc/shadow`, application config files containing secrets).
- Those file contents then flow through the alert pipeline and get sent to external webhooks (see #2), achieving **arbitrary file read + exfiltration**.

**Attack chain:** Create symlink `/var/log/app/evil.log -> /etc/environment` in a globbed directory. Sentinel picks it up, tails it, and any line matching `error_pattern` gets sent to the webhook endpoint.

---

### 6. ReDoS via Whitelist Patterns

**File:** `whitelist.rs:32-37`

```rust
let pattern = match Regex::new(pattern_str) {
    Ok(re) => re,
    Err(_) => {
        Regex::new(&regex::escape(pattern_str))?  // fallback to literal
    }
};
```

Whitelist patterns from the CSV file are compiled as regex with no complexity limits, size limits, or timeout. A malicious or poorly-written pattern (e.g., `(a+)+$`) will cause catastrophic backtracking when matched against log lines. Since `whitelist.check()` runs **synchronously on every error log line** in the main tokio task, a ReDoS pattern will:

- Block the entire main event loop.
- Halt all file monitoring, new file discovery, and alert processing.
- Effectively DoS the sentinel — no alerts fire for any error.

The same risk applies to `analyzer.rs:10` where `error_pattern` from config is compiled without safeguards.

---

### 7. `std::sync::Mutex` in Async Context — Potential Deadlock

**File:** `notifier.rs:7,16,98`

```rust
use std::sync::Mutex;
// ...
let mut times = self.last_alert_times.lock().unwrap();
```

A `std::sync::Mutex` is used inside an async function. While the lock is held briefly in this specific code, `lock().unwrap()` will **panic and crash the entire process** if the mutex is poisoned (i.e., if a previous holder panicked). Since `Notifier` is cloned and shared with the `Probe` task (`main.rs:55`), a panic in the probe's alert path poisons the mutex and crashes the main monitoring loop on the next alert attempt.

---

## Medium Severity

### 8. Silent Error Swallowing in Reader

**File:** `reader.rs:21`

```rust
pub async fn next_line(&mut self) -> Option<Line> {
    self.lines.next_line().await.ok().flatten()
}
```

All `linemux` errors (file deletion, permission changes, inotify failures, fd exhaustion) are converted to `None` via `.ok()`. In the main loop (`main.rs:78`), `None` simply means "no line available" — the sentinel silently stops monitoring files that encounter errors with no alert, no log, and no recovery mechanism. An attacker can cause targeted file monitoring to cease by triggering filesystem errors.

---

### 9. Unbounded Memory Growth in Watcher

**File:** `watcher.rs:13,29`

```rust
known_files: HashSet<PathBuf>,  // grows forever, never shrinks
```

`known_files` is append-only. In environments with log rotation (which is the common case for the exact files this tool monitors), old files are removed but never purged from the `HashSet`. Over days/weeks of runtime, this causes unbounded memory growth. More importantly, rotated-away file paths stay in `known_files`, so if a new log file is created with the same path (standard rotation behavior), **it will never be picked up** because `HashSet::insert` returns `false` for existing entries.

**Impact:** After log rotation, sentinel silently stops monitoring the new log file. No alerts fire for new errors. This is a silent monitoring failure.

---

### 10. Hardcoded Slack Channel — Misconfigured Alert Routing

**File:** `notifier.rs:50`

```rust
"channel": "#alerts-devops",
```

The Slack channel is hardcoded. Per-priority webhook routing was explicitly implemented (the whole `priorities` config system), but the channel override is always `#alerts-devops`. If operators configure different Slack webhook URLs expecting messages to route to different channels based on priority, the hardcoded channel override may silently redirect all alerts to the wrong channel (depending on Slack webhook configuration).

---

## Summary

| # | Finding | Severity | File(s) |
|---|---------|----------|---------|
| 1 | SSRF via unconstrained webhook/probe URLs | **Critical** | notifier.rs, probe.rs, config.rs |
| 2 | Log content injection / secret exfiltration via webhooks | **Critical** | notifier.rs |
| 3 | Negative threshold bypass silently suppresses all alerts | **Critical** | whitelist.rs |
| 4 | Global per-priority rate limiting enables cross-error suppression | **High** | notifier.rs |
| 5 | Symlink following enables arbitrary file read + exfiltration | **High** | watcher.rs |
| 6 | ReDoS in whitelist/analyzer patterns DoSes entire sentinel | **High** | whitelist.rs, analyzer.rs |
| 7 | `std::sync::Mutex` poisoning crashes process | **High** | notifier.rs |
| 8 | Silent error swallowing stops monitoring without notice | **Medium** | reader.rs |
| 9 | Unbounded `known_files` growth + post-rotation monitoring failure | **Medium** | watcher.rs |
| 10 | Hardcoded Slack channel defeats priority routing intent | **Medium** | notifier.rs |

---

## [gravity_cli_operations] Review the gravity_cli binary (bin/gravity_cli/src/) for: (1) command.rs and mai

### 🎯 Intention

Review the gravity_cli binary (bin/gravity_cli/src/) for: (1) command.rs and main.rs for CLI argument parsing safety; (2) stake/create.rs and stake/get.rs for staking operation correctness; (3) validator/join.rs, leave.rs, list.rs for validator management security; (4) dkg/randomness.rs and dkg/status.rs for DKG key handling; (5) genesis/key.rs and genesis/account.rs for private key generation and storage; (6) contract.rs for smart contract interaction safety; (7) util.rs for utility function security.

### 🔍 Discovered Implementation

# Implementation Analysis: `gravity_cli` Binary

## Files/Contracts Involved

| File | Description |
|------|-------------|
| `bin/gravity_cli/src/main.rs` | Entry point; parses CLI args via clap and dispatches to subcommand handlers |
| `bin/gravity_cli/src/command.rs` | Defines top-level `Command` struct, `SubCommands` enum, `Executable` trait, and version info |
| `bin/gravity_cli/src/contract.rs` | Solidity ABI bindings for `ValidatorManagement` and `Staking` system contracts; hardcoded addresses |
| `bin/gravity_cli/src/util.rs` | ETH/wei conversion helpers (`format_ether`, `parse_ether`) |
| `bin/gravity_cli/src/stake/mod.rs` | Module root for `Create` and `Get` staking subcommands |
| `bin/gravity_cli/src/stake/create.rs` | Creates a new StakePool via on-chain transaction |
| `bin/gravity_cli/src/stake/get.rs` | Queries PoolCreated events to list StakePools by owner |
| `bin/gravity_cli/src/validator/mod.rs` | Module root for `Join`, `Leave`, `List` validator subcommands |
| `bin/gravity_cli/src/validator/join.rs` | Registers a validator and joins the validator set |
| `bin/gravity_cli/src/validator/leave.rs` | Leaves the validator set |
| `bin/gravity_cli/src/validator/list.rs` | Read-only listing of all validators as JSON |
| `bin/gravity_cli/src/dkg/mod.rs` | Module root for `Status` and `Randomness` DKG subcommands |
| `bin/gravity_cli/src/dkg/randomness.rs` | Queries DKG randomness for a block via HTTP |
| `bin/gravity_cli/src/dkg/status.rs` | Queries DKG status via HTTP |
| `bin/gravity_cli/src/genesis/mod.rs` | Module root for `GenerateKey`, `GenerateWaypoint`, `GenerateAccount` |
| `bin/gravity_cli/src/genesis/key.rs` | Generates full validator identity (BLS12-381 + X25519 + Ed25519 keys) |
| `bin/gravity_cli/src/genesis/account.rs` | Generates a standard Ethereum secp256k1 account |
| `bin/gravity_cli/src/genesis/waypoint.rs` | Generates a waypoint from genesis validator config JSON |
| `bin/gravity_cli/src/node/start.rs` | Starts a node by executing `script/start.sh` |
| `bin/gravity_cli/src/node/stop.rs` | Stops a node by executing `script/stop.sh` |

---

## Execution Path: Entry Point

1. `main()` calls `Command::parse()` (clap derives CLI parsing).
2. Pattern-matches on `SubCommands` enum variants: `Genesis`, `Validator`, `Stake`, `Node`, `Dkg`.
3. Each variant is further matched to its sub-subcommand, calling `.execute()` on the resolved struct.
4. If `execute()` returns `Err`, the error is printed to stderr and the process exits with code 1.

---

## Key Functions by Module

### 1. `command.rs` — CLI Structure

- **`Command`**: Top-level clap `Parser` struct. Name = `"gravity-cli"`, version derived from `build_information!()` macro via `OnceLock` statics.
- **`SubCommands`**: Enum with variants `Genesis`, `Validator`, `Stake`, `Node`, `Dkg`.
- **`Executable` trait**: Single method `fn execute(self) -> Result<(), anyhow::Error>`.
- **`short_version()`**: Returns package version string from build info.
- **`long_version()`**: Returns all build info key-value pairs joined by newlines.

### 2. `contract.rs` — ABI Bindings

- **`VALIDATOR_MANAGER_ADDRESS`**: Hardcoded to `0x00000000000000000000000000000001625F2001`.
- **`STAKING_ADDRESS`**: Hardcoded to `0x00000000000000000000000000000001625F2000`.
- **`alloy_sol_macro::sol!` block**: Defines Solidity-compatible types:
  - `ValidatorStatus` enum: `INACTIVE(0)`, `PENDING_ACTIVE(1)`, `ACTIVE(2)`, `PENDING_INACTIVE(3)`.
  - `ValidatorConsensusInfo` struct: validator, consensusPubkey, consensusPop, votingPower, validatorIndex, networkAddresses, fullnodeAddresses.
  - `ValidatorRecord` struct: full validator record with moniker, status, bond, feeRecipient, pendingFeeRecipient, stakingPool, validatorIndex.
  - `ValidatorManagement` contract: `registerValidator`, `joinValidatorSet`, `leaveValidatorSet`, `rotateConsensusKey`, `setFeeRecipient`, plus view functions and events.
  - `Staking` contract: `createPool` (payable), `isPool`, `getPoolVotingPower`, `getPoolVotingPowerNow`, `getPoolOperator`, `getPoolOwner`, `getPoolLockedUntil`, `getPoolActiveStake`, `getPoolCount`, `getPool`, `getAllPools`, plus `PoolCreated` event.
- **`status_from_u8(value: u8) -> ValidatorStatus`**: Maps 0–3 to enum variants; anything else maps to `__Invalid`.

### 3. `util.rs` — Conversion Helpers

- **`format_ether(wei: U256) -> String`**: Converts wei to ETH decimal string. If the wei string has ≤18 digits, it produces `0.{zero-padded}{digits}`. Otherwise splits at position `len-18`. Trailing zeros are trimmed from the decimal portion.
- **`parse_ether(eth_amount: &str) -> Result<U256>`**: Splits on `'.'`. Integer-only input gets 18 zeros appended. Fractional part is right-padded with zeros to 18 digits. Rejects inputs with >1 decimal point or >18 fractional digits.

### 4. `stake/create.rs` — CreateCommand

**Parameters**: `--rpc-url`, `--private-key`, `--gas-limit` (default 2,000,000), `--gas-price` (default 20), `--stake-amount`, `--lockup-duration` (default 2,592,000 = 30 days).

**Execution path**:
1. Strips `0x` prefix from private key, hex-decodes to bytes, constructs `SigningKey` → `PrivateKeySigner`.
2. Builds an `alloy` HTTP provider with the signer wallet.
3. Fetches chain ID and wallet balance (printed).
4. Parses `stake_amount` via `parse_ether()` to get wei value.
5. Fetches latest block timestamp; computes `locked_until = (timestamp + lockup_duration) * 1_000_000` (microseconds).
6. Constructs `Staking::createPoolCall` with `owner = staker = operator = voter = wallet_address`.
7. Sends transaction with `value = stake_wei` to `STAKING_ADDRESS`.
8. Waits for 2 confirmations (60s timeout).
9. Fetches receipt, parses `PoolCreated` event logs to extract the new pool address.

**State changes**: Creates a new StakePool on-chain. The sent ETH value becomes the pool's initial stake. All four roles (owner, staker, operator, voter) are set to the signing wallet's address.

### 5. `stake/get.rs` — GetCommand

**Parameters**: `--rpc-url`, `--owner`, `--from-block` (default `"auto"`), `--to-block` (default `"latest"`), `--show-voting-power` (default `true`).

**Execution path**:
1. Parses owner address, zero-pads to 32 bytes for topic filtering.
2. Creates an unauthenticated provider (no wallet needed — read-only).
3. Resolves block range: `"auto"` or `"earliest"` → `latest_block - 90_000` (saturating subtraction); `"latest"` → `Latest`; otherwise → parsed number.
4. Builds an event `Filter` for `STAKING_ADDRESS` with `topic0 = POOL_CREATED_EVENT_SIGNATURE` (hardcoded keccak256 `0x45d43f0d...`) and `topic3 = owner`.
5. Calls `provider.get_logs(&filter)`.
6. For each log, extracts pool address from `topics[2]` (last 20 bytes of 32-byte topic).
7. If `show_voting_power` is true, calls `Staking::getPoolVotingPowerNow` for each pool via `eth_call`; on failure, shows `"N/A"`.

**State changes**: None (read-only).

### 6. `validator/join.rs` — JoinCommand

**Parameters**: `--rpc-url`, `--private-key`, `--gas-limit` (default 2,000,000), `--gas-price` (default 20), `--stake-pool`, `--moniker` (default `"Gravity1"`), `--consensus-public-key`, `--consensus-pop`, `--network-public-key`, `--validator-network-address`, `--fullnode-network-address`.

**Execution path**:
1. Decodes private key → signer → provider (same pattern as create).
2. Calls `Staking::isPool(stake_pool)` to validate the pool exists.
3. Calls `Staking::getPoolVotingPowerNow(stake_pool)` to display voting power.
4. Calls `ValidatorManagement::isValidator(stake_pool)` to check if already registered.
5. **If not registered**, validates inputs:
   - Moniker: ≤31 bytes
   - Consensus public key: exactly 96 hex chars (48 bytes, BLS12-381)
   - Network public key: exactly 64 hex chars (32 bytes)
   - Consensus PoP: exactly 192 hex chars (96 bytes, BLS signature)
   - Network addresses: must match `/ip4/{host}/tcp/{port}` format
   - Full addresses constructed by appending `/noise-ik/{network_pk}/handshake/0`
6. Sends `ValidatorManagement::registerValidator` transaction (BCS-encodes network/fullnode addresses). Waits for 2 confirmations, checks for `ValidatorRegistered` event.
7. Calls `ValidatorManagement::getValidator(stake_pool)`, decodes full `ValidatorRecord`, prints all fields.
8. **If status is `INACTIVE`**, sends `ValidatorManagement::joinValidatorSet(stake_pool)`. Waits for 2 confirmations, checks for `ValidatorJoinRequested` event.
9. Final status check: expects `PENDING_ACTIVE` or `ACTIVE`.

**State changes**: Registers a new validator record on-chain (if not already registered) and transitions it from `INACTIVE` to `PENDING_ACTIVE`.

### 7. `validator/leave.rs` — LeaveCommand

**Parameters**: `--rpc-url`, `--private-key`, `--gas-limit` (default 2,000,000), `--gas-price` (default 20), `--stake-pool`.

**Execution path**:
1. Decodes private key → signer → provider.
2. Calls `ValidatorManagement::isValidator(stake_pool)`; errors if not registered.
3. Calls `ValidatorManagement::getValidator(stake_pool)`, decodes `ValidatorRecord`.
4. Status checks:
   - `PENDING_ACTIVE` or `ACTIVE`: proceeds to leave.
   - `PENDING_INACTIVE`: prints "already leaving", returns `Ok(())`.
   - `INACTIVE`: prints "already left", returns `Ok(())`.
   - Any other: returns error.
5. Sends `ValidatorManagement::leaveValidatorSet(stake_pool)`. Waits for 2 confirmations, checks for `ValidatorLeaveRequested` event.
6. Final status check: expects `PENDING_INACTIVE` or `INACTIVE`.

**State changes**: Transitions the validator from `ACTIVE`/`PENDING_ACTIVE` to `PENDING_INACTIVE`.

### 8. `validator/list.rs` — ListCommand

**Parameters**: `--rpc-url` only.

**Execution path**:
1. Creates an unauthenticated provider (no wallet — read-only).
2. Makes 6 `eth_call` queries to `VALIDATOR_MANAGER_ADDRESS`:
   - `getCurrentEpoch()`
   - `getTotalVotingPower()`
   - `getActiveValidatorCount()`
   - `getActiveValidators()`
   - `getPendingActiveValidators()`
   - `getPendingInactiveValidators()`
3. Converts results to `SerializableValidatorSet` (with nested `SerializableValidatorInfo` entries).
4. `convert_validator_info()` helper: hex-encodes consensus pubkey, BCS-decodes network/fullnode addresses (falls back to hex on failure), formats voting power via `format_ether`.
5. Outputs the entire set as pretty-printed JSON to stdout.

**State changes**: None (read-only).

### 9. `dkg/randomness.rs` — RandomnessCommand

**Parameters**: `--server-url`, `--block-number`.

**Execution path**:
1. `normalize_url()`: trims trailing `/`, prepends `http://` if no scheme present.
2. Builds a `reqwest::Client` with `danger_accept_invalid_certs(true)` and `danger_accept_invalid_hostnames(true)`.
3. Sends GET to `{base_url}/dkg/randomness/{block_number}`.
4. On non-success HTTP status, attempts to parse an `ErrorResponse` JSON body.
5. On success, parses `RandomnessResponse` JSON, prints `block_number: randomness_hex`.

**State changes**: None (read-only HTTP query).

### 10. `dkg/status.rs` — StatusCommand

**Parameters**: `--server-url`.

**Execution path**:
1. `normalize_url()`: same logic as randomness.
2. Builds a `reqwest::Client` with `danger_accept_invalid_certs(true)` and `danger_accept_invalid_hostnames(true)`.
3. Sends GET to `{base_url}/dkg/status`.
4. On success, parses `DKGStatusResponse` JSON with fields: `epoch`, `round`, `block_number`, `participating_nodes`.
5. Prints each field.

**State changes**: None (read-only HTTP query).

### 11. `genesis/key.rs` — GenerateKey

**Parameters**: `--random-seed` (optional, 64 hex chars), `--output-file`.

**Execution path**:
1. `key_generator()`: If `--random-seed` provided, strips `0x`, hex-decodes to `[u8; 32]`, creates `KeyGen::from_seed(seed_slice)`. Otherwise creates `KeyGen::from_os_rng()`.
2. Generates three key types:
   - **X25519 network key**: `key_gen.generate_x25519_private_key()`
   - **BLS12-381 consensus key**: `key_gen.generate_bls12381_private_key()`
   - **Ed25519 account key**: `key_gen.generate_ed25519_private_key()`
3. Derives `account_address` from consensus public key via SHA3-256 (`tiny_keccak::Sha3::v256`). Also prints the last 20 bytes as an ETH-format address.
4. Creates `ProofOfPossession` from the consensus private key.
5. Serializes all keys/addresses as a `ValidatorIndentity` struct (note: typo in "Indentity") to YAML, writes to `--output-file`.

**State changes**: Writes a YAML file containing `account_address`, `account_private_key`, `consensus_private_key`, `network_private_key`, `consensus_public_key`, `consensus_pop`, `network_public_key`.

### 12. `genesis/account.rs` — GenerateAccount

**Parameters**: `--output-file`.

**Execution path**:
1. `generate_eth_account()`:
   - Generates a random secp256k1 signing key via `SigningKey::random(&mut OsRng)`.
   - Derives the uncompressed public key via `verifying_key().to_sec1_bytes()`.
   - Strips the `0x04` prefix byte.
   - Hashes the remaining 64 bytes with Keccak-256.
   - Takes the last 20 bytes of the hash as the Ethereum address (prefixed with `0x`).
2. Serializes `Account { private_key, public_key, address }` to YAML, writes to `--output-file`.

**State changes**: Writes a YAML file containing `private_key`, `public_key`, `address`.

### 13. `genesis/waypoint.rs` — GenerateWaypoint

**Parameters**: `--input-file` (JSON), `--output-file`.

**Execution path**:
1. `load_genesis_config()`: Reads JSON file, deserializes to `GenesisConfig { validators: Vec<ValidatorEntry> }`. Each `ValidatorEntry` has `consensusPubkey`, `networkAddresses`, `fullnodeAddresses`, `votingPower`.
2. `generate_validator_set()`: For each validator:
   - Strips `0x` from consensus pubkey, hex-decodes, constructs `bls12381::PublicKey`.
   - Derives `AccountAddress` from consensus pubkey bytes via SHA3-256 (same derivation as `genesis/key.rs`).
   - Parses `votingPower` as `u128` (wei), divides by `10^18` to convert to ether, casts to `u64`.
   - Creates `ValidatorConfig` with BCS-encoded network/fullnode addresses (each wrapped in a `Vec` before BCS encoding).
   - Creates `ValidatorInfo` with account address, voting power, config, and empty vec for extra data.
   - Collects into `ValidatorSet`.
3. `generate_waypoint()`: Creates `LedgerInfoWithSignatures::genesis()` with the accumulator placeholder hash and the validator set, then calls `Waypoint::new_epoch_boundary()`.
4. Writes the waypoint string to `--output-file`.

**State changes**: Writes a waypoint string to a file.

### 14. `node/start.rs` — StartCommand

**Parameters**: `--deploy-path`.

**Execution path**:
1. Constructs paths: `{deploy_path}/script/start.sh` and `{deploy_path}/script/node.pid`.
2. Checks that `start.sh` exists.
3. `check_pid_file()`: If PID file exists, reads the PID, runs `ps -p {pid}` to check if the process is alive. Returns error whether the process is running ("already running") or not ("zombie PID file").
4. Executes `bash start.sh` with `current_dir` set to `deploy_path`, using `status()` (waits for script exit, not output).
5. Sleeps 500ms, then checks if PID file was created and prints the PID.

**State changes**: Spawns a node process via shell script. Does not directly write PID file (script does).

### 15. `node/stop.rs` — StopCommand

**Parameters**: `--deploy-path`.

**Execution path**:
1. Constructs path `{deploy_path}/script/stop.sh`.
2. Checks that `stop.sh` exists.
3. Executes `bash stop.sh` with `current_dir` set to `deploy_path`, using `output()` (captures stdout/stderr).
4. On failure, prints stderr. On success, prints stdout if non-empty.

**State changes**: Stops a node process via shell script.

---

## External Dependencies

| Dependency | Usage |
|------------|-------|
| `clap` | CLI argument parsing with derive macros |
| `alloy_primitives`, `alloy_provider`, `alloy_rpc_types`, `alloy_signer`, `alloy_signer_local`, `alloy_sol_types`, `alloy_sol_macro` | Ethereum JSON-RPC provider, transaction construction, ABI encoding/decoding, signing |
| `k256` | secp256k1 key generation (ETH accounts) |
| `sha3` (Keccak256) | Ethereum address derivation |
| `tiny_keccak` (SHA3-256) | Validator account address derivation from consensus pubkey |
| `gaptos::aptos_crypto` | BLS12-381 keys, X25519 keys, Ed25519 keys, ProofOfPossession |
| `gaptos::aptos_keygen` | `KeyGen` (seeded or OS-RNG based key generation) |
| `gaptos::aptos_types` | `AccountAddress`, `ValidatorSet`, `ValidatorConfig`, `ValidatorInfo`, `Waypoint`, `LedgerInfoWithSignatures` |
| `bcs` | Binary Canonical Serialization for network addresses |
| `reqwest` | HTTP client for DKG API queries |
| `tokio` | Async runtime (created per command via `Runtime::new()`) |
| `serde`, `serde_yaml`, `serde_json` | Serialization/deserialization for YAML/JSON output |
| `hex` | Hex encoding/decoding throughout |
| `build_info` | Build metadata for `--version` |
| `anyhow` | Error handling |
| `rand_core::OsRng` | Cryptographic RNG for account generation |

---

## Access Control Summary

| Command | Authentication | On-chain Writes |
|---------|---------------|-----------------|
| `stake create` | Private key required (signs tx) | Creates StakePool, transfers ETH |
| `stake get` | None (read-only) | None |
| `validator join` | Private key required (signs tx) | Registers validator, joins set |
| `validator leave` | Private key required (signs tx) | Leaves validator set |
| `validator list` | None (read-only) | None |
| `dkg randomness` | None | None (HTTP GET) |
| `dkg status` | None | None (HTTP GET) |
| `genesis generate-key` | None | None (writes local file) |
| `genesis generate-account` | None | None (writes local file) |
| `genesis generate-waypoint` | None | None (writes local file) |
| `node start` | Filesystem access | Executes shell script |
| `node stop` | Filesystem access | Executes shell script |

All on-chain authorization is delegated to the smart contracts (`ValidatorManagement` and `Staking`) which enforce their own access control based on the transaction sender's relationship to the stake pool roles. The CLI itself does not enforce any role-based access beyond requiring the private key to sign transactions.

---

## Notable Implementation Details

1. **Private key handling**: All commands accepting `--private-key` strip the `0x` prefix, hex-decode to raw bytes, construct a `SigningKey`, then a `PrivateKeySigner`. The private key is passed as a plaintext CLI argument (visible in process listings).

2. **DKG HTTP clients**: Both `randomness.rs` and `status.rs` build `reqwest::Client` with `danger_accept_invalid_certs(true)` and `danger_accept_invalid_hostnames(true)`, disabling TLS certificate validation entirely.

3. **Timestamp unit conversion**: `stake/create.rs` computes `locked_until = (current_timestamp + lockup_duration) * 1_000_000` converting from seconds to microseconds. The contract's `lockedUntil` parameter is a `u64`, so the maximum representable value is ~18.4 × 10^18 microseconds (~584,942 years from epoch).

4. **Voting power conversion**: `genesis/waypoint.rs` parses voting power as `u128` (wei) and performs integer division by `10^18`, then casts to `u64`. This truncates fractional ether amounts.

5. **Address derivation**: Two distinct derivation schemes exist:
   - **Validator account address**: SHA3-256 hash of BLS12-381 consensus public key bytes (used in `genesis/key.rs` and `genesis/waypoint.rs`)
   - **Ethereum account address**: Keccak-256 hash of uncompressed secp256k1 public key, last 20 bytes (used in `genesis/account.rs`)

6. **Node start/stop**: These commands delegate entirely to external shell scripts (`start.sh`/`stop.sh`). The `deploy_path` is user-provided and used directly in `PathBuf::from()` and as `current_dir` for subprocess execution.

7. **Transaction confirmation**: All write operations wait for 2 block confirmations with a 60-second timeout before fetching receipts and parsing events.

8. **`format_ether` edge case**: When the decimal portion is all zeros after trimming, the `trim_end_matches('.')` call removes the trailing dot, producing output like `"1"` instead of `"1.0"` or `"1."`.

### 🛠️ Ground Review

# Ground Review (Blue Team) — `gravity_cli` Binary

## Code Quality & Security Audit Report

---

### Critical

#### 1. **Private Keys Exposed via CLI Arguments**
**Files:** `stake/create.rs`, `validator/join.rs`, `validator/leave.rs`

All commands accepting `--private-key` take it as a plaintext CLI argument. This means the key is:
- Visible in `ps aux` / `/proc/<pid>/cmdline` to any user on the system
- Persisted in shell history (`~/.bash_history`, `~/.zsh_history`)
- Potentially logged by process-monitoring and audit tools

```rust
#[clap(long)]
pub private_key: String,
```

**Recommendation:** Accept private keys via environment variable (`$GRAVITY_PRIVATE_KEY`), file path (`--private-key-file`), or stdin. At minimum, add `#[clap(long, env = "GRAVITY_PRIVATE_KEY", hide_env_values = true)]`.

---

#### 2. **TLS Certificate Validation Completely Disabled**
**Files:** `dkg/randomness.rs`, `dkg/status.rs`

Both DKG HTTP clients unconditionally disable all TLS verification:

```rust
reqwest::Client::builder()
    .danger_accept_invalid_certs(true)
    .danger_accept_invalid_hostnames(true)
    .build()?
```

This makes all DKG queries vulnerable to man-in-the-middle attacks. An attacker on the network can intercept or forge DKG status and randomness responses.

**Recommendation:** Remove the `danger_*` flags. If self-signed certificates are needed for development, gate this behind an explicit `--insecure` flag (defaulting to `false`) and emit a warning when used.

---

#### 3. **Generated Key Files Written Without Restrictive Permissions**
**Files:** `genesis/key.rs`, `genesis/account.rs`

Private key material (BLS, Ed25519, X25519, secp256k1) is serialized to YAML and written via `fs::write()`, which inherits the process umask — typically resulting in world-readable `0644` permissions:

```rust
fs::write(&self.output_file, yaml)?;
```

**Recommendation:** Set file permissions to `0600` before or immediately after writing:
```rust
#[cfg(unix)]
{
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&self.output_file, fs::Permissions::from_mode(0o600))?;
}
```

---

### Warning

#### 4. **Path Traversal / Command Injection via `--deploy-path`**
**Files:** `node/start.rs`, `node/stop.rs`

The `deploy_path` is taken directly from user input and used to construct the script path and set `current_dir` for subprocess execution. There is no sanitization or validation that the path is within an expected directory:

```rust
let script_path = PathBuf::from(&self.deploy_path).join("script/start.sh");
Command::new("bash").arg(&script_path).current_dir(&self.deploy_path).status()?;
```

While this is a local CLI tool (the user controls input), the pattern is risky if the CLI is ever wrapped in a service or called with untrusted input.

**Recommendation:** Canonicalize the path (`fs::canonicalize`) and validate it resolves to an expected parent directory. At minimum, verify the resolved script path doesn't escape the deploy directory via symlinks.

---

#### 5. **`check_pid_file` Returns Error on Stale PID — Prevents Recovery**
**File:** `node/start.rs`

When the PID file exists but the process is not running, the function still returns an error ("zombie PID file") instead of cleaning up the stale file and allowing a restart:

```rust
// If PID file exists and process is NOT running → error
```

**Recommendation:** Log a warning, remove the stale PID file, and allow the start to proceed. Stale PID files are a normal occurrence after crashes.

---

#### 6. **Duplicated Code: `normalize_url` and HTTP Client Construction**
**Files:** `dkg/randomness.rs`, `dkg/status.rs`

The `normalize_url()` function and the `reqwest::Client` builder are copy-pasted identically across both files.

**Recommendation:** Extract into a shared module (e.g., `dkg/client.rs` or `util.rs`) to ensure consistent behavior and a single point of change for TLS configuration.

---

#### 7. **`format_ether` Inconsistent Output for Sub-Wei Values**
**File:** `util.rs`

When `wei` has ≤18 digits, `format_ether` always includes trailing zeros in the decimal portion (e.g., `"0.000000000000000001"`), but when `wei` has >18 digits, it trims trailing zeros (e.g., `"1"` instead of `"1.0"`). The two branches produce stylistically inconsistent output.

```rust
if len <= 18 {
    format!("0.{}", "0".repeat(18 - len) + &wei_str)  // NOT trimmed
} else {
    let (integer, decimal) = wei_str.split_at(len - 18);
    format!("{}.{}", integer, decimal.trim_end_matches('0').trim_end_matches('.'))  // Trimmed
}
```

**Recommendation:** Apply the same trimming logic in both branches for consistency.

---

#### 8. **`parse_ether` Does Not Validate Leading Zeros or Negative Input**
**File:** `util.rs`

Inputs like `"007.5"`, `"-1.0"`, or `""` are not explicitly rejected and may produce unexpected results depending on `U256::from_str` behavior.

**Recommendation:** Add explicit validation: reject empty strings, strings starting with `-`, and optionally warn on leading zeros.

---

#### 9. **Voting Power Truncation Without Warning**
**File:** `genesis/waypoint.rs`

Voting power is converted from wei (`u128`) to ether by integer division, then narrowed to `u64`. Fractional ether amounts are silently truncated, and values > `u64::MAX` ether would silently overflow:

```rust
let voting_power_eth = voting_power_wei / 10u128.pow(18);
let voting_power = voting_power_eth as u64;
```

**Recommendation:** Add a range check before casting and log a warning if any fractional amount is lost.

---

### Info

#### 10. **Typo in Struct Name: `ValidatorIndentity`**
**File:** `genesis/key.rs`

```rust
struct ValidatorIndentity { ... }
```

Should be `ValidatorIdentity`. This struct is serialized to YAML output, so the typo is user-visible only if the struct name itself is emitted (it isn't with `serde`, but it's poor hygiene).

---

#### 11. **Hardcoded Gas Defaults May Become Stale**
**Files:** `stake/create.rs`, `validator/join.rs`, `validator/leave.rs`

Gas limit (2,000,000) and gas price (20 wei) are hardcoded defaults. A gas price of 20 wei is extremely low and may not be appropriate for all network conditions.

**Recommendation:** Consider adding `eth_gasPrice` estimation as a fallback when no price is explicitly provided.

---

#### 12. **Each Async Command Creates Its Own Tokio Runtime**
**Files:** All `execute()` implementations

Every subcommand creates a new `tokio::runtime::Runtime::new()`. This is functional but wasteful if the CLI ever needs to run multiple commands.

```rust
fn execute(self) -> Result<(), anyhow::Error> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(self.execute_async())
}
```

**Recommendation:** Low priority — for a CLI tool this is fine, but a shared `#[tokio::main]` on the entry point would be cleaner.

---

#### 13. **`ErrorResponse` Struct Duplicated Across DKG Modules**
**Files:** `dkg/randomness.rs`, `dkg/status.rs`

```rust
#[derive(Deserialize, Debug)]
struct ErrorResponse {
    error: String,
}
```

Identical definition in both files. Should be extracted to a common location.

---

## Summary

| Severity | Count | Key Themes |
|----------|-------|------------|
| **Critical** | 3 | Private key exposure via CLI args, TLS disabled, key files world-readable |
| **Warning** | 6 | Path traversal risk, stale PID handling, code duplication, ether parsing edge cases, silent truncation |
| **Info** | 4 | Naming typo, hardcoded gas defaults, runtime creation pattern, struct duplication |

**Top 3 Fixes to Prioritize:**
1. Move private key input to env var / file / stdin — eliminates process-list exposure
2. Remove `danger_accept_invalid_certs` / `danger_accept_invalid_hostnames` — restore TLS guarantee
3. Set `0600` permissions on generated key files — prevent local credential theft

### ⚔️ Intention Attacker

# Intention Attack Report: `gravity_cli` Security Audit

## Critical Severity

### 1. Private Keys Exposed in Process Listings and Shell History (Critical)

**Files:** `stake/create.rs`, `validator/join.rs`, `validator/leave.rs`

All commands accepting `--private-key` take the raw hex secret as a CLI argument. On Unix systems:
- **Any user on the machine** can see the key via `ps aux`, `/proc/<pid>/cmdline`, or `top`.
- The key is permanently recorded in shell history (`~/.bash_history`, `~/.zsh_history`).
- Process accounting and audit logs capture it.

This is not a theoretical concern — it is an **active key exfiltration vector** on any multi-user or compromised system. A validator's private key controls the ability to create stake pools, join/leave the validator set, and transfer ETH.

**Impact:** Full compromise of validator funds and identity. An attacker with read access to process listings or shell history can steal the private key and drain the associated wallet.

**Recommendation:** Accept private keys via environment variable, file path (with permission checks), or stdin — never as a CLI argument.

---

### 2. Generated Private Keys Written to Disk with No File Permission Hardening (Critical)

**Files:** `genesis/key.rs:91`, `genesis/account.rs:56`

Both `GenerateKey` and `GenerateAccount` use `fs::write()` to write private keys (consensus, network, account, secp256k1) as plaintext hex into YAML files. The files are created with the **default umask** (typically `0644` — world-readable).

```rust
// key.rs
fs::write(self.output_file, yaml_string)?;

// account.rs
fs::write(self.output_file, yaml_string)?;
```

**Impact:** Any local user or process can read the generated key material. On shared infrastructure or CI environments, this silently leaks validator identity keys and Ethereum private keys.

**Recommendation:** Use `std::os::unix::fs::OpenOptionsExt` to set mode `0600` (owner-read/write only), or encrypt the output.

---

### 3. TLS Certificate Validation Completely Disabled on DKG Endpoints (Critical)

**Files:** `dkg/randomness.rs:47-49`, `dkg/status.rs:42-44`

```rust
let client = reqwest::Client::builder()
    .danger_accept_invalid_certs(true)
    .danger_accept_invalid_hostnames(true)
    .build()?;
```

Both DKG query commands disable **all** TLS verification. An attacker performing a MITM attack can:
- Return fabricated randomness values, potentially influencing consensus or leader election.
- Return false DKG status, making operators believe the DKG ceremony is healthy when it is not (or vice versa, triggering unnecessary operator intervention).

The `normalize_url()` function defaults to `http://` (no TLS at all), compounding the issue — even users who explicitly type `https://` get no protection.

**Impact:** Full MITM on DKG protocol queries. In a blockchain context, manipulated randomness is a direct attack vector against consensus fairness.

---

## High Severity

### 4. Voting Power Truncation Can Produce Zero-Power Validators (High)

**File:** `genesis/waypoint.rs:82-83`

```rust
let voting_power_wei: u128 = v.voting_power.parse()?;
let voting_power: u64 = (voting_power_wei / 1_000_000_000_000_000_000) as u64;
```

Two bugs compound here:
1. **Silent truncation:** Integer division discards the fractional ether. A validator with `999_999_999_999_999_999` wei (just under 1 ETH) gets `voting_power = 0`. A zero-power validator in the genesis set could cause consensus failures or be excluded from BFT quorum calculations in ways that violate assumptions.
2. **`as u64` overflow:** If `voting_power_wei / 10^18` exceeds `u64::MAX` (~18.4 × 10^18 ETH), the cast silently wraps. While unlikely now, this is a latent correctness hole in genesis generation.

**Impact:** Malformed genesis validator set. A validator with sub-1-ETH stake silently becomes a 0-power participant, distorting quorum requirements.

---

### 5. Node Start/Stop: Unsanitized Path Used in Shell Execution (High)

**Files:** `node/start.rs:54`, `node/stop.rs:20`

```rust
let deploy_path = PathBuf::from(&self.deploy_path);
// ...
Command::new("bash").arg(script_path.as_os_str()).current_dir(&deploy_path).status()?;
```

`deploy_path` is user-supplied and used directly as the working directory for `bash` execution, with no validation that:
- The path doesn't traverse outside expected boundaries (e.g., `../../etc/init.d`)
- The `start.sh`/`stop.sh` scripts at that path are trusted

An attacker who can influence the `--deploy-path` argument (e.g., via a wrapper script, config file, or CI pipeline) can execute arbitrary code as the CLI user.

**Impact:** Arbitrary code execution if the deploy path is attacker-controlled.

---

### 6. Zombie PID File Blocks Node Restart — No Recovery Path (High)

**File:** `node/start.rs:31-40`

```rust
if output.status.success() {
    Err(anyhow::anyhow!("Node is already running with PID {}...", pid, ...))
} else {
    Err(anyhow::anyhow!("PID file exists but process {} is not running (zombie PID file: {})", ...))
}
```

If a node crashes without cleaning up its PID file, `check_pid_file()` returns an error in **both branches** — whether the process exists or not. There is no way to restart the node via the CLI without manually deleting the PID file.

In a production validator operation scenario, this means:
- A node crash → operator cannot restart via CLI → manual intervention required → **extended validator downtime**.
- Automated restart scripts using this CLI will permanently fail until human intervention.

**Impact:** Denial of service to the operator's own node. Extended downtime impacts validator liveness and may trigger slashing in PoS systems.

---

### 7. Stake Creation: All Four Roles Set to Same Address — No Separation of Concerns (High)

**File:** `stake/create.rs` (full `execute_async` flow)

The `createPool` call hardcodes `owner = staker = operator = voter = wallet_address`:

```rust
// owner, staker, operator, voter all set to wallet_address
```

This defeats the purpose of the role-based access control model in the Staking contract. If the operator key is compromised (which is more likely since it's used for day-to-day operations), the attacker has full owner-level control including:
- Changing the fee recipient
- Withdrawing the entire stake
- Changing the voter and operator addresses

**Impact:** Eliminates the security benefit of role separation in staking. A single key compromise gives total control over the stake pool.

---

## Medium Severity

### 8. `format_ether` Off-by-One Potential and Missing Zero-Value Edge Case (Medium)

**File:** `util.rs`

The `format_ether` function manually implements wei-to-ETH conversion with string manipulation. When `wei_str.len() <= 18`, it constructs a zero-padded decimal. However:
- For `U256::ZERO`, `to_string()` produces `"0"` (length 1), which enters the `<= 18` branch producing `"0.000000000000000000"`, then trimmed to `"0"`. While functionally correct, the hand-rolled nature of this conversion (vs. using a library) makes it fragile for a financial display function.
- No rounding is ever performed — pure truncation — which could cause display inconsistencies vs. what the chain actually uses.

**Impact:** Display-level inaccuracy could cause operators to misjudge stake amounts when making financial decisions.

---

### 9. Deterministic Key Generation Has No Runtime Guard (Medium)

**File:** `genesis/key.rs:42-49`

```rust
if let Some(ref seed) = self.random_seed {
    let seed = seed.strip_prefix("0x").unwrap_or(seed);
    let mut seed_slice = [0u8; 32];
    hex::decode_to_slice(seed, &mut seed_slice)?;
    Ok(KeyGen::from_seed(seed_slice))
}
```

The `--random-seed` flag produces fully deterministic, reproducible keys. While the doc comment warns against it, there is no runtime warning printed to stderr, no `--i-know-what-i-am-doing` confirmation gate, and no marking in the output file that the keys were deterministically generated.

If an operator copies a command line from documentation or a tutorial that includes a sample seed, they will generate keys that anyone reading the same documentation can reproduce.

**Impact:** Validator key compromise if deterministic seed is used in production (which the CLI does nothing to prevent at runtime).

---

### 10. TOCTOU Race in Validator Join: State Check Before Transaction Submission (Medium)

**File:** `validator/join.rs`

The `join` flow performs:
1. `isValidator()` check → **read**
2. `getValidator()` → **read** status
3. `registerValidator()` → **write** (if not registered)
4. `getValidator()` → **read** status again
5. `joinValidatorSet()` → **write** (if INACTIVE)

Between step 1 and step 3, another transaction could register the same validator, causing the `registerValidator` call to fail or behave unexpectedly on-chain. Similarly, between step 4 and step 5, the status could change. While the on-chain contracts likely handle this gracefully (revert), the CLI error handling path does not anticipate this race — it would surface as an opaque revert error rather than a clear diagnostic.

**Impact:** Poor operator experience in concurrent registration scenarios; potential gas waste from reverted transactions.

---

### 11. Event Signature Hardcoded as Magic Constant (Medium)

**File:** `stake/get.rs`

```rust
// topic0 = POOL_CREATED_EVENT_SIGNATURE (hardcoded keccak256)
```

The `PoolCreated` event signature hash is hardcoded as a hex literal rather than derived from the ABI at compile time. If the Solidity contract's `PoolCreated` event signature changes (e.g., parameter reordering, type change), this constant will silently filter for a non-existent event, returning **zero results** with no error.

**Impact:** Silent data loss — `stake get` would return no pools even when pools exist, giving operators a false belief that their stake pools don't exist.

---

## Summary

| # | Finding | Severity | Category |
|---|---------|----------|----------|
| 1 | Private keys exposed via CLI arguments | **Critical** | Credential Exposure |
| 2 | Key files written world-readable | **Critical** | Credential Exposure |
| 3 | TLS verification disabled on DKG endpoints | **Critical** | MITM / Integrity |
| 4 | Voting power truncation to zero | **High** | Consensus Integrity |
| 5 | Unsanitized path in shell execution | **High** | Code Execution |
| 6 | Zombie PID blocks node restart | **High** | Availability / DoS |
| 7 | No role separation in stake pool creation | **High** | Access Control |
| 8 | Hand-rolled ETH formatting fragility | **Medium** | Financial Display |
| 9 | Deterministic keygen with no runtime guard | **Medium** | Credential Exposure |
| 10 | TOCTOU race in validator join flow | **Medium** | Race Condition |
| 11 | Hardcoded event signature hash | **Medium** | Silent Failure |

---

## [shared_crates_and_dependencies] Review shared library crates for: (1) crates/gravity-sdk/src/lib.rs for the SDK 

### 🎯 Intention

Review shared library crates for: (1) crates/gravity-sdk/src/lib.rs for the SDK public API surface; (2) crates/proposer-reth-map/src/lib.rs for the proposer-to-reth-address mapping (get_reth_address_by_index) for race conditions and correctness; (3) crates/txn_metrics/src/lib.rs for transaction metrics collection; (4) crates/build-info/ for build information leakage; (5) crates/api/src/config_storage.rs for the ConfigStorageWrapper and GLOBAL_CONFIG_STORAGE safety; (6) crates/api/src/consensus_mempool_handler.rs for the consensus-to-mempool notification pipeline.

### 🔍 Discovered Implementation

# Shared Library Crates — Implementation Analysis

---

## 1. `crates/gravity-sdk/src/lib.rs` — SDK Public API Surface

### Files Involved
- `crates/gravity-sdk/src/lib.rs`

### Implementation
The SDK crate is a **pure re-export facade** consisting of 3 lines:

```rust
pub use api;
pub use block_buffer_manager;
pub use gaptos;
```

### Key Functions
None — no logic. The crate's sole purpose is to re-export three sub-crates (`api`, `block_buffer_manager`, `gaptos`) as the public API surface of `gravity-sdk`.

### State Changes
None.

### External Dependencies
- `api` crate
- `block_buffer_manager` crate
- `gaptos` crate

---

## 2. `crates/proposer-reth-map/src/lib.rs` — Proposer-to-Reth Address Mapping

### Files Involved
- `crates/proposer-reth-map/src/lib.rs`

### Implementation
A **global static `HashMap`** protected by an `InfallibleRwLock` (a `RwLock` that panics on poison), lazily initialized via `once_cell::Lazy`.

```rust
static PROPOSER_RETH_ADDRESS_MAP: Lazy<InfallibleRwLock<HashMap<u64, Vec<u8>>>>
```

### Key Functions

| Function | Signature | Behavior |
|---|---|---|
| `get_reth_address_by_index` | `(validator_index: u64) -> Option<Vec<u8>>` | Acquires a **read lock**, looks up the index, returns a `.cloned()` copy of the `Vec<u8>` address. |
| `update_proposer_reth_index_map` | `(validator_set: &ValidatorSet)` | Builds a **new** `HashMap` from scratch by iterating `validator_set.active_validators`, then acquires a **write lock** and replaces the entire map via `*map.write() = new_map`. |

### Execution Path — Read
1. Caller invokes `get_reth_address_by_index(index)`
2. `PROPOSER_RETH_ADDRESS_MAP.read()` acquires shared read lock
3. `.get(&validator_index).cloned()` returns `Option<Vec<u8>>`
4. Lock released on drop

### Execution Path — Write (Epoch Change)
1. Caller invokes `update_proposer_reth_index_map(validator_set)`
2. A new `HashMap` is built locally (no lock held during construction)
3. `PROPOSER_RETH_ADDRESS_MAP.write()` acquires exclusive write lock
4. Entire map is atomically swapped (old map dropped)
5. Lock released on drop

### State Changes
- Full replacement of the global `HashMap<u64, Vec<u8>>` on each epoch change.

### Concurrency Characteristics
- **Read path**: Multiple concurrent readers allowed (shared `RwLock` semantics).
- **Write path**: The new map is built entirely before acquiring the write lock, minimizing lock-hold duration. The swap is a single pointer-width assignment.
- **Atomicity boundary**: A reader calling `get_reth_address_by_index` during `update_proposer_reth_index_map` will see either the complete old map or the complete new map — never a partially updated state. However, there is **no cross-call atomicity** — two sequential reads (e.g., look up index A, then index B) could span an epoch boundary, returning values from different epochs.
- **`InfallibleRwLock`**: If a thread panics while holding the lock, subsequent lock attempts will also **panic** (intentional design — it treats poison as fatal rather than recoverable).

---

## 3. `crates/txn_metrics/src/lib.rs` — Transaction Lifecycle Metrics

### Files Involved
- `crates/txn_metrics/src/lib.rs`

### Implementation
A **singleton** (`OnceLock<TxnLifeTime>`) that tracks transaction timing through the pipeline stages. Gated by the `TXN_LIFE_ENABLED` environment variable (checked once at first call via `OnceLock`).

### Data Structures

```rust
struct TxnLifeTime {
    txn_initial_add_time: DashMap<(AccountAddress, u64), SystemTime>,  // (sender, seq_num) -> when added
    txn_hash_to_key: DashMap<HashValue, (AccountAddress, u64)>,        // tx_hash -> (sender, seq_num)
    txn_batch_id: DashMap<BatchId, HashSet<TxnKey>>,                   // batch -> set of tx keys
    txn_block_id: DashMap<HashValue, HashSet<TxnKey>>,                 // block -> set of tx keys
}
```

Capacity constants: `MAX_TXN_INITIAL_ADD_TIME_CAPACITY = 100,000`, `MAX_TXN_BATCH_ID_CAPACITY = 10,000`, `MAX_TXN_BLOCK_ID_CAPACITY = 1,000`.

### Key Functions — Lifecycle Recording

| Function | What it records | Histogram emitted |
|---|---|---|
| `record_added(txn)` | First-seen timestamp for `(sender, seq_num)` | None (stores initial time) |
| `record_batch(batch_id, txns)` | Time from add → batch inclusion | `aptos_txn_added_to_batch_time_seconds` |
| `record_broadcast_batch(batch_id)` | Time from add → broadcast | `aptos_txn_added_to_broadcast_batch_time_seconds` |
| `record_before_persist(batch_id)` | Time from add → pre-persist | `aptos_txn_added_to_before_batch_persist_time_seconds` |
| `record_after_persist(batch_id)` | Time from add → post-persist | `aptos_txn_added_to_after_batch_persist_time_seconds` |
| `record_proof(batch_id)` | Time from add → proof generated | `aptos_txn_added_to_proof_time_seconds` |
| `record_block(payload, block_id)` | Time from add → block inclusion | `aptos_txn_added_to_block_time_seconds` |
| `record_executing(block_id)` | Time from add → execution start | `aptos_txn_added_to_executing_time_seconds` |
| `record_executed(block_id)` | Time from add → execution complete | `aptos_txn_added_to_executed_time_seconds` |
| `record_block_committed(block_id)` | Time from add → block committed | `aptos_txn_added_to_block_committed_time_seconds` |
| `record_committed(sender, seq_num)` | Time from add → final commit | `aptos_txn_added_to_committed_time_seconds` |

### `record_block` — Payload Dispatch

Handles 5 payload variants:
- **`DirectMempool(txns)`**: Iterates inline transactions, inserts hash→key mapping, observes block time, stores keys in `txn_block_id`.
- **`InQuorumStore(proof_with_data)`**: Delegates to `process_proof_with_data`, which looks up `txn_batch_id` for each proof's batch_id.
- **`InQuorumStoreWithLimit`**: Same as above, unwraps `.proof_with_data` field.
- **`QuorumStoreInlineHybrid(vec_payload, proof_with_data, _)`**: Processes proof data first, then processes inline transactions — both contribute to the same `txn_block_id` entry via `.entry().or_default().extend()`.
- **`OptQuorumStore`**: No-op (empty match arm).

### `record_committed` — Cleanup Path

1. Observes the final "added to committed" metric.
2. Removes the `(sender, seq_num)` from `txn_initial_add_time`.
3. Performs a **full scan** of `txn_hash_to_key` via `.retain()` to remove all matching entries.
4. Performs a **full scan** of `txn_batch_id` via `iter_mut()` → `.remove()` on each batch's `HashSet`, then retains non-empty batches.
5. Same full scan for `txn_block_id`.

### `cleanup_old_entries` — Capacity Control

Triggered when `txn_initial_add_time.len() >= 100,000`:
1. Collects all keys with age > 60 seconds.
2. Removes those keys from `txn_initial_add_time` and `txn_hash_to_key`.
3. If `txn_batch_id >= 10,000`: retains only batches referencing still-existing transactions.
4. If `txn_block_id >= 1,000`: same retention logic.

### Concurrency Characteristics
- All maps are `DashMap` (sharded concurrent maps) — no global mutex.
- Each `record_*` method independently acquires per-shard locks on the relevant `DashMap`.
- `record_committed` and `cleanup_old_entries` perform full iteration across all shards of multiple `DashMap`s simultaneously. `DashMap::retain` holds shard locks one at a time during iteration.
- **The `cleanup_old_entries` call within `record_added`** (line 214) can call `self.txn_hash_to_key.retain()` while `self.txn_initial_add_time` shard locks are temporarily acquired during the inner `contains_key` check — the DashMap design prevents deadlocks since these are different maps, but there is **no transactional consistency** across the multi-map cleanup.
- The `cleanup_old_entries` inside `txn_batch_id.retain` calls `self.txn_initial_add_time.contains_key(key)` — this acquires a read lock on `txn_initial_add_time` from within a write lock callback on `txn_batch_id`. Since these are different `DashMap` instances, no single-map deadlock occurs, but concurrent `record_added` inserting into `txn_initial_add_time` could race with the `contains_key` check, causing a just-inserted transaction to be cleaned up.

### Histogram Initialization
All 10 histograms use `OnceLock` + `register_histogram!` with `.unwrap()`. Each is initialized on first use. The `.unwrap()` will panic if the metric name is already registered in the global Prometheus registry by another component.

---

## 4. `crates/build-info/` — Build Information

### Files Involved
- `crates/build-info/build.rs` — build script
- `crates/build-info/src/lib.rs` — runtime library

### Build Script (`build.rs`)
1. Propagates `CARGO_CFG_TOKIO_UNSTABLE` as `USING_TOKIO_UNSTABLE` rustc-env variable.
2. Sets `cargo:rerun-if-changed` on `build.rs` and `../../.git/HEAD` (if it exists).
3. Invokes `shadow_rs::ShadowBuilder` with `deny_const(Default::default())` to generate compile-time git/build metadata.

### Runtime Library (`lib.rs`)

**`get_build_information() -> BTreeMap<String, String>`**:
1. Calls `shadow!(build)` to access shadow_rs compile-time constants.
2. Populates a `BTreeMap` with 12 keys:
   - `build_branch` — git branch at build time
   - `build_cargo_version` — cargo version
   - `build_clean_checkout` — whether git working tree was clean
   - `build_commit_hash` — git commit SHA
   - `build_tag` — git tag
   - `build_time` — build timestamp
   - `build_os` — build OS
   - `build_rust_channel` — stable/nightly/beta
   - `build_rust_version` — rustc version
   - `build_is_release_build` — debug vs release
   - `build_profile_name` — derived from `OUT_DIR` path (3rd-from-last path segment)
   - `build_using_tokio_unstable` — whether tokio unstable features enabled
3. **Runtime environment override**: checks `GIT_SHA`, `GIT_BRANCH`, `GIT_TAG`, `BUILD_DATE` environment variables and overwrites the corresponding entries if present.

**`build_information!()` macro**: Calls `get_build_information()` then inserts `build_pkg_version` from `CARGO_PKG_VERSION`.

**`get_git_hash() -> String`**: Returns `GIT_SHA` env var if set, otherwise falls back to the shadow_rs `COMMIT_HASH`.

### Information Exposed
The crate exposes: git branch, commit hash, tag, build time, OS, cargo version, rust version/channel, profile name, clean checkout status, tokio unstable flag, and package version. All values are compile-time constants with optional runtime env-var overrides (for Docker builds).

---

## 5. `crates/api/src/config_storage.rs` — ConfigStorageWrapper

### Files Involved
- `crates/api/src/config_storage.rs` — wrapper implementation
- `crates/api/src/consensus_api.rs` — sets `GLOBAL_CONFIG_STORAGE` (line 136)
- `crates/api/src/https/consensus.rs` — reads `GLOBAL_CONFIG_STORAGE` (line 285)
- `crates/api/src/https/dkg.rs` — reads `GLOBAL_CONFIG_STORAGE` (line 111)
- `bin/gravity_node/src/relayer.rs` — reads `GLOBAL_CONFIG_STORAGE` (line 109)
- `aptos-core/consensus/src/consensusdb/include/reader.rs` — reads `GLOBAL_CONFIG_STORAGE` (line 26)

### Implementation

**`ConfigStorageWrapper`** wraps an `Arc<dyn ConfigStorage>` and implements the `ConfigStorage` trait:

```rust
fn fetch_config_bytes(&self, config_name: OnChainConfig, block_number: BlockNumber) -> Option<OnChainConfigResType>
```

**Execution Path**:
1. **Always** prints to stdout via `println!` and logs via `info!` — both include the `config_name` and `block_number`.
2. Checks `config_name` against an allowlist of 8 variants:
   - `Epoch`, `ValidatorSet`, `JWKConsensusConfig`, `ObservedJWKs`, `RandomnessConfig`, `OracleState`, `DKGState`, `ConsensusConfig`
3. If matched → delegates to the inner `self.config_storage.fetch_config_bytes(config_name, block_number)`.
4. If not matched (`_` wildcard) → returns `None`.

### `GLOBAL_CONFIG_STORAGE` Usage Pattern
- **Type**: Defined externally (in `gaptos::api_types::config_storage`), likely an `OnceLock` or similar static.
- **Writer**: `consensus_api.rs:136` — calls `GLOBAL_CONFIG_STORAGE.set(config)` (one-time initialization).
- **Readers**: 4 call sites across `consensus.rs`, `dkg.rs`, `relayer.rs`, and `consensusdb/reader.rs` — all access via `.get()` which returns `Option`.
- **OnceLock semantics**: The `.set()` succeeds only once; subsequent `.set()` calls return `Err`. Readers use `.get()` and handle the `None` (uninitialized) case, with explicit error logging in consensus.rs (line 326) and dkg.rs (line 178).

### State Changes
None — purely a read-through filter/proxy.

---

## 6. `crates/api/src/consensus_mempool_handler.rs` — Consensus-to-Mempool Notification Pipeline

### Files Involved
- `crates/api/src/consensus_mempool_handler.rs`

### Data Structures

**`MempoolNotificationHandler<M: MempoolNotificationSender>`**:
- Holds `mempool_notification_sender: M`
- Clone-able

**`ConsensusToMempoolHandler<M: MempoolNotificationSender>`**:
- `mempool_notification_handler: MempoolNotificationHandler<M>`
- `consensus_notification_listener: ConsensusNotificationListener`
- `event_subscription_service: Arc<Mutex<EventSubscriptionService>>` (tokio Mutex)

### Key Functions

**`MempoolNotificationHandler::notify_mempool_of_committed_transactions`**
- Signature: `(&mut self, committed_transactions: Vec<Transaction>, block_timestamp_usecs: u64) -> anyhow::Result<()>`
- Calls `self.mempool_notification_sender.notify_new_commit(committed_transactions, block_timestamp_usecs).await`
- On error: **calls `todo!()`** — this will panic at runtime if the mempool notification fails.

**`ConsensusToMempoolHandler::handle_consensus_commit_notification`**
- Signature: `(&mut self, consensus_commit_notification: ConsensusCommitNotification) -> anyhow::Result<()>`
- Execution path:
  1. Extracts committed transactions via `.get_transactions().clone()`
  2. Calls `notify_mempool_of_committed_transactions` with transactions and **`SystemTime::now()`** converted to seconds (not microseconds, despite the parameter name `block_timestamp_usecs`) — there is a TODO comment acknowledging this needs modification.
  3. Extracts `block_number` and `subscribable_events` from the notification.
  4. Acquires the `event_subscription_service` tokio mutex lock.
  5. Calls `event_subscription_service.notify_events(block_number, events).unwrap()` — panics on error.
  6. Responds to consensus via `respond_to_commit_notification(notification, Ok(()))`.

**`ConsensusToMempoolHandler::handle_consensus_notification`**
- Dispatches on `ConsensusNotification` variant:
  - **`NotifyCommit`** → delegates to `handle_consensus_commit_notification`
  - **`SyncToTarget`** → acquires `event_subscription_service` lock, calls `notify_initial_configs(block_number).unwrap()`, responds `Ok(())` to consensus. The `map_err` result on `respond_to_sync_target_notification` is bound to `let _` (silently discarded).
  - **`SyncForDuration`** → **calls `todo!()`** — will panic if this variant is received.
- On any `Err` from the dispatched handler: logs a warning via `warn!`.

**`ConsensusToMempoolHandler::start`**
- Runs an infinite loop using `futures::select!`.
- Single arm: awaits `consensus_notification_listener.select_next_some()`, then calls `handle_consensus_notification`.
- A progress check interval arm is commented out.

### Execution Path — Full Commit Notification Flow
```
ConsensusNotificationListener (channel)
  → start() loop via futures::select!
    → handle_consensus_notification(NotifyCommit)
      → handle_consensus_commit_notification
        → mempool_notification_sender.notify_new_commit(txns, timestamp)
        → event_subscription_service.lock().notify_events(block_number, events)
        → respond_to_commit_notification(notification, Ok(()))
```

### External Dependencies
- `gaptos::aptos_consensus_notifications` — `ConsensusCommitNotification`, `ConsensusNotification`, `ConsensusNotificationListener`
- `gaptos::aptos_event_notifications` — `EventNotificationSender`, `EventSubscriptionService`
- `gaptos::aptos_mempool_notifications` — `MempoolNotificationSender`
- `tokio::sync::Mutex` — used for `EventSubscriptionService`

### State Changes
- Mempool state: modified indirectly via `notify_new_commit` (removes committed transactions from mempool).
- Event subscriptions: `notify_events` and `notify_initial_configs` propagate events to downstream subscribers.
- Consensus acknowledgment: `respond_to_commit_notification` / `respond_to_sync_target_notification` send ack back through the consensus channel.

### Notable Implementation Details
- Two `todo!()` calls exist that will **panic** at runtime: one in `notify_mempool_of_committed_transactions` error path (line 41), one in `SyncForDuration` handler (line 112).
- The `block_timestamp_usecs` parameter receives seconds (`.as_secs()`) not microseconds — the TODO on line 75 acknowledges this mismatch.
- `notify_events().unwrap()` on line 85 will panic if event notification fails.
- The `SyncToTarget` handler discards the error from `respond_to_sync_target_notification` via `let _`.

### 🛠️ Ground Review

# Ground Review (Blue Team) — Shared Library Crates

---

## Critical

### 1. `todo!()` panics in production paths — `consensus_mempool_handler.rs`

Two `todo!()` macros exist in reachable runtime code paths:

- **Line ~41**: `notify_mempool_of_committed_transactions` error handler — if the mempool notification sender ever returns an error, the node crashes.
- **Line ~112**: `SyncForDuration` variant handler — if consensus ever sends this notification variant, the node crashes.

These are not compile-time placeholders; they are live landmines in the consensus commit pipeline. A mempool channel backpressure event or a future protocol upgrade enabling `SyncForDuration` will bring down the node.

**Severity: Critical**

---

### 2. `notify_events().unwrap()` panic — `consensus_mempool_handler.rs:85`

```rust
event_subscription_service.notify_events(block_number, events).unwrap()
```

If event notification fails for any reason (subscriber dropped, channel full), this `unwrap()` crashes the node mid-commit-handling. The consensus commit notification has already been partially processed at this point, creating an inconsistent state.

**Severity: Critical**

---

### 3. Timestamp unit mismatch — `consensus_mempool_handler.rs:75`

The parameter is named `block_timestamp_usecs` but the value passed is `SystemTime::now().as_secs()` (seconds, not microseconds). Any downstream consumer interpreting this as microseconds will compute timestamps ~1,000,000x too small.

The code has a TODO acknowledging this, but it remains unfixed. If any mempool eviction logic, transaction ordering, or metric computation relies on this timestamp being in microseconds, it is silently wrong.

**Severity: Critical**

---

## Warning

### 4. `record_committed` full-scan cost — `txn_metrics/src/lib.rs`

`record_committed` performs **three full scans** across `DashMap` instances (`txn_hash_to_key.retain()`, `txn_batch_id` iter+remove, `txn_block_id` iter+remove) on every single committed transaction. At high throughput (thousands of TPS), this is O(n) per commit across maps that can hold up to 100,000 entries.

This is called in the commit hot path. Under load, the cumulative cost of iterating 100k-entry maps per committed transaction could become a bottleneck.

**Severity: Warning**

---

### 5. Race in `cleanup_old_entries` — `txn_metrics/src/lib.rs`

During cleanup, the `txn_batch_id.retain()` callback calls `self.txn_initial_add_time.contains_key(key)`. A concurrent `record_added` inserting a new transaction into `txn_initial_add_time` can race with this check:

1. `cleanup_old_entries` checks `contains_key(key)` → false (not yet inserted)
2. `record_added` inserts the key into `txn_initial_add_time`
3. `record_batch` inserts the key into `txn_batch_id`
4. But `cleanup_old_entries` already decided to remove the batch entry

Result: a just-added transaction's batch association is silently dropped. Metrics for that transaction's batch→commit lifecycle will be incomplete. Not data-corrupting, but produces silent metric gaps under load.

**Severity: Warning**

---

### 6. Histogram `register_histogram!().unwrap()` — `txn_metrics/src/lib.rs`

All 10 histograms use `OnceLock` + `register_histogram!(...).unwrap()`. If any other component in the process registers a metric with the same name before this code runs, the `.unwrap()` panics. This is a fragile initialization pattern — especially in a modular codebase where metric names might collide across crate boundaries.

**Severity: Warning**

---

### 7. `InfallibleRwLock` poison-panic propagation — `proposer-reth-map/src/lib.rs`

`InfallibleRwLock` intentionally panics on lock poisoning. If any thread panics while holding a write lock on `PROPOSER_RETH_ADDRESS_MAP`, all subsequent read or write attempts will also panic, cascading the failure across all threads that query validator-to-reth-address mappings.

This is a deliberate design choice (fail-fast on poison), but it means a single panic during `update_proposer_reth_index_map` will make the entire address-lookup subsystem permanently unavailable for the lifetime of the process.

**Severity: Warning**

---

### 8. `println!` in production code — `config_storage.rs`

```rust
println!("fetch_config_bytes: config_name: {:?}, block_number: {:?}", config_name, block_number);
```

`ConfigStorageWrapper::fetch_config_bytes` writes to stdout via `println!` on every call. This is redundant with the `info!` log on the next line. In a production node, `println!` bypasses structured logging, pollutes stdout, and cannot be filtered or leveled. Under high query rates this adds measurable I/O overhead.

**Severity: Warning**

---

### 9. Silently discarded error — `consensus_mempool_handler.rs`

```rust
let _ = self.consensus_notification_listener
    .respond_to_sync_target_notification(notification, Ok(()))
    .map_err(|e| warn!("Failed to respond: {}", e));
```

The `let _` binding discards the `Result`. While the `map_err` does log, the overall error is swallowed. If the consensus layer depends on receiving this acknowledgment, a dropped response could cause consensus to stall or retry indefinitely.

**Severity: Warning**

---

### 10. No cross-call atomicity on proposer map reads — `proposer-reth-map/src/lib.rs`

Two sequential calls to `get_reth_address_by_index` can return values from different epochs if `update_proposer_reth_index_map` runs between them. Any caller that needs a consistent snapshot of multiple validator addresses within the same epoch must be aware of this — but the API surface provides no "snapshot" or "read-consistent" interface.

**Severity: Warning**

---

## Info

### 11. Build metadata exposure — `build-info/src/lib.rs`

`get_build_information()` exposes 12 fields including git commit hash, branch name, build timestamp, OS, Rust version, cargo version, and profile name. While useful for debugging, if this information is surfaced via an API endpoint or included in error responses, it provides an attacker with:
- Exact binary version → known CVE targeting
- Build environment details → supply chain attack surface mapping
- Branch name → internal workflow visibility

Verify that the `BTreeMap` returned by `build_information!()` is not exposed to unauthenticated callers.

**Severity: Info**

---

### 12. `build_profile_name` derived from path parsing — `build-info/src/lib.rs`

The profile name is extracted as the 3rd-from-last segment of `OUT_DIR`. This is a fragile heuristic that depends on Cargo's internal directory layout. A change in Cargo's build directory structure (or use of `--target-dir` overrides) could produce an incorrect or empty profile name.

**Severity: Info**

---

### 13. `gravity-sdk/src/lib.rs` — opaque re-export facade

The SDK surface is three `pub use` lines with no documentation, no version gating, and no feature flags. Any breaking change in `api`, `block_buffer_manager`, or `gaptos` is automatically a breaking change in `gravity-sdk` with no isolation boundary. This is a semver hazard if `gravity-sdk` is published as a standalone crate.

**Severity: Info**

---

### 14. `OptQuorumStore` no-op in `record_block` — `txn_metrics/src/lib.rs`

The `OptQuorumStore` payload variant is handled with an empty match arm in `record_block`. If this variant carries transactions in production, their lifecycle metrics will be silently untracked. This should either be explicitly documented as intentional or wired up.

**Severity: Info**

---

## Summary

| Severity | Count | Key Themes |
|----------|-------|------------|
| **Critical** | 3 | Runtime panics via `todo!()`/`unwrap()` in consensus pipeline; timestamp unit mismatch |
| **Warning** | 7 | Metric scan cost; race conditions in cleanup; lock poisoning cascade; stdout logging; silent error drops |
| **Info** | 4 | Build info exposure; fragile path parsing; opaque API surface; untracked payload variant |

The most urgent items are the three **Critical** findings in `consensus_mempool_handler.rs` — the `todo!()` panics and `unwrap()` in the consensus commit pipeline represent single points of failure that will crash the node under specific (but reachable) conditions. The timestamp unit mismatch is a silent correctness bug propagating wrong values downstream.

### ⚔️ Intention Attacker

# Intention Attack Report: Shared Library Crates — Fatal Logic Vulnerabilities

---

## CRITICAL Severity

### 1. Potential Deadlock in `txn_metrics` Due to Lock Ordering Inversion
**File:** `crates/txn_metrics/src/lib.rs`

`cleanup_old_entries` and `record_committed` acquire DashMap shard locks in **opposite order**:

| Thread A (`record_committed`) | Thread B (`cleanup_old_entries`) |
|---|---|
| Write lock `txn_initial_add_time` shard M (via `.remove()`) | Write lock `txn_batch_id` shard N (via `.retain()`) |
| Then write lock `txn_batch_id` shard N (via `.iter_mut()`) | Then read lock `txn_initial_add_time` shard M (via `.contains_key()` inside retain closure) |

This is a **textbook ABBA deadlock**. If Thread A holds shard M of `txn_initial_add_time` and waits for shard N of `txn_batch_id`, while Thread B holds shard N of `txn_batch_id` and waits for shard M of `txn_initial_add_time`, both threads are permanently blocked. The same inversion exists for the `txn_block_id` ↔ `txn_initial_add_time` pair. Under high transaction throughput, this is a livelock/deadlock waiting to happen.

---

### 2. `todo!()` Panics in Production Consensus Pipeline
**File:** `crates/api/src/consensus_mempool_handler.rs`

Two `todo!()` macros exist in the consensus-to-mempool notification handler — the **critical path** for block commitment:

- **Line 41:** If `mempool_notification_sender.notify_new_commit()` returns an error, the handler calls `todo!()` → **unconditional panic**. Any transient mempool communication failure (backpressure, channel closed, timeout) crashes the node.
- **Line 112:** If a `SyncForDuration` consensus notification is received, the handler calls `todo!()` → **unconditional panic**. If this variant is ever emitted by the consensus layer, the node crashes immediately.

These are in an async task loop with no restart/supervisor logic — a panic here kills the consensus notification pipeline permanently for the running node.

---

### 3. Cascading `unwrap()` Panics on Event Notification Failure
**File:** `crates/api/src/consensus_mempool_handler.rs`

- **Line 85:** `event_subscription_service.notify_events(block_number, events).unwrap()` — panics if event notification fails.
- **Line 104:** `notify_initial_configs(...).unwrap()` — panics if initial config notification fails during `SyncToTarget`.

Both are on the consensus commit path. A single failed event subscriber (e.g., a slow downstream consumer or a dropped channel) will crash the entire node via panic propagation. There is no graceful degradation.

---

## HIGH Severity

### 4. Timestamp Unit Mismatch — Seconds Passed as Microseconds
**File:** `crates/api/src/consensus_mempool_handler.rs`, line 79

```rust
SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
//                                                   ^^^^^^^^^ seconds
```

This value is passed to `notify_new_commit` as the `block_timestamp_usecs` parameter, which expects **microseconds**. The timestamp is off by a factor of **1,000,000×**. Any downstream logic that uses this timestamp for:
- Transaction expiration (TTL checks)
- Replay protection windows
- Time-based ordering or deduplication

...will compute wildly incorrect durations. A transaction that should expire in 30 seconds would appear to expire in ~34 days. A TODO comment acknowledges the bug but it remains unfixed.

### 5. Race Condition in `txn_metrics`: Just-Inserted Transaction Can Lose Metrics
**File:** `crates/txn_metrics/src/lib.rs`

In `record_added`, the insert into `txn_hash_to_key` (step B) happens before the insert into `txn_initial_add_time` (step C). A concurrent `cleanup_old_entries` running between steps B and C will:
1. See the key in `txn_hash_to_key`
2. Check `txn_initial_add_time.contains_key()` → returns `false` (not yet inserted)
3. Remove the key from `txn_hash_to_key`

The key is now orphaned: it exists in `txn_initial_add_time` but has no mapping in `txn_hash_to_key`. All subsequent batch/block/proof metrics for this transaction are silently lost.

Similarly, `record_committed` does a non-atomic two-pass cleanup on `txn_batch_id` (first `iter_mut` to remove, then `retain` to drop empties). Between the passes, a concurrent `record_batch` can re-insert the committed key, leaving stale data permanently in the map.

### 6. Silent Error Swallowing on `SyncToTarget` Response
**File:** `crates/api/src/consensus_mempool_handler.rs`, lines 105–108

```rust
let _ = self.consensus_notification_listener
    .respond_to_sync_target_notification(sync_notification, Ok(()))
    .map_err(|e| anyhow::anyhow!(e));
```

The error is converted via `map_err` and then **immediately discarded** via `let _`. If the response fails to reach the consensus layer, consensus believes the sync target was never acknowledged. This can cause consensus to stall, retry indefinitely, or make incorrect assumptions about synchronization state — with zero log output to diagnose the failure.

### 7. `InfallibleRwLock` Poison Propagation Is Fatal and Unrecoverable
**File:** `crates/proposer-reth-map/src/lib.rs`

The `PROPOSER_RETH_ADDRESS_MAP` uses `InfallibleRwLock`, which panics on lock poisoning rather than returning a `Result`. If any thread panics while holding the write lock (e.g., during `update_proposer_reth_index_map` if the `ValidatorSet` iteration hits a panic), **every subsequent call** to `get_reth_address_by_index` from any thread will also panic. Since this map is queried on the block proposal path, a single poison event permanently disables block production for the node with no recovery path short of a restart.

---

## MEDIUM Severity

### 8. Unbounded Capacity Growth in `txn_metrics`
**File:** `crates/txn_metrics/src/lib.rs`

The capacity limits (`MAX_TXN_INITIAL_ADD_TIME_CAPACITY = 100,000`, etc.) are **advisory, not enforced**:
- The check-then-act pattern is non-atomic — concurrent `record_added` calls all pass the `>=` check simultaneously and all insert.
- `cleanup_old_entries` only removes entries older than 60s. If all 100,000 entries are fresh, zero are evicted and the map grows unbounded.
- `txn_block_id` has **no capacity check at insert time** in `record_block` — only the indirect cleanup from `record_added`/`record_batch` protects it.

Under sustained high throughput, memory consumption grows without bound.

### 9. Build Metadata Spoofing via Runtime Environment Variables
**File:** `crates/build-info/src/lib.rs`

`GIT_SHA`, `GIT_BRANCH`, `GIT_TAG`, and `BUILD_DATE` are runtime overrides (checked via `std::env::var` at call time, not compile time). Any process that can set environment variables — including a compromised container entrypoint, sidecar, or co-tenant — can inject arbitrary values into version reporting. If downstream systems use `build_commit_hash` for audit trails, deployment verification, or trust decisions, that trust is trivially undermined with no log trace.

### 10. TOCTOU in Proposer-Reth Address Lookups Across Epoch Boundaries
**File:** `crates/proposer-reth-map/src/lib.rs`

Two sequential calls to `get_reth_address_by_index` can return values from different epochs because each call independently acquires and releases the read lock. Any block production code that looks up multiple validator addresses in sequence (e.g., "get proposer A's address, then get proposer B's address") may operate on an inconsistent view of the validator set. The API provides no epoch token or snapshot mechanism to prevent this.

### 11. Debug `println!` in Production Config Storage Path
**File:** `crates/api/src/config_storage.rs`, line 23

An unconditional `println!` logs every `fetch_config_bytes` call to stdout, duplicating the structured `info!()` log on line 25. While the values printed (enum variant name + block number) are not currently secrets, stdout output in production is often captured into unstructured logs with different retention/access policies than the structured logging system. The code comment on line 134 of `consensus_api.rs` ("It seems stupid, refactor when debugging finished") confirms this is an unfinished debug artifact.

---

## Summary

| # | Vulnerability | Severity | Impact |
|---|---|---|---|
| 1 | DashMap lock ordering deadlock in `txn_metrics` | **Critical** | Node freeze under load |
| 2 | `todo!()` panics on consensus commit path | **Critical** | Node crash on any mempool error or `SyncForDuration` |
| 3 | `unwrap()` panics on event notification failure | **Critical** | Node crash from single failed subscriber |
| 4 | Timestamp off by 10⁶× (seconds vs microseconds) | **High** | Broken TTL, expiration, and time-based logic |
| 5 | Non-atomic multi-map operations lose metrics | **High** | Silent metric data loss, orphaned map entries |
| 6 | `SyncToTarget` response error silently discarded | **High** | Consensus stall with no diagnostic output |
| 7 | `InfallibleRwLock` poison cascades fatally | **High** | Permanent block production failure |
| 8 | Unbounded capacity growth in metrics maps | **Medium** | Memory exhaustion under sustained load |
| 9 | Runtime env var spoofing of build metadata | **Medium** | Audit trail / version trust undermined |
| 10 | TOCTOU on epoch-boundary validator lookups | **Medium** | Inconsistent validator set reads |
| 11 | Debug `println!` in production config path | **Medium** | Unstructured information leakage |

---

## [concurrency_and_async_safety] Cross-cutting review of concurrency patterns across the entire codebase: (1) mix

### 🎯 Intention

Cross-cutting review of concurrency patterns across the entire codebase: (1) mixing std::sync::Mutex with tokio async code (mempool.rs cached_best, notifier.rs last_alert_times) risking blocking the async runtime; (2) DashMap usage for txn_cache without size bounds; (3) OnceLock/OnceLock globals (GLOBAL_RELAYER, GLOBAL_CONFIG_STORAGE, GLOBAL_CRYPTO_TXN_HASHER) for initialization ordering; (4) broadcast channel usage for shutdown coordination; (5) AtomicU64 ordering consistency (SeqCst vs relaxed) for current_epoch; (6) tokio::spawn with unwrap() in spawned tasks causing silent panics; (7) potential for priority inversion in the block pipeline.

### 🔍 Discovered Implementation

# Cross-Cutting Concurrency Patterns Audit — gravity-sdk

---

## 1. `std::sync::Mutex` in Async Contexts

### Finding: `cached_best` in `mempool.rs`

**Location:** `bin/gravity_node/src/mempool.rs:67`

```rust
pub struct Mempool {
    pool: RethTransactionPool,
    txn_cache: TxnCache,
    cached_best: Arc<std::sync::Mutex<CachedBest>>,  // blocking mutex
    runtime: tokio::runtime::Runtime,                  // tokio runtime on same struct
}
```

**How it's used:** The lock is acquired synchronously at line 126 inside `best_txns()`:

```rust
let mut best_txns = self.cached_best.lock().unwrap();
```

The lock guard is held while iterating the entire transaction pool (lines 137–167), inserting into `txn_cache` via DashMap, and collecting results. This is a **synchronous, potentially long-held lock** — the duration scales with pool size and `limit`.

**Risk assessment:** The `Mempool` struct owns its own dedicated `tokio::runtime::Runtime` (line 68, created at line 78). The `best_txns()` method is called via the `TxPool` trait, which is a synchronous trait (no `async fn`). The lock is never held across a `.await` point because the method itself is not async. However, if `best_txns()` is ever called from a tokio worker thread (e.g., from consensus pulling transactions), the blocking `Mutex::lock()` will block that worker thread for the duration of the iteration. With the Mempool owning its own separate runtime, the primary risk is blocking the *caller's* runtime thread, not the Mempool's own runtime.

**No other first-party `std::sync::Mutex` usage was found.** The user-mentioned `notifier.rs last_alert_times` was not found in the codebase — it may have been refactored or removed.

---

## 2. DashMap `txn_cache` — Unbounded Growth

**Location:** `bin/gravity_node/src/reth_cli.rs:71`, `bin/gravity_node/src/mempool.rs:76`

```rust
pub(crate) type TxnCache = Arc<DashMap<[u8; 32], Arc<ValidPoolTransaction<EthPooledTransaction>>>>;
```

| Operation | Location | Trigger |
|-----------|----------|---------|
| `DashMap::new()` | `mempool.rs:76` | Construction — no initial capacity |
| `.insert(tx_hash, pool_txn)` | `mempool.rs:163` | Every `best_txns()` call, per yielded transaction |
| `.remove(&key)` | `reth_cli.rs:198` | Block execution — only for transactions included in a block |

**No size limit. No TTL. No eviction.** Transactions that are inserted into the cache during `best_txns()` but never included in an executed block (dropped, replaced, expired from the reth pool) remain in the DashMap indefinitely. This is an unbounded memory leak proportional to the volume of transactions that are proposed but never committed.

**Contrast with `txn_metrics`:** The `crates/txn_metrics/src/lib.rs` DashMaps have explicit capacity constants (`MAX_TXN_INITIAL_ADD_TIME_CAPACITY = 100_000`) and age-based cleanup. The `TxnCache` has neither.

**Deadlock note:** `batch_store.rs:204-206` documents an explicit lock-ordering constraint for its DashMaps: `db_cache` must always be acquired before `peer_quota`. No such ordering documentation exists for `TxnCache`, but since it's the only DashMap accessed in that code path, no deadlock is possible there.

---

## 3. OnceLock Globals — Initialization Ordering

### Initialization Dependency Chain

```
fn main()
  │
  ├─► run_reth(...)                              [reth fully initializes]
  │       └─► datadir_rx received
  │
  ├─► GLOBAL_RELAYER.set(Arc<RelayerWrapper>)    ← main.rs:278
  │
  └─► ConsensusEngine::init(...)
          └─► GLOBAL_CONFIG_STORAGE.set(config)  ← consensus_api.rs:136
```

| Global | Set Location | Read Location | Guard |
|--------|-------------|---------------|-------|
| `GLOBAL_RELAYER` | `main.rs:278` | External (`gaptos` crate) | `panic!` on double-set |
| `GLOBAL_CONFIG_STORAGE` | `consensus_api.rs:136` | `relayer.rs:109` via `.get().unwrap()` | `panic!` on double-set |
| `CACHE_TTL` | `mempool.rs:29` | Same file, `cache_ttl()` | `OnceLock::get_or_init` (safe) |

**Critical implicit ordering:** `GLOBAL_CONFIG_STORAGE.get().unwrap()` in `relayer.rs:109` will **panic** if called before `ConsensusEngine::init()` completes. The safety relies entirely on call-site ordering — there is no compile-time or runtime guard beyond the `.unwrap()` that would produce a meaningful error. If the relayer's async polling starts before consensus init finishes (e.g., due to a future refactor changing task spawn order), the node panics.

**`GLOBAL_CRYPTO_TXN_HASHER`** was not found in this repository — it exists only in the external `gaptos` crate.

---

## 4. Broadcast Channel Shutdown Coordination

**Location:** `bin/gravity_node/src/main.rs:231`

```rust
let (shutdown_tx, _shutdown_rx) = broadcast::channel(1);
```

**Architecture:**

| Component | Role |
|-----------|------|
| `broadcast::channel(1)` | Capacity-1 shutdown bus |
| Signal handler | Dedicated `std::thread::spawn` with own single-threaded tokio runtime |
| `tokio::select!` | Races `ctrl_c()` against `SIGTERM` |
| Subscribers | Call `shutdown_tx.subscribe()` (reth at line 254, RethCli at line 261) |

The initial `_shutdown_rx` is immediately dropped (underscore-prefixed); all consumers subscribe independently. This is correct usage — the sender is the subscription source.

**Capacity concern:** The channel capacity is 1. If `shutdown_tx.send(())` fires and a subscriber hasn't polled its receiver yet, the message is buffered. With capacity 1, a second send before the first is consumed would hit `RecvError::Lagged`. This is acceptable for a one-shot shutdown signal (only one signal is ever sent).

**The signal handler thread** creates its own tokio runtime (`new_current_thread`) separate from the main runtime. This is intentional — the signal handler must work even if the main runtime is stalled.

**No other broadcast channels are used for shutdown.** The `block_buffer_manager.rs` and `pipeline_builder.rs` broadcast channels are used for block-state notifications and pipeline proof coordination respectively, not shutdown.

---

## 5. AtomicU64 Ordering Consistency

### Production Variables — All Consistent (`SeqCst`)

| Variable | File | All Operations |
|----------|------|---------------|
| `current_epoch` | `reth_cli.rs` | `SeqCst` (store, load, swap) |
| `epoch` / `epoch_start_block_number` | `mock_consensus/mock.rs` | `SeqCst` (store, load) |
| `ongoing_tasks` | `pipeline/buffer_manager.rs` | `SeqCst` (load) |
| `counter` (CountedRequest) | `pipeline/pipeline_phase.rs` | `SeqCst` (fetch_add, fetch_sub) |
| `buffer_state` (AtomicU8) | `block_buffer_manager.rs` | `SeqCst` (store, load) |
| `reset_flag` (AtomicBool) | `buffer_manager.rs` | `SeqCst` (store, load) |

### One Inconsistency: `last_certified_time`

**Location:** `aptos-core/consensus/src/quorum_store/batch_store.rs`

```rust
// Line 327 — WRITE uses SeqCst
self.last_certified_time.fetch_max(certified_time, Ordering::SeqCst);

// Line 344 — READ uses Relaxed
self.last_certified_time.load(Ordering::Relaxed)
```

The write path (`update_certified_timestamp`) uses full sequential consistency, but the read path (`last_certified_time()` getter) uses `Relaxed`, meaning readers may observe stale values. The `last_certified_time()` getter is called from `get_batch_from_db_or_mem` to check if a batch has expired. A stale read means a batch could be temporarily served after it should have been considered expired — a minor consistency window, not a safety violation, since the certified time is monotonically increasing and the worst case is briefly serving a soon-to-expire batch.

---

## 6. `tokio::spawn` and Silent Panic Patterns

### Global Panic Infrastructure: **None**

| Mechanism | Present? |
|-----------|----------|
| `catch_unwind` | Not found |
| `panic::set_hook` | Not found |
| `JoinSet` (structured concurrency) | Not found |

### Spawn Pattern: Overwhelmingly Fire-and-Forget

**28 files** contain `tokio::spawn`. Only **6 files** store the `JoinHandle`, and most of those are in tests or DAG bootstrap code. The remaining ~22 spawn sites drop the handle, meaning:

1. If the spawned task panics, tokio prints the panic but **no other task is notified**
2. The parent task continues running, unaware that a critical subtask has died
3. There is no restart/retry mechanism

### High-Risk Spawn Sites

| File | What's Spawned | Risk |
|------|---------------|------|
| `main.rs:273` | `mock.run()` — entire mock consensus loop | Panic kills consensus silently |
| `mempool.rs:223` | `pool.add_external_transaction()` — fire-and-forget on Mempool's own runtime | Panics in pool logic are swallowed |
| `buffer_manager.rs` | Pipeline phase tasks | Panic in execution/signing/persisting silently halts the pipeline |
| `epoch_manager.rs` | Epoch management tasks | Panic breaks epoch transitions |
| `dag/bootstrap.rs` | DAG bootstrap tasks (JoinHandle stored) | Better — handles are tracked |

The `mempool.rs:223` spawn is notable: it uses `self.runtime.spawn()` on the Mempool's own dedicated runtime, not the main tokio runtime. A panic here would print to stderr but the `add_external_txn()` caller always returns `true` regardless.

---

## 7. Priority Inversion in the Block Pipeline

### Architecture: Lock-Free by Design

The pipeline (`aptos-core/consensus/src/pipeline/`) uses **no mutexes or RwLocks** in the hot path. All stages communicate via unbounded async channels (`futures::channel::mpsc::unbounded`), and the `BufferManager` runs as a single-threaded `tokio::select!` loop that owns its `Buffer<BufferItem>` exclusively.

The only synchronization primitives are `Arc<AtomicU64>` (`ongoing_tasks`) and `Arc<AtomicBool>` (`reset_flag`), both lock-free.

### One Priority Inversion Vector: Reset Drain

**Location:** `buffer_manager.rs:434-447`

```rust
async fn reset(&mut self) {
    self.buffer = Buffer::new();
    // ...
    get_block_buffer_manager().release_inflight_blocks().await;
    while self.ongoing_tasks.load(Ordering::SeqCst) > 0 {
        tokio::time::sleep(Duration::from_millis(10)).await;  // busy-poll
    }
}
```

During a reset (epoch change), the `BufferManager` **busy-polls** with 10ms sleeps until all in-flight `CountedRequest` guards are dropped. While this loop runs:
- The entire `BufferManager` event loop is stalled
- No new ordered blocks are processed
- No execution responses are handled
- No commit votes are aggregated

This is bounded only by how long the in-flight pipeline tasks take to complete. If execution is slow (e.g., a large block), the reset drain holds the pipeline hostage.

### Backpressure Gate

```rust
fn need_backpressure(&self) -> bool {
    const MAX_BACKLOG: Round = 20;
    self.highest_committed_round + MAX_BACKLOG < self.latest_round
}
```

When >20 rounds are in flight, `block_rx` is suppressed in `tokio::select!`. This prevents unbounded buffering but also means consensus block production can be stalled by slow execution — a form of head-of-line blocking, though intentional by design.

---

## Summary Table

| Concern | Severity | Status |
|---------|----------|--------|
| `std::sync::Mutex` blocking async runtime | **Medium** | `cached_best` lock held during full pool iteration; no `.await` across lock, but blocks caller's thread |
| `TxnCache` DashMap unbounded growth | **High** | No size limit, no TTL, no eviction for unconfirmed transactions |
| OnceLock initialization ordering | **Medium** | `GLOBAL_CONFIG_STORAGE.get().unwrap()` relies on implicit call ordering; panic if violated |
| Broadcast shutdown coordination | **Low** | Correct usage; capacity-1 is appropriate for one-shot signal |
| AtomicU64 ordering (current_epoch) | **Low** | All production atomics use consistent `SeqCst`; one `Relaxed` read on `last_certified_time` is a minor staleness window |
| `tokio::spawn` silent panics | **High** | ~22 of 28 spawn sites are fire-and-forget with zero panic recovery infrastructure |
| Block pipeline priority inversion | **Medium** | Reset drain busy-polls entire `BufferManager` loop; bounded by slowest in-flight task |

### 🛠️ Ground Review

# Ground Review: Cross-Cutting Concurrency Patterns — gravity-sdk

**Reviewer:** Blue Team (Code Hygiene & Engineering Safety)
**Scope:** Concurrency primitives, resource lifecycle, silent failure modes

---

## Critical

### 1. `TxnCache` DashMap — Unbounded Memory Leak
**Files:** `bin/gravity_node/src/mempool.rs:163`, `bin/gravity_node/src/reth_cli.rs:71,198`

The `TxnCache` (`Arc<DashMap<[u8;32], Arc<ValidPoolTransaction>>>`) has **no size limit, no TTL, and no eviction**. Entries are inserted on every `best_txns()` call but only removed when a transaction appears in a committed block. Transactions that are proposed but never committed, evicted from the reth pool, or simply not selected by consensus **leak indefinitely**. Contrast with `crates/txn_metrics/src/lib.rs` which defines explicit capacity constants and age-based cleanup — `TxnCache` has neither.

**Impact:** OOM under sustained load with transaction churn. Memory grows monotonically with unconfirmed transaction volume.

---

### 2. No Panic Recovery Infrastructure — Globally
**Files:** Entire codebase (28 `tokio::spawn` sites across 28 files)

There is **zero** `panic::set_hook`, **zero** `catch_unwind`, and **zero** `JoinSet` usage in the entire codebase. Of ~28 `tokio::spawn` call sites, **~22 are fire-and-forget** (handle dropped immediately). A panic in any of these tasks is silently swallowed — the node continues running in a degraded state with no indication of subsystem death.

High-risk fire-and-forget spawns:

| Spawn Site | What Dies Silently |
|---|---|
| `main.rs:273` | Mock consensus loop — node produces no blocks |
| `crates/api/src/https/mod.rs:205` | HTTPS API server — node becomes unreachable |
| `crates/block-buffer-manager/src/block_buffer_manager.rs:243` | Block buffer management — pipeline stalls |
| `pipeline/execution_client.rs:245,289` | Six pipeline phase runners — execution halts |
| `pipeline/buffer_manager.rs:274,841` | Buffer manager — commit pipeline breaks |
| `epoch_manager.rs` (6 sites) | Epoch transitions — consensus breaks on epoch boundary |

**Impact:** Any single panic in a core subsystem renders the node non-functional while it continues to appear "running" to monitoring.

---

## Warning

### 3. `std::sync::Mutex` Held Over Full Pool Iteration
**File:** `bin/gravity_node/src/mempool.rs:126-177`

`cached_best.lock().unwrap()` acquires a **blocking** `std::sync::Mutex` (not `tokio::Mutex`) and holds the guard for the entire duration of `best_txns()` — iterator consumption, hash computation, `txn_cache.insert()` calls, and nonce tracking. Duration scales with pool size and `limit`. While no `.await` crosses the lock (the method is synchronous), any caller on a tokio worker thread will **block that thread** for the full iteration.

Additionally, `.unwrap()` on the lock means a **mutex poison from a prior panic cascades as a new panic** rather than being handled.

---

### 4. OnceLock Initialization — Implicit Ordering with Panic on Violation
**Files:** Multiple

Several `OnceLock` globals rely on implicit call-site ordering with `.get().unwrap()` as the only "guard":

| Global | Set Location | Panic If Read Before Set |
|---|---|---|
| `GLOBAL_BLOCK_BUFFER_MANAGER` | `crates/block-buffer-manager/src/lib.rs:6` | Any block arrival before init |
| `GLOBAL_CONFIG_STORAGE` | `consensus_api.rs:136` | Relayer async polling starts before consensus init |
| `ORDERED_INTERVAL_MS` / `MAX_EXECUTED_GAP` | `mock_consensus/mock.rs:25,42` | Mock loop runs before `MockConsensus::new()` |
| 12 txn_metrics statics incl. `TXN_LIFE_ENABLED` | `crates/txn_metrics/src/lib.rs:31-184` | Any transaction processed before metrics registration |

There are no compile-time guarantees, no `is_initialized()` checks, and no fallback paths. A future refactor that reorders task spawning will produce a panic with no meaningful error message.

---

### 5. `add_external_txn` — Unbounded Task Spawning
**File:** `bin/gravity_node/src/mempool.rs:220-233`

`self.runtime.spawn(async move { pool.add_external_transaction(pool_txn).await })` is fire-and-forget with **no concurrency cap and no backpressure**. Under high transaction ingest rates, an unbounded number of tasks accumulate on the dedicated runtime. The method always returns `true` regardless of outcome.

---

### 6. Spin-Based Locks on Hot Path
**File:** `aptos-core/consensus/src/pipeline/execution_client.rs:350,360,394`

`aptos_infallible::RwLock` (spin-wait, not async) is read-locked on **every block execution and commit message**. During a concurrent epoch reset (write lock at line 259/455), all readers spin rather than yield. Similarly, `commit_reliable_broadcast.rs:89` acquires a spin `Mutex` on every commit acknowledgment.

---

### 7. Reset Drain Busy-Poll Stalls Pipeline
**File:** `aptos-core/consensus/src/pipeline/buffer_manager.rs:444-446`

```rust
while self.ongoing_tasks.load(Ordering::SeqCst) > 0 {
    tokio::time::sleep(Duration::from_millis(10)).await;
}
```

During epoch reset, the `BufferManager` event loop **stops processing all events** (ordered blocks, execution responses, commit votes) while busy-polling at 10ms intervals until in-flight tasks complete. Duration is bounded only by the slowest in-flight pipeline task. No exponential backoff, no timeout, no condition variable.

---

## Info

### 8. `last_certified_time` Atomic Ordering Mismatch
**File:** `aptos-core/consensus/src/quorum_store/batch_store.rs:327,344`

Write uses `Ordering::SeqCst` (`fetch_max` at line 327), read uses `Ordering::Relaxed` (`load` at line 344). A `Relaxed` load does not synchronize-with the `SeqCst` store, so readers may observe stale timestamps. Practical impact is minor — worst case is briefly serving a soon-to-expire batch — but the inconsistency suggests the intent was stronger ordering.

---

### 9. Signal Handler Thread — Unguarded
**File:** `bin/gravity_node/src/main.rs:235-250`

The signal handler runs in a bare `std::thread::spawn` with no `JoinHandle` stored and no panic guard. `Runtime::new().unwrap()` and `signal(...).unwrap()` inside this thread will panic silently if they fail, leaving the node unable to respond to SIGTERM/SIGINT.

---

### 10. DashMap Lock Ordering — Documented but Not Enforced
**File:** `aptos-core/consensus/src/quorum_store/batch_store.rs:204-206`

The `db_cache → peer_quota` ordering constraint is correctly documented in a comment. However, this is purely conventional — there is no type-system or runtime enforcement. No similar ordering documentation exists for `TxnCache`, though since it's the sole DashMap in its code path, no deadlock is currently possible there.

---

## Summary

| Severity | Count | Key Themes |
|---|---|---|
| **Critical** | 2 | Unbounded memory growth (`TxnCache`); zero panic recovery across ~22 fire-and-forget spawns |
| **Warning** | 5 | Blocking mutex in async context; OnceLock init races; unbounded task spawning; spin locks on hot path; reset busy-poll |
| **Info** | 3 | Atomic ordering mismatch; unguarded signal thread; comment-only lock ordering |

The two critical findings — `TxnCache` unbounded growth and the complete absence of panic handling infrastructure — represent systemic risks that grow worse under load and over time. The warning-level items are individually manageable but collectively indicate a pattern of missing defensive boundaries (backpressure, initialization guards, poison handling) across the concurrency surface.

### ⚔️ Intention Attacker

# Intention Attack Report — gravity-sdk Security Audit

## Attack Surface: Cross-Cutting Concurrency & Business Logic

---

## CRITICAL Severity

### C1: Unauthenticated Remote Fail-Point Injection (`/set_failpoint`)

**File:** `crates/api/src/https/mod.rs:107-116`

The `/set_failpoint` and `/mem_prof` endpoints are bound to `http_routes` with **zero authentication, zero TLS requirement, and zero IP allowlisting**. Any network-reachable attacker can POST arbitrary fail-points into a running validator node.

**Business Logic Flaw:** Fail-point injection is a *complete remote control primitive*. An attacker can:
- Halt consensus by injecting a fail-point on the block proposal path
- Corrupt execution state by triggering panics at chosen code locations
- Cause targeted denial-of-service at will

The `https_routes` group gets the `ensure_https` middleware, but these two endpoints were explicitly placed on the unprotected `http_routes` group. This is not a missing feature — it's an architectural misplacement that turns a debug tool into a weaponizable backdoor.

**Verdict:** Remote code-control without authentication. This is the single highest-priority fix.

---

### C2: `TxnCache` Unbounded Memory Growth — Exploitable OOM

**File:** `bin/gravity_node/src/mempool.rs:163` (insert), `bin/gravity_node/src/reth_cli.rs:198` (remove)

**The invariant violation:** Entries are inserted on every `best_txns()` call (per transaction yielded) but are *only* removed when a transaction is included in an executed block. There is no size limit, no TTL, no eviction, no background cleanup.

**Attack scenario:**
1. Attacker submits thousands of valid-gas-price transactions from many sender addresses.
2. Consensus calls `best_txns()` repeatedly during block proposals — each call inserts all candidate transactions into `TxnCache`.
3. If attacker's transactions are never selected for inclusion (e.g., they are outbid, or the attacker replaces them with new nonces in the reth pool), the reth pool internally evicts them, but **`TxnCache` never learns about the eviction**.
4. The DashMap grows monotonically. Each entry holds an `Arc<ValidPoolTransaction<EthPooledTransaction>>` — a full decoded transaction with signature and sender cached.
5. Node OOMs.

**Compounding factor:** `TxPool::remove_txns()` at line 243 removes transactions from the reth pool but **never calls `txn_cache.remove()`**. This is a second orphaning pathway for cache entries.

**Contrast:** The `txn_metrics` DashMaps in `crates/txn_metrics/src/lib.rs` have explicit `MAX_TXN_INITIAL_ADD_TIME_CAPACITY = 100_000` and age-based cleanup. The `TxnCache` has neither — the mitigation pattern was applied elsewhere but missed here.

---

### C3: Pipeline Reset Drain Has No Timeout — Permanent Node Freeze

**File:** `aptos-core/consensus/src/pipeline/buffer_manager.rs:444-446`

```rust
while self.ongoing_tasks.load(Ordering::SeqCst) > 0 {
    tokio::time::sleep(Duration::from_millis(10)).await;
}
```

During an epoch change, `reset()` busy-polls until all in-flight `CountedRequest` guards are dropped. **There is no timeout.** If any execution/signing/persisting task hangs, this loop spins forever.

**Business Logic Flaw — the chain of consequences:**
1. `BufferManager::start()` select loop is suspended (it `await`s `process_reset_request` inline at line 825).
2. No new ordered blocks are processed, no commit votes are aggregated.
3. The reset ack is never sent → epoch-change handler blocks indefinitely.
4. **The node is permanently frozen.** It cannot recover without a restart.
5. Blocks arriving during the drain (after the `try_next()` flush at line 441) are silently dropped with no retry mechanism.

**This converts any single hung pipeline task into a permanent liveness failure for the entire node.**

---

## HIGH Severity

### H1: Silent Consensus Death via Fire-and-Forget `tokio::spawn`

**~22 of 28 spawn sites** drop the `JoinHandle`. There is no `catch_unwind`, no `panic::set_hook`, no `JoinSet`, and no watchdog anywhere in the codebase.

**Most dangerous spawn sites:**

| Spawn Site | What Dies Silently |
|---|---|
| `main.rs:273` — `mock.run()` | Entire block production loop. Node runs but produces nothing. |
| `consensus_api.rs:334` — `consensus_mempool_handler.start()` | All consensus→mempool commit notifications. Mempool never GCs committed txns — memory grows, stale transactions are re-proposed. |
| `mempool.rs:223` — `pool.add_external_transaction()` | Transaction ingestion. Caller receives `true` (success) even when the spawned task panics. |

**Business Logic Flaw:** The node enters an **undetectable degraded state**. The tokio runtime stays alive, the process appears healthy, but critical subsystems are silently dead. There are no health checks, no restart mechanisms, and no metrics that surface the failure.

### H2: `add_external_txn` Returns `true` Regardless of Pool Outcome

**File:** `bin/gravity_node/src/mempool.rs:206-241`

The method spawns pool insertion as fire-and-forget and returns `true` immediately. The actual pool result (duplicate, underpriced, pool full, invalid) is logged and discarded.

**Business Logic Flaw — false success signal:**
- VFN transaction forwarding (added in commit `48135e64`) trusts this return value. A VFN that forwards a transaction and receives `true` believes submission succeeded, but the pool may have rejected it.
- No backpressure is ever communicated to callers. If the pool is full, callers keep submitting and keep receiving `true`.
- The `bool` return creates a **contract violation** where the documented semantics ("was the transaction added?") diverge from the actual semantics ("was the transaction decodable?").

---

## MEDIUM Severity

### M1: `start_commit_num` Cursor Advanced Before Persistence Confirmation

**File:** `bin/gravity_node/src/reth_cli.rs:470-496`

`start_commit_num = last_block.num + 1` is set **before** `wait_for_block_persistence` is called. If persistence fails after the cursor advances, the in-memory cursor has already skipped past blocks that were never persisted. On restart, those blocks are silently skipped — **committed state is lost without any record of the loss**.

### M2: Epoch Race in `start_commit_vote`

**File:** `bin/gravity_node/src/reth_cli.rs:432-436`

`current_epoch` is snapshot-loaded and passed to `set_compute_res`. The epoch can be updated concurrently by `start_execution` (line 341) in a separate task. A block's compute result could be attributed to the wrong epoch in the buffer manager.

### M3: `ensure_https` Middleware Checks URI Scheme, Not Transport

**File:** `crates/api/src/https/mod.rs:32-37`

The middleware checks `req.uri().scheme_str()` — this is the scheme the *client* puts in the request, not the actual transport layer. Behind a TLS-terminating proxy (standard in Kubernetes), plain HTTP requests with an `https` scheme pass this check. The middleware provides a false sense of transport security.

### M4: `cached_best` Blocking Mutex Held During Full Pool Iteration

**File:** `bin/gravity_node/src/mempool.rs:126-176`

A `std::sync::Mutex` (not `tokio::sync::Mutex`) is held while iterating the entire transaction pool, running filter closures, encoding transactions, and inserting into `txn_cache`. An attacker who floods the pool with thousands of transactions makes each `best_txns()` call block the consensus proposer's executor thread for tens of milliseconds, starving other async tasks.

### M5: Unbounded `get_all` Table Scan on Unauthenticated Endpoints

**File:** `crates/api/src/https/consensus.rs:117-136`

`/consensus/ledger_info/:epoch` and `/consensus/validator_count/:epoch` load the **entire** epoch-by-block-number table into memory on every request. These endpoints are unauthenticated with no rate limiting — a trivial DoS vector.

### M6: `GLOBAL_CONFIG_STORAGE.get().unwrap()` — Fragile Init Ordering

**File:** `bin/gravity_node/src/relayer.rs:110`

This `.unwrap()` panics with no diagnostic message if called before `ConsensusEngine::init()` completes. The safety relies entirely on implicit call-site ordering — no compile-time or runtime guard exists. Any refactor that changes task spawn order causes an immediate node panic.

---

## Consolidated Remediation Priority

| Priority | Finding | Fix |
|---|---|---|
| **P0** | C1: Unauthenticated `/set_failpoint` | Move to `https_routes`, add auth, or remove from non-debug builds |
| **P0** | C2: Unbounded `TxnCache` | Add capacity cap + eviction on reth pool removal events |
| **P0** | C3: Reset drain no timeout | Add bounded timeout with forced cancellation and error escalation |
| **P1** | H1: Fire-and-forget spawns | Store `JoinHandle`s, add panic hooks, implement critical-task watchdog |
| **P1** | H2: False `true` return | Await pool result before returning, or change return type to signal async |
| **P1** | M1: Cursor before persistence | Move `start_commit_num` advance to after persistence confirmation |
| **P2** | M2: Epoch race | Atomic read-and-use or pass epoch through the compute result channel |
| **P2** | M3: `ensure_https` bypass | Enforce TLS at bind/socket level, not URI scheme |
| **P2** | M4: Blocking mutex | Use `tokio::sync::Mutex` or `spawn_blocking` |
| **P2** | M5: Unbounded scan | Index-based lookup + rate limiting on endpoints |
| **P2** | M6: OnceLock ordering | Replace `.unwrap()` with `.get().expect("descriptive message")` or return `Result` |

---

## [error_handling_and_panic_safety] Cross-cutting audit of error handling patterns: (1) unwrap() and expect() calls 

### 🎯 Intention

Cross-cutting audit of error handling patterns: (1) unwrap() and expect() calls that could crash the node in production — especially in reth_coordinator::run(), reth_cli start_* loops, and block_buffer_manager state transitions; (2) assert_eq! in production paths (set_compute_res, get_executed_res, set_commit_blocks) that will panic rather than return errors; (3) panic!() calls in block state machine for unexpected states; (4) todo!() in submit_tx that will crash on any transaction submission; (5) error swallowing patterns where errors are logged but not propagated; (6) the overall resilience of the node to transient failures vs permanent ones.

### 🔍 Discovered Implementation

# Cross-Cutting Error Handling Audit — Final Report

## Executive Summary

The gravity-sdk node has **systemic fragility** in its error handling. The three core loops (`start_execution`, `start_commit_vote`, `start_commit`) are spawned with bare `.unwrap()` in the coordinator, meaning any error in these critical paths crashes the entire node via a tokio task panic. The block state machine uses `panic!()` as the primary response to invariant violations rather than returning errors, and several production HTTP/consensus paths contain `todo!()` that will crash on first use.

---

## 1. `reth_coordinator::run()` — Unwrap on Spawned Tasks

**File:** `bin/gravity_node/src/reth_coordinator/mod.rs:40-53`

```rust
pub async fn run(&self) {
    tokio::spawn(async move {
        reth_cli.start_execution().await.unwrap();   // line 43
    });
    tokio::spawn(async move {
        reth_cli.start_commit_vote().await.unwrap();  // line 47
    });
    tokio::spawn(async move {
        reth_cli.start_commit().await.unwrap();       // line 51
    });
}
```

**Impact:** Each of the three core node loops is spawned as an independent tokio task. If any returns `Err`, the `.unwrap()` panics **inside the spawned task**. In tokio, a panicked task aborts silently by default — the other two loops and main continue running, leaving the node in a **partially operational zombie state** (e.g., executing blocks but not committing them, or committing but not executing). There is no `JoinHandle` monitoring, no restart logic, and no alert.

Additionally at line 36: `execution_args_tx.send(execution_args).unwrap()` — if the oneshot receiver has been dropped (e.g. reth startup failed), this panics the coordinator.

---

## 2. `start_execution` / `start_commit` Loop Error Patterns

**File:** `bin/gravity_node/src/reth_cli.rs`

### start_execution (lines 309-370)
- **Line 349:** `let exec_blocks = exec_blocks.unwrap()` — Follows an `if let Err(e) = exec_blocks { ... continue; }` guard, so **logically safe** but fragile to refactoring.
- **Line 356:** `.last().expect("checked non-empty above")` — **Safe**, guarded by `is_empty()` check on line 350.

### start_commit (lines 441-499)
- **Line 461:** `let block_ids = block_ids.unwrap()` — Same guarded pattern. Logically safe but fragile.
- **Lines 466-468:** `self.pipe_api.get_block_id(last_block.num).unwrap_or_else(|| panic!(...))` — **Panics** if the pipe API doesn't have the block ID. This is a production commit path where transient storage lag could trigger this.
- **Line 469:** `assert_eq!(ExternalBlockId::from_bytes(block_id.as_slice()), last_block.block_id)` — **Panics** on block ID mismatch during commit. Kills the entire commit task rather than returning an error.

### start_commit_vote (lines 372-439)
- **Positive finding:** This loop has the best error handling in the codebase (GSDK-022). It tracks consecutive errors (max 5) and returns a graceful `Err` after sustained failures. However, this `Err` then hits the `.unwrap()` in `reth_coordinator::run()` line 47, **negating the benefit**.

---

## 3. `assert_eq!` in Production Paths — Block Buffer Manager

**File:** `crates/block-buffer-manager/src/block_buffer_manager.rs`

| Line | Function | Assert | Consequence |
|------|----------|--------|-------------|
| 600 | `get_executed_res` | `assert_eq!(id, &block_id)` | Panics if Computed block's stored ID doesn't match requested ID |
| 630 | `get_executed_res` | `assert_eq!(id, &block_id)` | Same check in the Committed branch |
| 721 | `set_compute_res` | `assert_eq!(block.block_meta.block_id, block_id)` | Panics if ordered block ID doesn't match compute result's block ID |

All three are inside `pub async fn` methods called from the consensus pipeline. A mismatch from a corrupted block or reorg **terminates the task** rather than returning `Err`. These are invariant checks that _should_ never fire, but the consequence of firing is disproportionate — silent task death via the coordinator's unwrap chain.

---

## 4. `panic!()` in Block State Machine

**File:** `crates/block-buffer-manager/src/block_buffer_manager.rs`

| Lines | Function | Trigger |
|-------|----------|---------|
| 544-546 | `get_ordered_blocks` | Block found in non-Ordered state (Computed/Committed/Historical) |
| 755 | `set_compute_res` | Compute result pushed for block that was never ordered |
| 802-805 | `set_commit_blocks` | Block ID mismatch in Computed state |
| 810 | `set_commit_blocks` | Block ID mismatch in Committed state (re-commit with conflicting ID) |
| 814-817 | `set_commit_blocks` | Block in Ordered state (commit before compute) |
| 823-826 | `set_commit_blocks` | Historical block ID mismatch |
| 831-834 | `set_commit_blocks` | Block not found at all |

**Pattern:** The state machine treats **every unexpected state transition as a fatal invariant violation** and panics. There is no error recovery path. A single corrupted or out-of-order block takes down the entire pipeline. `set_commit_blocks` alone has **6 distinct panic paths**.

---

## 5. `todo!()` and `unimplemented!()` in Production Paths

| File | Line | Function | Risk |
|------|------|----------|------|
| `crates/api/src/https/tx.rs` | 28 | `submit_tx` | **Critical.** HTTP endpoint handler. Any transaction submission via HTTPS API panics immediately. |
| `crates/api/src/consensus_mempool_handler.rs` | 41 | Error branch in txn handling | `if let Err(_error) = result { todo!() }` — First transaction error panics the mempool handler |
| `crates/api/src/consensus_mempool_handler.rs` | 112 | `SyncForDuration` notification | Any `SyncForDuration` consensus notification panics the handler |
| `crates/block-buffer-manager/src/block_buffer_manager.rs` | 327 | `recv_unbroadcasted_txn` | Public API method is fully `unimplemented!()` |

---

## 6. Error Swallowing Patterns

### Errors Logged but Not Propagated

| Location | Pattern | Impact |
|----------|---------|--------|
| `reth_cli.rs:345` | `warn!("failed to get ordered blocks: {}", e)` then `continue` | Timeout or buffer errors in `start_execution` silently retried forever. No backoff, no error count, no circuit breaker (contrast with `start_commit_vote`'s GSDK-022). |
| `reth_cli.rs:458` | `warn!("failed to get committed blocks: {}", e)` then `continue` | Same infinite silent retry pattern in `start_commit`. |
| `block_buffer_manager.rs:246` | `clone.remove_committed_blocks().await.unwrap()` in background task | Currently safe (always returns `Ok(())`), but any future error variant would crash the background cleanup task silently. |

### `let _ =` Discarded Results

| Location | What's Discarded |
|----------|-----------------|
| `main.rs:97,138` | Startup channel sends — silent drop means startup failures could deadlock at `rx.recv().unwrap()` (line 157) |
| `main.rs:248` | Shutdown broadcast send — acceptable during teardown |
| `block_buffer_manager.rs:265,495,752,837,982` | Broadcast notifications — acceptable (fails only when all receivers dropped) |

---

## 7. Startup Path `unwrap()` Chains

**File:** `bin/gravity_node/src/main.rs`

| Line | Expression | Risk |
|------|-----------|------|
| 112 | `provider.recover_block_number().unwrap()` | DB read error crashes startup |
| 115 | `provider.block_hash(n).unwrap().unwrap()` | **Double unwrap** — outer: IO error, inner: block not found |
| 118-119 | `provider.block(...).unwrap().unwrap()` | **Double unwrap** — same pattern |
| 157 | `rx.recv().unwrap()` | Channel receive — panics if reth thread panics before sending |
| 263 | `datadir_rx.await.expect("datadir should be sent")` | Panics if reth drops sender |
| 270 | `.parse::<bool>().unwrap()` | Env var `MOCK_CONSENSUS` with non-bool value crashes startup |
| 281 | `panic!("failed to set global relayer")` | Double-init of global singleton |

The double-unwraps on lines 115 and 118 are particularly concerning: if the database is corrupted or the block is missing, the node crashes with an opaque unwrap panic rather than a diagnostic message.

---

## 8. Overall Resilience Assessment

### Transient Failure Handling

| Component | Resilience | Mechanism |
|-----------|-----------|-----------|
| `start_commit_vote` | **Good** | GSDK-022: consecutive error counter, graceful shutdown after 5 failures |
| `start_execution` | **Poor** | Silent infinite retry via `warn!` + `continue`, no backoff |
| `start_commit` | **Poor** | Silent infinite retry via `warn!` + `continue`, no backoff |
| `get_ordered_blocks` / `get_executed_res` / `get_committed_blocks` | **Moderate** | Timeout-based retry with bounded wait (5s default), returns `Err` |

### Permanent Failure Handling

| Failure Mode | Behavior |
|-------------|----------|
| Database corruption at startup | Immediate panic (double-unwrap) |
| Block state machine invariant violation | Immediate panic (6+ paths) |
| Execution/commit task crash | **Silent zombie state** — other tasks keep running |
| Channel closure | Returns `Err` (good), but then unwrapped by coordinator |
| HTTPS `submit_tx` called | Immediate panic via `todo!()` |
| Consensus `SyncForDuration` notification | Immediate panic via `todo!()` |

### Failure Propagation Architecture

```
start_execution()  ─Err──▶  tokio::spawn { .unwrap() }  ──▶  task panic (silent)
start_commit_vote() ─Err──▶  tokio::spawn { .unwrap() }  ──▶  task panic (silent)
start_commit()     ─Err──▶  tokio::spawn { .unwrap() }  ──▶  task panic (silent)

                         ┌── No JoinHandle stored
                         ├── No health monitoring
                         ├── No restart mechanism
                         └── Main thread blocks at shutdown_rx.recv() (line 303)
                              waiting for Ctrl+C/SIGTERM that never comes
                              ═══▶  Zombie process
```

The node's main thread waits on a shutdown signal that only comes from Ctrl+C/SIGTERM. If all three worker tasks panic, the main thread **hangs indefinitely** as a zombie process.

---

## Severity Summary

| Severity | Count | Key Examples |
|----------|-------|-------------|
| **Critical** | 4 | `todo!()` in `submit_tx`, coordinator `.unwrap()` on spawned tasks, zombie state on task crash, `todo!()` in mempool error handler |
| **High** | 9 | 6 panics in `set_commit_blocks`, 3 `assert_eq!` in production paths |
| **Medium** | 6 | Double-unwraps at startup, infinite silent retry in `start_execution`/`start_commit`, `panic!` in `get_ordered_blocks` |
| **Low** | 5 | Startup env var parse unwrap, profiler mutex unwraps, metric registration unwraps |

### 🛠️ Ground Review

# Ground Review: Code Quality & Engineering Safety Audit

## Scope

Reviewed the cross-cutting error handling audit report against the actual codebase. Focus: resource leaks, concurrency safety, unhandled exceptions, naming, hardcoded secrets, input validation, and code hygiene. Business logic is out of scope.

---

## Critical

### 1. Detached `tokio::spawn` with `.unwrap()` — Silent Zombie State
**File:** `bin/gravity_node/src/reth_coordinator/mod.rs:42-51`

All three core node loops are spawned as detached tasks — `JoinHandle`s are immediately discarded. The `.unwrap()` inside each spawned task means:
- A returned `Err` causes a **silent task panic** (tokio swallows panicked detached tasks by default).
- No health monitoring, no restart logic, no alerting.
- Remaining tasks continue running, leaving the node in a **partially operational zombie state** (e.g., executing but not committing).
- The main thread blocks forever on `shutdown_rx.recv()` waiting for a signal that never comes.

This is a textbook resource leak of **liveness** — the process holds its port, PID, and file descriptors while doing nothing useful.

**Severity: Critical**

### 2. `todo!()` in Production HTTP Endpoint
**File:** `crates/api/src/https/tx.rs:28`

`submit_tx` — a publicly routable HTTP handler — contains `todo!()`. Any external transaction submission **panics the node**. This is a denial-of-service vector: any unauthenticated client can crash the process with a single HTTP request.

**Severity: Critical**

### 3. `todo!()` in Mempool Error Handler
**File:** `crates/api/src/consensus_mempool_handler.rs:41`

The error branch for transaction handling is `todo!()`. The first transaction that fails validation crashes the consensus mempool handler. Combined with the detached-spawn issue above, this silently kills the mempool with no recovery.

**Severity: Critical**

### 4. `todo!()` on `SyncForDuration` Notification
**File:** `crates/api/src/consensus_mempool_handler.rs:112`

A legitimate consensus notification type panics the handler. If the network ever sends this notification variant, the node dies.

**Severity: Critical**

---

## Warning

### 5. Six `panic!()` Paths in `set_commit_blocks`
**File:** `crates/block-buffer-manager/src/block_buffer_manager.rs:802-834`

The block state machine treats every unexpected state as a fatal invariant violation. Six distinct panic paths in a single function means a corrupted or out-of-order block kills the entire pipeline. These should return `Err` and let the caller decide on recovery.

**Severity: Warning (High)**

### 6. `assert_eq!` in Production Consensus Paths
**File:** `crates/block-buffer-manager/src/block_buffer_manager.rs:600, 630, 721`

Three `assert_eq!` calls in `get_executed_res` and `set_compute_res` — async methods called from the consensus pipeline. Assertion failures panic the task, which then dies silently via the coordinator's detached spawn. These should be error returns.

**Severity: Warning (High)**

### 7. Double-`unwrap()` on Startup DB Reads
**File:** `bin/gravity_node/src/main.rs:115, 118-119`

```rust
provider.block_hash(n).unwrap().unwrap()
provider.block(...).unwrap().unwrap()
```

Outer unwrap = IO error, inner unwrap = missing data. A corrupted or incomplete database produces an opaque panic with no diagnostic context. Should use `.context()` or a descriptive `expect()` at minimum.

**Severity: Warning**

### 8. Inconsistent Retry Strategy Across Core Loops
**Files:** `bin/gravity_node/src/reth_cli.rs:345, 458 vs 372-439`

| Loop | Strategy |
|------|----------|
| `start_commit_vote` | Consecutive error counter (max 5), graceful `Err` return |
| `start_execution` | `warn!()` + `continue` — infinite silent retry, no backoff |
| `start_commit` | `warn!()` + `continue` — infinite silent retry, no backoff |

Two of three critical loops have no circuit breaker, no backoff, and no error budget. This means transient failures (e.g., temporary storage unavailability) spin-loop indefinitely, burning CPU and flooding logs.

**Severity: Warning**

### 9. `unimplemented!()` in Public API
**File:** `crates/block-buffer-manager/src/block_buffer_manager.rs:327`

`recv_unbroadcasted_txn` is a `pub` method that is fully `unimplemented!()`. If any caller reaches this path, the task panics. Dead code that can kill you is worse than no code at all.

**Severity: Warning**

### 10. Oneshot Channel Send `.unwrap()` Without Receiver Guarantee
**File:** `bin/gravity_node/src/reth_coordinator/mod.rs:36`

`execution_args_tx.send(execution_args).unwrap()` — if the reth startup thread panicked or dropped the receiver, this crashes the coordinator. No guard, no error message.

**Severity: Warning**

---

## Info

### 11. Fragile Post-Guard Unwrap Pattern
**Files:** `bin/gravity_node/src/reth_cli.rs:349, 461`

```rust
if let Err(e) = exec_blocks { ... continue; }
let exec_blocks = exec_blocks.unwrap(); // "safe" because of guard above
```

Logically safe today, but a refactoring hazard. The guard and the unwrap are separated by logic that could diverge. Prefer `let Ok(exec_blocks) = exec_blocks else { ... continue; };` (let-else) for structural safety.

**Severity: Info**

### 12. Env Var Parse Unwrap
**File:** `bin/gravity_node/src/main.rs:270`

`.parse::<bool>().unwrap()` on `MOCK_CONSENSUS` env var. A typo like `MOCK_CONSENSUS=yes` (instead of `true`) crashes startup with no helpful message.

**Severity: Info**

### 13. `let _ =` Discarded Channel Sends at Startup
**File:** `bin/gravity_node/src/main.rs:97, 138`

Startup channel sends are silently discarded. If the sender fails, the receiver at line 157 (`rx.recv().unwrap()`) will hang or panic. The discarded result masks where the actual failure occurred.

**Severity: Info**

---

## Summary

| Severity | Count | Key Themes |
|----------|-------|------------|
| **Critical** | 4 | Zombie process on task failure, `todo!()` in reachable production paths (incl. HTTP endpoint = DoS vector) |
| **Warning** | 6 | Panic-on-invariant-violation in state machine (6+ paths), inconsistent retry/backoff, double-unwrap at startup |
| **Info** | 3 | Fragile unwrap patterns, env var parsing, masked channel errors |

### Top Recommendations (by impact)

1. **Store `JoinHandle`s and monitor task health.** Use `tokio::select!` or a supervisor loop to detect task death, log diagnostics, and initiate graceful shutdown (or restart). This single change addresses the zombie-state class of failures.
2. **Replace all `todo!()` with proper error responses** — especially `submit_tx`, which is externally reachable and constitutes a trivial DoS vector.
3. **Convert `panic!()`/`assert_eq!()` in the block state machine to `Result` returns.** Let callers decide recovery policy. The current approach makes every invariant violation process-fatal.
4. **Normalize retry strategy** across `start_execution` and `start_commit` to match `start_commit_vote`'s error-budget pattern (GSDK-022). Add exponential backoff.

### ⚔️ Intention Attacker

# Intention Attack Report — gravity-sdk Security Audit

## Executive Summary

The gravity-sdk node contains **multiple fatal business logic vulnerabilities** that go far beyond the error-handling surface described in the implementation report. The system has broken invariants in state management, missing trust boundaries between consensus and execution, economic loss vectors, and several paths where the node enters an irrecoverable zombie state. Below are the findings, ordered by severity.

---

## CRITICAL Findings

### C1: Zombie Node — Silent Task Death with No Recovery

**Files:** `reth_coordinator/mod.rs:42-52`, `main.rs:300-303`

All three core loops (`start_execution`, `start_commit_vote`, `start_commit`) are spawned with discarded `JoinHandle`s and `.unwrap()`. When any loop dies:

- The other two loops **continue running in a degraded, inconsistent state**
- The main thread **hangs indefinitely** at `shutdown_rx.recv()` waiting for a Ctrl+C that may never come
- There is no health monitoring, no restart mechanism, no alerting

**Attack scenario:** An attacker who can trigger a single `panic!` in any of the 6+ panic paths in `set_commit_blocks` or the `assert_eq!` in `get_executed_res` permanently kills that pipeline stage. The node appears alive (responds to health checks, keeps its P2P connections) but **stops producing or committing blocks**, silently stalling the chain.

---

### C2: Committed Hash Never Verified Against Computed Hash — Block Forgery

**File:** `block_buffer_manager.rs` — `set_compute_res` vs `set_commit_blocks`

The hash stored during execution (`set_compute_res`) and the hash stored during commit (`set_commit_blocks`) are **two independent values that are never cross-checked**. A caller can commit a block with a different hash than what was computed, and the buffer manager silently accepts it.

**Furthermore**, if `start_commit_vote` is behind `start_commit`, the hash field is `Option<[u8; 32]>` and will be `None`. The code passes `None` directly to `pipe_api.commit_executed_block_hash` — **committing a block with no execution hash**.

**Impact:** This breaks the fundamental invariant that a committed block was actually executed with the claimed result.

---

### C3: Consensus `parent_id` Silently Overridden — Fork Acceptance

**File:** `block_buffer_manager.rs:478-484`

```rust
info!("set_ordered_blocks parent_id is not the same as actual_parent_id ... might be epoch change");
actual_parent_id  // silently uses local value, ignoring consensus
```

When the consensus layer provides a `parent_id` that doesn't match the local chain, the execution layer **silently overrides it** and continues. The `TODO(gravity_alex): assert epoch` confirms this logic is incomplete. This means the execution layer can accept a forked chain without any error, alert, or halt — **a fundamental trust boundary violation between consensus and execution**.

---

### C4: `chain_id` Hardcoded to `0` — Cross-Chain Replay Protection Broken

**File:** `mempool.rs:102, 115`

```rust
chain_id: ExternalChainId::new(0),
```

All `VerifiedTxn` objects are constructed with `chain_id: 0` regardless of the actual chain. A transaction signed for one gravity chain can be replayed on any other gravity chain. This defeats EIP-155 replay protection at the mempool layer.

---

### C5: `todo!()` in Production Consensus Path — Node Crash on First Error

**File:** `consensus_mempool_handler.rs:41, 112`

Two `todo!()` macros exist in **ungated production code**:

1. **Line 41:** If `notify_new_commit` ever returns an error, `todo!()` panics. The `Con2Mempool` task dies permanently. Committed transactions **are never evicted from the mempool again**, causing unbounded memory growth and potential re-proposal of already-committed transactions.

2. **Line 112:** A `SyncForDuration` consensus notification panics the handler. After this, commit acknowledgments stop flowing back to consensus, which can **stall consensus liveness** across all validators.

Both are denial-of-service vectors: any condition that triggers either path permanently degrades the node.

---

## HIGH Findings

### H1: Partial Batch Commit with No Rollback — Inconsistent State

**File:** `reth_cli.rs:472-491`

`start_commit` commits blocks in a batch loop. If the loop succeeds for blocks N and N+1 but fails on N+2, the `?` operator returns `Err`. The spawned task panics (see C1). Blocks N and N+1 are committed in the pipe layer, but the commit task is now dead. The execution and vote tasks continue running unaware. The buffer manager and reth pipe state are now permanently inconsistent — **there is no rollback of partial commits**.

---

### H2: Epoch Race — Old-Epoch Blocks Stamped with New Epoch

**Files:** `reth_cli.rs:337-343, 432, 448`

`current_epoch` is an `AtomicU64` written only by `start_execution` and read by `start_commit_vote` and `start_commit`. There is **no synchronization barrier** ensuring the other loops have finished processing old-epoch blocks before seeing the new epoch value. A block from epoch N still in-flight through `start_commit_vote` will be stamped with epoch N+1's value, violating epoch-boundary invariants.

---

### H3: Double Execution Risk — No In-Flight Guard

**File:** `block_buffer_manager.rs` — `get_ordered_blocks`

`get_ordered_blocks` returns blocks in `Ordered` state **without mutating state to mark them as dispatched**. If called twice (e.g., by a restarting executor or concurrent task), the same blocks are returned and executed again. The second `set_compute_res` call panics (block already in `Computed` state), crashing the execution task silently (see C1).

---

### H4: Coinbase Silently Zeroed — Validator Fee Burn

**File:** `reth_cli.rs:142-181`

Three separate failure paths in proposer-to-coinbase lookup all fall back to `Address::ZERO`:

```rust
warn!("Failed to get reth coinbase for proposer index {}, using ZERO.");
Address::ZERO
```

Block fees and MEV rewards are **sent to the zero address (burned)** instead of the correct validator. A malicious consensus layer that omits `proposer_index` from block metadata causes economic loss for every affected block.

---

### H5: TOCTOU on `is_epoch_change()` Check

**File:** `block_buffer_manager.rs:509-511`

`is_epoch_change()` reads an `AtomicU8` **without holding the mutex**, then the caller acquires the mutex afterward. Between the two operations, another task can trigger an epoch change. The existing `TODO` comments confirm this was a known concern only partially addressed.

---

### H6: `remove_committed_blocks` Is a No-Op — Memory Leak

**File:** `block_buffer_manager.rs:259-263`

```rust
block_state_machine.latest_finalized_block_number = std::cmp::max(
    block_state_machine.latest_finalized_block_number,
    latest_persist_block_num,  // same variable — max(x, x) = x
);
```

The cleanup function never advances its own watermark. The `blocks` map, `profile` map, and `block_number_to_block_id` map grow without bound. Under sustained load, this is a **deterministic OOM**.

---

### H7: Sensitive Credentials Logged

**File:** `relayer.rs:97, 224`

`RelayerConfig` containing RPC URLs (which may include embedded API keys like `https://eth-mainnet.alchemyapi.io/v2/SECRET_KEY`) is logged at `info` level on every startup. Oracle states are similarly dumped. These logs are typically shipped to external aggregators.

---

### H8: Malformed Transactions Silently Dropped — No Slashing Signal

**File:** `reth_cli.rs:210-222`

Transactions from consensus blocks that fail signature recovery are silently skipped with a `warn!` log. There is no counter, no metric, no signal to the consensus/slashing layer. A malicious proposer can fill blocks with garbage transactions to waste block space, and the only evidence is buried in log files with no alerting.

---

## MEDIUM Findings

| # | Finding | File | Impact |
|---|---------|------|--------|
| M1 | Integer underflow: `block_number - 1` when `block_number == 0` wraps to `u64::MAX` | `block_buffer_manager.rs:461` | Nonsensical block key lookup |
| M2 | `SystemTime::now()` used for block timestamp in mempool GC instead of block's consensus timestamp | `consensus_mempool_handler.rs:79` | TTL eviction incorrect under clock skew |
| M3 | `get_committed_blocks` destructively `take()`s `persist_notifier` — second call loses the ack channel | `block_buffer_manager.rs:879` | Epoch-change wait hangs indefinitely |
| M4 | Nonce ordering in mempool only tracks iterator-local state, not on-chain committed nonce | `mempool.rs:145-151` | Nonce gaps undetected, potential replay within mempool window |
| M5 | Epoch state (`next_epoch`, `latest_epoch_change_block_number`) mutated before block state transition; no rollback on partial failure | `block_buffer_manager.rs` `calculate_new_epoch_state` | Permanent epoch/block desync |
| M6 | `shutdown.resubscribe()` per loop iteration can miss shutdown signals under load | `reth_cli.rs:323,378,449` | Delayed shutdown, zombie loops |
| M7 | Unbounded `txn_buffer` with no size cap in `push_txns` | `block_buffer_manager.rs:332` | OOM under transaction flood |
| M8 | `run_reth` calls `process::exit(1)` on error, bypassing async cleanup and shutdown broadcast | `main.rs:153` | Unclean shutdown, data corruption risk |
| M9 | Mutex held across `.await` in `set_compute_res` | `block_buffer_manager.rs:724-726` | Lock starvation under contention |

---

## Architecture-Level Assessment

The systemic issue is a **failure propagation architecture that creates zombie states**:

```
start_execution()   ─Err──▶ .unwrap() ──▶ task panic (silent)
start_commit_vote() ─Err──▶ .unwrap() ──▶ task panic (silent)
start_commit()      ─Err──▶ .unwrap() ──▶ task panic (silent)

    ┌── No JoinHandle stored
    ├── No health monitoring
    ├── No restart mechanism
    └── Main thread blocks on shutdown_rx forever
         ═══▶ Zombie process that looks alive but is dead
```

Combined with the block buffer manager's state machine having **6 distinct panic paths** and **no in-flight execution guard**, any transient failure cascades into permanent, irrecoverable corruption. The design assumes infallible execution in a distributed system where failures are guaranteed.

---

## Top 5 Recommendations (by blast radius)

1. **Store JoinHandles, monitor tasks, and implement coordinated shutdown** — Any task death must trigger a full node restart or at minimum halt all three loops atomically.

2. **Verify committed hash against computed hash** — The commit path must assert that the block hash being committed matches what was computed during execution. This is the core safety invariant of any execute-then-commit blockchain.

3. **Replace `todo!()` in `consensus_mempool_handler.rs`** with proper error handling — These are live DoS vectors in production code.

4. **Fix `chain_id` from hardcoded `0`** to the actual chain ID — Cross-chain replay protection is currently non-functional.

5. **Add an `Executing` state to the block state machine** — Prevent double-dispatch of blocks for execution, which currently leads to a crash.

---

## [aptos_consensus_safety_rules] Security audit of aptos-core/consensus/safety-rules/ for double-signing preventi

### 🎯 Intention

Security audit of aptos-core/consensus/safety-rules/ for double-signing prevention, equivocation guards, private key handling within the safety rules engine, and any modifications gravity-sdk has made to the upstream aptos-core safety rules. This is the most safety-critical component in any BFT system.

### 🔍 Discovered Implementation

# Safety Rules Engine — Implementation Analysis

## Entry Point & Crate Structure

**Crate:** `aptos-core/consensus/safety-rules/`
**Modification status:** No gravity-sdk-specific patches found. All files carry upstream Aptos copyright headers. The crate depends on `gaptos` (gravity-sdk's re-packaged Aptos libraries), so any type-level changes would reside there, not in this crate. `#![forbid(unsafe_code)]` is declared in `lib.rs`.

---

## Files/Contracts Involved

| File | Purpose |
|---|---|
| `src/safety_rules.rs` | Core `SafetyRules` struct, guard methods, `TSafetyRules` impl |
| `src/safety_rules_2chain.rs` | 2-chain protocol vote/timeout/order-vote guards |
| `src/persistent_safety_storage.rs` | `PersistentSafetyStorage` — typed KV wrapper for consensus state persistence |
| `src/t_safety_rules.rs` | `TSafetyRules` trait (public interface) |
| `src/consensus_state.rs` | `ConsensusState` — read-only monitoring snapshot (no keys) |
| `src/safety_rules_manager.rs` | `SafetyRulesManager` — factory, storage bootstrap, key rotation |
| `src/local_client.rs` | `LocalClient` — in-process `Arc<RwLock<SafetyRules>>` wrapper |
| `src/serializer.rs` | `SerializerClient`/`SerializerService` — JSON-over-wire IPC |
| `src/thread.rs` | `ThreadService` — dedicated-thread isolation |
| `src/process.rs` | `ProcessService` — separate-process isolation |
| `src/error.rs` | `Error` enum — all rejection reasons |
| `../consensus/src/metrics_safety_rules.rs` | `MetricsSafetyRules` — retry wrapper with metrics |
| `../consensus/src/epoch_manager.rs` | Creates `SafetyRulesManager`, calls `initialize()` on epoch changes |
| `../consensus/src/round_manager.rs` | Calls `sign_proposal`, `construct_and_sign_vote_two_chain`, `sign_timeout_with_qc` |

---

## Core Data Structures

### `SafetyRules` (safety_rules.rs:44)
```rust
pub struct SafetyRules {
    pub(crate) persistent_storage: PersistentSafetyStorage,
    pub(crate) validator_signer: Option<ValidatorSigner>,  // None until initialize()
    pub(crate) epoch_state: Option<EpochState>,
}
```

### `SafetyData` (from aptos-consensus-types)
```rust
pub struct SafetyData {
    pub epoch: u64,
    pub last_voted_round: u64,       // prevents double-voting
    pub preferred_round: u64,         // 2-chain head parent round (prevents equivocation)
    pub one_chain_round: u64,         // highest certified block round
    pub last_vote: Option<Vote>,      // idempotent re-delivery
    pub highest_timeout_round: u64,   // for order-vote guards
}
```

### `PersistentSafetyStorage` (persistent_safety_storage.rs:27)
```rust
pub struct PersistentSafetyStorage {
    enable_cached_safety_data: bool,
    cached_safety_data: Option<SafetyData>,  // write-through cache
    internal_store: Storage,                  // pluggable KV backend
}
```

Storage keys: `CONSENSUS_KEY`, `OWNER_ACCOUNT`, `SAFETY_DATA`, `WAYPOINT`.

---

## Execution Paths

### 1. Initialization (`guarded_initialize`, safety_rules.rs:236–316)

```
guarded_initialize(proof: &EpochChangeProof)
  ├─ Read current waypoint from persistent_storage
  ├─ proof.verify(&waypoint) — validate epoch change proof against trust anchor
  ├─ Extract next_epoch_state from final LedgerInfo
  ├─ If new_waypoint.version() > waypoint.version():
  │     └─ persistent_storage.set_waypoint(new_waypoint) — monotonic advance only
  ├─ Read current_epoch from safety_data
  ├─ Match current_epoch vs epoch_state.epoch:
  │     Greater → Err(WaypointOutOfDate)
  │     Less    → set_safety_data(SafetyData::new(epoch, 0, 0, 0, None, 0))
  │                *** ALL voting state reset to zero ***
  │     Equal   → no-op
  ├─ self.epoch_state = Some(epoch_state)
  └─ Key reconciliation:
        ├─ Look up expected public key from epoch verifier for this author
        ├─ If current signer already matches → Ok
        ├─ Else → consensus_sk_by_pk(expected_key):
        │     ├─ Try explicit key: "consensus_key_<pk_hex>"
        │     ├─ Fallback: default "consensus_key"
        │     ├─ Verify key.public_key() == expected pk
        │     └─ Set self.validator_signer = Some(ValidatorSigner::new(author, key))
        └─ On failure → self.validator_signer = None (disarms signing)
```

### 2. Voting (`guarded_construct_and_sign_vote_two_chain`, safety_rules_2chain.rs:54–96)

```
guarded_construct_and_sign_vote_two_chain(vote_proposal, timeout_cert)
  ├─ self.signer()? — fail if not initialized
  ├─ verify_proposal(vote_proposal):
  │     ├─ Read safety_data from storage
  │     ├─ verify_epoch(proposed_block.epoch(), &safety_data)
  │     ├─ verify_qc(proposed_block.quorum_cert()) — check QC signatures
  │     ├─ validate_signature(&epoch_state.verifier) — check proposal signature
  │     ├─ verify_well_formed()
  │     └─ gen_vote_data()
  ├─ If timeout_cert present → verify_tc(tc)
  ├─ Read safety_data from storage
  ├─ *** IDEMPOTENCY CHECK ***: if last_vote.round == proposed_block.round → return last_vote
  ├─ *** RULE 1 ***: verify_and_update_last_vote_round(round, &mut safety_data)
  │     └─ round <= last_voted_round → Err(IncorrectLastVotedRound)
  │     └─ safety_data.last_voted_round = round
  ├─ *** RULE 2 ***: safe_to_vote(block, timeout_cert)
  │     └─ round == qc_round + 1
  │        OR (round == tc_round + 1 AND qc_round >= hqc_round)
  │     └─ Else → Err(NotSafeToVote)
  ├─ observe_qc() — update one_chain_round, preferred_round
  ├─ construct_ledger_info_2chain() — 2-chain commit rule
  ├─ self.sign(&ledger_info)
  ├─ safety_data.last_vote = Some(vote)
  └─ persistent_storage.set_safety_data(safety_data) — persist atomically
```

### 3. Timeout Signing (`guarded_sign_timeout_with_qc`, safety_rules_2chain.rs:22–52)

```
guarded_sign_timeout_with_qc(timeout, timeout_cert)
  ├─ self.signer()?
  ├─ Read safety_data
  ├─ verify_epoch(timeout.epoch())
  ├─ timeout.verify(&verifier) — check timeout signatures
  ├─ If timeout_cert → verify_tc(tc)
  ├─ safe_to_timeout(timeout, tc, &safety_data):
  │     └─ (round == qc_round + 1 OR round == tc_round + 1)
  │        AND qc_round >= one_chain_round
  ├─ If timeout.round() < last_voted_round → Err
  ├─ If timeout.round() > last_voted_round → update last_voted_round
  ├─ update_highest_timeout_round(timeout, &mut safety_data)
  ├─ persistent_storage.set_safety_data(safety_data)
  └─ self.sign(&timeout.signing_format())
```

### 4. Proposal Signing (`guarded_sign_proposal`, safety_rules.rs:318–348)

```
guarded_sign_proposal(block_data)
  ├─ self.signer()?
  ├─ verify_author(block_data.author()) — must be this validator
  ├─ Read safety_data
  ├─ verify_epoch(block_data.epoch())
  ├─ If block_data.round() <= last_voted_round → Err(InvalidProposal)
  ├─ verify_qc(block_data.quorum_cert())
  ├─ verify_and_update_preferred_round(qc, &mut safety_data)
  │     *** Note: preferred_round NOT persisted here (comment: "save latency") ***
  └─ self.sign(block_data)
```

### 5. Commit Vote Signing (`guarded_sign_commit_vote`, safety_rules.rs:350–391)

```
guarded_sign_commit_vote(ledger_info_with_sigs, new_ledger_info)
  ├─ self.signer()?
  ├─ Verify old_ledger_info.commit_info().is_ordered_only()
  │   OR old == new commit_info (fast-forward sync path)
  ├─ Verify match_ordered_only(old, new)
  ├─ ledger_info.verify_signatures(&epoch_state.verifier) — 2f+1 check
  └─ self.sign(&new_ledger_info)
```

### 6. Order Vote (`guarded_construct_and_sign_order_vote`, safety_rules_2chain.rs:98–120)

```
guarded_construct_and_sign_order_vote(order_vote_proposal)
  ├─ self.signer()?
  ├─ verify_order_vote_proposal() — epoch, QC consistency
  ├─ Read safety_data
  ├─ observe_qc() — update one_chain_round, preferred_round
  ├─ safe_for_order_vote(block, &safety_data):
  │     └─ round > highest_timeout_round → Ok
  │     └─ Else → Err(NotSafeForOrderVote)
  ├─ self.sign(&ledger_info)
  └─ persistent_storage.set_safety_data(safety_data)
```

---

## Key Functions — Detailed Signatures and Behavior

### Double-Voting Prevention

| Function | Location | Behavior |
|---|---|---|
| `verify_and_update_last_vote_round(round, &mut safety_data)` | safety_rules.rs:191 | **Rejects if `round <= safety_data.last_voted_round`**. On success, sets `safety_data.last_voted_round = round`. This is the primary double-voting guard. |
| Idempotency check in `guarded_construct_and_sign_vote_two_chain` | safety_rules_2chain.rs:71–74 | If `last_vote.round == proposed_block.round`, returns the cached `last_vote` instead of re-signing — prevents conflicting votes on the same round while allowing retransmission. |

### Equivocation Prevention

| Function | Location | Behavior |
|---|---|---|
| `verify_and_update_preferred_round(qc, &mut safety_data)` | safety_rules.rs:156 | **Rejects if `qc.certified_block().round() < safety_data.preferred_round`**. Prevents voting for a fork with a weaker QC chain. |
| `safe_to_vote(block, maybe_tc)` | safety_rules_2chain.rs:146 | Enforces: `round == qc_round + 1` OR `(round == tc_round + 1 AND qc_round >= hqc_round)`. Prevents voting on rounds that don't properly extend the chain. |
| `safe_to_timeout(timeout, maybe_tc, safety_data)` | safety_rules_2chain.rs:125 | Enforces: `(round == qc_round + 1 OR round == tc_round + 1) AND qc_round >= one_chain_round`. |

### Private Key Handling

| Function | Location | Behavior |
|---|---|---|
| `signer(&self)` | safety_rules.rs:114 | Returns `&ValidatorSigner` or `Err(NotInitialized)`. Every signing path calls this first. |
| `sign<T>(&self, message)` | safety_rules.rs:106 | Delegates to `validator_signer.sign(message)`. Generic over `Serialize + CryptoHash`. |
| `consensus_sk_by_pk(pk)` | persistent_safety_storage.rs:101 | Looks up private key first by explicit key `"consensus_key_<pk_hex>"`, then fallback to default `"consensus_key"`. **Verifies `key.public_key() == pk` before returning.** |
| Key rotation in `storage()` | safety_rules_manager.rs:76–90 | Writes each historical consensus key under `"consensus_key_<pk_hex>"` to support offline validators catching up across key rotations. |

---

## State Changes

| Operation | What Changes | Persisted? |
|---|---|---|
| `guarded_initialize` (new epoch) | `SafetyData` reset to `(epoch, 0, 0, 0, None, 0)`, waypoint advanced, validator_signer set/cleared | Yes (safety_data + waypoint) |
| `guarded_construct_and_sign_vote_two_chain` | `last_voted_round` increased, `one_chain_round`/`preferred_round` potentially increased, `last_vote` set | Yes |
| `guarded_sign_timeout_with_qc` | `last_voted_round` potentially increased, `highest_timeout_round` potentially increased | Yes |
| `guarded_sign_proposal` | `preferred_round` potentially increased | **No** — comment: "we don't persist the updated preferred round to save latency (it'd be updated upon voting)" |
| `guarded_construct_and_sign_order_vote` | `one_chain_round`/`preferred_round` potentially increased | Yes |
| `guarded_sign_commit_vote` | None | No state mutation |

---

## Persistence Layer

### Storage Architecture

`PersistentSafetyStorage` wraps a pluggable `Storage` backend with a write-through cache:

- **Read path**: If `enable_cached_safety_data`, serves from `cached_safety_data`. Otherwise reads from `internal_store` on every call.
- **Write path**: Writes to `internal_store` first. On success, updates cache. **On failure, invalidates cache** (`cached_safety_data = None`) — conservative correctness.
- **Backend options**: `InMemoryStorage` (tests), `OnDiskStorage` (production default), `VaultStorage` (HSM-grade).
- **No WAL**: Crash safety depends entirely on the backend. `OnDiskStorage` performs synchronous filesystem writes with no atomic rename or fsync guarantees at this layer.

### Key Storage Schema

| Key | Value | Notes |
|---|---|---|
| `"safety_data"` | `SafetyData` (serialized) | Core BFT state — loss causes equivocation risk |
| `"consensus_key"` | `bls12381::PrivateKey` | Default consensus key |
| `"consensus_key_<pk_hex>"` | `bls12381::PrivateKey` | Epoch-specific rotated keys |
| `"owner_account"` | `Author` | Validator address |
| `"waypoint"` | `Waypoint` | Trust anchor — monotonically advancing |

---

## External Dependencies

| Dependency | Source | Used For |
|---|---|---|
| `gaptos::aptos_crypto::bls12381` | Re-packaged Aptos crypto | BLS signing primitives |
| `gaptos::aptos_types::ValidatorSigner` | Re-packaged Aptos types | Key holder, signing delegation |
| `gaptos::aptos_secure_storage::Storage` | Re-packaged Aptos secure storage | Pluggable KV backend |
| `gaptos::aptos_safety_rules::counters` | Re-packaged Aptos safety rules | Prometheus metrics counters |
| `aptos_consensus_types::safety_data::SafetyData` | Consensus types crate | BFT state struct |
| `aptos_consensus_types::timeout_2chain::*` | Consensus types crate | 2-chain timeout types |

---

## MetricsSafetyRules Retry Behavior

`MetricsSafetyRules` (in `consensus/src/metrics_safety_rules.rs`) wraps `Box<dyn TSafetyRules>` and provides:

1. **`monitor!("safety_rules", ...)` instrumentation** on every call.
2. **Auto-retry on transient errors**: catches `NotInitialized`, `IncorrectEpoch`, or `WaypointOutOfDate`, calls `perform_initialize()`, and retries once.
3. **`perform_initialize()`** loops calling `initialize(&proofs)` while the waypoint version keeps advancing — handles chunked epoch change proofs.
4. **`CommitSignerProvider` bridge**: exposes `sign_commit_vote` through `Mutex<MetricsSafetyRules>` for the pipeline signing phase.

---

## Service Deployment Modes

| Mode | Isolation | Description |
|---|---|---|
| `Local` | None (in-process) | `SafetyRules` behind `Arc<RwLock>` |
| `Thread` | Thread boundary | Dedicated OS thread |
| `Serializer` | Serialized IPC | JSON request/response over channel |
| `Process` | Process boundary | Separate OS process via TCP socket (strongest isolation) |

### 🛠️ Ground Review

# Ground Review: Safety Rules Engine — Code Quality & Engineering Safety

**Crate:** `aptos-core/consensus/safety-rules/`
**Modification status:** Unmodified upstream Aptos code. All files carry original Aptos/Meta copyright headers. Gravity-sdk integration is via the `gaptos` dependency re-export layer only.

---

## Critical

### 1. `println!` in Production Signing Path
**File:** `safety_rules.rs:329`

A raw `println!` sits inside `guarded_sign_proposal()` — the hot path for every block proposal a validator signs. The rest of the crate uses structured `aptos_logger` macros (`trace!`, `warn!`). This:
- Bypasses log level filtering and structured log collection pipelines
- Leaks diagnostic info to raw stdout in production
- Duplicates the error message verbatim (once in `println!`, once in the `Err(...)`)
- Is almost certainly a debug leftover — not present in upstream Aptos

**Recommendation:** Remove the `println!` and replace with `warn!` using the existing `SafetyLogSchema` pattern.

---

### 2. Background Thread Leak on `ThreadService` Drop
**File:** `thread.rs`

```rust
pub struct ThreadService {
    _child: JoinHandle<()>,  // underscore-prefixed to suppress unused warning
    ...
}
```

`JoinHandle` does **not** terminate its thread on drop — it detaches it. The thread runs a blocking TCP listener loop (`remote_service::execute()`). There is no `Drop` impl, no shutdown channel, and no join. When `ThreadService` is dropped, the thread continues running indefinitely, holding a port open. In test environments, multiple detached threads accumulate across test runs.

**Recommendation:** Add a shutdown signal (`Arc<AtomicBool>` or `oneshot` channel) and a `Drop` impl that signals and joins.

---

## Warning

### 3. `panic!` Inside `From` Impl for `PermissionDenied`
**File:** `error.rs:82–91`

```rust
gaptos::aptos_secure_storage::Error::PermissionDenied => {
    panic!("A permission error was thrown: {:?}. ...", error);
}
```

A Vault token expiry causes an immediate `panic!` inside a `From` conversion — violating the Rust convention that `From` impls are infallible. In `Local` or `Thread` deployment modes, this panic propagates up and crashes the entire validator node without structured logging, metric emission, or graceful connection teardown.

**Recommendation:** Propagate as an `Error::InternalError` variant and let the caller decide to crash or alert.

---

### 4. Unconditional `.unwrap()` on `identity_blob()`
**File:** `safety_rules_manager.rs:77`

```rust
let blob = config.initial_safety_rules_config.identity_blob().unwrap();
```

Called unconditionally after an `if/else if/else` block where the `else` branch already panics. The guard is implicit — if the earlier panic is ever refactored to return an error, this line becomes a second, less informative panic.

**Recommendation:** Use `?` or `.expect("identity_blob required after config validation")` with clear context.

---

### 5. All Storage Tests Commented Out
**File:** `persistent_safety_storage.rs:185–269`

The entire `#[cfg(test)] mod tests` block is commented out. These tests verify that Prometheus counters (`EPOCH`, `LAST_VOTED_ROUND`, `PREFERRED_ROUND`, `WAYPOINT_VERSION`) are correctly set on reads/writes through `PersistentSafetyStorage`. The `rusty-fork` dev-dependency still sits in `Cargo.toml` serving no purpose.

**Recommendation:** Re-enable the tests or remove the dead code and unused dev-dependency.

---

## Info

### 6. Redundant Storage Reads in `guarded_consensus_state()`
**File:** `safety_rules.rs:218–234`

`safety_data()` and `waypoint()` are each called **twice** — once for logging, once for constructing the return value. When caching is disabled, each call hits the backing KV store. `waypoint()` has no cache and always hits storage.

**Recommendation:** Reuse the already-bound `safety_data` and `waypoint` locals.

---

### 7. Vague Error Message in `consensus_sk_by_pk()`
**File:** `persistent_safety_storage.rs:117`

```rust
(Err(_), Err(_)) => {
    return Err(Error::ValidatorKeyNotFound("not found!".to_string()));
}
```

Both underlying storage errors are silently discarded. An operator sees `ValidatorKeyNotFound("not found!")` with no indication of which key was missing or why storage failed.

**Recommendation:** Include both error details and the `explicit_storage_key` value in the message.

---

### 8. Misleading Comment: "Timeout in Seconds"
**File:** `process.rs:45`

The comment says `// Timeout in Seconds for network operations` but all call sites name the parameter `timeout_ms` and the config field is `network_timeout_ms`. A 1000x misconfiguration risk for operators reading the struct comment.

**Recommendation:** Fix the comment to say milliseconds.

---

### 9. Unnecessary Clippy Suppression
**File:** `error.rs:73`

`#[allow(clippy::fallible_impl_from)]` is applied to a `From` impl that simply calls `.to_string()` — it cannot panic. The suppression is cargo-culted and should be removed to keep lint coverage tight.

---

### 10. JSON Wire Format on Latency-Critical Path
**File:** `serializer.rs`

`serde_json` is used as the serialization format for `SafetyRulesInput` even in the in-process `LocalService` path. Every vote, proposal, and timeout pays for a full JSON encode/decode round-trip. A binary format (BCS, bincode) would be significantly more efficient on this latency-critical BFT path.

---

## Summary

| Severity | Count | Key Themes |
|----------|-------|------------|
| **Critical** | 2 | Debug `println!` in prod signing path; thread leak with no shutdown |
| **Warning** | 3 | `panic!` in `From` impl; fragile `.unwrap()`; disabled test coverage |
| **Info** | 5 | Redundant I/O; vague errors; misleading comments; lint suppression; suboptimal wire format |

The crate is unmodified from upstream Aptos, so these issues are inherited rather than introduced by gravity-sdk. Items #1 (the `println!`) and #7 (thread leak) are the most actionable — both are straightforward fixes with no protocol-level risk.

### ⚔️ Intention Attacker

# Safety Rules Engine — Red Team Attack Report

## Scope
Security audit of `aptos-core/consensus/safety-rules/` — the BFT-critical signing guard that prevents double-voting, equivocation, and private key misuse.

---

## Critical Findings

### CRITICAL-01: Proposal Signing Does Not Persist `preferred_round` — Equivocation Window After Crash

**Location:** `safety_rules.rs:318–348` (`guarded_sign_proposal`)

**Detail:** After signing a proposal, `preferred_round` is updated in-memory but **explicitly not persisted** (comment: *"we don't persist the updated preferred round to save latency"*). If the validator crashes after signing a proposal but before casting a vote (which does persist), the restarted node will reload a stale `preferred_round` from disk.

**Attack:** A Byzantine leader can:
1. Feed this validator a proposal that advances `preferred_round` to R.
2. Crash the validator (network partition, OOM, power loss).
3. After restart, `preferred_round` reverts to a value < R.
4. Feed the validator a conflicting proposal on a fork with a weaker QC that should have been rejected by the `preferred_round` guard.
5. The validator signs a proposal on a conflicting fork — an equivocation.

**Severity: CRITICAL** — This is an intentional correctness/performance tradeoff that creates a real equivocation window. The comment acknowledges it ("it'd be updated upon voting"), but the assumption that a vote always follows a proposal before any crash is not guaranteed.

---

### CRITICAL-02: No Crash-Safety Guarantee at the Persistence Layer — Double-Sign After Power Loss

**Location:** `persistent_safety_storage.rs:145–162` (`set_safety_data`)

**Detail:** The write path delegates entirely to `internal_store.set()`. No `fsync`, no atomic rename, no WAL, no CRC/integrity check exists at this layer. The crash-safety guarantee is wholly dependent on the `gaptos::aptos_secure_storage` backend, which is a vendored dependency **not auditable from this codebase**.

**Attack scenario:**
1. Validator votes on round R, `set_safety_data` returns `Ok`, `last_voted_round = R` is in the page cache.
2. Power loss before OS flushes to disk.
3. On restart, `last_voted_round` reloads as the prior value (< R).
4. Validator is presented with a conflicting block for round R and votes again — **double-sign**.

**Compounding factor:** There is no integrity check (CRC, hash, HMAC) on the serialized `SafetyData`. A partially written or corrupted value that is structurally valid (e.g., zeroed `last_voted_round`) will be silently accepted.

**Severity: CRITICAL** — In PoS systems, double-signing leads to slashing. The entire safety guarantee of this component rests on an unverified backend.

---

## High Findings

### HIGH-01: Idempotency Check Compares Round Only — Not Block Hash

**Location:** `safety_rules_2chain.rs:71–74`

**Detail:** The idempotency guard checks `last_vote.round == proposed_block.round` and returns the cached `last_vote`. It does **not** verify that the `proposed_block` (its hash/ID) matches the block the cached vote was cast for.

**Attack:** If the cached `last_vote` is for block A at round R, and a Byzantine leader presents block B (different content, same round R), the guard returns the vote for block A. This is actually **safe** (it doesn't create a conflicting vote), but it means the idempotency check silently papers over a Byzantine leader equivocating. The validator will never detect or report that it received conflicting proposals for the same round from the same leader. This is a missed detection opportunity rather than a direct exploit, but in combination with other bugs it could mask attacks.

**Severity: HIGH** — Not directly exploitable for equivocation, but suppresses Byzantine fault detection.

### HIGH-02: Epoch Transition Resets ALL Safety State to Zero

**Location:** `safety_rules.rs:236–316` (`guarded_initialize`)

**Detail:** When `current_epoch < new_epoch`, all of `SafetyData` is reset:
```
SafetyData::new(epoch, 0, 0, 0, None, 0)
```
This sets `last_voted_round = 0`, `preferred_round = 0`, `one_chain_round = 0`, `highest_timeout_round = 0`.

**Attack:** If a validator processes an epoch change proof but the network has not yet fully transitioned, and old-epoch messages are still in-flight, the zeroed state could potentially interact with stale messages. The `verify_epoch` check should prevent this in the normal case, but any bug or bypass in epoch verification during the transition window would have catastrophic impact since all guards are effectively disabled (all thresholds are 0).

**Severity: HIGH** — The epoch check is the single point of defense during transition. If it has any edge-case bypass, all safety invariants are void.

### HIGH-03: Prometheus Counters Updated Before Durable Write

**Location:** `persistent_safety_storage.rs:145–152`

**Detail:** `counters::set_state(LAST_VOTED_ROUND, ...)` is called **before** `internal_store.set()`. If the write fails, Prometheus reports a `last_voted_round` that was never persisted.

**Attack:** An operator monitoring dashboards sees `last_voted_round = R` while the actual persisted value is R-1. During incident response, operators may believe the validator is in a safe state when it is not, leading to incorrect manual interventions (e.g., restarting the node thinking it's safe, when it's actually vulnerable to double-signing round R).

**Severity: HIGH** — Misleads operational monitoring for the most safety-critical metric in the system.

---

## Medium Findings

### MEDIUM-01: `consensus_sk_by_pk` Fallback to Default Key

**Location:** `persistent_safety_storage.rs:101`

**Detail:** Key lookup first tries `"consensus_key_<pk_hex>"`, then falls back to the default `"consensus_key"`. The fallback verifies `key.public_key() == expected_pk`, which is correct. However, this means the default key slot is read on every cache miss for a rotated key, creating a timing side-channel that reveals whether a key rotation has occurred.

**Severity: MEDIUM** — Information leak, not directly exploitable for signing.

### MEDIUM-02: `LocalClient` Uses `Arc<RwLock>` — No Process Isolation

**Location:** `local_client.rs`

**Detail:** In `Local` deployment mode, `SafetyRules` lives behind `Arc<RwLock<SafetyRules>>` in the same process as consensus. Any memory corruption, buffer overflow, or RCE in the consensus module gives direct access to the signing key in memory. The `Thread` and `Process` modes exist but are not the default.

**Severity: MEDIUM** — Reduces defense-in-depth. The `#![forbid(unsafe_code)]` mitigates but does not eliminate this risk (dependencies may contain unsafe code).

### MEDIUM-03: Write-Through Cache Has No Cross-Process Guard

**Location:** `persistent_safety_storage.rs:27`

**Detail:** The `cached_safety_data` is process-local with no file lock or fencing token. If two processes (e.g., misconfigured deployment, zombie process) share the same storage file, they will have independent caches and can independently advance `last_voted_round`, resulting in conflicting votes.

**Severity: MEDIUM** — Requires operational misconfiguration, but the system provides no defense against it (no PID file, no flock, no fencing).

---

## Summary

| ID | Severity | Finding | Exploitable? |
|---|---|---|---|
| CRITICAL-01 | **Critical** | `preferred_round` not persisted after proposal signing — equivocation after crash | Yes, by Byzantine leader + crash |
| CRITICAL-02 | **Critical** | No verified crash-safety (fsync/atomic write) — double-sign after power loss | Yes, by power loss timing |
| HIGH-01 | High | Idempotency check on round only, not block hash — suppresses Byzantine detection | Detection gap |
| HIGH-02 | High | Epoch transition zeros all safety state — single-point-of-failure on epoch check | Conditional |
| HIGH-03 | High | Prometheus counters updated before durable write — misleads operators | Operational risk |
| MEDIUM-01 | Medium | Key lookup fallback leaks rotation timing | Information leak |
| MEDIUM-02 | Medium | Default `Local` mode has no process isolation for signing keys | Reduces defense-in-depth |
| MEDIUM-03 | Medium | No cross-process fencing on storage file | Requires misconfig |

**Bottom line:** The two critical findings (CRITICAL-01 and CRITICAL-02) represent real double-signing/equivocation vectors that are exploitable under crash conditions. For a BFT validator where slashing penalties exist, these are not theoretical — they are the exact class of bug that causes fund loss. The `preferred_round` persistence skip (CRITICAL-01) is the most concerning because it is an **intentional design decision** that trades correctness for latency, documented in a code comment.

---

## [aptos_consensus_types_and_voting] Review aptos-core/consensus/consensus-types/ for vote and quorum certificate val

### 🎯 Intention

Review aptos-core/consensus/consensus-types/ for vote and quorum certificate validation correctness, BLS signature aggregation safety, block header integrity checks, and any gravity-sdk modifications to upstream types. Also review the main aptos-core/consensus/ crate for leader election, block proposal pipelines, and the consensus DB (consensusdb/) schema and data integrity.

### 🔍 Discovered Implementation

# Implementation Analysis: Consensus Types & Core Consensus

## 1. Consensus Types (`aptos-core/consensus/consensus-types/src/`)

### Directory Structure

```
consensus-types/src/
├── lib.rs                      # 26 public modules
├── block.rs                    # Block struct with gravity block_number extension
├── block_data.rs               # BlockData + BlockType enum
├── block_retrieval.rs          # Block retrieval protocol messages
├── block_test.rs / block_test_utils.rs
├── common.rs                   # Shared type aliases (Round, Author, etc.)
├── delayed_qc_msg.rs           # Delayed QC messaging
├── epoch_retrieval.rs          # Epoch retrieval protocol
├── order_vote.rs               # Order votes (pipelined execution)
├── order_vote_msg.rs / order_vote_proposal.rs
├── payload.rs                  # Transaction payload types
├── pipeline/                   # Commit decision & commit vote types
│   ├── mod.rs
│   ├── commit_decision.rs
│   └── commit_vote.rs
├── pipelined_block.rs          # PipelinedBlock wrapper
├── proof_of_store.rs           # Quorum store proofs
├── proposal_ext.rs / proposal_msg.rs
├── quorum_cert.rs              # QuorumCert with gravity merge extensions
├── randomness.rs               # On-chain randomness types
├── safety_data.rs              # Persisted safety rules state
├── sync_info.rs                # Sync protocol info
├── timeout_2chain.rs           # 2-chain timeout protocol
├── vote.rs                     # Vote struct with BLS signature
├── vote_data.rs                # VoteData (proposed + parent block IDs)
├── vote_msg.rs / vote_proposal.rs
└── wrapped_ledger_info.rs      # Gravity-specific WrappedLedgerInfo type
```

---

### Key Structures & Execution Paths

#### `Vote` (`vote.rs`)

```rust
pub struct Vote {
    vote_data: VoteData,
    author: Author,
    ledger_info: LedgerInfo,
    signature: bls12381::Signature,
    two_chain_timeout: Option<(TwoChainTimeout, bls12381::Signature)>,
}
```

**Construction path (`Vote::new`)**:
1. Accepts `vote_data`, `author`, `ledger_info`, and a `ValidatorSigner`
2. Signs the `LedgerInfo` with BLS12-381 via `validator_signer.sign(&ledger_info)`
3. If a `TwoChainTimeout` is provided, signs it separately with a second BLS signature
4. Returns the constructed `Vote`

**Verification path (`Vote::verify`)**:
1. Computes `ledger_info.consensus_data_hash()` and compares it against `vote_data.hash()`
2. If these do not match, returns `VoteError` — this binds the vote to the specific block data it references
3. Delegates BLS signature verification to the `ValidatorVerifier` (quorum-aware verifier)
4. If `two_chain_timeout` is present, verifies the timeout signature separately

**State changes**: None — `Vote` is a read-only message type.

---

#### `VoteData` (`vote_data.rs`)

```rust
pub struct VoteData {
    proposed: BlockInfo,   // the block being voted on
    parent: BlockInfo,     // the parent of the proposed block
}
```

- Implements `CryptoHash` — the hash covers both `proposed` and `parent` `BlockInfo` values
- The hash of `VoteData` is what gets embedded in `LedgerInfo.consensus_data_hash`, creating the binding between vote, block, and ledger state

---

#### `QuorumCert` (`quorum_cert.rs`)

```rust
pub struct QuorumCert {
    vote_data: VoteData,
    signed_ledger_info: LedgerInfoWithSignatures,
}
```

**Verification path (`QuorumCert::verify`)**:
1. If `vote_data.proposed().round() == 0`, skips verification (genesis block)
2. Otherwise, calls `signed_ledger_info.verify_signatures(validator)` — this checks the `AggregateSignature` against the validator set's BLS public keys and verifies a quorum (≥2f+1 voting power)
3. Then checks `signed_ledger_info.ledger_info().consensus_data_hash() == vote_data.hash()` to ensure the aggregate signature covers the correct vote data

**BLS Aggregation**: The `LedgerInfoWithSignatures` type (from `gaptos::aptos_types`) carries an `AggregateSignature` — a BLS12-381 aggregate of individual validator signatures. Quorum verification checks that the aggregate covers sufficient voting power per the `ValidatorVerifier`.

**Gravity-SDK additions** (3 methods):
| Method | Description |
|---|---|
| `create_merged_with_executed_state(li)` | Merges this QC with an executed `LedgerInfoWithSignatures`, checks consistency via `match_ordered_only()` |
| `create_merged_with_executed_state_without_checked(li)` | Same merge, no consistency check |
| `into_wrapped_ledger_info()` | Converts to `WrappedLedgerInfo` (gravity-sdk container that separates ordered vs. executed state) |

---

#### `Block` (`block.rs`)

```rust
pub struct Block {
    id: HashValue,
    block_data: BlockData,
    signature: Option<bls12381::Signature>,
    block_number: OnceCell<u64>,   // GRAVITY-SDK ADDITION
}
```

**Block ID computation**: `id = block_data.hash()` — the block ID is the cryptographic hash of the `BlockData` contents.

**Signature**: The proposer's BLS signature over `block_data`. `None` for genesis and nil blocks. Verified by checking the signature against the proposer's BLS public key from the validator set.

**Gravity `block_number` extension**:
- Uses `OnceCell<u64>` — set-once semantics, can only be written once after construction
- Custom `Serialize`/`Deserialize` implementations persist `block_number` as `Option<&u64>`
- `set_block_number(&self, block_number: u64)` writes to the `OnceCell`
- `make_genesis_block_from_ledger_info` initializes via `ledger_info.block_number()` — a gravity-extended method on `LedgerInfo`
- Construction methods (`new_nil`, `new_proposal`, `new_for_dag`, etc.) all initialize `block_number: OnceCell::new()` (empty/unset)

---

#### `BlockData` (`block_data.rs`)

```rust
pub struct BlockData {
    epoch: u64,
    round: Round,
    timestamp_usecs: u64,
    quorum_cert: QuorumCert,
    block_type: BlockType,
}
```

**`BlockType` enum**:
| Variant | Fields | Description |
|---|---|---|
| `Proposal` | `payload`, `author`, `failed_authors` | Standard block proposal with transactions |
| `NilBlock` | `failed_authors` | Empty block for round advancement |
| `Genesis` | (none) | Genesis block marker |
| `ProposalExt` | `ProposalExt` | Extended proposal (validator transactions) |
| `DAGBlock` | `author`, `failed_authors`, `validator_txns`, `payload`, `node_digests`, `parent_block_id`, `parents_bitvec` | DAG consensus block — `#[serde(skip_deserializing)]` prevents network receipt |

- `BlockData` derives `CryptoHasher` and `BCSCryptoHash` — its hash is the `Block.id`
- `is_nil_block()` returns true if `BlockType::NilBlock`
- `new_genesis_from_ledger_info` constructs genesis `BlockData` with `epoch`, `round=0`, `timestamp` from the ledger info

---

#### `timeout_2chain.rs`

```rust
pub struct TwoChainTimeout {
    epoch: u64,
    round: Round,
    quorum_cert: QuorumCert,
}

pub struct TwoChainTimeoutWithPartialSignatures {
    timeout: TwoChainTimeout,
    signers: BitVec,
    sig: Option<bls12381::Signature>,  // partial BLS aggregate
}

pub struct TwoChainTimeoutCertificate {
    timeout: TwoChainTimeout,
    signatures_with_rounds: AggregateSignatureWithRounds,
}
```

- The 2-chain timeout protocol allows validators to advance rounds without a QC
- `TwoChainTimeout::verify` checks the embedded `quorum_cert` via `qc.verify(validator)`
- `TwoChainTimeoutCertificate::verify` confirms the timeout QC, then verifies the aggregate signature over `(round, author)` pairs covers a quorum

---

#### `safety_data.rs`

```rust
pub struct SafetyData {
    pub epoch: u64,
    pub last_voted_round: Round,
    pub preferred_round: Round,
    pub last_vote: Option<Vote>,
    pub one_chain_round: Round,
}
```

- Persisted state for safety rules — prevents equivocation by tracking the last voted round
- `last_voted_round` ensures monotonically increasing vote rounds
- `preferred_round` enforces the locking mechanism (must not vote for a block unless its QC round ≥ `preferred_round`)

---

### Files/Contracts Involved

| File | Role |
|---|---|
| `vote.rs` | Individual validator vote with BLS signature |
| `vote_data.rs` | Hash-binding between proposed block and parent |
| `vote_msg.rs` | Vote message wrapper for network transport |
| `vote_proposal.rs` | Proposal sent to safety rules for voting |
| `quorum_cert.rs` | Aggregated quorum certificate (≥2f+1) |
| `block.rs` | Block with ID, proposer signature, and gravity block_number |
| `block_data.rs` | Block content (epoch, round, QC, payload type) |
| `timeout_2chain.rs` | Timeout certificates for round advancement |
| `safety_data.rs` | Persisted safety state (equivocation prevention) |
| `sync_info.rs` | Synchronization protocol messages |
| `order_vote.rs` | Order votes for pipelined execution |
| `pipeline/commit_decision.rs` | Commit decision messages |
| `pipeline/commit_vote.rs` | Commit vote messages |
| `wrapped_ledger_info.rs` | Gravity-SDK type separating ordered/executed state |
| `pipelined_block.rs` | Block wrapper for pipeline execution |
| `payload.rs` | Transaction payload types |
| `proof_of_store.rs` | Quorum store batch proof |

---

### External Dependencies

All crypto and type imports flow through the `gaptos::` re-export shim:
- `gaptos::aptos_crypto::bls12381` — BLS12-381 signature primitives
- `gaptos::aptos_crypto::hash::{CryptoHash, HashValue}` — cryptographic hashing
- `gaptos::aptos_types::aggregate_signature::AggregateSignature` — BLS aggregate signatures
- `gaptos::aptos_types::ledger_info::{LedgerInfo, LedgerInfoWithSignatures}` — ledger state commitments
- `gaptos::aptos_types::validator_verifier::ValidatorVerifier` — quorum-based signature verification
- `gaptos::aptos_types::validator_signer::ValidatorSigner` — validator key management

---

### Gravity-SDK Modifications Summary

| Location | Change | Effect |
|---|---|---|
| `block.rs` — `Block` struct | Added `block_number: OnceCell<u64>` | Tracks Ethereum-style sequential block number alongside consensus round |
| `block.rs` — `Serialize`/`Deserialize` | Custom serde for `block_number` | Persists block number through serialization; backward-compatible (Optional) |
| `block.rs` — `set_block_number` | New method | Allows one-time write of block number post-construction |
| `block.rs` — `make_genesis_block_from_ledger_info` | Reads `ledger_info.block_number()` | Seeds genesis block number from extended LedgerInfo |
| `quorum_cert.rs` — 3 new methods | `create_merged_with_executed_state`, `_without_checked`, `into_wrapped_ledger_info` | Supports ordered/executed state separation in gravity's pipelined consensus |
| All files — import prefix | `gaptos::` instead of direct `aptos_*` | All Aptos crate imports go through gravity's re-export shim |
| `wrapped_ledger_info.rs` | New gravity-SDK type | Container separating ordered vs. executed `LedgerInfoWithSignatures` |

---

### State Changes Summary

The consensus-types crate is a **data type library** — it defines structures and their validation logic. No direct storage mutations occur within this crate. State flows:

1. **Vote creation**: `ValidatorSigner` signs `LedgerInfo` → BLS signature stored in `Vote.signature`
2. **QC formation**: Individual `Vote` signatures are aggregated externally into `AggregateSignature` → stored in `QuorumCert.signed_ledger_info`
3. **Block construction**: `BlockData` is hashed to produce `Block.id`; proposer signs `BlockData` → stored in `Block.signature`
4. **Safety data**: `SafetyData.last_voted_round` and `preferred_round` are updated by the safety rules module (not this crate) to prevent equivocation
5. **Block number** (gravity): Written once via `set_block_number()` into `OnceCell`; read from `LedgerInfo` for genesis blocks

### 🛠️ Ground Review

# Ground Review: Consensus Types & Core Consensus — Code Quality & Engineering Safety Audit

---

## Critical Findings

### C1. `panic!` on DB write error — `ledger_metadata_db.rs:116`
```rust
Err(e) => panic!("{}", e),   // in write_schemas()
```
Any transient I/O or RocksDB write failure crashes the entire consensus node unconditionally. This should propagate `Result` to the caller for graceful degradation or controlled shutdown.

### C2. Double `.unwrap()` in network-reachable RPC path — `reader.rs:83`
```rust
self.get::<LedgerInfoSchema>(&block_number).unwrap().unwrap()
```
`get_state_proof` panics on either a DB error or a missing `LedgerInfo` for a known block number. This path is reachable from network peers — an attacker or benign inconsistency causes a full node crash.

### C3. Stale global validator set across epoch boundaries — `reader.rs:20-36`
```rust
static VALIDATOR_SET: OnceCell<ValidatorSet> = OnceCell::new();
```
Process-global `OnceCell` is initialized once and never refreshed. After an epoch change, `ConsensusDB::validator_set()` returns the stale set, causing incorrect validator verification for the entire node lifetime.

### C4. Integer divide-by-zero in proposer election — `rotating_proposer_election.rs:36`
```rust
(round / u64::from(self.contiguous_rounds)) % self.proposers.len() as u64
```
No validation that `contiguous_rounds != 0` at construction. A misconfiguration causes an unconditional panic in the leader election hot path.

### C5. `debug_assert` compiled out in release for epoch/round validation — `timeout_2chain.rs:231-240`
```rust
debug_assert_eq!(self.timeout.epoch(), timeout.epoch(), ...);
debug_assert_eq!(self.timeout.round(), timeout.round(), ...);
```
In release builds, a mismatched epoch or round is silently accepted into the timeout certificate, corrupting the aggregate signature. Must be a hard `assert!` or return `Result`.

### C6. Blocking mutex inside async context — `pipelined_block.rs:125-130`
```rust
pre_commit_fut: Arc<Mutex<Option<BoxFuture<'static, ExecutorResult<()>>>>>,
pipeline_futures: Arc<Mutex<Option<PipelineFutures>>>,
```
`aptos_infallible::Mutex` (blocking) is locked inside async code paths (`take_pre_commit_fut`, `abort_pipeline`). Under contention, this blocks Tokio executor threads and can cause cascading pipeline stalls.

### C7. No `Drop` impl — pipeline abort handles leak — `pipelined_block.rs`
If a `PipelinedBlock` is dropped without explicit `abort_pipeline()`, the `AbortHandle`s are dropped but the associated Tokio tasks continue running, holding locks and performing I/O indefinitely.

### C8. `set_block_number` panics on double-set — `block.rs:479-481`
```rust
assert!(self.block_number.set(block_number).is_ok());
```
Since `Block` derives `Clone`, a cloned block carries the already-set `OnceCell`. Calling `set_block_number` on the clone panics. Concurrent callers also race to a non-deterministic panic.

### C9. Unresolved correctness TODO in security-critical `verify()` — `wrapped_ledger_info.rs:89-90`
```rust
// TODO: Earlier, we were comparing self.certified_block().round() to 0. Now, we are
// comparing self.ledger_info().ledger_info().round() to 0. Is this okay?
```
An open question about the correctness of the genesis round check in the verification function of a consensus-critical type. Must be resolved before production use.

---

## Warning Findings

### W1. `expect()` panics in `certificate_for_genesis_from_ledger_info` — `quorum_cert.rs:88,102`
Two bare `.expect()` calls: one on `checked_add(1)` (epoch overflow) and one on `.next_epoch_state()` (missing epoch state). These are constructor-time panics on a consensus-critical object. Should return `Result`.

### W2. `failed_authors_to_indices` panics on missing author — `block.rs:468-475`
```rust
validators.iter().position(|&v| v == *failed_author).unwrap_or_else(|| {
    panic!("Failed author {} not in validator list {:?}", ...)
})
```
Public method reachable from `new_block_metadata`. An invalid `failed_author` crashes the process.

### W3. Narrowing `usize as u16` cast without bounds check — `quorum_cert.rs:110`
```rust
validator_set_size as u16
```
Silent truncation if validator set exceeds 65,535 members. Should use `u16::try_from(...)?`.

### W4. `take_pre_commit_fut` panics on second invocation — `pipelined_block.rs:254`
```rust
self.pre_commit_fut.lock().take().expect("pre_commit_result_rx missing.")
```
No guard against double-call. Second invocation panics.

### W5. `set_randomness` panics on second call — `pipelined_block.rs:246`
```rust
assert!(self.randomness.set(randomness.clone()).is_ok());
```
Should return `Result` or log a warning instead of crashing.

### W6. All `SafetyData` fields are `pub` — `safety_data.rs:12-22`
Unrestricted mutation on a safety-critical struct that governs equivocation prevention. Any caller can set `last_voted_round` to 0, bypassing safety rules.

### W7. Silent duplicate signature drop — `timeout_2chain.rs:308`
```rust
self.signatures.entry(validator).or_insert((round, signature));
```
A validator's second signature is silently ignored with no logging or error return.

### W8. Non-atomic block number writes — `consensusdb/mod.rs:232-244`
`save_block_numbers` runs as a separate batch from `save_blocks_and_quorum_certificates`. A crash between the two leaves blocks in the DB without block number mappings.

### W9. `block_number` in wire format but not in block `id` hash — `block.rs`
Two `Block` values differing only in `block_number` produce the same `id`. A relay or cache keying on `id` could serve a block with an incorrect `block_number`.

### W10. `verify()` does not check `vote_data` hash — `wrapped_ledger_info.rs`
When order votes are enabled, `verify()` skips the `consensus_data_hash` check with no runtime guard confirming order votes are actually enabled. Inconsistent with `certified_block()` and `into_quorum_cert()` which do enforce this.

### W11. Env var parse panics — `reader.rs:43,47`
```rust
std::env::var("FIXED_PROPOSER").map(|s| s.parse().unwrap()).unwrap_or(true)
```
Setting `FIXED_PROPOSER=yes` (instead of `true`/`false`) panics the node at startup.

### W12. `zip` truncation risk — `timeout_2chain.rs:361-366`
`get_signers_addresses` and `self.rounds` are zipped without a length assertion on the read path. The constructor guards this, but deserialized instances bypass the constructor.

### W13. Deserialization silently discards `state_compute_result` — `pipelined_block.rs:176`
Deserialized execution results are thrown away and replaced with a dummy without any log or comment at the replacement site.

---

## Info Findings

### I1. Verification order inefficiency — `vote.rs:153`
`vote_data().verify()` (cheap structural check) is called *after* BLS signature verification (expensive). Reordering would provide a faster fast-fail path.

### I2. Magic value sentinels unnamed — `vote_data.rs:78`, `quorum_cert.rs:124`
`version == 0` and `round == 0` are hardcoded sentinels for decoupled execution and genesis respectively. Named constants would improve readability.

### I3. Naming inconsistencies
| Location | Issue |
|---|---|
| `quorum_cert.rs` | `create_merged_with_executed_state_without_checked` — unidiomatic; prefer `_unchecked` |
| `vote.rs` | `generate_2chain_timeout` doesn't mutate the vote; misleading name |
| `block_data.rs:147` | `dag_nodes` accessor vs `node_digests` field vs `nodes_digests` local variable |
| `block.rs:118` | `payload_size` returns a count, not byte size |
| `safety_data.rs` | `preferred_round` vs `one_chain_round` naming asymmetry |

### I4. Non-English comments — `pipelined_block.rs:367-368`
Chinese comments mixed into an English codebase create a maintenance barrier for international contributors.

### I5. `&Vec<T>` in public APIs — `timeout_2chain.rs`, `pipelined_block.rs`
Idiomatic Rust prefers `&[T]` over `&Vec<T>` for public accessor return types.

### I6. Commented-out test assertions — `consensusdb_test.rs:41-53`
`test_put_get` no longer asserts the full read-after-write round-trip, leaving a significant test coverage gap for the consensus DB.

### I7. Dead constant — `consensusdb/mod.rs:52`
`RECENT_BLOCKS_RANGE: u64 = 256` is defined but never referenced.

### I8. Hardcoded test path — `ledger_metadata_db.rs:188`
Test opens DB at `/tmp/node3/data/consensus_db` instead of using `TempPath`, causing cross-run state sharing and potential test pollution.

### I9. `get_data()` returns unnamed 5-tuple — `consensusdb/mod.rs`
```rust
(Option<Vec<u8>>, Option<Vec<u8>>, Vec<Block>, Vec<QuorumCert>, bool)
```
Positional semantics are unclear. A named struct would eliminate caller confusion.

### I10. Doc comment typo — `block.rs:53`, `block_data.rs:71`
Both contain `"QuorurmCertificate"` (double `r`).

---

## Summary by Severity

| Severity | Count | Key Themes |
|---|---|---|
| **Critical** | 9 | Panics on DB/IO errors in hot paths, stale global state across epochs, blocking mutex in async, validation compiled out in release, unresolved correctness TODO |
| **Warning** | 13 | Constructor panics, unsafe casts, non-atomic DB writes, identity/hash inconsistency, public fields on safety-critical structs |
| **Info** | 10 | Naming inconsistencies, dead code, test gaps, non-English comments, minor API ergonomics |

---

## Highest-Priority Remediation Targets

1. **C1–C2**: Replace `panic!`/`.unwrap()` on DB operations with `Result` propagation — a single DB hiccup should not crash the node
2. **C3**: Replace `OnceCell<ValidatorSet>` with an epoch-aware refresh mechanism — stale validator sets invalidate all verification after an epoch change
3. **C5**: Promote `debug_assert` to hard `assert!` for timeout epoch/round validation — release builds currently accept corrupted timeout certificates
4. **C6–C7**: Implement `Drop` for `PipelinedBlock` and migrate to `tokio::sync::Mutex` — blocking mutexes in async and orphaned tasks are a pipeline reliability hazard
5. **C9**: Resolve the open TODO in `WrappedLedgerInfo::verify()` — an open correctness question in a verification function is unacceptable for production consensus

### ⚔️ Intention Attacker

# Security Audit Report: Consensus Types & Core Consensus

## Intention vs. Implementation Attack Surface Analysis

---

## CRITICAL Findings

### C1: `WrappedLedgerInfo::verify` Genesis Bypass Uses Different Field Than `QuorumCert::verify`

**File:** `wrapped_ledger_info.rs:91` vs `quorum_cert.rs:124`

`QuorumCert::verify` checks `self.certified_block().round() == 0` (the vote data's proposed block round), while `WrappedLedgerInfo::verify` checks `self.ledger_info().ledger_info().round() == 0` (the ledger info's round). The code itself contains a TODO acknowledging this inconsistency:

```rust
// TODO: Earlier, we were comparing self.certified_block().round() to 0. Now, we are
// comparing self.ledger_info().ledger_info().round() to 0. Is this okay?
```

**Impact:** These are semantically different fields. The `LedgerInfo` round and the certified block round can diverge (especially in pipelined execution where ordering and execution are decoupled). An attacker who can craft a `WrappedLedgerInfo` where `ledger_info.round() == 0` but the actual certified block is non-genesis could **bypass all signature verification**. The only guard is that `get_num_voters() == 0`, but the attacker controls the `AggregateSignature` bitmap — an empty signature set trivially passes this check. This could allow a forged genesis-like `WrappedLedgerInfo` to be accepted without any quorum proof.

**Severity: CRITICAL**

---

### C2: `create_merged_with_executed_state_without_checked` — Unchecked State Merge (Latent Bomb)

**File:** `quorum_cert.rs:159-164`

```rust
pub fn create_merged_with_executed_state_without_checked(
    &self, executed_ledger_info: LedgerInfoWithSignatures,
) -> QuorumCert {
    Self::new(self.vote_data.clone(), executed_ledger_info)
}
```

This method replaces the `signed_ledger_info` inside a `QuorumCert` with an arbitrary `LedgerInfoWithSignatures` **without any validation** that the new ledger info matches the original vote data. This breaks the fundamental binding between what was voted on and what is being committed.

**Current status:** Zero callers found — this is dead code. However, it is `pub` (public API), not `#[cfg(test)]`-gated, and not marked deprecated. Any future caller could silently bypass the `match_ordered_only` consistency check that the checked variant enforces, enabling a state commitment that was never actually voted on by the quorum.

**Severity: CRITICAL (latent — no current exploitation path, but public API surface)**

---

### C3: Non-Atomic Block + Block-Number Persistence Creates Crash-Recovery Inconsistency

**File:** `consensusdb/mod.rs:214-244`

`save_blocks_and_quorum_certificates` and `save_block_numbers` are **two separate RocksDB write batches**. A crash between the two writes leaves blocks persisted without their block-number mapping in `BlockNumberSchema`.

On recovery, `get_data()` builds a `block_number_to_block_id` map from `BlockNumberSchema` and uses it to find the root block. If the block-number mapping is missing, the root block cannot be located, causing the recovery logic to fall back to `(epoch, 0, HashValue::zero())` — **losing the actual recovery root**.

Additionally, `get_data()` line 157-158 does `.unwrap()` on the root block lookup:
```rust
let block = self.get::<BlockSchema>(&(epoch, block_number_to_block_id[&latest_block_number]))?.unwrap();
```

If `BlockNumberSchema` references a block that was pruned from `BlockSchema` (since `BlockNumberSchema` entries are **never deleted** — see H2 below), this panics the node on startup, rendering it unable to recover.

**Severity: CRITICAL**

---

## HIGH Findings

### H1: SafetyData Cache Can Diverge from Persistent Store

**File:** `persistent_safety_storage.rs:129-142`

When `enable_cached_safety_data = true`, `safety_data()` returns the in-memory cache without ever re-validating against the backing store. If the backing store is modified externally (snapshot restore, another process, manual intervention), the cache will serve **stale `last_voted_round`** values.

**Impact:** If the restored snapshot has a higher `last_voted_round` than the cache, the node could sign a vote for a round it already voted for — violating BFT safety guarantees (equivocation). If the restored snapshot has a lower `last_voted_round`, the node loses votes it already cast, which is less dangerous but still incorrect.

**Severity: HIGH**

---

### H2: `BlockNumberSchema` Entries Never Pruned on Block Deletion

**File:** `consensusdb/mod.rs:246-258`

`delete_blocks_and_quorum_certificates` removes entries from `BlockSchema` and `QCSchema` but **never removes** corresponding `BlockNumberSchema` entries. This causes:
1. Unbounded growth of the block-number index over the node's lifetime
2. Dangling references to non-existent blocks that can trigger panics during recovery (feeds into C3)

**Severity: HIGH**

---

### H3: `LedgerMetadataDb::write_schemas` Panics on DB Error

**File:** `consensusdb/ledger_db/ledger_metadata_db.rs:116`

```rust
Err(e) => panic!("{}", e),
```

Any RocksDB write failure (disk full, corruption, I/O error) causes an **unrecoverable panic** instead of returning an error. This prevents graceful degradation, retry logic, or clean shutdown. In a consensus system, a panic during ledger metadata write can leave the DB in a partially-committed state that cascades into recovery failures.

**Severity: HIGH**

---

### H4: `DAGNode` Schema Uses `()` Key — Only One Node Storable

**File:** `consensusdb/schema/dag/mod.rs:23`

`NodeSchema` is defined with key type `()`, meaning `encode_key` produces an empty `Vec<u8>`. Every write to the `"node"` column family overwrites the single entry. If DAG consensus depends on persisting multiple nodes, this is a silent data loss bug.

**Severity: HIGH (if DAG consensus is enabled; otherwise inert)**

---

## MEDIUM Findings

### M1: `VoteData::verify` Allows `proposed.version() == 0` to Bypass Version Ordering

**File:** `vote_data.rs:77-78`

```rust
self.proposed.version() == 0 || self.parent.version() <= self.proposed.version(),
```

The comment explains this is for "decoupled execution" where versions are dummy values. However, a malicious proposer could set `proposed.version() = 0` on a non-decoupled-execution block to bypass the version monotonicity check, potentially causing state confusion downstream if version ordering is assumed elsewhere.

**Severity: MEDIUM**

---

### M2: `get_max_epoch` Silently Defaults to Epoch 1 on Iterator Error

**File:** `consensusdb/mod.rs:390-396`

```rust
let max_epoch = match iter.next() {
    Some(Ok(((epoch, _), _))) => epoch,
    _ => 1,  // silent fallback
};
```

Any RocksDB iterator error (corruption, missing CF) is swallowed and treated as "epoch 1". This masks critical storage failures and could cause the node to operate in the wrong epoch, reading/writing blocks under incorrect epoch keys.

**Severity: MEDIUM**

---

### M3: `get_state_proof` Double-Unwrap Without Error Propagation

**File:** `consensusdb/include/reader.rs:83`

```rust
self.get::<LedgerInfoSchema>(&block_number).unwrap().unwrap()
```

A missing or corrupted `LedgerInfoSchema` entry panics the node during state proof serving rather than returning an error. This is a denial-of-service vector if an RPC client can trigger `get_state_proof` for a block number where the ledger info was not yet persisted.

**Severity: MEDIUM**

---

### M4: `commit_callback` Panics on Merge Mismatch Instead of Graceful Error Handling

**File:** `block_storage/block_tree.rs:550`

```rust
.create_merged_with_executed_state(commit_decision)
    .expect("Inconsistent commit proof and evaluation decision, cannot commit block");
```

All three call sites of `create_merged_with_executed_state` use `.expect()` (panic on error) rather than propagating the error. In the live consensus commit path (`commit_callback`), a mismatch between ordered and executed state — even due to a transient bug — takes down the entire node. The checked variant correctly detects inconsistency, but the caller converts the graceful error into a crash.

**Severity: MEDIUM**

---

### M5: Leader Election Predictability When `use_root_hash = false`

**File:** `liveness/leader_reputation.rs`

When `use_root_hash = false`, the leader selection seed is only `epoch || round` — fully deterministic and predictable by any observer. An adversary who knows the validator set and stake weights can predict all future leaders and target them for DoS before their round arrives.

**Severity: MEDIUM** (configuration-dependent)

---

## Summary

| ID | Severity | Finding | Status |
|---|---|---|---|
| C1 | **Critical** | `WrappedLedgerInfo::verify` genesis bypass uses wrong round field, may skip all signature verification | Active |
| C2 | **Critical** | Public unchecked merge method can forge QuorumCerts binding wrong execution state | Latent (dead code, but public) |
| C3 | **Critical** | Non-atomic block/block-number persist + unwrap-on-recovery = crash loop or lost root | Active |
| H1 | High | SafetyData cache divergence enables potential equivocation after snapshot restore | Active |
| H2 | High | BlockNumberSchema never pruned — unbounded growth + dangling refs | Active |
| H3 | High | LedgerMetadataDb panics on write error, no graceful degradation | Active |
| H4 | High | DAGNode schema uses `()` key — silent single-entry overwrite | Active if DAG enabled |
| M1 | Medium | `version == 0` bypass in VoteData verification | Active |
| M2 | Medium | Silent epoch=1 fallback masks DB errors | Active |
| M3 | Medium | Double-unwrap in state proof serving = DoS-triggerable panic | Active |
| M4 | Medium | All merge callers panic on mismatch instead of error propagation | Active |
| M5 | Medium | Predictable leader election seed without root hash | Configuration-dependent |

---

**Recommendation priorities:**
1. **Immediately** resolve C1 — validate whether `ledger_info().round()` vs `certified_block().round()` is intentional, and if so, add defensive checks to prevent forged genesis bypass.
2. **Gate C2** behind `#[cfg(test)]` or delete the unchecked merge variant entirely.
3. **Merge C3** block + block-number writes into a single atomic `SchemaBatch`, and add `BlockNumberSchema` cleanup to the deletion path.
4. Audit all `.expect()`/`.unwrap()` calls in the consensus DB and commit pipeline for graceful error handling.

---

## [genesis_tool_key_material] Security audit of external/gravity_chain_core_contracts/genesis-tool/ for genesi

### 🎯 Intention

Security audit of external/gravity_chain_core_contracts/genesis-tool/ for genesis key material generation and handling, validator set bootstrapping, genesis block construction, and any hardcoded secrets or insecure key derivation. Review how genesis ceremony artifacts are produced and whether they could be tampered with.

### 🔍 Discovered Implementation

# Implementation Analysis: `genesis-tool/`

## Execution Path

### `generate` subcommand (full genesis creation)

```
main.rs::main()
  → run_generate(byte_code_dir, config_file, output)
    → fs::read_to_string(config_file) → serde_json::from_str::<GenesisConfig>()
    → execute::genesis_generate(byte_code_dir, output, &config)
      → calculate_total_stake(config)                          // sum of all validator stakeAmounts
      → deploy_bsc_style(byte_code_dir, total_stake)           // create InMemoryDB, insert 21 contracts
        → for each (name, addr) in CONTRACTS:
            read_hex_from_file("{byte_code_dir}/{name}.hex")
            extract_runtime_bytecode()                         // heuristic passthrough
            db.insert_account_info(addr, AccountInfo { code, balance })
        → db.insert_account_info(SYSTEM_CALLER, { balance: total_stake + 10M ETH })
        → db.insert_account_info(GENESIS_ADDR, { balance: total_stake + 1M ETH })
      → prepare_env(config.chain_id)                           // sets block.timestamp = now()
      → build_genesis_transactions(config)
        → GenesisTransactionBuilder::new(config)
          → call_genesis_initialize(GENESIS_ADDR, config)
            → convert_config_to_sol(config)                    // Rust config → Solidity ABI structs
            → Genesis::initializeCall { params }.abi_encode()
            → new_system_call_txn_with_value(GENESIS_ADDR, calldata, total_stake)
      → execute_revm_sequential(db, LATEST, env, &[txn], None)
        → EvmBuilder → for each tx: evm.transact() → db.commit()
        → merge_transitions(BundleRetention::Reverts) → take_bundle()
      → merge bundle_state into genesis_state HashMap
      → write bundle_state.json, genesis_accounts.json, genesis_contracts.json
    → post_genesis::verify_result(db, bundle_state, config)
      → verify_active_validators()
        → call_get_active_validators() → execute_revm_sequential()
        → print_active_validators_result()                     // ABI decode + compare to config
```

### `verify` subcommand (existing genesis.json validation)

```
main.rs::main()
  → run_verify(genesis_file)
    → verify::verify_genesis_file(genesis_file)
      → parse genesis.json alloc → build InMemoryDB
      → check ValidatorManagement exists at 0x...625F2001
      → verify_epoch_interval() → call EpochConfig.epochIntervalMicros()
      → call getActiveValidators() → ABI decode with 7-field struct
    → verify::print_verify_summary()
```

### Orchestration script (`cluster/genesis.sh`)

```
cluster/genesis.sh
  → parse genesis.toml (TOML config)
  → verify identity keys exist in output/{node_id}/config/identity.yaml
  → python3 aggregate_genesis.py → produces validator_genesis.json
  → inject faucet_alloc.json into genesis_template.json (jq merge)
  → cd gravity_chain_core_contracts && ./scripts/generate_genesis.sh --config <path>
  → gravity_cli genesis generate-waypoint --input-file=... --output-file=waypoint.txt
```

---

## Files Involved

| File | Role |
|---|---|
| `genesis-tool/src/main.rs` | CLI entry — `clap` parser, `generate` and `verify` subcommands |
| `genesis-tool/src/lib.rs` | Module root — re-exports `execute`, `utils`, `genesis`, `post_genesis`, `verify` |
| `genesis-tool/src/genesis.rs` | Config structs, Solidity ABI definitions (`sol!` macro), config→ABI conversion, address derivation |
| `genesis-tool/src/execute.rs` | BSC-style deployment, EVM execution orchestration, output file writing |
| `genesis-tool/src/utils.rs` | 21 system address constants, transaction builders, EVM execution helper, revert decoder |
| `genesis-tool/src/post_genesis.rs` | Post-generation verification (re-queries active validators from in-memory state) |
| `genesis-tool/src/verify.rs` | Standalone genesis.json file verifier (loads alloc, replays view calls) |
| `genesis-tool/config/genesis_config.json` | 4-validator config (testnet) |
| `genesis-tool/config/genesis_config_single.json` | 1-validator config with bridge + oracle task |
| `genesis-tool/config/genesis_template.json` | Reth/geth genesis template (chain ID 7771625, pre-funded accounts) |
| `cluster/genesis.sh` | End-to-end orchestration script |

---

## Key Functions

### `genesis.rs`

| Function | Signature | Description |
|---|---|---|
| `derive_account_address_from_consensus_pubkey` | `fn(consensus_pubkey: &[u8]) -> [u8; 32]` | SHA3-256 hash of BLS consensus public key to produce 32-byte account address |
| `convert_config_to_sol` | `fn(config: &GenesisConfig) -> SolGenesisInitParams` | Maps every Rust config field to its Solidity ABI counterpart. BCS-encodes network address strings. Keccak-256 hashes non-hex `taskName` strings. |
| `calculate_total_stake` | `fn(config: &GenesisConfig) -> U256` | Sum of all `validator.stake_amount` values |
| `call_genesis_initialize` | `fn(genesis_address: Address, config: &GenesisConfig) -> TxEnv` | ABI-encodes `Genesis.initialize(SolGenesisInitParams)` and creates a payable system call transaction with `value = total_stake` |
| `call_get_active_validators` | `fn() -> TxEnv` | ABI-encodes `IValidatorManagement.getActiveValidators()` view call |
| `print_active_validators_result` | `fn(result: &ExecutionResult, config: &GenesisConfig)` | Decodes return data, validates count matches config, derives account addresses from consensus pubkeys |

### `execute.rs`

| Function | Signature | Description |
|---|---|---|
| `deploy_bsc_style` | `fn(byte_code_dir: &str, total_stake: U256) -> InMemoryDB` | Creates in-memory DB, inserts bytecode for 21 contracts at fixed system addresses. Pre-funds `SYSTEM_CALLER` with `total_stake + 10M ETH` and `Genesis` contract with `total_stake + 1M ETH`. |
| `extract_runtime_bytecode` | `fn(constructor_bytecode: &str) -> Vec<u8>` | Heuristic: if first byte is `0x60`/`0x61` and length > 100, warns but passes through unchanged. Otherwise returns bytes as-is. |
| `prepare_env` | `fn(chain_id: u64) -> Env` | Sets chain ID, 30M gas limit, `block.timestamp` = current system time (seconds since epoch) |
| `genesis_generate` | `fn(byte_code_dir, output_dir, config) -> (InMemoryDB, BundleState)` | Full pipeline: deploy → execute Genesis.initialize → merge state → write 3 JSON output files |

### `utils.rs`

| Function | Signature | Description |
|---|---|---|
| `execute_revm_sequential` | `fn<DB>(db, spec_id, env, txs, pre_bundle) -> Result<(Vec<ExecutionResult>, BundleState), EVMError>` | Runs transactions sequentially through revm, commits after each, merges transitions, returns bundle |
| `new_system_call_txn` | `fn(contract: Address, input: Bytes) -> TxEnv` | Builds a call from `SYSTEM_CALLER` with `gas_limit = u64::MAX`, `gas_price = 0` |
| `new_system_call_txn_with_value` | `fn(contract, input, value) -> TxEnv` | Same as above but with ETH value attached (for payable calls) |
| `analyze_txn_result` | `fn(result: &ExecutionResult) -> String` | Decodes revert selectors (`OnlySystemCaller`, `InvalidValue`, `Error(string)`, `Panic(uint256)`, etc.) |

---

## State Changes

1. **InMemoryDB creation** (`deploy_bsc_style`):
   - 21 system contracts inserted at addresses `0x1625F0000`–`0x1625F5000` with runtime bytecode, zero nonce, zero balance (except Genesis)
   - `SYSTEM_CALLER` (`0x...1625F0000`) gets balance = `total_stake + 10M ETH`, nonce = 1
   - `Genesis` contract (`0x...1625F0001`) gets balance = `total_stake + 1M ETH`

2. **Genesis.initialize() execution** (single EVM transaction):
   - Called from `SYSTEM_CALLER` → `GENESIS_ADDR` with `msg.value = total_stake`
   - The Solidity `Genesis.initialize` internally calls all system contracts to set up: validator config, staking config, governance config, epoch config, version config, consensus config, execution config, randomness config, oracle config, JWK config, and registers each initial validator with their stake
   - All resulting storage slot changes captured in `BundleState`

3. **Post-execution merge**:
   - `SYSTEM_CALLER` entry removed from bundle state (line 206 of execute.rs)
   - Bundle state merged with genesis_state HashMap — storage from initialize() merged into pre-deployed contract entries
   - Three files written: `bundle_state.json`, `genesis_accounts.json` (full state), `genesis_contracts.json` (bytecode only)

---

## External Dependencies

### Software dependencies
- **revm** (Galxe fork, branch `v19.5.0-gravity`): EVM execution engine
- **grevm** (Galxe): Gravity parallel EVM — imported in Cargo.toml but not directly used in genesis-tool source
- **alloy-sol-macro / alloy-sol-types**: Solidity ABI encoding/decoding via `sol!` macro
- **bcs** (aptos-labs fork): Binary Canonical Serialization for encoding network address strings
- **tiny-keccak**: SHA3-256 (account address derivation) and Keccak-256 (oracle task name hashing)
- **clap**: CLI arg parsing
- **revm-primitives**: Address, U256, Bytes, hex encoding

### File system dependencies
- 21 `.hex` bytecode files read from `byte_code_dir` (one per system contract)
- Genesis config JSON read from `--config-file` argument
- `genesis_template.json` used by the orchestration script
- Identity keys (`identity.yaml`) expected in `output/{node_id}/config/` during orchestration

### External tools (orchestration)
- `forge` (Foundry) — required by `generate_genesis.sh`
- `gravity_cli genesis generate-waypoint` — generates waypoint from validator genesis
- `python3 aggregate_genesis.py` — aggregates validator identity data into genesis config

---

## System Address Map

| Range | Address | Contract |
|---|---|---|
| Consensus Engine | `0x...1625F0000` | SYSTEM_CALLER |
| | `0x...1625F0001` | Genesis |
| Runtime Configs | `0x...1625F1000` | Timestamp |
| | `0x...1625F1001` | StakingConfig |
| | `0x...1625F1002` | ValidatorConfig |
| | `0x...1625F1003` | RandomnessConfig |
| | `0x...1625F1004` | GovernanceConfig |
| | `0x...1625F1005` | EpochConfig |
| | `0x...1625F1006` | VersionConfig |
| | `0x...1625F1007` | ConsensusConfig |
| | `0x...1625F1008` | ExecutionConfig |
| | `0x...1625F1009` | OracleTaskConfig |
| | `0x...1625F100A` | OnDemandOracleTaskConfig |
| Staking & Validator | `0x...1625F2000` | Staking |
| | `0x...1625F2001` | ValidatorManagement |
| | `0x...1625F2002` | DKG |
| | `0x...1625F2003` | Reconfiguration |
| | `0x...1625F2004` | Blocker (Block) |
| | `0x...1625F2005` | PerformanceTracker |
| Governance | `0x...1625F3000` | Governance |
| Oracle | `0x...1625F4000` | NativeOracle |
| | `0x...1625F4001` | JWKManager |
| | `0x...1625F4002` | OracleRequestQueue |
| Precompiles | `0x...1625F5000` | NativeMintPrecompile |

---

## Access Control

- **All genesis transactions** use `SYSTEM_CALLER` (`0x...1625F0000`) as `msg.sender` with `gas_limit = u64::MAX` and `gas_price = 0`
- The Solidity `Genesis.initialize()` is `external payable` — the `onlySystemCaller` modifier (present on system contracts) gates who can call initialization functions; the genesis tool bypasses this by using `SYSTEM_CALLER` as the transaction origin
- **No private keys are generated or handled** by the genesis-tool itself — it only reads pre-existing consensus public keys and proof-of-possession from the JSON config
- The orchestration script (`genesis.sh`) reads identity keys from `output/{node_id}/config/identity.yaml` which are generated by a prior `make init` step via `gravity_cli`

---

## Config File Observations

### `genesis_config.json` (4-validator)
- 4 validators, each with 20,000 ETH stake and identical voting power
- `operator == owner` for all validators (same address controls both roles)
- `consensusPop` is `"0x"` (empty) for all validators
- All network addresses point to `127.0.0.1` with different TCP ports (2024–2026, 6180)
- JWK issuer: `0x68747470733a2f2f6163636f756e74732e676f6f676c652e636f6d` = ASCII "https://accounts.google.com"
- One hardcoded RSA JWK (Google's key ID `f5f4c0ae...`)
- Oracle callback address `0x...625F2018` does not match any of the 21 system contract addresses

### `genesis_config_single.json` (1-validator)
- Same structure as above with 1 validator
- Adds `bridgeConfig` with `trustedBridge: 0x3fc870008B1cc26f3614F14a726F8077227CA2c3`
- Adds an oracle task pointing to Sepolia contract `0x0f761B1B3c1aC9232C9015A7276692560aD6a05F`

### `genesis_template.json`
- Chain ID: `7771625`
- Pre-funded account `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266` — this is the default Hardhat/Anvil account #0 (derived from the well-known mnemonic `test test test test test test test test test test test junk`)
- Pre-funded account `0x...2000` with ~200 ETH
- Hardcoded timestamp `0x6490fdd2` (June 2023)
- All EIPs enabled at block/time 0 through Cancun

---

## Key Derivation Method

Account addresses are derived from BLS consensus public keys using **SHA3-256** (not Keccak-256):

```rust
fn derive_account_address_from_consensus_pubkey(consensus_pubkey: &[u8]) -> [u8; 32] {
    let mut hasher = Sha3::v256();
    hasher.update(consensus_pubkey);
    let mut output = [0u8; 32];
    hasher.finalize(&mut output);
    output
}
```

Oracle task names are hashed using **Keccak-256** when the input is a plain string (not hex-prefixed).

---

## `extract_runtime_bytecode` Heuristic

```rust
fn extract_runtime_bytecode(constructor_bytecode: &str) -> Vec<u8> {
    let bytes = hex::decode(constructor_bytecode.trim()).unwrap_or_default();
    if bytes.len() > 100 && (bytes[0] == 0x60 || bytes[0] == 0x61) {
        warn!("Using constructor bytecode as runtime bytecode");
        bytes  // returns constructor bytecode unchanged
    } else {
        bytes  // assumes already runtime bytecode
    }
}
```

When the heuristic detects constructor-like bytecode (starts with `PUSH1`/`PUSH2` and is >100 bytes), it logs a warning but still returns the bytes unchanged — it does not actually extract the runtime portion. The code comment acknowledges this: "In a real implementation, we'd execute the constructor and extract the returned bytecode."

### 🛠️ Ground Review

# Ground Review: `genesis-tool/` — Code Quality & Engineering Safety Audit

---

## Critical

### C1. `extract_runtime_bytecode` silently deploys wrong bytecode
**File:** `execute.rs:74-89`

The function detects constructor bytecode (starts with `PUSH1`/`PUSH2`, length > 100) but **returns it unchanged** with only a `warn!()` log. This means genesis contracts could be deployed with constructor bytecode instead of runtime bytecode, producing contracts that are **non-functional or behave unpredictably** post-genesis. If `.hex` files ever ship with constructor bytecode, the entire chain starts with broken system contracts and there is no runtime error — just silent corruption.

```rust
warn!("   [!] Warning: Using constructor bytecode as runtime bytecode");
bytes // ← returns constructor bytecode unchanged
```

**Recommendation:** Either execute the constructor and extract the returned runtime bytecode, or **fail hard** (`panic!`) instead of silently deploying known-wrong bytecode.

---

### C2. `hex::decode().unwrap_or_default()` silently produces empty contracts
**File:** `execute.rs:75`

```rust
let bytes = hex::decode(constructor_bytecode.trim()).unwrap_or_default();
```

If any `.hex` file contains invalid hex data, this silently produces an **empty byte vector** — deploying a contract with no code at a system address. This is a data-loss-equivalent scenario for a genesis ceremony: no error is raised, the contract simply doesn't exist, and downstream calls to that address will succeed vacuously or fail with inscrutable errors.

**Recommendation:** Replace with `.expect()` or propagate the error with `?`.

---

### C3. Hardcoded well-known private key in `genesis_template.json` (pre-funded account)
**File:** `config/genesis_template.json` (noted in implementation report)

The address `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266` is the default Hardhat/Anvil Account #0, derived from the **publicly known mnemonic** `test test test test test test test test test test test junk`. This account is pre-funded in the genesis template. If this template is used in any non-local deployment, those funds are immediately drainable by anyone.

**Recommendation:** Add a CI check or config validation step that rejects known test-mnemonic addresses in non-devnet configs. At minimum, add a prominent comment/lint.

---

## Warning

### W1. Pervasive use of `panic!()` / `.unwrap()` / `.expect()` instead of error propagation
**Files:** `execute.rs:150-161, 209, 239, 257`, `genesis.rs:341-368`, `utils.rs:279, 293`

The genesis tool uses `panic!` and `.unwrap()` extensively throughout core logic:
- `execute.rs:157` — panics on EVM execution error
- `execute.rs:170` — panics on transaction failure  
- `execute.rs:209, 239, 257` — `.unwrap()` on file creation (no path context in error)
- `genesis.rs:343, 348, 353, 361, 367` — `.expect()` with `format!()` (allocates before panic)
- `utils.rs:293` — `.expect()` on file read

For a **genesis ceremony tool**, a crash mid-execution can leave partial output files on disk. A subsequent run may silently pick up stale artifacts.

**Recommendation:** Propagate errors with `anyhow::Result` and clean up partial outputs on failure (or write to a temp directory and atomically rename on success).

---

### W2. Double bytecode reads — redundant file I/O
**File:** `execute.rs:31-67` (first read in `deploy_bsc_style`) and `execute.rs:183-197` (second read in `genesis_generate`)

Every `.hex` file is read from disk **twice**: once during `deploy_bsc_style()` to populate the in-memory DB, and again in `genesis_generate()` to populate `genesis_state`. This is a correctness concern, not just performance: if files change between reads (unlikely but possible in scripted pipelines), the in-memory DB state and the written genesis state could **diverge**.

**Recommendation:** Read bytecode once and pass it through, or cache it in a `HashMap`.

---

### W3. `db.clone()` copies entire in-memory database unnecessarily
**File:** `execute.rs:150`

```rust
let r = execute_revm_sequential(db.clone(), SpecId::LATEST, env.clone(), &txs, None);
```

`InMemoryDB` is cloned before execution, but only the original `db` reference is returned in the `ret` tuple (line 164). The purpose appears to be preserving the pre-execution state for the return value, but the cloned database (with all 21 contracts + balances) is passed by value into `execute_revm_sequential` where it's wrapped in a `State` builder and consumed. This is a significant memory overhead for large genesis configs and could be avoided by restructuring the ownership.

---

### W4. Non-deterministic genesis — `block.timestamp` uses wall clock
**File:** `execute.rs:97-102`

```rust
env.block.timestamp = U256::from(
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs(),
);
```

Genesis output depends on the **current system time**. Two runs of the same config at different times produce different genesis states (any Solidity logic using `block.timestamp`, e.g. `lockedUntil` calculations). This makes genesis generation **non-reproducible** and complicates multi-party genesis ceremonies where independent parties need to generate identical genesis files.

**Recommendation:** Accept an explicit `--timestamp` CLI parameter (with a sensible default) and document that all ceremony participants must use the same value.

---

### W5. `SYSTEM_CALLER` state silently removed from bundle
**File:** `execute.rs:206`

```rust
bundle_state.state.remove(&SYSTEM_CALLER);
```

The `SYSTEM_CALLER` account modifications (nonce changes, balance decreases from the payable call) are silently stripped from the output bundle. While this is intentional (the system caller shouldn't appear in genesis alloc), it means the **returned `(db, bundle_state)` tuple is inconsistent** — `db` still contains `SYSTEM_CALLER` changes but `bundle_state` does not. The `post_genesis::verify_result` function receives this inconsistent pair.

---

### W6. Sensitive config data logged at INFO level
**Files:** `genesis.rs:539-568`, `utils.rs:225-233`

Consensus public keys, validator addresses, stake amounts, bridge config, oracle endpoints, function selectors, and full EVM state diffs are all logged at `INFO` level. In production genesis ceremonies, log files may be retained or shared, leaking operational infrastructure details.

**Recommendation:** Move sensitive details to `DEBUG` level. Keep only high-level progress at `INFO`.

---

### W7. No validation of `consensusPop` (Proof of Possession)
**File:** `genesis.rs:502`

```rust
consensusPop: parse_hex_bytes(&v.consensus_pop).into(),
```

The test configs show `consensusPop: "0x"` (empty), and the tool passes this through without validation. A missing or invalid PoP means the genesis tool cannot verify that the entity submitting a consensus public key actually possesses the corresponding private key — a foundational check for preventing rogue-key attacks in BLS-based validator sets.

**Recommendation:** Either validate PoP cryptographically before encoding, or at minimum warn when PoP is empty/missing.

---

## Info

### I1. `RSA_JWK_Json` violates Rust naming conventions
**File:** `genesis.rs:200`

The struct name uses `SCREAMING_SNAKE_CASE` mixed with `PascalCase` (`RSA_JWK_Json`). Rust convention is `RsaJwkJson`. The same applies to `SolRSA_JWK` in the sol! macro (line 293), though that may be constrained by ABI matching.

---

### I2. Unused constant `SYSTEM_ACCOUNT_INFO`
**File:** `utils.rs:134-139`

`SYSTEM_ACCOUNT_INFO` is defined with a hardcoded 1 ETH balance and nonce 1, but it is never used anywhere in the genesis-tool source. The actual system caller setup in `execute.rs:24-29` uses a dynamically calculated balance.

---

### I3. Unused import `std::u64`
**File:** `utils.rs:11`

```rust
use std::u64;
```

This import is unnecessary — `u64` is a primitive type available without import. `u64::MAX` is used in `new_system_call_txn` but doesn't require this import.

---

### I4. Unused function `new_system_create_txn`
**File:** `utils.rs:278-290`

This function constructs a `CREATE` transaction but is never called anywhere in the genesis-tool. It may be dead code from a prior implementation approach.

---

### I5. `NativeMintPrecompile` deployed as a contract but is a precompile
**File:** `utils.rs:103, 131`

The precompile address `0x...1625F5000` is listed in `CONTRACTS` and gets regular bytecode deployed to it. Precompiles typically have their logic handled by the EVM host, not by deployed bytecode. Deploying bytecode at a precompile address may cause confusing behavior if the EVM checks for precompile addresses before executing stored code.

---

### I6. Thread sleep for log flushing is fragile
**File:** `main.rs:26, 34, 124`

Multiple `thread::sleep()` calls (500ms, 1000ms, 1200ms) are used to "ensure" log flushing. The `tracing_appender::non_blocking::WorkerGuard` already guarantees flush on drop — the sleeps are redundant and add unnecessary latency.

---

## Summary

| Severity | Count | Key Themes |
|----------|-------|------------|
| **Critical** | 3 | Silent wrong-bytecode deployment, silent empty-contract on bad hex, pre-funded test-mnemonic account |
| **Warning** | 7 | Non-deterministic timestamps, pervasive panics without cleanup, PoP not validated, sensitive data in logs |
| **Info** | 6 | Dead code, naming conventions, redundant sleeps |

The most impactful finding is **C1+C2**: the bytecode extraction pipeline can silently produce a broken genesis with no error signal. For a genesis ceremony tool — whose output bootstraps an entire blockchain — this class of silent failure is the highest-risk code hygiene issue.

### ⚔️ Intention Attacker

# Intention Attack Report: `genesis-tool/` Security Audit

## Attack Surface: Genesis Ceremony Artifact Production & Tampering

The genesis-tool is responsible for bootstrapping the entire chain's initial state — validators, staking, governance, and system contracts. A flaw here compromises the chain from block 0.

---

## CRITICAL

### 1. `extract_runtime_bytecode` Deploys Constructor Initcode as Runtime Bytecode

**File:** `execute.rs:72-89`

The function is named "extract runtime bytecode" but **never actually extracts it**. When it detects constructor-style bytecode (leading `0x60`/`0x61`, length > 100), it logs a warning and returns the bytes unchanged:

```rust
warn!("Using constructor bytecode as runtime bytecode");
bytes  // returns constructor bytecode, NOT runtime bytecode
```

**Attack Vector:** All 21 system contracts in genesis may contain deployment initcode instead of executable contract logic. The EVM will interpret constructor preamble opcodes as contract logic, producing **completely undefined behavior** at every system contract address. A malicious actor who controls the `.hex` files can craft initcode that *appears* to deploy correctly in the genesis simulation but contains a backdoor in the actual runtime portion that is never inspected.

**Compounding factor:** `hex::decode(...).unwrap_or_default()` silently returns an empty `Vec` on decode failure — a missing or corrupted `.hex` file produces an empty-bytecode contract with **zero error signal**.

---

## HIGH

### 2. Genesis Contract Retains ~1,000,000 Phantom ETH

**File:** `execute.rs:23-45, 206`

The Genesis contract is pre-funded with `total_stake + 1,000,000 ETH` to facilitate the simulation. After execution, only `SYSTEM_CALLER` is removed from the bundle state:

```rust
bundle_state.state.remove(&SYSTEM_CALLER);
// Genesis contract's 1M ETH buffer is NEVER cleaned up
```

**Attack Vector:** The Genesis contract starts the live chain holding ~1M ETH of phantom supply that was never backed by any deposit. If the Genesis contract has any withdrawal, `selfdestruct`, or delegate-callable path, this phantom ETH becomes extractable — inflating real token supply. Even without an explicit withdrawal path, this breaks total supply accounting from genesis.

### 3. Proof-of-Possession (PoP) and Consensus Public Keys Are Never Validated

**File:** `genesis.rs:494-509`

`consensusPop` and `consensusPubkey` are parsed from hex and passed straight through to `Genesis.initialize()` with **zero cryptographic validation**:

- No check that `consensusPubkey` is a valid BLS12-381 G1/G2 point
- No check that `consensusPop` proves knowledge of the corresponding secret key
- No check for rogue-key attacks (malicious pubkey that cancels out honest validators' aggregate key)

**Attack Vector — Rogue Key Attack:** A malicious validator submits a crafted BLS public key computed as `g^x - PK_honest` (where `PK_honest` is an honest validator's key). In threshold signing, the attacker can now forge aggregate signatures without the honest validator's cooperation. The genesis tool is the **only pre-chain validation layer** — if it doesn't verify PoPs, the chain launches with a compromised validator set.

**Confirmed by config:** `genesis_config.json` shows all validators have `consensusPop: "0x"` (empty), proving this path is exercised with zero-length PoPs in practice.

---

## MEDIUM

### 4. `calculate_total_stake` Silently Wraps on U256 Overflow

**File:** `genesis.rs:527-533`

```rust
.fold(U256::ZERO, |acc, stake| acc + stake)  // unchecked addition
```

U256 `+` wraps on overflow in release builds. A crafted config with stake amounts summing past `2^256 - 1` produces a near-zero `total_stake`. This propagates to:
- `SYSTEM_CALLER` balance (insufficient to fund `Genesis.initialize`)
- `msg.value` of the initialize call (validators receive near-zero stake)
- Genesis contract balance (near-zero instead of total stake)

**Impact:** A validator set could be bootstrapped where the on-chain staking contract believes total stake is ~0, making the cost of a 2/3 majority attack essentially free.

### 5. `verify` Subcommand Hardcodes Chain ID 1337, Misses Critical Checks

**File:** `verify.rs:193`

```rust
let env = prepare_env(1337);  // hardcoded, ignores actual genesis chain ID
```

The verification step:
- Always simulates against chain ID 1337 regardless of the genesis file's actual chain ID
- Does **not** check whether bytecode is constructor vs. runtime (Finding 1)
- Does **not** check phantom balances (Finding 2)
- Does **not** validate PoPs (Finding 3)
- Does **not** verify voting power, stake amounts, or operator/owner relationships
- Has multiple verification functions commented out (`verify_jwks`, `verify_epoch_config`, `verify_randomness_config`)

**Impact:** The verify step provides a **false sense of security** — it passes on genesis files containing all of the above critical/high issues.

### 6. Pre-funded Hardhat Account #0 in `genesis_template.json`

**File:** `genesis_template.json`

Account `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266` is pre-funded. This is the default Hardhat/Anvil account derived from the **well-known mnemonic** `test test test test test test test test test test test junk`. The private key is `0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80`.

**Attack Vector:** If this template reaches a production or public testnet deployment without stripping this entry, **anyone** can immediately drain those funds. The orchestration script (`genesis.sh`) merges this template with additional allocs via `jq`, and there is no guard that removes test accounts for non-local deployments.

### 7. Default Chain ID 1337 Enables Cross-Chain Replay

**File:** `genesis.rs:69-71`

If `chainId` is omitted from the genesis config, it silently defaults to `1337`. A deployment that accidentally omits this field will share chain ID `1337` with every Hardhat/local test network, making all EIP-155 signed transactions **replayable across chains**.

---

## Attack Chain Summary

A sophisticated attacker controlling a single validator's genesis config entry could:

1. Submit a **rogue BLS public key** with empty PoP (Finding 3) → gain threshold signing capability
2. Craft stake amounts to **overflow total_stake** (Finding 4) → reduce economic security to ~0
3. Supply a **backdoored `.hex` file** that passes the broken bytecode heuristic (Finding 1) → inject arbitrary logic into a system contract
4. None of the above would be caught by `verify` (Finding 5)

The genesis ceremony produces a chain that is **compromised from block 0** with no detection mechanism.

---

## [aptos_executor_state_transitions] Review dependencies/aptos-executor/ and dependencies/aptos-executor-types/ for s

### 🎯 Intention

Review dependencies/aptos-executor/ and dependencies/aptos-executor-types/ for state transition execution safety, WriteSet application correctness, and how gravity-sdk integrates with or modifies the aptos executor layer. Check for any shim logic that could introduce inconsistencies between consensus-agreed state and executed state.

### 🔍 Discovered Implementation

# Implementation Analysis: Aptos Executor Layer in gravity-sdk

## Architecture Overview

Gravity-sdk replaces the upstream Aptos executor with a **dual-layer architecture**:

1. **`dependencies/aptos-executor/` and `dependencies/aptos-executor-types/`** — Minimal stub crates that define the `BlockExecutorTrait` interface and provide a no-op `BlockExecutor`. These exist so that the Aptos consensus layer compiles without the full upstream Aptos VM.

2. **`aptos-core/consensus/src/gravity_state_computer.rs`** — The `GravityBlockExecutor` wrapper that implements `BlockExecutorTrait` by delegating execution to the inner stub and **replacing the entire commit path** with gravity's `BlockBufferManager` pipeline.

Real EVM execution happens externally via Reth's `PipeExecLayerApi`, completely outside these crates.

---

## Files/Contracts Involved

| File | Description |
|---|---|
| `dependencies/aptos-executor/src/lib.rs` | No-op `BlockExecutor` stub |
| `dependencies/aptos-executor/src/mock_block_tree.rs` | In-memory block ID tracker |
| `dependencies/aptos-executor-types/src/lib.rs` | `BlockExecutorTrait` trait definition + `StateComputeResult` type |
| `aptos-core/consensus/src/gravity_state_computer.rs` | `GravityBlockExecutor` — the shim between consensus and execution |

---

## Key Functions

### `BlockExecutorTrait` (trait, `aptos-executor-types/src/lib.rs:67-111`)

| Method | Signature | Behavior |
|---|---|---|
| `committed_block_id` | `&self -> HashValue` | Returns last committed block ID |
| `reset` | `&self -> Result<()>` | Resets internal state |
| `execute_and_state_checkpoint` | `(&self, ExecutableBlock, HashValue, BlockExecutorConfigFromOnchain) -> ExecutorResult<()>` | Executes block and produces state checkpoint |
| `ledger_update` | `(&self, block_id, parent_block_id) -> ExecutorResult<StateComputeResult>` | Produces `StateComputeResult` with root hash |
| `commit_blocks` | `(&self, Vec<HashValue>, LedgerInfoWithSignatures) -> ExecutorResult<()>` | Default impl: calls `pre_commit_block` per block, then `commit_ledger` |
| `pre_commit_block` | `(&self, HashValue) -> ExecutorResult<()>` | Pre-commit hook |
| `commit_ledger` | `(&self, Vec<HashValue>, LedgerInfoWithSignatures, Vec<(u64, Vec<u8>)>) -> ExecutorResult<()>` | Final commit with randomness |

### Stub `BlockExecutor` (`dependencies/aptos-executor/src/lib.rs:21-83`)

- **`execute_and_state_checkpoint`** (line 41-48): Returns `Ok(())` immediately — **no actual transaction execution**.
- **`ledger_update`** (line 50-57): Returns `StateComputeResult::with_root_hash(block_id)` — the root hash is set to the **block ID itself** (identity mapping, not a real state hash).
- **`commit_blocks`** (line 59-66): Appends block IDs to `MockBlockTree.commited_blocks`. Does not write to any persistent storage.
- **`pre_commit_block`** (line 70-72): `todo!()` — **panics if called**.
- **`commit_ledger`** (line 74-81): `todo!()` — **panics if called**.

### `StateComputeResult` (`aptos-executor-types/src/lib.rs:113-192`)

- Wraps `gaptos::api_types::compute_res::ComputeRes` which carries a 32-byte `data` field (root hash), `txn_num`, `txn_status`, and `events`.
- **`version()`** (line 129-132): Hardcoded to return `0` with a TODO comment.
- **`transactions_to_commit()`** (line 171-187): Filters input transactions by `TxnStatus.is_discarded`, returning only non-discarded ones. Contains a commented-out assertion `assert_eq!(status_len, input_txns.len())` (line 178) with a TODO about unifying recovery and execution paths.
- **`root_hash()`** (line 154-156): Reads `HashValue` directly from `execution_output.data`.

### `GravityBlockExecutor` (`gravity_state_computer.rs:44-193`)

**Pass-through methods** (no modification):
- `committed_block_id()` → `self.inner.committed_block_id()`
- `reset()` → `self.inner.reset()`
- `execute_and_state_checkpoint()` → `self.inner.execute_and_state_checkpoint()`
- `ledger_update()` → `self.inner.ledger_update()`
- `finish()` → `self.inner.finish()`

**Intercepted methods:**

- **`pre_commit_block()`** (line 136-138): Returns `Ok(())` unconditionally — upstream pre-commit hook is **entirely bypassed**.

- **`commit_blocks()`** (line 86-130):
  1. Extracts `block_id` and `block_hash` from `LedgerInfoWithSignatures`
  2. Records transaction lifetime metrics
  3. **Asserts** `block_ids.last().unwrap().as_slice() == block_id.as_slice()` (line 98)
  4. Calls `self.inner.db.writer.save_transactions(None, Some(&ledger_info_with_sigs), false)` — persists only ledger info, **no transactions** (`None`)
  5. Routes block IDs into `BlockBufferManager::set_commit_blocks()` via async runtime
  6. Awaits `persist_notifiers` for each block — blocks until persistence is confirmed
  7. Only assigns `hash: Some(v)` to the **last block** (matching `block_id`); intermediate blocks get `hash: None`

- **`commit_ledger()`** (line 139-192):
  1. Increments `APTOS_COMMIT_BLOCKS` metric
  2. Asserts `!block_ids.is_empty()`
  3. **Persists randomness data** to `ConsensusDB` via `put_randomness()` (gravity-specific, not in upstream)
  4. Routes blocks into `BlockBufferManager::set_commit_blocks()` and awaits persistence
  5. Calls `save_transactions(None, Some(&ledger_info_with_sigs), false)` **after** persistence (ordering differs from `commit_blocks`)

---

## State Changes

| Operation | What Gets Modified |
|---|---|
| `execute_and_state_checkpoint` | Nothing (stub returns `Ok(())`) |
| `ledger_update` | Nothing (returns synthetic `StateComputeResult`) |
| `commit_blocks` | `MockBlockTree.commited_blocks` (in-memory); `DbReaderWriter.writer` (ledger info only); `BlockBufferManager` (committed block entries) |
| `commit_ledger` | `ConsensusDB` (randomness data); `BlockBufferManager` (committed block entries); `DbReaderWriter.writer` (ledger info only) |
| `pre_commit_block` | Nothing (no-op) |

---

## Execution Path: Consensus → Execution → Commit

```
Consensus orders blocks
        │
        ▼
execute_and_state_checkpoint(block, parent_id, config)
  [GravityBlockExecutor delegates to inner stub → returns Ok(()) immediately]
  [No actual transaction execution occurs here]
        │
        ▼
ledger_update(block_id, parent_block_id)
  [Inner stub returns StateComputeResult with root_hash = block_id]
  [Root hash is the block ID itself, NOT a computed state root]
        │
        ▼
commit_blocks(block_ids, ledger_info_with_sigs)  ─OR─  commit_ledger(block_ids, LI, randomness)
  │
  ├─ Assert last block_id matches LedgerInfo.commit_info().id()
  ├─ [commit_ledger only] Persist randomness to ConsensusDB
  ├─ save_transactions(None, Some(&LI), false) — ledger info only, no txns
  ├─ BlockBufferManager.set_commit_blocks(block_hash_refs, epoch)
  └─ Await persist_notifiers (blocks until Reth confirms persistence)
```

The actual EVM execution happens in a separate pipeline:
```
BlockBufferManager.set_ordered_blocks()
        │
        ▼
RethCli::start_execution() → PipeExecLayerApi::push_ordered_block()
        │
        ▼
Reth EVM executes transactions
        │
        ▼
PipeExecLayerApi::pull_executed_block_hash() → execution result
        │
        ▼
RethCli::start_commit_vote() → BlockBufferManager.set_compute_res()
        │
        ▼
Consensus votes on StateComputeResult
        │
        ▼
RethCli::start_commit() → PipeExecLayerApi::commit_executed_block_hash()
```

---

## External Dependencies

| Dependency | Source | Usage |
|---|---|---|
| `gaptos` | Gravity's vendored Aptos re-export crate | `HashValue`, `LedgerInfoWithSignatures`, `ExecutableBlock`, `ComputeRes`, `EpochState`, runtimes |
| `block_buffer_manager` | Gravity crate | `get_block_buffer_manager()`, `BlockHashRef`, `set_commit_blocks()` |
| `txn_metrics` | Gravity crate | `TxnLifeTime::record_block_committed()` |
| `ConsensusDB` | `crate::consensusdb` | `put_randomness()` for randomness persistence |
| `DbReaderWriter` | Via `gaptos::aptos_storage_interface` | `writer.save_transactions()` for ledger info persistence |

---

## Notable Implementation Details

1. **`commit_blocks` vs `commit_ledger` ordering inconsistency**: In `commit_blocks()` (line 100-101), `save_transactions()` is called **before** `set_commit_blocks()`. In `commit_ledger()` (line 188-189), `save_transactions()` is called **after** `set_commit_blocks()` and after persistence confirmation. These two methods write the ledger info at different points relative to the BlockBufferManager persistence flow.

2. **`commit_blocks` error handling vs `commit_ledger`**: `commit_blocks()` uses `unwrap_or_else(|e| panic!(...))` on the `set_commit_blocks` result (line 123), while `commit_ledger()` uses a bare `.unwrap()` (line 184). Both panic on failure but with different diagnostic messages.

3. **`commit_ledger` randomness persistence error propagation**: `put_randomness()` failure is converted to an `anyhow::Error` and propagated via `?` (line 160), meaning randomness persistence failure aborts the commit. However, this happens **before** `set_commit_blocks()`, so a randomness write failure leaves no partial state in the BlockBufferManager.

4. **Block hash assignment**: In both `commit_blocks` and `commit_ledger`, only the block matching `block_id` (the last one) receives `hash: Some(v)`. All preceding blocks in the batch receive `hash: None` (lines 115 and 176). The hash value `v` is the same `block_hash` for all blocks — it's copied from the `LedgerInfoWithSignatures`.

5. **`version()` placeholder**: `StateComputeResult::version()` always returns `0` (line 130), with a TODO indicating this needs real implementation.

6. **Commented-out assertion**: `transactions_to_commit()` has `assert_eq!(status_len, input_txns.len())` commented out (line 178), with a TODO about unifying recovery and execution. This means there is no runtime validation that the transaction status vector length matches the input transaction count.

7. **`pre_commit_block` bypass**: The `GravityBlockExecutor` returns `Ok(())` for `pre_commit_block` (line 137), entirely skipping whatever the upstream Aptos executor would have done in this hook. The default `commit_blocks` trait implementation (line 93-98) calls `pre_commit_block` per block before `commit_ledger`, but `GravityBlockExecutor` overrides `commit_blocks` entirely and never calls the default implementation.

8. **The stub executor's `ledger_update` sets root_hash = block_id**: This means the `StateComputeResult` root hash used by consensus for voting is the block ID itself, not a computed Merkle root. The real state root comes from Reth's execution pipeline via a separate channel (`set_compute_res`).

### 🛠️ Ground Review

# Ground Review: Aptos Executor Layer — Code Hygiene & Engineering Safety

---

## Critical

### 1. `todo!()` panics in production code paths
**File:** `dependencies/aptos-executor/src/lib.rs:70-81`

`pre_commit_block()` and `commit_ledger()` on the stub `BlockExecutor` contain `todo!()` which will **panic unconditionally** if ever called. The default trait implementation of `commit_blocks()` (`aptos-executor-types/src/lib.rs:93-98`) calls `pre_commit_block()` per block and then `commit_ledger()`. If any code path invokes the stub's `commit_blocks()` via the **default trait implementation** instead of the overridden one, the process crashes instantly.

This is currently safe only because `GravityBlockExecutor` overrides `commit_blocks` entirely. However, the stub itself implements `BlockExecutorTrait` independently — any test, fallback, or future refactor that instantiates a bare `BlockExecutor` and calls the default `commit_blocks` will hit an immediate panic.

**Recommendation:** Replace `todo!()` with either an explicit `Err(ExecutorError::InternalError { error: "not implemented for stub executor".into() })` or a `#[cfg(not(test))] compile_error!` to make the trap visible at compile time rather than runtime.

---

### 2. Commented-out invariant assertion — silent data corruption risk
**File:** `dependencies/aptos-executor-types/src/lib.rs:178`

```rust
// assert_eq!(status_len, input_txns.len());
```

`transactions_to_commit()` zips `input_txns` with `txn_status` via `.zip()`. If `status.len() < input_txns.len()`, the trailing transactions are **silently dropped** — they are never evaluated for discard/keep. If `status.len() > input_txns.len()`, the extra statuses are silently ignored. Both cases produce incorrect mempool GC behavior without any warning.

**Recommendation:** At minimum, log a warning when the lengths diverge. Ideally, restore the assertion or return an error. The TODO (`unify recover and execution`) has been open long enough to warrant a tracked issue.

---

### 3. Unbounded in-memory growth in `MockBlockTree`
**File:** `dependencies/aptos-executor/src/mock_block_tree.rs:7`

```rust
pub commited_blocks: Vec<HashValue>,
```

`commit_blocks()` calls `extend(block_ids)` on every commit. This vector **never shrinks** — over a long-running node's lifetime it grows without bound. Each `HashValue` is 32 bytes; at 1 block/sec that's ~2.7 MB/day, which is modest but represents an unbounded leak with no eviction strategy.

Additionally, `id_to_block: HashMap<HashValue, ExecutableBlock>` is declared but **never written to** by any code path, making it dead weight.

**Recommendation:** Either cap the vector (ring buffer / bounded deque), periodically truncate it, or remove it if it only serves the `committed_block_id()` query (which only needs the last element).

---

## Warning

### 4. `RwLock::read().unwrap()` / `RwLock::write().unwrap()` — poison panic
**File:** `dependencies/aptos-executor/src/lib.rs:34, 64`

```rust
self.block_tree.read().unwrap()
self.block_tree.write().unwrap()
```

If any thread panics while holding the write lock, the `RwLock` becomes poisoned and **all** subsequent reads and writes will panic. In a multi-threaded consensus node, a single thread's panic propagates to every caller.

**Recommendation:** Use `read().unwrap_or_else(|e| e.into_inner())` or handle poison explicitly, depending on the desired recovery semantics.

---

### 5. `version()` hardcoded to `0`
**File:** `dependencies/aptos-executor-types/src/lib.rs:129-132`

```rust
pub fn version(&self) -> Version {
    // TODO(gravity_byteyue): this is a placeholder
    Version::from(0u8)
}
```

Any downstream code relying on `version()` for sequencing, pagination, or consistency checks will silently malfunction. The `Version` type is a `u64` used pervasively in Aptos for ledger version ordering.

**Recommendation:** Either wire this to a real value (`txn_num` from `ComputeRes`?) or mark the method `#[deprecated]` / `unimplemented!()` so callers fail loudly rather than silently using `0`.

---

### 6. Typo: `commited_blocks` → `committed_blocks`
**Files:** `mock_block_tree.rs:7`, `lib.rs:34, 64`

The field is spelled `commited_blocks` (single 't') in all three usages. This is a minor naming issue but it will cause confusion when grepping and may lead to bugs if someone introduces a correctly-spelled field alongside.

**Recommendation:** Rename to `committed_blocks` across all references.

---

### 7. Unused `HashMap` field
**File:** `dependencies/aptos-executor/src/mock_block_tree.rs:6`

```rust
pub id_to_block: HashMap<HashValue, ExecutableBlock>,
```

This field is initialized as empty and never populated by any code path in the executor. It is dead code carrying trait bounds.

**Recommendation:** Remove unless there is a concrete plan to use it.

---

### 8. Unused function parameters (dead code smell)
**File:** `dependencies/aptos-executor/src/lib.rs:41-48`

```rust
fn execute_and_state_checkpoint(
    &self,
    block: ExecutableBlock,       // unused
    parent_block_id: HashValue,   // unused
    onchain_config: BlockExecutorConfigFromOnchain, // unused
) -> ExecutorResult<()> {
    ExecutorResult::Ok(())
}
```

The `block`, `parent_block_id`, and `onchain_config` parameters are accepted but entirely ignored. This is inherent to the stub design but should be annotated (e.g., prefixed with `_`) to suppress compiler warnings and signal intent.

---

## Info

### 9. `pub execution_output` exposes internals
**File:** `dependencies/aptos-executor-types/src/lib.rs:115`

```rust
pub execution_output: ComputeRes,
```

`StateComputeResult` provides accessor methods (`root_hash()`, `txn_status()`, `events()`) but also exposes `execution_output` as `pub`, allowing any consumer to bypass the accessor API and directly mutate or read internal fields. The other two fields (`epoch_state`, `block_end_info`) are correctly private.

**Recommendation:** Make `execution_output` private (`pub(crate)` at most) to enforce encapsulation through the accessor methods.

---

### 10. Empty test module
**File:** `dependencies/aptos-executor-types/src/lib.rs:211-212`

```rust
#[cfg(test)]
mod tests {}
```

The test module exists but contains no tests. Given the critical invariants this code is responsible for (root hash computation, transaction filtering, version tracking), even basic unit tests would increase confidence.

---

### 11. Phantom type struct with no usage
**File:** `dependencies/aptos-executor-types/src/lib.rs:206-208`

```rust
pub struct BlockExecutorInner<V> {
    phantom: PhantomData<V>,
}
```

`BlockExecutorInner<V>` appears to be a vestigial type from upstream Aptos. It is never constructed or referenced anywhere in the gravity-sdk codebase.

**Recommendation:** Remove if unused.

---

## Summary

| Severity | Count | Key Themes |
|----------|-------|------------|
| **Critical** | 3 | Runtime panics via `todo!()`, silent data truncation from commented-out assertion, unbounded memory growth |
| **Warning** | 5 | Lock poisoning, hardcoded placeholder version, naming typo, dead fields, unused params |
| **Info** | 3 | Broken encapsulation, empty tests, phantom dead code |

The stub architecture is sound in concept — the risk lies in the **gap between "intentionally no-op" and "accidentally invoked."** The `todo!()` panics and the commented-out assertion are the most actionable items: both represent invariants that the codebase *knows* it needs but has deferred enforcing.

### ⚔️ Intention Attacker

# Intention Attack Report: Aptos Executor Layer in gravity-sdk

## Intent vs. Implementation Summary

**User Intent:** Ensure state transition execution safety, WriteSet application correctness, and consistency between consensus-agreed state and executed state.

**Implementation Reality:** The executor layer is a hollow shim — no transaction execution occurs in it. Real execution is delegated to an external Reth/EVM pipeline via `BlockBufferManager`. This architectural choice introduces several critical gaps where consensus-agreed state can diverge from executed state.

---

## Critical Severity

### C1. `save_transactions` Error Silently Discarded

**File:** `gravity_state_computer.rs:100-101` and `gravity_state_computer.rs:188-189`

```rust
let _ = self.inner.db.writer.save_transactions(None, Some(&ledger_info_with_sigs), false);
```

Both `commit_blocks` and `commit_ledger` use `let _ =` to discard the `Result`. The investigation confirmed `save_transactions` is **not a no-op** — it writes `LedgerInfoWithSignatures` to the metadata DB and updates the in-memory latest ledger info cache. If this write fails (disk full, I/O error), the error is silently swallowed and the function returns `Ok(())`.

**Impact:** Consensus believes the ledger info was persisted when it was not. On restart, the node's ledger info is behind, causing it to disagree with peers about the latest committed state. This is an **irrecoverable consensus divergence** that cannot be detected at runtime.

**Fix:** Replace `let _ =` with `?` or explicit error handling.

---

### C2. `commit_blocks` vs `commit_ledger` — Ordering Inconsistency Creates Crash-Recovery Split-Brain

**File:** `gravity_state_computer.rs:86-130` vs `gravity_state_computer.rs:139-192`

| Step | `commit_blocks` (line 100) | `commit_ledger` (line 188) |
|------|---------------------------|---------------------------|
| 1st | `save_transactions` (ledger metadata) | `set_commit_blocks` + await persistence |
| 2nd | `set_commit_blocks` + await persistence | `save_transactions` (ledger metadata) |

If the node crashes between steps 1 and 2 in `commit_blocks`, the ledger metadata is persisted but BlockBufferManager never committed the block. On restart: consensus believes committed, execution has no record. **Irrecoverable split-brain.**

**Mitigating factor:** Investigation confirmed `commit_blocks` is dead code in the live pipeline — only `commit_ledger` is called from `ExecutionProxy::commit` and `PipelineBuilder`. However, it remains a valid, non-deprecated `BlockExecutorTrait` method that compiles and could be invoked by future code.

**Fix:** Delete or mark `commit_blocks` override as `unreachable!()`, or fix the ordering to match `commit_ledger`.

---

### C3. No Validation of `block_hash` in BlockBufferManager Commit Path

**File:** `gravity_state_computer.rs:110-115` and `block_buffer_manager.rs`

The `block_hash` from `LedgerInfoWithSignatures` is passed into `BlockHashRef` and stored by `set_commit_blocks` **without cross-checking** against the execution result hash stored in `BlockState::Computed`. Additionally, only the **last block** in a batch receives a hash — all intermediate blocks get `hash: None`:

```rust
hash: if x == block_id { Some(v) } else { None },
```

**Impact:** A corrupted or tampered `LedgerInfoWithSignatures` could inject a wrong hash that propagates to the execution layer as the "committed" hash. Intermediate blocks have no hash integrity whatsoever, so the execution layer cannot verify them.

---

## High Severity

### H1. `persist_notifier.take()` Is Destructive — Retry Causes Permanent Deadlock

**File:** `block_buffer_manager.rs` (set_commit_blocks / get_committed_blocks)

The `persist_notifier` is a one-shot `mpsc::Sender` consumed via `.take()` when `get_committed_blocks` retrieves a committed block. If `get_committed_blocks` is called twice for the same epoch-change block (e.g., retry after transient error):
1. Second call returns `persist_notifier: None`
2. No `Sender` exists to signal the `Receiver`
3. Consensus side blocks **forever** on `notifier.recv().await`

**Impact:** Any retry on the execution side permanently deadlocks the consensus commit pipeline. The node becomes unresponsive with no visible error or timeout.

---

### H2. `set_compute_res` Panics on Duplicate Calls — No Idempotency

**File:** `block_buffer_manager.rs:703-756`

If the execution layer retries `set_compute_res` for a block already in `Computed` state, the function **panics** — it only handles `Ordered` state. A transient network issue causing a re-submission crashes the node.

**Impact:** Node crash during mid-commit, requiring restart and potential state inconsistency.

---

### H3. TOCTOU Race in Epoch-Change Detection

**File:** `block_buffer_manager.rs` (is_epoch_change / get_ordered_blocks)

`is_epoch_change()` reads `AtomicU8` **without** holding the `block_state_machine` lock. In `get_ordered_blocks`, the pattern is:

```
if self.is_epoch_change() { ... }  // no lock
let guard = self.block_state_machine.lock().await;  // lock acquired later
```

Between these two operations, another task can change the epoch state. The codebase itself acknowledges this: `consume_epoch_change` comments say "Acquire lock first to prevent TOCTOU race" — but `get_ordered_blocks` does not follow this discipline.

**Impact:** Under concurrent epoch transitions, blocks could be processed in the wrong epoch context.

---

### H4. `StateComputeResult::version()` Hardcoded to `0`

**File:** `aptos-executor-types/src/lib.rs:129-132`

```rust
pub fn version(&self) -> Version {
    // TODO(gravity_byteyue): this is a placeholder
    Version::from(0u8)
}
```

All blocks report version 0. If any Aptos consensus internals use this for ordering, deduplication, or pruning, all blocks appear identical. No callers found in gravity-sdk itself, but upstream Aptos framework code may call it.

---

## Medium Severity

### M1. `transactions_to_commit()` Silently Truncates on Length Mismatch

**File:** `aptos-executor-types/src/lib.rs:170-187`

The assertion `assert_eq!(status_len, input_txns.len())` is **commented out**. When `txn_status` is shorter than `input_txns`, `zip` stops early — transactions at the end are **silently dropped** without being reported as committed or discarded. Mempool GC never cleans them up, causing a mempool leak.

---

### M2. Block Number Arithmetic Could Underflow

**File:** `gravity_state_computer.rs:114` and `gravity_state_computer.rs:175`

```rust
num: block_num - (len - 1 - i) as u64,
```

If `block_num < (len - 1)`, this wraps to `u64::MAX - delta` in release mode (Rust unsigned subtraction wraps). The corrupt block number propagates into BlockBufferManager lookups.

---

### M3. Randomness Data Logged at Debug Level

**File:** `gravity_state_computer.rs:161`

```rust
debug!("Persisted randomness data: {:?}", randomness_data);
```

Raw randomness bytes are logged in full. If used for leader election or cryptographic commitments, this exposes values to anyone with log access.

---

## Summary

| ID | Finding | Severity | Live? |
|----|---------|----------|-------|
| C1 | `save_transactions` error silently discarded (`let _ =`) | **Critical** | Yes |
| C2 | `commit_blocks` vs `commit_ledger` ordering — crash-recovery split-brain | **Critical** | Latent |
| C3 | No `block_hash` validation; intermediate blocks have `None` hash | **Critical** | Yes |
| H1 | `persist_notifier.take()` — retry causes permanent deadlock | **High** | Yes |
| H2 | `set_compute_res` panics on duplicate calls | **High** | Yes |
| H3 | TOCTOU race in epoch-change detection | **High** | Yes |
| H4 | `StateComputeResult::version()` hardcoded to `0` | **High** | TBD |
| M1 | `transactions_to_commit()` silently truncates on mismatch | **Medium** | Yes |
| M2 | Block number arithmetic underflow | **Medium** | Yes |
| M3 | Randomness data logged at debug level | **Medium** | Yes |

## Recommended Priority Actions

1. **C1** — Replace `let _ =` with proper error propagation on `save_transactions`. One-line fix, critical impact.
2. **C3** — Cross-check `block_hash` in `set_commit_blocks` against `BlockState::Computed` hash.
3. **H1** — Make `persist_notifier` retrieval idempotent or add timeout to `recv().await`.
4. **H2** — Make `set_compute_res` idempotent for duplicate calls with matching block_id/hash.
5. **C2** — Delete or deprecate the `commit_blocks` override since it is dead code with dangerous ordering.

---

## [txn_metrics_and_bench_binary] Review crates/txn_metrics/src/lib.rs for transaction metrics collection safety (

### 🎯 Intention

Review crates/txn_metrics/src/lib.rs for transaction metrics collection safety (unbounded metric storage, label injection, information leakage via exposed metrics) and bin/bench/ for any test harness code that exposes internal APIs or bypasses security controls that could be reused in production.

### 🔍 Discovered Implementation

# Implementation Analysis & Security Audit

## 1. Transaction Metrics (`crates/txn_metrics/src/lib.rs`)

### Files Involved

| File | Description |
|---|---|
| `crates/txn_metrics/src/lib.rs` | Sole source file (535 lines) — singleton transaction lifecycle latency tracker |
| `crates/txn_metrics/Cargo.toml` | Deps: `gaptos`, `dashmap`, `aptos-consensus-types` |

### Architecture

A global singleton `TxnLifeTime` (via `OnceLock`) tracks transactions from mempool entry to commit using four concurrent `DashMap`s and 10 label-free Prometheus histograms. Gated by env var `TXN_LIFE_ENABLED` (default: `false`).

### State Storage & Bounds

| Map | Key → Value | Capacity Constant | Eviction |
|---|---|---|---|
| `txn_initial_add_time` | `(AccountAddress, u64)` → `SystemTime` | 100,000 | Entries older than 60s removed |
| `txn_hash_to_key` | `HashValue` → `TxnKey` | **None (unbounded)** | Cleaned only as side-effect of primary map cleanup |
| `txn_batch_id` | `BatchId` → `HashSet<TxnKey>` | 10,000 | Entries referencing evicted txns removed |
| `txn_block_id` | `HashValue` → `HashSet<TxnKey>` | 1,000 | Same as batch cleanup |

### Metrics: No Label Injection Risk

All 10 histograms are registered with **zero label dimensions** — they observe only `f64` durations. No user-controlled data (addresses, hashes, nonces) is used as a metric label. Cardinality explosion via label injection is not possible.

### Security Findings

#### Finding 1: Unbounded `txn_hash_to_key` Map — Memory Growth Risk
**Severity: Medium**

The `txn_hash_to_key` DashMap has no independent capacity check. Cleanup is triggered only when `txn_initial_add_time` hits its 100,000 limit. If many distinct transaction hashes map to the same `(sender, sequence_number)` key (e.g., resubmissions with different payloads), `txn_hash_to_key` grows without bound while `txn_initial_add_time` stays within its limit. An attacker submitting many variants of the same `(address, nonce)` pair could grow this map indefinitely without triggering eviction.

#### Finding 2: O(n) Full-Map Scan per Commit — Performance DoS Vector
**Severity: Medium**

`record_committed()` calls `txn_hash_to_key.retain(|_, v| v != &key)`, which **iterates the entire map** for every committed transaction. It also iterates all entries in both `txn_batch_id` and `txn_block_id`. Under high throughput, this creates O(n × m) total work where n = committed txns per second and m = map size, potentially degrading consensus latency.

#### Finding 3: No Information Leakage via Metrics
**Severity: None**

Metrics expose only aggregate latency distributions with fixed buckets. No per-address, per-transaction, or per-account information is observable through the Prometheus interface. Internal tracking data in DashMaps is never serialized or exposed externally.

---

## 2. Benchmark Harness (`bin/bench/`)

### Files Involved

| File | Lines | Description |
|---|---|---|
| `bin/bench/src/main.rs` | 153 | Entry point — warp HTTP server + consensus layer driver |
| `bin/bench/src/cli.rs` | 46 | CLI argument parsing via clap |
| `bin/bench/src/kv.rs` | 150 | KV store stub (execution layer mock), mostly commented out |
| `bin/bench/src/txn.rs` | 62 | `RawTxn` — test transaction type with JSON serialization |
| `bin/bench/src/stateful_mempool.rs` | 82 | Simplified mempool with mpsc channels (1M capacity) |
| `bin/bench/Cargo.toml` | 35 | Depends on `api`, `gaptos`, `block-buffer-manager`, `warp` |

### Execution Path

1. CLI parses `--leader`, `--port`, `--log-dir`, and node config path
2. Initializes `ConsensusEngine` with `EmptyTxPool` and `chain_id: 1337`
3. If `--leader`: starts a warp HTTP server binding to `0.0.0.0:<port>` with route `GET /ProduceTxn?value=<bool>` to toggle transaction generation
4. Leader mode generates random transactions in a loop (configurable via `BLOCK_TXN_NUMS` env var), but the spawned task body is `todo!()` (will panic at runtime)

### Security Findings

#### Finding 4: HTTP Control Endpoint Binds to All Interfaces Without Authentication
**Severity: High**

`main.rs:143` — The `/ProduceTxn` warp endpoint binds to `0.0.0.0` with **no authentication, no TLS, and no access control**. Any network-reachable client can toggle transaction production on/off. While this is a bench binary, it imports and initializes the real `ConsensusEngine` from the `api` crate, meaning it exercises production consensus code paths.

```rust
warp::serve(route).run(([0, 0, 0, 0], port.expect("No port"))).await;
```

**Risk:** If this binary were accidentally deployed or its pattern copied, the unauthenticated endpoint would allow external control over consensus behavior.

#### Finding 5: Production Internal APIs Used Without Security Controls
**Severity: Medium**

The bench binary directly imports and instantiates the production `ConsensusEngine` (`api::consensus_api::ConsensusEngine`) with a hardcoded `chain_id: 1337` and `latest_block_number: 0`. It uses `check_bootstrap_config()` to load real node configuration. This means:

- The bench binary exercises the real consensus initialization path
- It connects to whatever peer network the config file specifies
- It does so with a test chain ID that may bypass chain-specific validation

If the bench binary's node config points to a production or staging network, it would connect as a peer with no safeguards.

#### Finding 6: `RawTxn` Bypasses Transaction Verification
**Severity: Medium**

`txn.rs:42-52` — `RawTxn::into_verified()` creates a `VerifiedTxn` by simply computing a hash over the JSON bytes and wrapping it with `ExternalChainId::new(0)`. There is **no signature verification, no schema validation, and no chain ID check**. The `from_bytes` and `From<VerifiedTxn>` impls use `unwrap()` on JSON deserialization, meaning malformed input causes a panic rather than returning an error.

```rust
pub fn into_verified(self) -> VerifiedTxn {
    let bytes = self.to_bytes();
    let hash = simple_hash::hash_to_fixed_array(&bytes);
    VerifiedTxn::new(bytes, self.account, self.sequence_number, ExternalChainId::new(0), TxnHash::new(hash))
}
```

**Risk:** This pattern — constructing `VerifiedTxn` without actual verification — could be incorrectly copied into production code, creating a transaction forgery vector.

#### Finding 7: Hardcoded Sequence Number in Transaction Generation
**Severity: Low**

`main.rs:52` — All generated test transactions use `sequence_number: 1`, which means the bench harness does not exercise nonce-ordering logic, replay protection, or sequence gap handling. This is a test fidelity issue rather than a direct security concern.

#### Finding 8: `todo!()` Panic in Transaction Processing Path
**Severity: Low (Incomplete Code)**

`main.rs:67` — The spawned task for each generated transaction contains `todo!()`, meaning the leader mode will panic on first transaction. This renders the benchmark non-functional in its current state, but it also means the `ExecutionChannel` trait implementation (commented out in `kv.rs`) is disconnected.

---

## Summary

| # | Component | Finding | Severity |
|---|---|---|---|
| 1 | `txn_metrics` | Unbounded `txn_hash_to_key` map — no independent capacity limit | Medium |
| 2 | `txn_metrics` | O(n) full-map `retain()` scan per committed transaction | Medium |
| 3 | `txn_metrics` | No label injection or information leakage (label-free histograms) | None |
| 4 | `bin/bench` | Unauthenticated HTTP endpoint on `0.0.0.0` controls consensus behavior | High |
| 5 | `bin/bench` | Production `ConsensusEngine` used with no security controls | Medium |
| 6 | `bin/bench` | `RawTxn::into_verified()` creates "verified" transactions without verification | Medium |
| 7 | `bin/bench` | All test txns hardcode `sequence_number: 1` | Low |
| 8 | `bin/bench` | `todo!()` panic makes leader mode non-functional | Low |

### 🛠️ Ground Review

# Ground Review (Blue Team) — Code Hygiene & Engineering Safety

## Scope

Reviewed `crates/txn_metrics/src/lib.rs` (535 lines) and `bin/bench/` (5 files, ~493 lines) against the implementation report. All findings verified against actual source code.

---

## Critical

### GR-1: Potential DashMap Deadlock in `cleanup_old_entries` (txn_metrics, line 519-521)

Inside `txn_batch_id.retain()`, the closure calls `self.txn_initial_add_time.contains_key(key)`. DashMap's `retain` holds shard locks internally while iterating. If any other thread is simultaneously calling a method that locks shards on `txn_initial_add_time` and then tries to access `txn_batch_id` (or vice versa), this creates a **lock-ordering inversion**. The same pattern exists at line 528-530 with `txn_block_id.retain()`.

Additionally, `record_committed()` (lines 479-488) iterates `txn_batch_id` with `iter_mut()` (acquiring shard locks) and separately calls `txn_batch_id.retain()` — under concurrent access from `cleanup_old_entries`, this is a contention hotspot that could deadlock with the cross-map access inside retain closures.

**Impact:** Under high concurrency, the node could hang indefinitely in the metrics subsystem.

### GR-2: `record_committed()` Also Performs Full-Map Scan of `txn_hash_to_key` (txn_metrics, line 476)

Confirmed: `self.txn_hash_to_key.retain(|_hash, key| key != &txn_key)` iterates the **entire** unbounded map for every single committed transaction. The same full-scan pattern is repeated inside `cleanup_old_entries` at line 512, once per evicted key. Under sustained throughput of N commits/sec with M entries in the map, this is O(N × M) per second — a quadratic blowup.

**Impact:** Consensus-critical path latency degradation; could cause block production timeouts under load.

---

## Warning

### GR-3: Unbounded `txn_hash_to_key` Map — No Independent Capacity Guard (txn_metrics, line 173)

Confirmed: `txn_hash_to_key` has **no capacity constant** and no independent eviction trigger. It is only cleaned as a side effect of `txn_initial_add_time` cleanup (line 512) or `record_committed` (line 476). Multiple distinct hashes mapping to the same `(sender, seq)` key will grow this map without bound while the primary map stays under its 100K limit.

**Recommendation:** Add `MAX_TXN_HASH_TO_KEY_CAPACITY` and trigger independent cleanup, or use a reverse index (store `Vec<HashValue>` per `TxnKey` in `txn_initial_add_time`) to avoid the full-scan `retain`.

### GR-4: Unauthenticated HTTP Endpoint on 0.0.0.0 (bench/main.rs, line 143)

Confirmed: `warp::serve(route).run(([0, 0, 0, 0], port))` — no auth, no TLS, no IP allowlist. The `/ProduceTxn?value=<bool>` endpoint directly controls whether the consensus layer receives transactions. While this is a bench binary, it instantiates the **production** `ConsensusEngine` (line 33-41), meaning network exposure would allow external control of a real consensus participant.

### GR-5: `unwrap()` on Deserialization in Public API Surface (bench/txn.rs, lines 19, 26)

`RawTxn::from_bytes()` and `From<VerifiedTxn> for RawTxn` both call `serde_json::from_slice(&bytes).unwrap()`. Any malformed input causes a **panic**, taking down the process. These are called in `stateful_mempool.rs:33` (`add_raw_txn`) which processes externally-submitted transactions.

**Recommendation:** Return `Result` and propagate errors, or at minimum use `expect()` with a descriptive message.

### GR-6: Hardcoded `account_seq_num: 1` in Mempool (bench/stateful_mempool.rs, line 70)

`pending_txns()` wraps every transaction with `account_seq_num: 1` regardless of the actual sequence number. This means the bench harness never exercises nonce-ordering, replay protection, or sequence gap handling in the consensus layer — any performance or correctness results from this bench are unreliable for those code paths.

### GR-7: `into_verified()` Creates "Verified" Transactions Without Verification (bench/txn.rs, lines 42-52)

Confirmed: `RawTxn::into_verified()` constructs a `VerifiedTxn` by hashing JSON bytes with no signature check and using `ExternalChainId::new(0)`. The type name `VerifiedTxn` implies cryptographic verification has occurred. If this pattern is copied to production code, it becomes a transaction forgery vector.

### GR-8: Nested Runtime Creation (bench/main.rs, lines 131-136)

A new `tokio::Runtime` is created inside a `thread::spawn` inside a `tokio::spawn`. This is a code smell — the outer async context already has a runtime. The inner `Runtime::new().unwrap().block_on()` creates an entirely separate executor, which:
- Wastes resources (separate thread pool)
- Can cause subtle bugs if the inner runtime's tasks try to interact with the outer runtime's resources
- The `unwrap()` on `Runtime::new()` will panic if the OS refuses to create more threads

---

## Info

### GR-9: `_store` Field Prefixed with Underscore But Actively Used (bench/kv.rs, line 46)

The `_store` field uses an underscore prefix (Rust convention for "unused"), but `get()` and `set()` methods actively use it. This is misleading.

### GR-10: `todo!()` Panic in Leader Transaction Path (bench/main.rs, line 67)

The spawned task for each generated transaction is `todo!()`, making leader mode crash immediately on first transaction. Combined with the `ExecutionChannel` impl being fully commented out in `kv.rs`, the entire bench binary is non-functional in its current state.

### GR-11: `port` Unnecessarily Cloned (bench/main.rs, line 125)

`cli.port.clone()` clones an `Option<u16>` — `u16` is `Copy`, so the clone is redundant. Minor, but indicates inattention to types.

### GR-12: Unused Import `DefaultHasher` / `Hash` / `Hasher` (txn_metrics, lines 2-4)

`std::hash::{Hash, Hasher}` and `DefaultHasher` are imported but never used anywhere in the file. Dead imports should be removed.

---

## Summary

| ID | Severity | Component | Finding |
|----|----------|-----------|---------|
| GR-1 | **Critical** | txn_metrics | Cross-map shard lock ordering inversion in `cleanup_old_entries` / `record_committed` — deadlock risk |
| GR-2 | **Critical** | txn_metrics | O(N×M) full-map `retain()` scans on every commit — quadratic performance degradation |
| GR-3 | Warning | txn_metrics | `txn_hash_to_key` has no capacity bound — unbounded memory growth |
| GR-4 | Warning | bench | Unauthenticated HTTP endpoint on `0.0.0.0` controls production `ConsensusEngine` |
| GR-5 | Warning | bench | `unwrap()` on deserialization of external input — panics on malformed data |
| GR-6 | Warning | bench | Hardcoded `account_seq_num: 1` — bench never tests nonce ordering |
| GR-7 | Warning | bench | `into_verified()` bypasses all cryptographic verification |
| GR-8 | Warning | bench | Nested tokio runtime inside async context — resource waste and potential bugs |
| GR-9 | Info | bench | `_store` field named as unused but actively accessed |
| GR-10 | Info | bench | `todo!()` makes leader mode non-functional |
| GR-11 | Info | bench | Redundant `.clone()` on `Copy` type |
| GR-12 | Info | txn_metrics | Unused imports (`DefaultHasher`, `Hash`, `Hasher`) |

**Key additions beyond the implementation report:** GR-1 (deadlock risk from cross-map lock ordering) and GR-8 (nested runtime) are new findings not covered in the original analysis. GR-1 in particular is the highest-severity issue — a deadlock in the metrics subsystem would halt the node entirely.

### ⚔️ Intention Attacker

# Intention Attack Report — Security Audit Findings

## Scope

- `crates/txn_metrics/src/lib.rs` — Transaction lifecycle metrics singleton
- `bin/bench/` — Benchmark harness (main.rs, txn.rs, kv.rs, stateful_mempool.rs)

---

## Critical Findings

### CRITICAL-1: DashMap Deadlock in `cleanup_old_entries` — Nested Iteration Causes Potential Livelock

**File:** `crates/txn_metrics/src/lib.rs:519-521`

```rust
self.txn_batch_id.retain(|_batch_id, txn_set| {
    txn_set.retain(|key| self.txn_initial_add_time.contains_key(key));
    !txn_set.is_empty()
});
```

The `retain()` closure on `txn_batch_id` calls `self.txn_initial_add_time.contains_key()` **while holding a shard lock on `txn_batch_id`**. DashMap's `retain` acquires shard locks sequentially. If another thread is simultaneously executing `record_committed()` — which calls `self.txn_batch_id.iter_mut()` (line 479) while also touching `txn_initial_add_time.remove()` (line 473) — you get a lock-ordering inversion between the two DashMaps. Under high contention, this can cause **deadlock or indefinite stall of the consensus commit path**.

The identical pattern exists at lines 528-531 with `txn_block_id` + `txn_initial_add_time`.

**Impact:** Consensus node hangs. The commit path (`record_committed`) and the cleanup path (`cleanup_old_entries`, triggered from `record_added`) race against each other on two different DashMap lock orderings. Since `record_committed` is called per-transaction on the critical commit path, a deadlock here halts block finalization.

---

### CRITICAL-2: `record_committed` Silently Drops Metrics for Untracked Transactions — Commits Without Audit Trail

**File:** `crates/txn_metrics/src/lib.rs:459-489`

```rust
pub fn record_committed(&self, sender: &AccountAddress, sequence_number: u64) {
    // ...
    let txn_key = (*sender, sequence_number);
    if let Some(initial_add_time_entry) = self.txn_initial_add_time.get(&txn_key) {
        // observe metric
    }
    // Unconditionally removes from ALL maps regardless of whether metric was recorded
    self.txn_initial_add_time.remove(&txn_key);
    self.txn_hash_to_key.retain(|_hash, key| key != &txn_key);
    // ...
}
```

If `cleanup_old_entries` evicts a transaction from `txn_initial_add_time` (entries >60s old, line 502), a subsequent `record_committed` call for that transaction will:
1. **Not record the committed latency** (the `if let Some` fails silently)
2. **Still perform expensive O(n) cleanup** of `txn_hash_to_key`, `txn_batch_id`, and `txn_block_id`

This means **slow transactions (>60s) are systematically excluded from commit latency metrics**. The Prometheus histogram will show an artificially healthy latency distribution — the worst-case transactions are never counted. This is the **opposite of the stated intention**: the system was designed to track transaction lifecycle latency, but it specifically loses visibility on the transactions that matter most (the slow ones).

**Impact:** Operators relying on `aptos_txn_added_to_committed_time_seconds` will see a false-healthy latency picture. Actual P99/P100 degradation from slow transactions becomes invisible.

---

## High Findings

### HIGH-1: `record_added` Capacity Check is Racy — Concurrent Inserts Bypass Memory Limits

**File:** `crates/txn_metrics/src/lib.rs:213-222`

```rust
if self.txn_initial_add_time.len() >= MAX_TXN_INITIAL_ADD_TIME_CAPACITY {
    self.cleanup_old_entries();
}
// No re-check — insert proceeds unconditionally
self.txn_hash_to_key.insert(txn_hash, txn_key);
self.txn_initial_add_time.entry(txn_key).or_insert(now);
```

The capacity check and the insert are **not atomic**. Under high-throughput concurrent `record_added` calls (N validator threads processing mempool additions simultaneously):
1. Thread A checks `len() >= 100,000` → false (99,999 entries)
2. Threads B, C, D, ... all also check → false
3. All threads proceed to insert → map grows well beyond 100,000

Since `DashMap::len()` is also an approximation (it sums shard lengths non-atomically), the check can be stale even without race conditions. The 100,000 limit is effectively advisory, not enforced.

**Impact:** Under sustained load, `txn_initial_add_time` can grow unboundedly. Combined with Finding CRITICAL-1 (cleanup can deadlock), the fallback eviction path may never execute, making this a memory exhaustion vector.

### HIGH-2: `txn_hash_to_key` Grows Without Independent Bounds — Asymmetric Memory Exhaustion

**File:** `crates/txn_metrics/src/lib.rs:218`

`txn_hash_to_key` is inserted into at lines 218, 236, 368, and 396 — every time a transaction is seen in **any** lifecycle event. But it is only cleaned during `record_committed` (O(n) retain) or as a side-effect of `cleanup_old_entries` (also O(n) retain). There is **no independent capacity limit**.

An attacker can exploit this: submit many transaction variants with the **same `(sender, sequence_number)`** but different payloads. Each variant produces a different `committed_hash()` → a new entry in `txn_hash_to_key`. But `txn_initial_add_time` uses `entry().or_insert()` so it doesn't grow. The capacity check on `txn_initial_add_time` never triggers, and `txn_hash_to_key` grows without bound.

**Impact:** Controllable memory exhaustion on any node with `TXN_LIFE_ENABLED=true`. Each entry is `HashValue (32 bytes) + AccountAddress (32 bytes) + u64 (8 bytes) + DashMap overhead ≈ ~120 bytes`. 10M resubmissions ≈ 1.2 GB of leaked memory.

### HIGH-3: Quadratic Complexity in Commit Path — O(n × m) Performance DoS

**File:** `crates/txn_metrics/src/lib.rs:476-488`

```rust
self.txn_hash_to_key.retain(|_hash, key| key != &txn_key);  // O(m)
for mut entry in self.txn_batch_id.iter_mut() {               // O(batches)
    entry.value_mut().remove(&txn_key);                        // O(1) per set
}
self.txn_batch_id.retain(|_batch_id, txn_set| !txn_set.is_empty()); // O(batches)
for mut entry in self.txn_block_id.iter_mut() {               // O(blocks)
    entry.value_mut().remove(&txn_key);
}
self.txn_block_id.retain(|_block_id, txn_set| !txn_set.is_empty()); // O(blocks)
```

`record_committed` is called **per committed transaction**. Each call iterates the **entire** `txn_hash_to_key` map. If a block commits 1,000 transactions and `txn_hash_to_key` has 100,000 entries, this is **100 million iterations per block** just for this one map. The batch and block map iterations add further linear overhead.

**Impact:** On a validator under sustained load, the commit path latency grows quadratically with map size. Since metrics collection runs on the consensus critical path (not async/background), this directly degrades block finalization throughput. An attacker inflating `txn_hash_to_key` (via HIGH-2) amplifies this into a **consensus-level performance DoS**.

---

## Medium Findings

### MEDIUM-1: `into_verified()` Constructs "Verified" Transactions Without Any Verification

**File:** `bin/bench/src/txn.rs:42-52`

```rust
pub fn into_verified(self) -> VerifiedTxn {
    let bytes = self.to_bytes();
    let hash = simple_hash::hash_to_fixed_array(&bytes);
    VerifiedTxn::new(bytes, self.account, self.sequence_number, ExternalChainId::new(0), TxnHash::new(hash))
}
```

This constructs a `VerifiedTxn` (a type whose name implies cryptographic verification has occurred) with:
- **No signature check**
- **Hardcoded `ExternalChainId::new(0)`** — not the bench chain ID (1337) or any real chain
- **Hash computed from JSON serialization** — not from the canonical transaction encoding

The `VerifiedTxn::new()` constructor evidently does not enforce verification at the type level, meaning **any code path that trusts `VerifiedTxn` as "already verified" can be bypassed** by constructing one through this pattern.

**Risk:** If `VerifiedTxn::new()` is accessible outside test code (it's a public constructor in `gaptos`), any crate in the dependency tree can forge "verified" transactions. The type provides no verification guarantee — it's a naming lie that could propagate into production.

### MEDIUM-2: Unauthenticated HTTP Endpoint Controlling Consensus Behavior

**File:** `bin/bench/src/main.rs:139-143`

```rust
let route = warp::path!("ProduceTxn")
    .and(warp::query::<ProduceTxnQuery>())
    .and_then(handle_produce_txn);
warp::serve(route).run(([0, 0, 0, 0], port.expect("No port"))).await;
```

Binds to `0.0.0.0` with no authentication. The endpoint toggles transaction production on/off for a node running the **real `ConsensusEngine`**. Any network-reachable actor can disable transaction production, effectively taking the node offline from a consensus participation perspective.

### MEDIUM-3: `pending_txns` Hardcodes `account_seq_num: 1` — Sequence Number Tracking is Broken

**File:** `bin/bench/src/stateful_mempool.rs:70`

```rust
txns.push(VerifiedTxnWithAccountSeqNum { txn, account_seq_num: 1 })
```

Every transaction returned from `pending_txns()` reports sequence number 1 regardless of actual state. If this mempool were connected (the `ExecutionChannel` impl is commented out but structurally present), it would cause the consensus layer to believe all accounts are at sequence 1, breaking replay protection and nonce ordering entirely.

---

## Summary

| ID | Severity | Component | Finding |
|---|---|---|---|
| CRITICAL-1 | **Critical** | txn_metrics | DashMap lock-ordering inversion between `cleanup_old_entries` and `record_committed` — potential deadlock on consensus commit path |
| CRITICAL-2 | **Critical** | txn_metrics | Transactions >60s silently dropped from commit metrics — creates false-healthy latency picture (opposite of stated intention) |
| HIGH-1 | **High** | txn_metrics | Non-atomic capacity check allows unbounded growth under concurrent inserts |
| HIGH-2 | **High** | txn_metrics | `txn_hash_to_key` has no capacity limit — controllable memory exhaustion via transaction resubmission |
| HIGH-3 | **High** | txn_metrics | O(n×m) quadratic complexity in `record_committed` — consensus-path performance DoS |
| MEDIUM-1 | **Medium** | bin/bench | `VerifiedTxn::new()` constructs "verified" transactions without verification — type-level safety violation |
| MEDIUM-2 | **Medium** | bin/bench | Unauthenticated `0.0.0.0` HTTP endpoint controls consensus transaction production |
| MEDIUM-3 | **Medium** | bin/bench | Hardcoded `account_seq_num: 1` breaks sequence number tracking |

**Key Insight:** The two Critical findings are especially dangerous in combination. CRITICAL-2 means the metrics system is *designed* to silently discard the most operationally important data (slow transactions), while CRITICAL-1 means the cleanup mechanism that causes this data loss can also deadlock the consensus commit path. The metrics system, intended as a passive observer, becomes an active threat to node liveness and observability correctness.

---

## [gravity_state_computer_and_epoch_manager] Security audit of aptos-core/consensus/src/gravity_state_computer.rs and aptos-c

### 🎯 Intention

Security audit of aptos-core/consensus/src/gravity_state_computer.rs and aptos-core/consensus/src/epoch_manager.rs for: (1) how GravityBlockExecutor bridges consensus-agreed state to BlockBufferManager, checking for state divergence between what consensus commits and what reth executes; (2) epoch_manager.rs validator set transition logic including reconfiguration event handling, validator set update validation, and potential for unauthorized validator injection during epoch changes; (3) the interaction between epoch transitions in the consensus layer vs BlockBufferManager's epoch state machine; (4) recovery_manager.rs crash recovery correctness and potential for state corruption on restart.

### 🔍 Discovered Implementation

# Implementation Analysis: Gravity Consensus-to-Execution Bridge

## Files/Contracts Involved

| File | Description |
|------|-------------|
| `aptos-core/consensus/src/gravity_state_computer.rs` | `GravityBlockExecutor` — bridges consensus commits to reth via BlockBufferManager |
| `aptos-core/consensus/src/state_computer.rs` | `ExecutionProxy` — implements `StateComputer` trait, orchestrates execution and commit lifecycle |
| `aptos-core/consensus/src/consensus_provider.rs` | Construction site — wires `GravityBlockExecutor` into `ExecutionProxy` |
| `block_buffer_manager/` (external crate) | GCEI bridge to reth execution engine |
| `aptos-core/consensus/src/consensusdb/` | RocksDB for consensus-layer persistence (randomness data) |

---

## Data Structures

### `GravityBlockExecutor`
```rust
pub struct GravityBlockExecutor {
    inner: BlockExecutor,           // Aptos BlockExecutor wrapping AptosDB
    consensus_db: Arc<ConsensusDB>, // RocksDB for randomness persistence
    runtime: Runtime,               // Dedicated Tokio runtime ("tmp") for sync→async bridging
}
```

### `ExecutionProxy`
```rust
pub struct ExecutionProxy {
    executor: Arc<dyn BlockExecutorTrait>,                          // GravityBlockExecutor
    txn_notifier: Arc<dyn TxnNotifier>,                            // Mempool rejection notifier
    state_sync_notifier: Arc<dyn ConsensusNotificationSender>,
    async_state_sync_notifier: aptos_channels::Sender<NotificationType>, // Bounded(10)
    write_mutex: AsyncMutex<LogicalTime>,                          // Serializes commits
    state: RwLock<Option<MutableState>>,                           // Epoch-scoped config
    // ...
}
```

### `MutableState` (epoch-scoped, held under `RwLock`)
```rust
struct MutableState {
    validators: Arc<[AccountAddress]>,
    payload_manager: Arc<dyn TPayloadManager>,
    transaction_shuffler: Arc<dyn TransactionShuffler>,
    block_executor_onchain_config: BlockExecutorConfigFromOnchain,
    transaction_deduper: Arc<dyn TransactionDeduper>,
    is_randomness_enabled: bool,
}
```

---

## Key Functions

### `GravityBlockExecutor::commit_ledger()` — Primary Commit Path

```rust
fn commit_ledger(
    &self,
    block_ids: Vec<HashValue>,
    ledger_info_with_sigs: LedgerInfoWithSignatures,
    randomness_data: Vec<(u64, Vec<u8>)>,
) -> ExecutorResult<()>
```

**What it does:**
1. Increments `APTOS_COMMIT_BLOCKS` Prometheus counter
2. Extracts `block_id`, `block_hash`, `block_num`, `epoch` from ledger info
3. Panics if `block_ids` is empty
4. Persists randomness data to `ConsensusDB` (returns `Err` on failure — does not panic)
5. Constructs `BlockHashRef` per block — only the **last** block carries the EVM hash; earlier blocks get `hash: None`
6. Blocks on async runtime:
   - Calls `get_block_buffer_manager().set_commit_blocks(block_hash_refs, epoch)` — signals reth to finalize
   - Awaits **all** `persist_notifiers` (one-shot receivers, one per block) — waits for reth to confirm persistence
   - **After** reth confirms: calls `self.inner.db.writer.save_transactions(None, Some(&ledger_info), false)` — writes ledger info to AptosDB

### `GravityBlockExecutor::commit_blocks()` — Legacy Path

Same logic as `commit_ledger` except:
- No randomness persistence
- `save_transactions` is called **before** the async block (opposite ordering from `commit_ledger`)

### `ExecutionProxy::schedule_compute()` — Execution Path

```rust
async fn schedule_compute(
    &self, block: &Block, parent_block_id: HashValue,
    randomness: Option<Randomness>, lifetime_guard: CountedRequest<()>,
) -> StateComputeResultFut
```

**What it does:**
1. Fetches user transactions from `payload_manager`
2. Processes validator transactions (DKG transcripts → `ExtraDataType::DKG`, JWK updates → `ExtraDataType::JWK`)
3. Builds `ExternalBlockMeta` with `block_id`, `block_number`, `epoch`, `randomness`, `proposer_index`
4. Returns a pinned future that:
   - Calls `get_block_buffer_manager().set_ordered_blocks(parent_id, external_block)` — sends block to reth
   - Calls `get_block_buffer_manager().get_executed_res(block_id, block_number, epoch)` — waits for reth result
   - Notifies mempool of rejected transactions
   - Returns `PipelineExecutionResult` (pre-commit future is a no-op)

### `ExecutionProxy::commit()` — Commit Orchestrator

```rust
async fn commit(
    &self, blocks: &[Arc<PipelinedBlock>],
    finality_proof: LedgerInfoWithSignatures,
    callback: StateComputerCommitCallBackType,
) -> ExecutorResult<()>
```

**What it does:**
1. Acquires `write_mutex` (serializes all commits)
2. Collects payloads, block IDs, transactions, subscribable events, randomness data from all blocks
3. Spawns blocking task calling `executor.commit_ledger(block_ids, proof, randomness_data)` — panics on failure
4. Sends `(wrapped_callback, txns, events, block_number)` to `async_state_sync_notifier` channel
5. Background task drains channel: calls `state_sync_notifier.notify_new_commit()` then fires callback
6. Updates `write_mutex` logical time

### `ExecutionProxy::sync_to()` — State Sync

1. Acquires `write_mutex`
2. Calls `executor.finish()` (frees in-memory SMT)
3. Early returns if already past target
4. Calls `state_sync_notifier.sync_to_target(target)`
5. Calls `executor.reset()` (re-reads from DB)

### `ExecutionProxy::new_epoch()` / `end_epoch()`

- `new_epoch`: Atomically writes `MutableState` with new validator set, payload manager, shuffler, deduper, randomness flag
- `end_epoch`: Atomically clears `MutableState` (`state.write().take()`)

---

## Execution Path: Consensus → Reth

```
HotStuff QC formed
        │
        ▼
ExecutionProxy::schedule_compute()
  ├─ fetch txns from QuorumStore
  ├─ process validator txns (DKG/JWK) → ExtraDataType
  ├─ build ExternalBlock { meta, txns, extra_data }
  │
  ▼ (returned BoxFuture, when awaited)
BlockBufferManager::set_ordered_blocks(parent_id, block)
  → sends to reth via GCEI
        │
        ▼
BlockBufferManager::get_executed_res(block_id, block_number, epoch)
  → waits for reth execution result (StateComputeResult)
        │
        ▼
ExecutionProxy::commit()
  ├─ acquires write_mutex
  ├─ spawn_blocking → GravityBlockExecutor::commit_ledger()
  │     ├─ persist randomness → ConsensusDB
  │     ├─ BlockBufferManager::set_commit_blocks(block_hash_refs, epoch)
  │     │     → signal reth to finalize
  │     ├─ await all persist_notifiers (reth confirms on-disk)
  │     └─ AptosDB::save_transactions(ledger_info)  ← AFTER reth confirms
  ├─ send (callback, txns, events) → async_state_sync_notifier channel
  │     → background: notify_new_commit() then callback()
  └─ update write_mutex LogicalTime
```

---

## State Changes

| Storage | What Gets Written | When |
|---------|-------------------|------|
| **ConsensusDB** (RocksDB) | Randomness data `Vec<(u64, Vec<u8>)>` | `commit_ledger`, before BlockBufferManager call |
| **AptosDB** (RocksDB) | `LedgerInfoWithSignatures` (no transactions) | `commit_ledger`, **after** reth persistence confirmed |
| **Reth** (via BlockBufferManager) | Finalized blocks, EVM state | `set_commit_blocks` → reth pipeline |
| **In-memory** (`MutableState`) | Validator set, epoch config | `new_epoch` / `end_epoch` |
| **In-memory** (`write_mutex`) | `LogicalTime { epoch, round }` | After each `commit` / `sync_to` |

---

## Synchronization Mechanisms

| Mechanism | Purpose |
|-----------|---------|
| `AsyncMutex<LogicalTime>` (`write_mutex`) | Serializes `commit` and `sync_to`; prevents concurrent commit/sync race |
| `RwLock<Option<MutableState>>` (`state`) | Epoch-scoped state; write on `new_epoch`/`end_epoch`, read on every `schedule_compute`/`commit` |
| `tokio::runtime::Runtime` (in `GravityBlockExecutor`) | Bridges sync `BlockExecutorTrait` methods → async BlockBufferManager via `block_on` |
| `aptos_channels` bounded(10) | Async state-sync notification with backpressure |
| Oneshot `persist_notifiers` | Per-block confirmation from reth; `commit_ledger` blocks until all fire |
| `CountedRequest<()>` (`lifetime_guard`) | Ref-counted guard ensuring pipelined block outlives execution |

---

## External Dependencies

| Dependency | Call Site | Effect |
|------------|-----------|--------|
| `get_block_buffer_manager()` | `schedule_compute`, `commit_ledger`, `commit_blocks` | Global singleton; GCEI bridge to reth |
| `ConsensusDB::put_randomness()` | `commit_ledger` | Persists VRF/randomness data per round |
| `AptosDB::writer.save_transactions()` | `commit_ledger`, `commit_blocks` | Writes ledger info (finality proof) only; no transaction data |
| `TxnNotifier::notify_failed_txn()` | `schedule_compute` future | Notifies mempool of rejected transactions |
| `ConsensusNotificationSender::notify_new_commit()` | Background task from `commit` | Triggers state-sync pipeline |
| `PayloadManager::notify_commit()` | Wrapped callback in `commit` | Frees QuorumStore batch references |

---

## Notable Implementation Details

1. **`pre_commit_block` is a no-op** — returns `Ok(())` immediately. Gravity's pre-commit is handled on the reth side.

2. **`transactions_to_commit` skips validator transactions and block epilogue** — marked with `TODO(gravity_byteyue)`.

3. **JWK signature verification is deferred** — `TODO(Gravity): Check the signature here instead of execution layer`.

4. **Pre-commit futures in `commit()` are commented out** — `TODO(gravity_lightman): The take_pre_commit_fut will cause a coredump`.

5. **`commit_ledger` vs `commit_blocks` ordering difference**: In `commit_ledger` (production path), AptosDB write happens **after** reth persistence. In `commit_blocks` (legacy), AptosDB write happens **before**. The production path ensures the Aptos ledger is only updated once reth has confirmed on-disk persistence.

6. **Only the last block in a commit batch carries the EVM block hash** — earlier blocks in the `BlockHashRef` array have `hash: None`.

7. **All BlockBufferManager failures are hard panics** — `unwrap()` / `unwrap_or_else(panic!)`. Randomness persistence failures propagate as `Err`. The design philosophy: reth being unavailable is unrecoverable.

### 🛠️ Ground Review

# Ground Review (Blue Team) — Code Quality & Security Audit

## Consensus-to-Execution Bridge: `gravity_state_computer.rs` + `state_computer.rs` + `consensus_provider.rs`

---

## Critical

### C1. Silently Discarded Errors on `save_transactions` and `persist_notifier`
**Files:** `gravity_state_computer.rs:100-101`, `gravity_state_computer.rs:188-189`, `gravity_state_computer.rs:124-126`

```rust
let _ = self.inner.db.writer.save_transactions(None, Some(&ledger_info_with_sigs), false);
```
```rust
let _ = notifier.recv().await;
```

The `let _ =` pattern silently discards the `Result`. If `save_transactions` fails, the ledger info is never persisted to AptosDB — but the function returns `Ok(())`. Callers believe the commit succeeded. The same applies to `persist_notifier.recv()` — if the channel is dropped (reth crashed mid-persist), the receive returns `Err`, which is silently eaten. This creates a **silent data loss** scenario: consensus believes a block is committed, reth may have persisted it, but AptosDB is inconsistent.

**Severity: CRITICAL**

---

### C2. Inconsistent Commit Ordering Between `commit_ledger` and `commit_blocks`
**File:** `gravity_state_computer.rs:86-130` vs `gravity_state_computer.rs:139-192`

In `commit_ledger` (production path), AptosDB write happens **after** reth persistence confirmation. In `commit_blocks` (legacy path), AptosDB write happens **before** the async block (`save_transactions` at line 101, then `block_on` at line 103). If the legacy `commit_blocks` path is ever exercised, a crash between lines 101 and 127 leaves AptosDB ahead of reth — the inverse of the safety invariant the production path was designed to ensure. If this path is truly dead code, it should be removed or marked `#[deprecated]` / `unreachable!()`.

**Severity: CRITICAL**

---

### C3. Nested Runtime `block_on` — Potential Deadlock
**File:** `gravity_state_computer.rs:55-56`, `gravity_state_computer.rs:103`, `gravity_state_computer.rs:164`

`GravityBlockExecutor` creates a dedicated Tokio runtime named `"tmp"` and uses `runtime.block_on()` from within `BlockExecutorTrait` methods. These methods are called via `tokio::task::spawn_blocking` from `ExecutionProxy::commit()` (line 566). If `spawn_blocking` runs on the same runtime's blocking thread pool, and that pool is exhausted, the `block_on` inside the blocked task can deadlock — both the calling runtime and the inner `"tmp"` runtime may starve if the `get_block_buffer_manager()` calls route back through the outer runtime's channels.

The runtime name `"tmp"` suggests this was a quick workaround. The dedicated runtime mitigates the most obvious nesting issue, but the risk remains if `get_block_buffer_manager()` internally depends on tasks in the outer consensus runtime.

**Severity: CRITICAL**

---

### C4. Hard `unwrap()` on `set_commit_blocks` in `commit_ledger` Without Error Context
**File:** `gravity_state_computer.rs:184`

```rust
.unwrap();
```

The `commit_blocks` path (line 123) uses `unwrap_or_else(|e| panic!("Failed to push commit blocks {}", e))` which at least logs the error. The `commit_ledger` path uses a bare `.unwrap()`, producing a panic with no diagnostic context. In a blockchain node, this bare panic makes crash forensics significantly harder.

**Severity: CRITICAL** (in a consensus-critical path, panics must have maximum diagnostic context)

---

## Warning

### W1. JWK Signature Verification Deferred — Security Gap
**Files:** `state_computer.rs:284`, `state_computer.rs:360`

```rust
// TODO(Gravity): Check the signature here instead of execution layer
```

JWK updates from validators are accepted and forwarded to reth without cryptographic signature verification at the consensus layer. The `multi_sig` field in `QuorumCertifiedUpdate` is destructured but never checked. If the execution layer also fails to verify (or has a bug), malicious JWK payloads could be injected. This is a **deferred security control** with no compensating check visible in this code.

**Severity: WARNING**

---

### W2. `unwrap()` on BCS/JSON Serialization
**Files:** `state_computer.rs:272`, `state_computer.rs:298`, `state_computer.rs:303`, `state_computer.rs:313`, `state_computer.rs:348`, `state_computer.rs:376`, `state_computer.rs:391`

Multiple `bcs::to_bytes(&...).unwrap()` and `serde_json::to_vec(&...).unwrap()` calls. While these are unlikely to fail on well-formed data, a malformed or adversarially crafted JWK/DKG payload from a faulty or compromised validator could trigger a panic, crashing the consensus node.

**Severity: WARNING**

---

### W3. `get_block_txns` Swallows Payload Retrieval Errors
**File:** `state_computer.rs:127-129`

```rust
Err(e) => {
    warn!("failed to get transactions from block {:?}, error {:?}", block, e);
}
```

If `payload_manager.get_transactions()` fails, the block is executed with **zero transactions** instead of propagating the error. This means an empty block gets finalized on-chain — silently dropping user transactions. The block appears committed but contains nothing. This should return an error, not produce a phantom empty block.

**Severity: WARNING**

---

### W4. Duplicated Code: `process_validator_transactions` Exists as Both Method and Free Function
**Files:** `state_computer.rs:238-314` (methods) vs `state_computer.rs:318-392` (free functions)

The entire validator transaction processing logic is duplicated — once as `ExecutionProxy` methods and once as `_util` free functions. This is a maintenance hazard: a security fix applied to one copy but not the other creates a silent vulnerability. Only the `_util` variants are called in `schedule_compute` (line 408); the method variants appear unused.

**Severity: WARNING**

---

### W5. `MutableState` Read Under Panic — No Graceful Degradation
**Files:** `state_computer.rs:121`, `state_computer.rs:216`, `state_computer.rs:411`, `state_computer.rs:525`

```rust
self.state.read().as_ref().cloned().expect("must be set within an epoch")
```

Every access to `MutableState` panics if called between `end_epoch()` and `new_epoch()`. A single race condition or ordering mistake in the epoch transition machinery crashes the node. There is no sentinel logging before the panic.

**Severity: WARNING**

---

### W6. Bounded Channel Backpressure Can Stall Consensus Commits
**File:** `state_computer.rs:141-144`, `state_computer.rs:586-590`

```rust
gaptos::aptos_channels::new::<NotificationType>(10, ...)
```

The state-sync notification channel has capacity 10. The `commit()` flow sends to this channel at line 586 **while holding the `write_mutex`**. If the consumer (state-sync notifier + callback) falls behind by 10 blocks, the `send().await` blocks, and because `write_mutex` is held, all subsequent `commit()` and `sync_to()` calls are also blocked. This creates a head-of-line blocking scenario where a slow state-sync consumer stalls consensus finality entirely.

**Severity: WARNING**

---

### W7. `sync_to` Updates `latest_logical_time` Even on Failure
**File:** `state_computer.rs:632`

```rust
*latest_logical_time = logical_time;
```

This line executes unconditionally before the `res.map_err(...)` return on line 638. If `sync_to_target` fails, the logical time is still updated, which means a subsequent successful `commit()` at a lower round would be accepted (since no comparison is done in `commit`), or a retry of `sync_to` at the same target would be short-circuited by the guard at line 608. This can cause the node to believe it's synchronized when it isn't.

**Severity: WARNING**

---

### W8. `block_number` Arithmetic Could Underflow
**File:** `gravity_state_computer.rs:114`, `gravity_state_computer.rs:175`

```rust
num: block_num - (len - 1 - i) as u64,
```

If `block_num` is less than `(len - 1 - i)` (e.g., genesis edge case or corrupted ledger info), this performs unsigned subtraction underflow, wrapping to a very large block number. No bounds check is performed.

**Severity: WARNING**

---

## Info

### I1. Runtime Named `"tmp"` — Poor Naming
**File:** `gravity_state_computer.rs:55`

A dedicated Tokio runtime named `"tmp"` is used for the lifecycle of the node. This name obscures its actual purpose (sync→async bridging for BlockBufferManager) and will be confusing in runtime monitoring, thread dumps, and metrics dashboards.

### I2. Commented-Out Pre-Commit Futures With TODO
**File:** `state_computer.rs:527-530`, `state_computer.rs:545`, `state_computer.rs:557-559`

```rust
// TODO(gravity_lightman): The take_pre_commit_fut will cause a coredump.
```

Commented-out code referencing a coredump suggests an unresolved memory safety issue. The pre-commit path is currently a no-op, so this doesn't impact correctness, but a coredump-inducing code path warrants a tracking issue, not just a TODO comment.

### I3. Global Singleton `get_block_buffer_manager()`
**Files:** `gravity_state_computer.rs:104`, `state_computer.rs:452`, `state_computer.rs:465`

The block buffer manager is accessed as a global singleton. This makes testing, dependency injection, and lifecycle management harder. It also means there's no compile-time guarantee that initialization happens before first use.

### I4. `consensus_provider.rs` Uses `#[allow(clippy::unwrap_used)]` Module-Wide
**File:** `consensus_provider.rs:46`

Blanket suppression of the `unwrap_used` lint across the entire function body masks individual unwrap decisions that should be explicitly justified.

### I5. `callback()` Invoked After `notify_new_commit` — Ordering Assumption
**File:** `state_computer.rs:153-155`

The callback (which includes `payload_manager.notify_commit()`) runs after the state-sync notification. If `notify_new_commit` fails, the callback still fires (line 155). If the callback panics, the background task dies silently, stalling all future notifications on this channel.

---

## Summary

| Severity | Count | Key Themes |
|----------|-------|------------|
| **Critical** | 4 | Silent error swallowing on DB writes, inconsistent commit ordering, deadlock risk, bare panics in consensus path |
| **Warning** | 8 | Deferred security checks (JWK sigs), duplicated code, backpressure stalls, logical time corruption, underflow |
| **Info** | 5 | Naming, commented-out coredump code, global singletons, lint suppression |

The most urgent items are **C1** (silent `save_transactions` failure) and **C3** (nested runtime deadlock risk). C1 can cause silent state divergence between AptosDB and reth; C3 can cause a full consensus halt under specific threading conditions.

### ⚔️ Intention Attacker

# Intention Attack Report: Gravity Consensus-to-Execution Bridge

## Scope

Security audit of `gravity_state_computer.rs` and the consensus commit pipeline, focusing on state divergence between what consensus commits and what reth executes, crash recovery correctness, and epoch transition safety.

---

## Critical Vulnerabilities

### CRITICAL-1: Silent Discard of Persistence Failures Creates Phantom Commits

**Files:** `gravity_state_computer.rs:101`, `gravity_state_computer.rs:188-189`

```rust
let _ = self.inner.db.writer.save_transactions(None, Some(&ledger_info), false);
```

Both `commit_blocks` and `commit_ledger` discard the `Result` from `save_transactions` using `let _ =`. If the AptosDB write fails (disk full, RocksDB corruption, I/O error), the method returns `Ok(())` to the caller. Consensus believes the ledger info is persisted. It is not.

**Impact:** After a crash, the node cannot recover the finality proof for committed blocks. The consensus layer's view of "what is finalized" permanently diverges from what is on disk. This is an **irrecoverable state corruption** — the node will either be unable to restart or will fork from the network.

**The intention was:** Persist the finality proof reliably. The implementation achieves the **opposite** — it silently succeeds even when persistence fails.

---

### CRITICAL-2: Inconsistent State Mutation Ordering Between Commit Paths Enables State Divergence on Crash

**Files:** `gravity_state_computer.rs:100-127` vs `gravity_state_computer.rs:164-190`

The two commit paths have **reversed ordering** of critical state mutations:

| Step | `commit_blocks` (legacy) | `commit_ledger` (production) |
|------|-------------------------|------------------------------|
| 1 | `save_transactions` (AptosDB) | `set_commit_blocks` (notify reth) |
| 2 | `set_commit_blocks` (notify reth) | await `persist_notifiers` (reth confirms) |
| 3 | await `persist_notifiers` | `save_transactions` (AptosDB) |

In the **production path** (`commit_ledger`), reth finalizes blocks and confirms on-disk persistence **before** the AptosDB ledger info is written. A crash between steps 2 and 3 means:

- Reth has finalized and persisted the blocks (irreversible)
- AptosDB has **no record** of the finality proof
- On restart, consensus has no finality proof for blocks that reth considers final

Combined with CRITICAL-1 (the `save_transactions` error being discarded), even a non-crash failure at step 3 goes undetected.

**The intention was:** Ensure atomicity between reth finalization and ledger info persistence. The implementation creates a **crash window** where the two stores permanently diverge.

---

### CRITICAL-3: Persist Notification Failures Silently Ignored — Commit Proceeds Without Confirmation

**Files:** `gravity_state_computer.rs:124-126`, `gravity_state_computer.rs:185-187`

```rust
let _ = notifier.recv().await;
```

The `persist_notifiers` are oneshot channels that reth uses to confirm on-disk persistence. If the sender is dropped (reth panics, channel error), `recv()` returns `Err`. This error is silently discarded with `let _ =`.

**Impact:** The commit path proceeds as if reth confirmed persistence when it may not have. The node's AptosDB records a finality proof for blocks that reth never actually persisted. After restart, reth's state is behind the consensus layer's view — a **permanent fork**.

**The intention was:** Wait for reth to confirm persistence before proceeding. The implementation **ignores the confirmation result**, defeating the entire purpose of the two-phase commit pattern.

---

## High Vulnerabilities

### HIGH-1: Integer Underflow in Block Number Arithmetic

**Files:** `gravity_state_computer.rs:114`, `gravity_state_computer.rs:175`

```rust
block_num - (len - 1 - i) as u64
```

If `block_num < (len - 1)`, this underflows. In debug mode it panics; in release mode it wraps to `u64::MAX - delta`, producing a nonsensical block number that gets sent to reth via `BlockHashRef`. Reth would attempt to finalize a block at an astronomically high number.

**Precondition:** A batch of blocks is committed where the first block's expected number would be negative. This could happen during epoch boundaries or state sync edge cases where `block_num` is reset or is lower than expected relative to the batch size.

---

### HIGH-2: All BlockBufferManager Failures Are Hard Panics — No Graceful Degradation

**Files:** `gravity_state_computer.rs:123`, `gravity_state_computer.rs:184`

```rust
// commit_blocks path - at least has an error message
.unwrap_or_else(|e| panic!("Failed to push commit blocks {}", e))

// commit_ledger path - bare unwrap, no diagnostics
.unwrap()
```

Any transient failure in the BlockBufferManager (network hiccup to reth, temporary resource exhaustion) immediately crashes the node. There is no retry logic, no circuit breaker, no graceful degradation. The `commit_ledger` path doesn't even log the error before crashing.

**Impact:** A single transient reth failure takes down the validator. In a scenario where multiple validators experience the same reth issue simultaneously, this could halt consensus.

---

### HIGH-3: Only Last Block in Commit Batch Carries EVM Hash — Earlier Blocks Are Unverifiable

**Files:** `gravity_state_computer.rs:108-120`, `gravity_state_computer.rs:169-181`

```rust
if i == len - 1 {
    hash: Some(block_hash),  // only the last block
} else {
    hash: None,              // all earlier blocks
}
```

In a multi-block commit, only the final block carries `block_hash`. All preceding `BlockHashRef` entries have `hash: None`. This means:

- Reth cannot independently verify the identity of earlier blocks in the batch
- If block ordering within the batch is corrupted (e.g., by a bug in the `block_ids` vector construction), reth will finalize the wrong blocks at the wrong numbers with no way to detect the mismatch
- The EVM block hash linkage (parent → child) is broken for all but the last block

---

## Medium Vulnerabilities

### MEDIUM-1: `block_on` Inside Dedicated Runtime — Fragile Sync→Async Bridge

**Files:** `gravity_state_computer.rs:103`, `gravity_state_computer.rs:164`

The `GravityBlockExecutor` spawns a dedicated Tokio runtime named `"tmp"` and uses `self.runtime.block_on()` to bridge sync trait methods to async BlockBufferManager calls. If `commit_blocks` or `commit_ledger` is ever called from within an async context (e.g., via `spawn_blocking` from `ExecutionProxy::commit`), `block_on` on the **same** runtime would deadlock. The separate runtime mitigates this, but the pattern is fragile and the runtime name `"tmp"` provides no diagnostic value.

### MEDIUM-2: Randomness Persistence Succeeds But Commit Fails — Orphaned Randomness Data

**Files:** `gravity_state_computer.rs:158-161`

In `commit_ledger`, randomness is persisted to `ConsensusDB` **before** the BlockBufferManager call. If the subsequent `set_commit_blocks` panics (HIGH-2), the randomness data is persisted for a commit that never completed. On restart, `ConsensusDB` contains randomness entries for rounds that were never finalized — a consistency violation between the two RocksDB instances.

---

## Attack Scenario: The Silent Corruption Chain

Combining CRITICAL-1, CRITICAL-2, and CRITICAL-3 into a single attack narrative:

1. Consensus commits a batch of 3 blocks via `commit_ledger`
2. Randomness is persisted to ConsensusDB (succeeds)
3. `set_commit_blocks` notifies reth (succeeds)
4. Reth begins finalization but the sender side of one `persist_notifier` is dropped due to a reth internal error
5. `let _ = notifier.recv().await` — **error silently discarded** (CRITICAL-3)
6. `save_transactions` is called — **disk is full, write fails**
7. `let _ = self.inner.db.writer.save_transactions(...)` — **error silently discarded** (CRITICAL-1)
8. `commit_ledger` returns `Ok(())`
9. Consensus believes 3 blocks are committed and finalized
10. Node crashes (OOM from full disk, or any other reason)
11. On restart:
    - ConsensusDB has randomness for rounds that "committed"
    - AptosDB has **no finality proof** for those blocks
    - Reth may or may not have the blocks (notification was dropped)
    - **The node cannot recover to a consistent state**

---

## Summary

| ID | Finding | Severity |
|----|---------|----------|
| CRITICAL-1 | `save_transactions` result silently discarded — phantom commits | **Critical** |
| CRITICAL-2 | Inconsistent mutation ordering between commit paths — crash window | **Critical** |
| CRITICAL-3 | Persist notification failures silently ignored — false confirmation | **Critical** |
| HIGH-1 | Integer underflow in block number arithmetic | **High** |
| HIGH-2 | All BlockBufferManager failures are hard panics | **High** |
| HIGH-3 | Only last block in batch carries EVM hash — unverifiable intermediates | **High** |
| MEDIUM-1 | Fragile `block_on` sync→async bridge | **Medium** |
| MEDIUM-2 | Orphaned randomness data on commit failure | **Medium** |

The core design flaw is that `commit_ledger` attempts a **two-phase commit** across three independent storage systems (ConsensusDB, AptosDB, Reth) without any transactional guarantees or rollback mechanism. Each `let _ =` is a hole in the safety net. The system **cannot distinguish** "fully committed" from "partially committed" after a crash, making recovery inherently unsafe.

---

## [qc_aggregator_and_vote_validation] Security audit of aptos-core/consensus/src/qc_aggregator.rs, aptos-core/consensu

### 🎯 Intention

Security audit of aptos-core/consensus/src/qc_aggregator.rs, aptos-core/consensus/src/pending_votes.rs, and aptos-core/consensus/src/pending_order_votes.rs for: (1) BLS signature aggregation correctness and resistance to rogue-key attacks; (2) quorum certificate construction thresholds and off-by-one errors in stake-weighted vote counting; (3) duplicate vote detection and equivocation evidence handling; (4) order vote aggregation logic for pipelined consensus; (5) any gravity-sdk modifications to upstream vote validation that could weaken safety guarantees.

### 🔍 Discovered Implementation

# Security Audit Report: Vote Aggregation & Quorum Certificate Construction

**Scope:** `qc_aggregator.rs`, `pending_votes.rs`, `pending_order_votes.rs`
**Codebase:** gravity-sdk (forked from aptos-core)

---

## 1. BLS Signature Aggregation — Correctness & Rogue-Key Resistance

### Flow

```
Vote::new() → validator_signer.sign(&ledger_info) → bls12381::Signature
  → PendingVotes::insert_vote()
    → li_with_sig.add_signature(author, signature)     // PartialSignatures: BTreeMap<Address, Sig>
    → [quorum reached] li_with_sig.aggregate_signatures(&validator_verifier)
      → validator_verifier.aggregate_signatures(partial_sig.signatures_iter())
        → AggregateSignature { bitmask + aggregated_bls_sig }
          → QuorumCert::new(vote_data, ledger_info_with_sig)
```

### Assessment

**No rogue-key vulnerability.** The system uses proof-of-possession (PoP) at validator registration time (standard Aptos BLS12-381 implementation). Each validator's public key is bound to their identity via PoP before joining the validator set. The `ValidatorVerifier` only contains PoP-verified public keys — the standard mitigation against rogue-key attacks in BLS multi-signature schemes.

**Signature binding is sound.** Each `Vote` signs the `LedgerInfo` (which contains `consensus_data_hash = vote_data.hash()`), and `Vote::verify()` checks `ledger_info.consensus_data_hash() == vote_data.hash()` before verifying the BLS signature. This prevents signature-vote_data unbinding attacks.

**Verdict: PASS**

---

## 2. Quorum Threshold & Stake-Weighted Vote Counting

### Threshold Calculation

```rust
quorum_voting_power = (total_voting_power * 2 / 3) + 1   // strict > 2/3
```

Standard BFT threshold (n = 3f+1, quorum = 2f+1). The `+1` ensures **strict** supermajority.

### Vote Power Checking

At `pending_votes.rs:195`:
```rust
validator_verifier.check_voting_power(li_with_sig.signatures().keys(), true)
```

The `true` parameter enforces the supermajority quorum threshold. The method sums voting power across all provided signers, rejecting unknown authors.

### Off-by-One Analysis

**`NoDelayQcAggregator::handle_aggregated_qc` (line 53):**
```rust
assert!(aggregated_voting_power >= validator_verifier.quorum_voting_power());
```
Uses `>=`, consistent with `check_voting_power` which returns `Ok(power)` only when `power >= quorum_voting_power`. **No off-by-one.**

**`pending_order_votes.rs:98`:** Same pattern. **No off-by-one.**

### Echo Timeout Threshold (f+1)

At `pending_votes.rs:257-258`:
```rust
let f_plus_one = validator_verifier.total_voting_power()
    - validator_verifier.quorum_voting_power() + 1;
```

With `quorum = floor(2n/3) + 1`, this yields `f_plus_one = n - floor(2n/3)`. For `n = 3f+1`: `f_plus_one = f+1`. **Correct.**

**Verdict: PASS**

---

## 3. Duplicate Vote Detection & Equivocation Handling

### `PendingVotes::insert_vote()` (pending_votes.rs:108-142)

The `author_to_vote: HashMap<Author, (Vote, HashValue)>` map tracks one vote per author per round.

**Detection logic:**
1. If `author` has voted before AND `li_digest == previous_li_digest` → **DuplicateVote** (unless it's a new timeout enhancement)
2. If `author` has voted before AND `li_digest != previous_li_digest` → **EquivocateVote** (logs `SecurityEvent::ConsensusEquivocatingVote`)

The original vote's signature remains in `li_digest_to_votes` and continues to count toward quorum. The second conflicting vote is rejected. Voting power is never double-counted. **Correct.**

---

## 4. Order Vote Aggregation for Pipelined Consensus

### Flow

```
PendingOrderVotes::insert_order_vote()
  → OrderVoteStatus::NotEnoughVotes: add signature, check quorum
    → [quorum reached] aggregate_signatures → OrderVoteStatus::EnoughVotes (cached)
  → OrderVoteStatus::EnoughVotes: return cached LedgerInfoWithSignatures
```

### State Transition

Once `EnoughVotes` is reached (line 103-104), the status is cached. Subsequent order votes for the same digest return the cached result immediately without adding more signatures.

### Garbage Collection

```rust
pub fn garbage_collect(&mut self, highest_ordered_round: u64)
```

Removes all entries at or below the highest ordered round. **Sound.**

---

## 5. Gravity-SDK Modifications to Upstream Aptos

The `gaptos` crate wraps upstream Aptos types. All cryptographic primitives are imported through `gaptos::aptos_types::*` rather than directly from `aptos-types`. The actual implementations of `check_voting_power`, `quorum_voting_power`, and `aggregate_signatures` live inside this compiled dependency.

The delayed QC aggregation feature from upstream Aptos has been removed — only `NoDelay` is supported. QCs are always formed at the minimum quorum threshold.

---

## Findings

### S-01 — Equivocation Logged But No Slashing Proof Constructed (INFORMATIONAL)

**Location:** `pending_votes.rs:131-141`

When equivocation is detected, the event is logged via `SecurityEvent::ConsensusEquivocatingVote` but no equivocation proof (containing both conflicting votes) is constructed or forwarded to any slashing mechanism. The conflicting votes are available in-memory — the original in `author_to_vote` and the new one in the `vote` parameter — but are only logged, not packaged as evidence.

---

### S-02 — No Equivocation Detection in `PendingOrderVotes` (MEDIUM)

**Location:** `pending_order_votes.rs` (entire file)

Unlike `PendingVotes`, the `PendingOrderVotes` struct has **no `author_to_vote` map** and performs **no duplicate or equivocation checking**. A validator can sign conflicting order votes for different `LedgerInfo` digests in the same round, contributing valid signatures to multiple competing aggregations simultaneously, with no detection or logging.

The voting power arithmetic remains correct — `add_signature` uses the author as a BTreeMap key so re-submission overwrites rather than double-counts, and `check_voting_power` iterates deduplicated keys. But the complete absence of equivocation detection means Byzantine validators can sign conflicting states without any evidence being captured.

---

### S-03 — Signature Verification Not Performed Inside `insert_order_vote` (HIGH — Conditional)

**Location:** `pending_order_votes.rs:92`

```rust
li_with_sig.add_signature(order_vote.author(), order_vote.signature().clone());
```

The method checks that the author exists in the validator set (line 81-84) and checks voting power, but **never verifies the BLS signature against the author's public key**. The `OrderVote::verify()` method exists but is not called within `insert_order_vote`.

The same pattern exists in `PendingVotes::insert_vote()` — `Vote::verify()` is not called inside insertion either. In both cases, verification is assumed to happen at the network message processing layer before these methods are called. **This is architecturally standard for Aptos.**

**However**, this represents a single-layer defense with no defense-in-depth. If any code path reaches `insert_order_vote` without prior `verify()`, an attacker could inject forged signatures that would be aggregated into an invalid `LedgerInfoWithSignatures`. The severity depends entirely on whether all callers are verified to perform upstream signature checks.

---

### S-04 — Delayed QC Aggregation Removed (LOW)

**Location:** `qc_aggregator.rs:39-41`

```rust
match qc_aggregator_type {
    QcAggregatorType::NoDelay => Box::new(NoDelayQcAggregator {}),
}
```

Upstream Aptos supports a `DelayedQcAggregator` that waits to collect additional signatures beyond the minimum quorum. Gravity-sdk has removed this entirely. QCs are always formed with the minimum quorum of signatures. The `time_service` and `delayed_qc_tx` parameters are accepted but unused.

This reduces the aggregate signature's "weight" (fewer signers) which may affect light-client verification robustness, but **does not affect safety** — the BFT threshold is still met.

---

### S-05 — `gaptos` Wrapper Hides Upstream Crypto From Source Audit (INFORMATIONAL)

**Location:** All files (imports via `gaptos::aptos_types::*`)

The `gaptos` crate's re-exports of `ValidatorVerifier`, `AggregateSignature`, `PartialSignatures`, and `LedgerInfoWithVerifiedSignatures` should be independently verified to confirm they are unmodified from upstream Aptos. Any modification to the quorum threshold calculation or signature aggregation within `gaptos` could silently weaken consensus safety.

---

## Summary Table

| ID | Severity | Finding |
|----|----------|---------|
| **S-01** | Info | Equivocation logged but no slashing proof constructed |
| **S-02** | **Medium** | No equivocation detection in `PendingOrderVotes` |
| **S-03** | **High** | No signature verification inside `insert_order_vote`; single-layer caller-contract defense |
| **S-04** | Low | Delayed QC aggregation removed; QCs formed at minimum quorum |
| **S-05** | Info | `gaptos` wrapper hides upstream crypto from source audit |

## Overall Assessment

The vote aggregation pipeline is **fundamentally sound** for BFT safety:
- Quorum threshold is correct (`> 2/3` strict supermajority)
- No double-counting of voting power (BTreeMap keyed by author)
- BLS aggregation uses PoP-verified keys (rogue-key resistant)
- No off-by-one errors in threshold comparisons

The two actionable findings are **S-02** (add equivocation detection to `PendingOrderVotes` to match `PendingVotes` behavior) and **S-03** (audit all callers of `insert_order_vote` to confirm upstream signature verification, or add defense-in-depth verification inside the method).

### 🛠️ Ground Review

# Ground Review (Blue Team): Code Quality & Engineering Safety

**Scope:** `qc_aggregator.rs`, `pending_votes.rs`, `pending_order_votes.rs`
**Focus:** Resource leaks, naming, error handling, concurrency, panics, input validation

---

## Critical

### CR-01 — Bare `panic!` in `process_delayed_qc` Reachable on Timing Edge Case

**File:** `pending_votes.rs:303`

```rust
Err(VerifyError::TooLittleVotingPower { .. }) => {
    panic!("Delayed QC aggregation should not be triggered if we don't have enough votes to form a QC");
}
```

`PendingVotes` is replaced wholesale on each round change (no incremental GC — the struct is created fresh via `PendingVotes::new()`). If a delayed QC timer fires *after* its round's `PendingVotes` has been discarded and a stale message reaches `process_delayed_qc`, this `panic!` will crash the consensus task. Even if the current code path makes this unlikely, a bare `panic!` on a `VerifyError` variant (which is a recoverable error by nature) is a process-killing hazard in production.

**Recommendation:** Replace with `error!()` + return a graceful `VoteReceptionResult` error variant. Never `panic!` on conditions that could arise from message-timing races.

---

### CR-02 — Single-Layer Signature Verification Contract (No Defense-in-Depth)

**Files:** `pending_votes.rs:191`, `pending_order_votes.rs:92`

Neither `insert_vote` nor `insert_order_vote` calls `Vote::verify()` / `OrderVote::verify()`. Signatures are accumulated raw:

```rust
li_with_sig.add_signature(vote.author(), vote.signature().clone());
```

Bulk cryptographic verification only happens lazily at quorum threshold (`aggregate_signatures`). Both `Vote::verify()` and `OrderVote::verify()` exist and are well-implemented — they are simply never called inside the insertion path.

The entire security model depends on `round_manager.rs` always calling `verify()` before `insert_*vote()`. This is an **implicit caller contract** with no compile-time or runtime enforcement. A single missed code path (new feature, refactor, network handler addition) silently breaks the invariant and allows forged signatures to be aggregated into a QC.

**Recommendation:** Add a `debug_assert!` or a verified-wrapper type (e.g., `VerifiedVote` / `VerifiedOrderVote` that can only be constructed through `verify()`) so the type system enforces the contract. This is the standard Aptos upstream pattern for network message types.

---

## Warning

### WR-01 — `assert!` Guards on Internal Invariants Will Kill the Consensus Task

**Files:** `qc_aggregator.rs:52`, `pending_order_votes.rs:97`

```rust
assert!(
    aggregated_voting_power >= validator_verifier.quorum_voting_power(),
    "QC aggregation should not be triggered if we don't have enough votes to form a QC"
);
```

These `assert!` calls guard internal arithmetic invariants (the caller already checked `check_voting_power`). In theory they should never fire. In practice, `assert!` in async consensus code means a single invariant violation — however transient — crashes the entire consensus task rather than allowing graceful recovery or restart.

**Recommendation:** Convert to `error!` + early return (or at minimum `debug_assert!`) for production builds. Reserve `assert!` for test code.

---

### WR-02 — No Equivocation Detection in `PendingOrderVotes`

**File:** `pending_order_votes.rs` (entire file)

`PendingVotes` maintains `author_to_vote: HashMap<Author, (Vote, HashValue)>` for duplicate/equivocation detection. `PendingOrderVotes` has **no equivalent tracking**. A Byzantine validator can submit conflicting order votes for different `LedgerInfo` digests in the same round with no detection or logging. The unit test at line 194–198 confirms: inserting the same order vote a second time returns `VoteAdded`, not `DuplicateVote`.

Voting power arithmetic is still correct (BTreeMap keying by author prevents double-counting within a single digest bucket), but:
- No equivocation evidence is captured for forensics/slashing
- Different digest buckets can each contain a valid signature from the same Byzantine author, allowing simultaneous contribution to competing aggregations

**Recommendation:** Add an `author_to_order_vote` map mirroring the `PendingVotes` pattern, with `SecurityEvent` logging on equivocation.

---

### WR-03 — Redundant `.expect()` After Explicit `is_none()` Guard

**File:** `pending_order_votes.rs:82–87`

```rust
if validator_voting_power.is_none() {
    // ... return UnknownAuthor
}
let validator_voting_power =
    validator_voting_power.expect("Author must exist in the validator set.");
```

The `.expect()` is unreachable given the guard above. While not a runtime risk, `expect()` with a message string implies the condition is meaningful — this is misleading.

**Recommendation:** Use `.unwrap()` (the guard guarantees `Some`) or, better, refactor to use `if let` / `match` to eliminate the redundant check entirely.

---

## Info

### IN-01 — `garbage_collect` Boundary Uses Strict `>` — Subtle Edge Condition

**File:** `pending_order_votes.rs:132–141`

```rust
li_with_sig.ledger_info().round() > highest_ordered_round
```

Entries at **exactly** `highest_ordered_round` are dropped, including `EnoughVotes` entries that may still be queried by downstream code shortly after GC runs. If any code path calls `has_enough_order_votes()` for a round equal to `highest_ordered_round` after GC, it will get a false negative, potentially triggering redundant re-aggregation work.

This is likely by-design but the boundary semantics are undocumented. A brief inline comment would prevent future confusion.

---

### IN-02 — Equivocation Logged But No Proof Object Constructed

**File:** `pending_votes.rs:131–141`

Both the original vote (in `author_to_vote`) and the conflicting vote (the `vote` parameter) are available in-memory when equivocation is detected. Only a structured log is emitted — no `EquivocationProof` object is constructed or forwarded. If a slashing mechanism is ever added, this will need to be retrofitted.

---

### IN-03 — `gaptos` Re-Export Layer Obscures Crypto Implementation

**Files:** All three files import via `gaptos::aptos_types::*`

The `gaptos` crate acts as a re-export layer for upstream Aptos types including `ValidatorVerifier`, `PartialSignatures`, and `LedgerInfoWithVerifiedSignatures`. Any modification within this layer (e.g., altered `quorum_voting_power()` or `aggregate_signatures()` behavior) would be invisible at the source level of the files under review. Not a code quality issue per se, but an audit coverage gap.

---

### IN-04 — Unused Parameters in `create_qc_aggregator`

**File:** `qc_aggregator.rs:39–41`

```rust
pub fn create_qc_aggregator(
    qc_aggregator_type: QcAggregatorType,
    _time_service: ...,     // unused
    _delayed_qc_tx: ...,    // unused
) -> Box<dyn QcAggregator> {
```

The `time_service` and `delayed_qc_tx` parameters exist for the removed `DelayedQcAggregator`. They are prefixed with `_` to suppress warnings but remain in the function signature. Dead parameters increase API surface and cognitive load.

---

## Summary

| ID | Severity | Finding |
|----|----------|---------|
| **CR-01** | **Critical** | Bare `panic!` in `process_delayed_qc` on recoverable timing race |
| **CR-02** | **Critical** | No defense-in-depth for signature verification — implicit caller contract only |
| **WR-01** | Warning | `assert!` in async consensus code crashes task on invariant violation |
| **WR-02** | Warning | No equivocation detection in `PendingOrderVotes` |
| **WR-03** | Warning | Redundant `.expect()` after explicit `is_none()` guard |
| IN-01 | Info | GC boundary condition undocumented |
| IN-02 | Info | Equivocation proof not constructed for evidence forwarding |
| IN-03 | Info | `gaptos` re-export layer hides upstream crypto from source audit |
| IN-04 | Info | Dead parameters in `create_qc_aggregator` signature |

**Bottom line:** The data structures are sound (no resource leaks, no double-counting, correct threshold math). The two critical findings are about **crash resilience** (bare panics on recoverable conditions in async consensus code) and **missing type-level enforcement** of the signature-verification-before-insertion contract. Both are fixable without architectural changes.

### ⚔️ Intention Attacker

# Red Team Attack Report: Vote Aggregation & Quorum Certificate Construction

**Target:** gravity-sdk consensus layer (`pending_votes.rs`, `pending_order_votes.rs`, `qc_aggregator.rs`)
**Methodology:** Adversarial analysis of implementation report against verified source code

---

## ATTACK-1: Unverified Order Vote Signature Injection (CRITICAL)

**Location:** `round_manager.rs:1117-1161` → `pending_order_votes.rs:56-129`

**The report (S-03) understated this as "HIGH — Conditional." It is worse than described.**

The implementation report correctly identified that `insert_order_vote` does not call `order_vote.verify()`. However, my code trace reveals the full attack surface:

**Verified call chain:**
```
Network → ConsensusMsg::OrderVoteMsg 
  → UnverifiedEvent::OrderVoteMsg 
    → UnverifiedEvent::verify() [calls v.verify_order_vote(validator)?]
      → VerifiedEvent::OrderVoteMsg
        → process_order_vote_msg()
          → insert_order_vote()  ← NO individual sig verification here
```

The outer `UnverifiedEvent::verify()` gate is the **sole** cryptographic barrier. But critically:

1. **The `self_message` bypass** (`round_manager.rs` lines 99-169): When `self_message == true`, **all cryptographic verification is skipped**, and the message is promoted directly to `VerifiedEvent`. If any internal code path incorrectly marks a received message as `self_message`, forged signatures flow straight into aggregation.

2. **No defense-in-depth inside `insert_order_vote`**: Unlike a well-layered design, there is zero per-signature verification at the accumulation layer. Signatures are blindly added via `li_with_sig.add_signature(author, signature.clone())`. An invalid signature is only detected at `aggregate_signatures()` time — **but only if quorum is reached**. If an attacker submits `f` forged order votes (below quorum), they are silently stored with no detection, no logging, and no alert.

3. **Poisoning attack**: A Byzantine validator sends an `OrderVoteMsg` with a valid author identity but a corrupted BLS signature. The forged signature is stored in `PartialSignatures`. When legitimate votes later bring the count to quorum, `aggregate_signatures()` is called — and **fails**. The result: `ErrorAggregatingSignature`. Legitimate order votes for that digest are now permanently poisoned. New honest votes for the same `li_digest` hit the `EnoughVotes` early-exit check and are discarded, or the corrupted `NotEnoughVotes` entry contains the poisoned partial signature set. **The attacker has denied liveness for that ordering round with a single forged message.**

**Severity: CRITICAL** — A single Byzantine validator can deny liveness of order vote aggregation for any round by submitting one forged signature before honest votes arrive. The poisoned entry persists until garbage collection.

**Contrast with `pending_votes.rs`:** The same pattern exists for regular votes (`insert_vote` also doesn't verify individually), but regular votes have equivocation detection that at least limits an author to one stored signature per round. Order votes have **no such protection** (Attack-2 below).

---

## ATTACK-2: Order Vote Equivocation — Silent Double-Signing with No Detection (HIGH)

**Location:** `pending_order_votes.rs` — entire file (no `author_to_vote` map)

The report identified this as S-02 (Medium). **It should be HIGH** given the concrete attack:

**Attack scenario:**
1. Byzantine validator V signs `OrderVote` for `LedgerInfo` digest A (ordering block X)
2. Byzantine validator V signs a **conflicting** `OrderVote` for `LedgerInfo` digest B (ordering block Y) in the same round
3. Both pass `UnverifiedEvent::verify()` (both are validly signed)
4. Both reach `insert_order_vote()`:
   - Vote for digest A → creates entry in `li_digest_to_votes[A]`, adds V's signature
   - Vote for digest B → creates entry in `li_digest_to_votes[B]`, adds V's signature
5. V's voting power now **counts toward two competing orderings simultaneously**

**Impact:** While `add_signature` uses a BTreeMap keyed by author (preventing double-counting within a single digest), the validator's power counts separately across different digests. With `f` Byzantine validators all double-signing, they contribute `f` units of voting power to **both** competing orderings. This lowers the effective threshold honest validators need to reach quorum on either side — or, in adversarial conditions near the `2f+1` boundary, could allow two conflicting orderings to each independently reach quorum.

**Concrete risk:** In a network with `3f+1 = 100` validators (f=33), Byzantine validators sign both digest A and B. They need only `2f+1 = 67` signatures per digest, but 33 Byzantine validators already contribute to both. Honest validators need only `67 - 33 = 34` to accidentally form a quorum on both sides. If honest validators are split (e.g., due to network partition or timing), **two conflicting `LedgerInfoWithSignatures` can be produced for the same round.**

**No evidence is captured.** Unlike `PendingVotes`, there is no `SecurityEvent::ConsensusEquivocatingVote` logged. The double-signing is completely invisible.

**Severity: HIGH** — Equivocation in order votes can contribute to conflicting quorum certificates, and leaves no forensic trail.

---

## ATTACK-3: Poisoned Aggregation Cache Denial-of-Service (MEDIUM)

**Location:** `pending_order_votes.rs:103-104` (the `EnoughVotes` caching)

**Attack scenario leveraging Attack-1:**

```rust
// line 68-72: early return for cached results
OrderVoteStatus::EnoughVotes(li_with_sig) => {
    return OrderVoteReceptionResult::NewLedgerInfoWithSignatures(
        li_with_sig.clone(),
    );
}
```

Once `aggregate_signatures` succeeds and `EnoughVotes` is cached, the cached `LedgerInfoWithSignatures` is returned for **all subsequent** votes with the same digest. But consider the inverse:

1. If an attacker's forged vote is submitted **first** and quorum is not yet reached, the entry is `NotEnoughVotes` with a poisoned `PartialSignatures` set.
2. Legitimate votes arrive and push the count to quorum.
3. `aggregate_signatures()` is called on the set **including the forged signature** → fails with `ErrorAggregatingSignature`.
4. The entry remains as `NotEnoughVotes` with the poisoned partial signatures.
5. Subsequent legitimate votes for the same digest call `add_signature`, which **overwrites** the attacker's entry only if it's the same author. If the attacker used their own valid author identity with a forged signature, and then a legitimate vote from the same author arrives, the BTreeMap key collision **replaces** the forged signature. But if the attacker is a distinct Byzantine validator, the poisoned signature persists.
6. Each time quorum is re-checked after adding a new legitimate vote, `aggregate_signatures` fails again because the poisoned signature is still in the set.

**Result:** The attacker permanently blocks order vote aggregation for that digest until garbage collection removes it. There is no mechanism to evict a poisoned signature from `PartialSignatures` once inserted.

**Severity: MEDIUM** — Liveness degradation, not safety violation. GC will eventually clean up, but the attacker can re-poison each new round.

---

## ATTACK-4: `self_message` Flag Trust Boundary Violation (MEDIUM)

**Location:** `round_manager.rs` lines 99-169, `UnverifiedEvent::verify()`

```rust
if !self_message {
    v.verify(validator)?;
    // ... signature_verification_timer
}
VerifiedEvent::VoteMsg(v)  // promoted regardless of self_message
```

The `self_message` boolean is the **only** gate between "fully untrusted network input" and "cryptographically verified event." This is a single boolean parameter flowing through the call chain. If any refactoring, feature addition, or integration path incorrectly sets `self_message = true` for an externally-received message, the entire cryptographic verification layer is bypassed.

**Current risk:** Low — the current codebase appears to set `self_message` correctly. But this is a **fragile invariant** with no type-level enforcement. A `VerifiedEvent` constructed from an `UnverifiedEvent` with `self_message = true` is type-identical to one that passed full cryptographic verification.

**Recommendation:** The `self_message` bypass should use a newtype wrapper or separate constructor to make it impossible to accidentally bypass verification. The current design is one `true` literal away from a consensus-breaking vulnerability.

**Severity: MEDIUM** — Not currently exploitable but represents a latent critical vulnerability waiting for a careless code change.

---

## ATTACK-5: Timeout Vote Echo Threshold — Monotonicity Not Enforced (LOW)

**Location:** `pending_votes.rs:257-258`

```rust
let f_plus_one = validator_verifier.total_voting_power()
    - validator_verifier.quorum_voting_power() + 1;
```

The `echo_timeout` flag is set once and never reset within a `PendingVotes` instance. However, `PendingVotes` is recreated each round (`PendingVotes::new()`), so there is no cross-round state leakage. **No vulnerability found.** The report's analysis is correct.

**Severity: LOW / Informational** — Correct behavior confirmed.

---

## Summary Table

| ID | Severity | Finding | Report's Rating | Delta |
|----|----------|---------|-----------------|-------|
| **ATTACK-1** | **CRITICAL** | Forged order vote signatures poison aggregation permanently; single Byzantine validator denies liveness | S-03: High (Conditional) | **Upgraded — concrete exploit path confirmed** |
| **ATTACK-2** | **HIGH** | Order vote equivocation enables double-counting across competing digests; no detection or logging | S-02: Medium | **Upgraded — can produce conflicting quorum** |
| **ATTACK-3** | **MEDIUM** | Poisoned PartialSignatures cache blocks order vote aggregation until GC | Not in report | **New finding** |
| **ATTACK-4** | **MEDIUM** | `self_message` boolean is single gate for all crypto verification; fragile invariant | Not in report | **New finding** |
| **ATTACK-5** | Low | Timeout echo threshold — confirmed correct | S-04: Low | No change |

---

## Critical Remediation Recommendations

1. **Add per-signature verification inside `insert_order_vote`** — Call `order_vote.verify(validator_verifier)` before `add_signature()`. The ~1ms BLS verify cost per vote is negligible vs. the risk. This eliminates ATTACK-1 and ATTACK-3.

2. **Add `author_to_vote` deduplication to `PendingOrderVotes`** — Mirror the `PendingVotes` equivocation detection pattern. Log `SecurityEvent::ConsensusEquivocatingOrderVote`. Reject the second conflicting vote. This eliminates ATTACK-2.

3. **Replace `self_message: bool` with a type-level verification witness** — e.g., a `VerificationProof` enum (`CryptoVerified(Signature)` | `SelfGenerated(LocalSignerProof)`) that must be constructed at the verification site and cannot be forged by passing a boolean. This hardens ATTACK-4.

4. **Add signature eviction to `PendingOrderVotes`** — When `aggregate_signatures` fails, identify and remove the invalid signature(s) from `PartialSignatures` rather than leaving them permanently poisoned. Retry aggregation after eviction.

---

## [consensus_network_and_message_auth] Security audit of aptos-core/consensus/src/network.rs and aptos-core/consensus/s

### 🎯 Intention

Security audit of aptos-core/consensus/src/network.rs and aptos-core/consensus/src/network_interface.rs for: (1) consensus message authentication and sender identity verification; (2) message deserialization safety and handling of malformed/oversized messages; (3) DoS resistance against message flooding from malicious peers; (4) whether gravity-sdk has modified upstream network authentication checks; (5) the interaction with the mutual_authentication setting from network.rs in crates/api/.

### 🔍 Discovered Implementation

## Implementation Analysis: Consensus Network Layer

### Files/Contracts Involved

| File | Description |
|---|---|
| `aptos-core/consensus/src/network.rs` | Core inbound/outbound message infrastructure: `NetworkSender`, `NetworkTask`, `NetworkReceivers`, RPC request wrappers |
| `aptos-core/consensus/src/network_interface.rs` | `ConsensusMsg` enum definition, `ConsensusNetworkClient` wrapper, protocol ID constants |
| `crates/api/src/network.rs` | gravity-sdk startup guard enforcing `mutual_authentication: true` on validator networks |
| `docker/gravity_node/config/validator.yaml` / `template_config/validator.yaml` | Config files setting `mutual_authentication: true` |
| `aptos-core/consensus/src/dag/types.rs` | Second-level BCS deserialization for DAG messages |
| `aptos-core/consensus/src/rand/rand_gen/network_messages.rs` | Second-level BCS deserialization for randomness messages |
| `gaptos` (external crate) | Upstream aptos-network transport layer (noise protocol, TLS handshake, frame-level size limits) — not vendored |

---

### Execution Path: Inbound Message Processing

```
1. TCP connection established with noise protocol handshake
   └─ mutual_authentication enforced at transport layer (PeerId cryptographically bound)

2. Raw bytes received on wire
   └─ ProtocolId::from_bytes() dispatches to BCS / JSON / LZ4+BCS deserializer
        └─ Produces Event<ConsensusMsg>

3. NetworkTask::start() event loop (network.rs:705)
   ├─ Event::Message(peer_id, msg)  [direct-send path]
   │    ├─ SignedBatchInfo | BatchMsg | ProofOfStoreMsg  → quorum_store_messages_tx (cap 50)
   │    ├─ ProposalMsg | VoteMsg | OrderVoteMsg | SyncInfo | EpochRetrievalRequest | EpochChangeProof
   │    │    → consensus_messages_tx (cap 10)
   │    ├─ CommitVoteMsg | CommitDecisionMsg | RandGenMessage
   │    │    → wrapped as IncomingRpcRequest → rpc_tx (cap 10), response channel dropped
   │    └─ _ → warn!("Unexpected direct send msg"), dropped
   │
   └─ Event::RpcRequest(peer_id, msg, protocol, callback)  [RPC path]
        ├─ BlockRetrievalRequest → IncomingRpcRequest::BlockRetrieval → rpc_tx
        ├─ BatchRequestMsg → IncomingRpcRequest::BatchRetrieval → rpc_tx
        ├─ DAGMessage → IncomingRpcRequest::DAGRequest (sender = peer_id) → rpc_tx
        ├─ CommitMessage → IncomingRpcRequest::CommitRequest → rpc_tx
        ├─ RandGenMessage → IncomingRpcRequest::RandGenRequest (sender = peer_id) → rpc_tx
        ├─ SyncInfoRequest → IncomingRpcRequest::SyncInfoRequest → rpc_tx
        └─ _ → warn!("Unexpected msg"), dropped (callback dropped → caller gets channel-closed error)

4. (DAGMessage / RandGenMessage only) second-level deserialization:
   bcs::from_bytes(&msg.data) at handler level
```

---

### Key Functions

#### `NetworkTask::new(network_service_events, self_receiver) → (NetworkTask, NetworkReceivers)` — network.rs:653
Creates three `aptos_channel` pairs:
- `consensus_messages`: FIFO, capacity **10**
- `quorum_store_messages`: FIFO, capacity **50**
- `rpc_tx/rpc_rx`: FIFO, capacity **10**

Merges all network event streams with the self-receiver into a single unified `all_events` stream using `select_all` + `select`.

#### `NetworkTask::start(mut self)` — network.rs:705
Long-lived async loop consuming `all_events`. Dispatches each `Event<ConsensusMsg>` to the appropriate internal channel based on variant. Increments `CONSENSUS_RECEIVED_MSGS` Prometheus counter per message. Logs `SecurityEvent` on verification failures.

#### `NetworkTask::push_msg(peer_id, msg, tx)` — network.rs:689
Static helper. Pushes `(peer_id, msg)` into an `aptos_channel` keyed by `(peer_id, discriminant(&msg))`. On failure: `warn!` log, message dropped.

#### `extract_network_configs(node_config) → Vec<NetworkConfig>` — crates/api/src/network.rs:38
Collects all network configs. **Panics** if validator network has `mutual_authentication: false`.

#### `NetworkSender::request_block(retrieval_request, from, timeout) → Result<BlockRetrievalResponse>` — network.rs:236
Sends `BlockRetrievalRequest` via RPC. Asserts `from != self.author`. On response: expects `BlockRetrievalResponse` variant. If `block_id != zero`, calls `response.verify(request, &self.validators)` — on failure logs `SecurityEvent::InvalidRetrievedBlock`.

#### `RpcResponder::respond<R: TConsensusMsg>(self, response) → Result<()>` — network.rs:81
Serializes response via `protocol.to_bytes(...)`, sends over oneshot. Returns error if receiver dropped.

#### `ConsensusNetworkClient::get_peer_network_id_for_peer(peer) → PeerNetworkId` — network_interface.rs:202
Hard-codes all consensus peers as `NetworkId::Validator`. All outbound consensus traffic goes over the validator network exclusively.

---

### State Changes

- **Prometheus counters**: `CONSENSUS_SENT_MSGS` incremented on every outbound message; `CONSENSUS_RECEIVED_MSGS` incremented on every inbound message; `CONSENSUS_CHANNEL_MSGS`, `QUORUM_STORE_CHANNEL_MSGS`, `RPC_CHANNEL_MSGS` track channel depths.
- **`observe_block`**: Called for inbound `ProposalMsg` with `BlockStage::NETWORK_RECEIVED` timestamp.
- **Channel state**: Messages pushed into bounded `aptos_channel`s keyed by `(AccountAddress, Discriminant)` — at most one message per (sender, message-type) pair is buffered; new pushes for the same key replace older entries.

---

### Sender Identity (PeerId) Flow

1. **Transport layer** (noise protocol handshake within `gaptos`): PeerId is cryptographically authenticated during connection setup when `mutual_authentication: true`.
2. **Network event delivery**: `Event::Message(peer_id, ...)` and `Event::RpcRequest(peer_id, ...)` carry the authenticated `peer_id` from the transport layer.
3. **Consensus layer**: `NetworkTask` takes `peer_id` as-is — **no additional per-message identity verification** at this layer. The `peer_id` is used as:
   - Channel key component `(peer_id, discriminant)` for per-sender queuing
   - `sender: Author` field in `IncomingDAGRequest` and `IncomingRandGenRequest`
   - Log field `remote_peer`
4. **Content-level verification** (separate from network identity): Proposals, votes, and other cryptographic messages carry signatures verified by consensus key (BLS) at the application handler layer, independently of the transport-layer PeerId.

---

### Message Deserialization Safety

| Layer | Mechanism | Error Handling |
|---|---|---|
| Wire → `ConsensusMsg` | `ProtocolId::from_bytes()` dispatches to BCS/JSON/LZ4+BCS | Deserialization error → `Event` never emitted; message silently dropped at network layer |
| Self-RPC loopback | `tokio::task::spawn_blocking(move \|\| protocol.from_bytes(&bytes))` | Error propagated via `??`; caller receives `bail!("self rpc failed")` |
| Inner DAG/Rand data | `bcs::from_bytes(&msg.data)` at handler | `?` propagates `bcs::Error` as `anyhow::Error` |
| Unknown direct-send variant | `_ => warn!("Unexpected direct send msg")` | Dropped with `continue` |
| Unknown RPC variant | `_ => warn!("Unexpected msg: {:?}", msg)` | Dropped with `continue`; callback dropped → caller gets channel-closed |
| Wrong RPC response type | Pattern match mismatch | `Err(anyhow!("Invalid response to request"))` or `Err(anyhow!("Invalid batch response"))` |
| Block retrieval verification | `response.verify(request, &validators)` | `SecurityEvent::InvalidRetrievedBlock` logged; error propagated |

---

### Size Limits and Rate Limiting

**No message size constants, byte-level limits, or rate limiters are defined in the consensus layer.** Specifically:

- No `MAX_MESSAGE_SIZE`, `max_frame_size`, or similar constants exist in `network.rs` or `network_interface.rs`.
- No `RateLimiter`, `rate_limit`, or `throttle` constructs exist in the consensus source files.
- Size enforcement is **fully delegated** to the `gaptos::aptos_network` transport layer (not vendored in this repo).

**The only flow-control mechanisms present are:**

1. **Bounded channel capacities**: consensus messages (10), quorum store messages (50), RPC requests (10). Overflow → `warn!` log, message dropped.
2. **Per-key replacement**: `aptos_channel` keyed by `(peer_id, discriminant)` — only one message per (sender, message-type) pair is buffered simultaneously. A new push for an existing key replaces the old entry.

---

### gravity-sdk Modifications to Upstream Authentication

gravity-sdk has added **one explicit modification** beyond upstream Aptos:

**`crates/api/src/network.rs:38–48`** — A startup panic guard that enforces `mutual_authentication: true` on validator networks. This is **not present** in standard Aptos node startup code. If a validator config has `mutual_authentication: false`, the process immediately panics with:

> "Validator networks must always have mutual_authentication enabled!"

This is applied only to the validator network; full node networks (`full_node_networks`) are exempt from this check. The `mutual_authentication` field appears in exactly 5 locations in the codebase, and is never set to `false` in any of them.

---

### External Dependencies

| Dependency | Usage |
|---|---|
| `gaptos::aptos_network` | Transport layer: noise protocol, connection management, `Event<T>` stream delivery, `ProtocolId::from_bytes`/`to_bytes`, `NetworkClient`, frame-level size enforcement |
| `gaptos::aptos_channels` | `UnboundedSender` for self-message loopback |
| `aptos_channel` | Bounded FIFO channels with `(key, value)` semantics for internal message routing |
| `tokio` | Async runtime, `spawn_blocking` for deserialization offloading, `timeout` |
| `bcs` | Binary Canonical Serialization (primary wire format) |
| `serde_json` | JSON serialization (fallback wire format) |
| `lz4` (via ProtocolId) | Compression for `Compressed` protocol variants |
| `aptos_crypto` / `aptos_types` | `AccountAddress`, `ValidatorVerifier`, signature types |
| `futures` | `select`, `select_all`, `StreamExt` for event stream merging |

### 🛠️ Ground Review

# Ground Review: Consensus Network Layer

## Code Hygiene & Engineering Safety Audit

---

### Critical

#### C1. Oneshot receivers intentionally dropped — silent response black-holes
**Files:** `network.rs:725`, `740`, `776`

When `CommitVoteMsg`, `CommitDecisionMsg`, and `RandGenMessage` arrive via **direct-send** (not RPC), the code creates a oneshot channel but immediately drops the receiver (`_rx`):

```rust
let (tx, _rx) = oneshot::channel();
```

The `response_sender: tx` is stuffed into an `IncomingRpcRequest`, meaning downstream handlers may attempt to send a response on `tx` — which will silently fail (the `send` returns `Err`, but whether the handler checks it depends on the call site). This is a **resource waste** and a **correctness risk**: handlers cannot distinguish "real RPC with a waiting caller" from "fake RPC wrapper of a direct-send message." If a handler unconditionally awaits a response confirmation, it will get a spurious error.

**Severity: Critical** — Not a leak per se, but a design-level correctness hazard. Downstream code that calls `RpcResponder::respond()` on these will get `Err("unable to respond to rpc")` with no indication that this is expected behavior.

**Recommendation:** Either (a) introduce a dedicated `IncomingRpcRequest` variant for fire-and-forget messages, or (b) use `Option<oneshot::Sender>` so handlers know no response is expected.

---

#### C2. No consensus-layer message size limits or rate limiting
**Files:** `network.rs`, `network_interface.rs`

There are **zero** message size constants, byte-level limits, or rate-limiting constructs anywhere in the consensus network layer. Size enforcement is entirely delegated to the external `gaptos::aptos_network` crate, which is not vendored and therefore not auditable from this repository.

The only back-pressure mechanism is the bounded `aptos_channel` capacities (10 for consensus, 50 for quorum store, 10 for RPC). While the per-key replacement semantics prevent unbounded queue growth, a malicious peer can still force repeated deserialization of arbitrarily large messages at the BCS layer before they hit the channel.

**Severity: Critical** — A compromised validator peer (authenticated via noise) could send pathologically large serialized payloads that consume CPU/memory during deserialization before any channel-level back-pressure kicks in. The defense relies entirely on an external, unauditable dependency.

**Recommendation:** Add explicit `MAX_MESSAGE_SIZE` constants at the consensus layer as a defense-in-depth measure independent of the transport layer.

---

### Warning

#### W1. `NetworkTask::start()` is a 160-line monolithic match block
**File:** `network.rs:705–868`

The `start()` method is a single `while let` loop containing a deeply nested `match` with ~160 lines of dispatch logic. The outer match (Message vs RpcRequest) contains inner matches on every `ConsensusMsg` variant. This makes the function hard to review, test, and modify safely.

**Severity: Warning** — Maintenance and review hazard. Adding a new message type requires modifying this function in multiple places, increasing the risk of dispatch errors.

---

#### W2. Channel capacity of 10 for consensus messages is very small
**File:** `network.rs:657–658`

```rust
aptos_channel::new(QueueStyle::FIFO, 10, ...)
```

With `aptos_channel` keyed by `(peer_id, discriminant)`, the effective capacity is 10 total entries. In a validator set of, say, 100 nodes, messages from different peers sharing the same discriminant will compete for these 10 slots. Overflow results in silent message drops with only a `warn!` log.

**Severity: Warning** — Under high load or during epoch changes (when many `EpochChangeProof` messages arrive simultaneously), legitimate consensus messages may be silently dropped, potentially stalling consensus progress. The `TODO` comment on the quorum store channel (line 661) acknowledges this tuning concern but no similar note exists for consensus messages.

---

#### W3. Commented-out network validation check
**File:** `network.rs:670–676`

```rust
/// TODO(nekomoto): fullnode does not have validator network events...
//if (network_and_events.values().len() != 1)
//    || !network_and_events.contains_key(&NetworkId::Validator)
//{
//    panic!("The network has not been setup correctly for consensus!");
//}
```

A safety invariant check that ensures consensus only runs on the validator network has been commented out entirely. While there may be a valid reason (VFN support), this removes an important startup guard without replacement.

**Severity: Warning** — The `extract_network_configs` panic in `crates/api/src/network.rs:42-43` partially compensates, but only checks `mutual_authentication`, not that the consensus module is exclusively bound to the validator network.

---

#### W4. Hardcoded `NetworkId::Validator` assumption
**File:** `network_interface.rs:202–204`

```rust
fn get_peer_network_id_for_peer(&self, peer: PeerId) -> PeerNetworkId {
    PeerNetworkId::new(NetworkId::Validator, peer)
}
```

Every consensus peer is unconditionally mapped to `NetworkId::Validator`. The `TODO` comment (line 200) acknowledges this should be migrated. If the codebase ever supports consensus over non-validator networks (e.g., for testing or VFN forwarding), this hard-coding will silently route messages to the wrong network.

**Severity: Warning** — Latent correctness risk. Currently safe because of the `mutual_authentication` startup guard, but fragile.

---

#### W5. `send_rpc_to_self` error handling collapses three distinct failure modes
**File:** `network.rs:283–289`

```rust
if let Ok(Ok(Ok(bytes))) = timeout(timeout_duration, rx).await {
    ...
} else {
    bail!("self rpc failed");
}
```

Three nested `Result`/`Option` layers are collapsed into a single `bail!` with no indication of **which** layer failed: timeout, channel closed, or RPC error. This makes debugging production issues significantly harder.

**Severity: Warning** — Observability gap. A timeout, a dropped channel, and an explicit RPC error all produce the same undifferentiated error message.

---

#### W6. `unwrap().expect()` on `spawn_blocking` in `broadcast_fast_share`
**File:** `network.rs:419–423`

```rust
let msg = tokio::task::spawn_blocking(|| {
    RandMessage::<Share, AugmentedData>::FastShare(share).into_network_message()
})
.await
.expect("task cannot fail to execute");
```

If the tokio runtime is shutting down or the blocking thread pool is exhausted, `spawn_blocking` will return a `JoinError` and this `expect` will panic, taking down the consensus task.

**Severity: Warning** — Under resource exhaustion or during shutdown, this panic can cause an ungraceful crash rather than a clean error propagation.

---

### Info

#### I1. JSON as a fallback wire protocol
**File:** `network_interface.rs:135`

```rust
ProtocolId::ConsensusRpcJson,
```

JSON is listed as a lowest-priority fallback for both RPC and direct-send. JSON deserialization is significantly slower than BCS and produces larger payloads. While it's the last resort, its presence means a peer could negotiate down to JSON, increasing deserialization cost.

**Severity: Info** — Performance consideration. Not a bug, but worth noting for DoS surface analysis.

---

#### I2. `debug!` log in hot path includes full message debug output
**File:** `network.rs:706`

```rust
debug!("NetworkTask received message {:?}", message);
```

At `debug` log level, every single inbound consensus message is formatted via `Debug`. For large messages (e.g., `BatchMsg` with many transactions), this formatting is expensive even if the log is ultimately filtered.

**Severity: Info** — Performance. `Debug` formatting allocates even when the log level is filtered, depending on the logging framework implementation.

---

#### I3. Startup guard uses `panic!` rather than returning `Result`
**File:** `crates/api/src/network.rs:43`

```rust
panic!("Validator networks must always have mutual_authentication enabled!");
```

While effective, panicking during config validation is a blunt instrument. Returning a structured error would allow callers to handle the failure more gracefully (e.g., logging context, cleaning up resources).

**Severity: Info** — Style/robustness preference. The `panic!` does achieve its goal of preventing insecure startup.

---

#### I4. `#[allow(dead_code)]` on `send_commit_vote`
**File:** `network.rs:394`

The `send_commit_vote` method is explicitly marked as dead code. If it's truly unused, it should be removed to reduce maintenance surface. If it's planned for future use, a `TODO` comment would clarify intent.

**Severity: Info** — Code hygiene.

---

## Summary

| Severity | Count | Key Themes |
|----------|-------|------------|
| **Critical** | 2 | Fake RPC wrappers for direct-send messages; no consensus-layer size/rate limits |
| **Warning** | 6 | Monolithic dispatch function; tiny channel capacities; collapsed error handling; commented-out safety check; hardcoded network ID; panic on spawn failure |
| **Info** | 4 | JSON fallback; debug logging in hot path; panic-based config validation; dead code |

The most **actionable** findings are **C1** (the oneshot receiver drop pattern creating indistinguishable real-RPC vs fake-RPC requests downstream) and **C2** (complete absence of consensus-layer message size bounds). Both represent defense-in-depth gaps that could be exploited by an authenticated-but-malicious validator peer.

### ⚔️ Intention Attacker

# Red Team Attack Report: Consensus Network Layer

## Verified Source Analysis

After reading the actual source code across all three files, here are the confirmed vulnerabilities:

---

### FINDING 1: Channel Key-Collision Allows a Malicious Validator to Silently Drop Legitimate Messages

**Severity: HIGH**

**Location:** `network.rs:689-703` (`push_msg`) and `network.rs:705-868` (`start`)

The `aptos_channel` is keyed by `(peer_id, discriminant(&msg))`. With `QueueStyle::FIFO` and capacity 10, when the channel is full, a new push for the **same key** replaces the older entry silently.

**Attack:** A compromised validator can flood `ProposalMsg` or `VoteMsg` direct-send messages at high rate. Because the channel key is `(attacker_peer_id, ProposalMsg_discriminant)`, each new message from the attacker replaces its own prior message — but the channel has only **10 slots total across all peers and all message types**. If the consumer is slow, legitimate messages from honest validators can be dropped when capacity is exceeded (`push` returns `Err`, logged as a `warn!`, then silently discarded at line 697-702).

**Impact:** An attacker who is a valid validator can cause vote or proposal starvation for other honest validators by saturating the consensus channel (capacity 10) with rapid-fire messages, degrading liveness. The `warn!` log is the only signal — no back-pressure, no peer penalty, no alert.

---

### FINDING 2: Direct-Send → RPC Shimming Drops the Response Channel, Creating Silent One-Way Messages

**Severity: MEDIUM**

**Location:** `network.rs:724-790` (handling of `CommitVoteMsg`, `CommitDecisionMsg`, `RandGenMessage` on the direct-send path)

When `CommitVoteMsg`, `CommitDecisionMsg`, or `RandGenMessage` arrive via **direct-send** (`Event::Message`), the code fabricates a fake RPC callback:

```rust
let (tx, _rx) = oneshot::channel();  // _rx is immediately dropped
```

The `_rx` receiver is dropped on the spot. The handler that processes this `IncomingRpcRequest` may attempt to send a response via `tx` — which will always fail silently (the oneshot sender returns `Err` when the receiver is dropped). This means:

1. **Any response the handler computes is thrown away** — wasted computation.
2. **The handler cannot distinguish a real RPC from a shimmed direct-send.** If the handler's logic depends on response delivery confirmation (e.g., an ack-then-commit pattern), it will silently believe the response was sent but the peer never receives it.
3. **A malicious validator can deliberately send `CommitVoteMsg`/`CommitDecisionMsg` via direct-send instead of RPC** to force the target into a state where it processes the commit but never sends a response — potentially causing the sender to retry indefinitely or timeout.

---

### FINDING 3: No Consensus-Layer Rate Limiting or Per-Peer Message Throttling

**Severity: HIGH**

**Location:** `network.rs` (entire file) and `network_interface.rs` (entire file)

There is **zero** rate limiting at the consensus layer:
- No per-peer message rate tracking
- No per-message-type throttling
- No cooldown or penalty for peers that flood messages
- No `MAX_MESSAGE_SIZE` constant in the consensus layer

The only protection is the bounded channel capacity (10 for consensus, 50 for quorum store, 10 for RPC). The code comment at line 661 says:

```rust
// TODO: tune this value based on quorum store messages with backpressure
```

This is explicitly acknowledged as unfinished.

**Attack:** A compromised validator can send an unbounded stream of valid `ConsensusMsg` variants. Each message is deserialized, counter-incremented, and pushed into the channel. Even though excess messages are dropped at the channel boundary, the **deserialization and Prometheus counter update cost is paid for every message**, creating CPU and memory pressure. The transport layer (`gaptos`) may have frame-level size limits, but there is nothing preventing a validator from sending millions of small, valid-looking messages per second.

---

### FINDING 4: Validator Network Check Bypassed for Full Node Networks

**Severity: MEDIUM**

**Location:** `crates/api/src/network.rs:38-48`

```rust
pub fn extract_network_configs(node_config: &NodeConfig) -> Vec<NetworkConfig> {
    let mut network_configs: Vec<NetworkConfig> = node_config.full_node_networks.to_vec();
    if let Some(network_config) = node_config.validator_network.as_ref() {
        if !network_config.mutual_authentication {
            panic!("Validator networks must always have mutual_authentication enabled!");
        }
        network_configs.push(network_config.clone());
    }
    network_configs
}
```

The `mutual_authentication` check is **only applied to `validator_network`**. Full node networks (`full_node_networks`) are added without any authentication check. Additionally, in `network.rs:668-676`, the original upstream check that verified consensus only runs on the Validator network has been **commented out**:

```rust
// TODO(nekomoto): fullnode does not have validator network events, but it still needs to
// handle sync info requests.
//if (network_and_events.values().len() != 1)
//    || !network_and_events.contains_key(&NetworkId::Validator)
//{
//    panic!("The network has not been setup correctly for consensus!");
//}
```

**Attack:** If a full node network is somehow configured to carry consensus traffic (which the commented-out guard would have prevented), consensus messages could flow over an unauthenticated channel. The `get_peer_network_id_for_peer` function (`network_interface.rs:202`) hard-codes `NetworkId::Validator` for outbound traffic, but the **inbound path** uses `select_all` over all network events (line 679), meaning events from any configured network — including unauthenticated full node networks — are processed identically. A misconfiguration or intentional attack could inject consensus messages through an unauthenticated full node network.

---

### FINDING 5: Block Retrieval Verification Skipped for `HashValue::zero()` Requests

**Severity: MEDIUM**

**Location:** `network.rs:260-268`

```rust
if retrieval_request.block_id() != HashValue::zero() {
    response.verify(retrieval_request, &self.validators).map_err(|e| {
        error!(SecurityEvent::InvalidRetrievedBlock, ...);
        e
    })?;
}
```

When `block_id` is `HashValue::zero()`, signature verification of the response is **completely skipped**. The response is returned as-is and trusted.

**Attack:** If any code path constructs a `BlockRetrievalRequest` with `block_id = HashValue::zero()` (e.g., to fetch the genesis block or as a sentinel), a malicious peer can return arbitrary, unverified block data. The caller receives the response and treats it as valid. This is a classic TOCTOU issue where the security check is conditional on a value the caller controls.

---

### FINDING 6: Debug Logging of Full Message Content

**Severity: MEDIUM**

**Location:** `network.rs:707`

```rust
debug!("NetworkTask received message {:?}", message);
```

Every single inbound consensus message — including proposals containing transactions, votes containing signatures, and commit decisions — is logged at `debug!` level with full `Debug` formatting. If debug logging is enabled in production (misconfiguration or diagnostic mode), this leaks:
- Transaction contents before finality
- Validator vote patterns and signatures
- Commit decisions before broadcast

This is an information disclosure risk that could give an attacker observing logs insight into upcoming consensus decisions.

---

## Summary

| # | Finding | Severity | Category |
|---|---------|----------|----------|
| 1 | Channel saturation allows message starvation | **HIGH** | DoS / Liveness |
| 2 | Direct-send → RPC shim silently drops responses | **MEDIUM** | Logic / State Inconsistency |
| 3 | No consensus-layer rate limiting | **HIGH** | DoS / Resource Exhaustion |
| 4 | Disabled network validation + unauthenticated full node path | **MEDIUM** | Authentication Bypass |
| 5 | Block verification skipped for zero-hash requests | **MEDIUM** | Integrity / TOCTOU |
| 6 | Debug logging of full consensus messages | **MEDIUM** | Information Disclosure |

---

## [randomness_dkg_subsystem] Security audit of aptos-core/consensus/src/rand/ directory for: (1) DKG protocol

### 🎯 Intention

Security audit of aptos-core/consensus/src/rand/ directory for: (1) DKG protocol implementation correctness including share verification and threshold enforcement; (2) randomness generation and reveal protocol for bias resistance; (3) private key share handling and secure storage; (4) interaction with the DKG HTTP endpoints reviewed in https_api_security; (5) any gravity-sdk modifications to upstream Aptos randomness that could weaken unpredictability guarantees; (6) the rand manager's integration with epoch transitions.

### 🔍 Discovered Implementation

# Security Audit: `aptos-core/consensus/src/rand/` — Randomness & DKG Implementation

## Executive Summary

The `rand/` directory implements an on-chain randomness generation protocol based on a **Weighted Verifiable Unpredictable Function (WVUF)** with a two-phase augmented data ceremony followed by share-based randomness generation per block. The core cryptographic DKG primitives (PVSS, threshold enforcement, key generation) live in the upstream `gaptos` crate — this directory contains the **protocol orchestration**, **share aggregation**, **network messaging**, and **persistent storage** layers.

---

## Architecture Overview

```
[RoundManager / DAG]
       │
       │ OrderedBlocks
       ▼
[RandManager]  ◄────►  [Peers via ReliableBroadcast]
  │  (shares, aug data, certified aug data)
  │
  │ OrderedBlocks (with randomness attached)
  ▼
[BufferManager]
  │
  ▼
[Execution / Commit]
```

### Files Involved (25 files, 4 directories)

| Path | Role |
|------|------|
| `rand/mod.rs` | Top-level: re-exports `dkg` and `rand_gen` |
| `rand/dkg/mod.rs` | Test-only DKG harness (no production code) |
| `rand/dkg/*.rs` (8 files) | Unit tests for PVSS, FFT, WVUF, accumulators |
| `rand/rand_gen/rand_manager.rs` | Core orchestration event loop |
| `rand/rand_gen/types.rs` | Traits (`TShare`, `TAugmentedData`), production + mock impls, `RandConfig` |
| `rand/rand_gen/rand_store.rs` | Share storage, aggregation state machine |
| `rand/rand_gen/block_queue.rs` | Ordered block queue awaiting randomness |
| `rand/rand_gen/aug_data_store.rs` | Augmented data certification store |
| `rand/rand_gen/reliable_broadcast_state.rs` | Reliable broadcast protocol states |
| `rand/rand_gen/network_messages.rs` | Wire protocol message types |
| `rand/rand_gen/storage/{db,interface,in_memory,schema}.rs` | Persistent storage (RocksDB) |

---

## Detailed Findings

### 1. DKG Protocol Implementation Correctness

**Finding: Core DKG logic is externalized to `gaptos` crate — not auditable in this repo.**

The `rand/dkg/` module contains **zero production code**. All 8 submodules are `#[cfg(test)]` only. The actual PVSS transcript creation, share verification, and threshold enforcement are implemented in:
- `gaptos::aptos_types::dkg::DKGState`
- `gaptos::aptos_crypto::bls12381` (for BLS signatures)
- The WVUF library (`WVUF::verify_share`, `WVUF::aggregate_shares`, `WVUF::derive_eval`)

A comment in `dkg/mod.rs` states: *"DKG runtime tests are temporarily disabled due to API changes in gaptos"* — confirming Gravity-specific modifications to the upstream.

**Risk: MEDIUM** — The DKG runtime tests being disabled means the integration between Gravity's modified `gaptos` APIs and the DKG test harness is not being validated. If `gaptos` API changes introduced behavioral differences in share generation or verification, these tests would not catch it.

### 2. Randomness Generation and Reveal Protocol — Bias Resistance

**The protocol has a sound commit-then-reveal structure with two key anti-bias mechanisms:**

#### Phase 1: Augmented Data Ceremony (per epoch)
1. Each validator generates a `Delta` (public delta from WVUF) and broadcasts it as `AugData`
2. Peers verify via `WVUF::augment_pubkey` and sign — signatures are collected
3. Once a quorum of signatures is reached, a `CertifiedAugData` with an aggregate BLS signature is formed
4. `CertifiedAugData` is broadcast and acknowledged by all validators
5. **Blocks are NOT processed until the local node's own `CertifiedAugData` exists** (`rand_manager.rs` line ~420: blocks are only processed after `my_certified_aug_data_exists()`)

This prevents a validator from participating in randomness generation before committing to its augmented public key, which is essential for bias resistance.

#### Phase 2: Share Generation (per block)
1. For each ordered block, the validator calls `WVUF::create_share` with BCS-serialized `RandMetadata`
2. The share is broadcast to all peers
3. After 300ms, missing shares are requested via `ReliableBroadcast`
4. `ShareAggregator` collects shares weighted by validator stake
5. Once `total_weight >= threshold()`, `WVUF::aggregate_shares` → `WVUF::derive_eval` → **SHA3-256 hash** produces final randomness

**The SHA3-256 final hash step** (`types.rs` lines ~130-137) is critical: it ensures the raw WVUF evaluation is domain-separated and uniformly distributed.

**Finding: The randomness is deterministic given the shares — no additional entropy source or nonce.** The input to share generation is solely the BCS-serialized `RandMetadata` (epoch + round + block ID). This is correct for a VUF-based scheme where unpredictability comes from the distributed key, not from per-round entropy.

**Risk: LOW** — The protocol structure is sound. Bias would require controlling enough stake to reach the threshold unilaterally, which is a validator set composition issue, not a protocol flaw.

### 3. Private Key Share Handling and Secure Storage

**Finding: Key material management has both strengths and a notable concern.**

#### Storage path
- Key pair bytes are persisted via `RandStorage::save_key_pair_bytes(epoch, key_pair)` → `RandDb` (RocksDB at `<db_root>/rand_db`)
- The `KeyPairSchema` column family stores serialized key pairs keyed by epoch
- On startup, `RandStorage::get_key_pair_bytes()` retrieves the stored key pair

#### In-memory handling
- The secret key (`ask` — augmented secret key) lives inside `RandKeys` wrapped in `Arc`:
  ```rust
  keys: Arc<RandKeys>  // contains ask, apk, pk_shares, certified_apks
  ```
- `RandConfig` holds `keys: Arc<RandKeys>` — the secret key is accessible to any code holding a `RandConfig` reference
- `Share::generate` calls `WVUF::create_share(&rand_config.keys.ask, ...)` directly

**Risk: MEDIUM** — The secret key share (`ask`) is stored in a plain RocksDB column family without encryption-at-rest. Any process with read access to the `rand_db` directory can extract the key material. There is no secure memory handling (e.g., `zeroize` on drop) visible for the `RandKeys` struct. This follows the upstream Aptos pattern but is worth noting for a production Gravity deployment.

### 4. Interaction with DKG HTTP Endpoints

**Finding: The DKG HTTP API (`crates/api/src/https/dkg.rs`) is read-only and does not interact with the randomness generation protocol.**

The API exposes two endpoints:
- `GET /dkg/status` — reads `DKGState` from on-chain config storage (via `ConsensusDB::DbReader`)
- `GET /dkg/randomness/<block_number>` — reads pre-computed randomness from `ConsensusDB`

These endpoints **do not**:
- Write to any randomness state
- Participate in share generation or verification
- Expose private key material
- Accept any input that could influence randomness generation

**Risk: LOW** — The API surface is purely observational. The only risk vector would be information disclosure (e.g., revealing randomness for future blocks), but `get_randomness` only returns randomness for already-decided blocks stored in `ConsensusDB`.

### 5. Gravity-SDK Modifications to Upstream Aptos Randomness

**Finding: Modifications are minimal and structural, not cryptographic.**

The only explicit Gravity-specific marker found is:
```
// DKG runtime tests are temporarily disabled due to API changes in gaptos
```

No inline code modifications (no `// MODIFIED`, `// GRAVITY`, `// CUSTOM` markers) were found in the `rand/` directory. The code appears to be a close fork of upstream Aptos with:
1. **Disabled DKG runtime tests** — the integration tests between the rand module and the DKG protocol are not running
2. **`gaptos` dependency** — the upstream `aptos-types` and `aptos-crypto` are replaced with `gaptos` equivalents
3. **`#![allow(dead_code)]`** on the top-level `mod.rs` — suggests some code paths may not be wired up in Gravity's configuration

**Risk: MEDIUM** — The combination of (a) disabled DKG tests and (b) dependency on a modified `gaptos` crate creates a gap in test coverage. If `gaptos` changed any cryptographic API semantics (parameter ordering, serialization format, threshold calculation), those changes would not be caught by the disabled tests.

### 6. Rand Manager's Integration with Epoch Transitions

**The lifecycle is cleanly managed through a three-tier shutdown:**

#### Epoch Start
1. `EpochManager` creates a new `aptos_channel` for rand gen RPC messages
2. `ExecutionProxyClient` constructs `RandManager::new(...)` with the epoch's config
3. `RandManager::start()` is spawned as an async tokio task
4. The manager blocks block processing until its own `CertifiedAugData` is established

#### Mid-Epoch Reset (e.g., after state sync)
1. `BufferManagerHandle` sends `ResetSignal::TargetRound(round)` through `reset_tx`
2. `RandManager::process_reset()` clears the `block_queue`, updates `highest_known_round` in `rand_store`
3. The manager continues running with the updated state
4. The reset is acknowledged via a oneshot channel

#### Epoch End
1. `EpochManager` sets `rand_manager_msg_tx = None` — drops the sender, causing the verification task to exit
2. `execution_client.end_epoch()` sends `ResetSignal::Stop`
3. `RandManager` sets `self.stop = true` and exits its event loop
4. A new `RandManager` is constructed for the next epoch

**Risk: LOW** — The epoch transition logic is well-structured with proper channel cleanup. The use of oneshot acknowledgment channels ensures the reset/stop completes before the caller proceeds.

---

## Specific Code-Level Concerns

### 6.1. Share Deduplication by Author Only (rand_store.rs)

`ShareAggregator::add_share` deduplicates by `author` key:
```rust
if !self.shares.contains_key(&share.author()) {
    self.shares.insert(share.author(), share);
    self.total_weight += weight;
}
```

This is correct — a validator can only contribute one share per round. However, there is no explicit check that the share's metadata matches the expected metadata *at insertion time* in the `PendingMetadata` state. Shares buffered before metadata arrives are later pruned via `retain()` when metadata is added, but a malicious validator could temporarily inflate the share count before pruning.

**Risk: LOW** — The `retain()` call recalculates `total_weight` from scratch, so premature aggregation cannot occur. The state machine transitions (`PendingMetadata` → `PendingDecision`) gate aggregation attempts properly.

### 6.2. `get_id()` Panic on Unknown Peer (types.rs line 593)

```rust
pub fn get_id(&self, peer: &Author) -> usize {
    *self.validator.address_to_validator_index().get(peer)
        .expect("peer not found in validator index")
}
```

This will **panic** and crash the node if called with an unknown peer address. While callers in `Share::verify` and `Share::aggregate` use safe alternatives (`get` + `ok_or_else`), any future caller that uses `get_id` directly without pre-validation would trigger a crash.

**Risk: LOW** — Current callers handle this safely, but the panic-on-unknown pattern is a latent crash vector if new code is added without the same care.

### 6.3. Future Round Acceptance Window (types.rs line 29)

```rust
pub const FUTURE_ROUNDS_TO_ACCEPT: u64 = 200;
```

Shares for rounds up to 200 ahead of `highest_known_round` are accepted and buffered. This bounds pre-computation attacks but also means a malicious peer could cause the node to allocate memory for up to 200 × N_validators buffered shares.

**Risk: LOW** — The window is bounded and reasonable for preventing memory exhaustion while allowing pipelining.

### 6.4. Equivocation Detection in AugDataStore (aug_data_store.rs)

`add_aug_data` checks for equivocation (same author, different data) but only logs/returns an error — it does not slash or report the equivocating validator:
```rust
// Checks for equivocation (same author, different data)
// Returns error but does not trigger slashing
```

**Risk: INFORMATIONAL** — This follows the upstream Aptos pattern. Equivocation evidence is not persisted or forwarded to a slashing mechanism within the rand module.

### 6.5. Mock Implementations Available at `pub(super)` Visibility

`MockShare` and `MockAugData` unconditionally return `Ok(())` for all verification. They are scoped as `pub(super)`, limiting them to the `rand_gen` module and its tests.

**Risk: LOW** — The visibility is properly restricted. These cannot be instantiated from outside the `rand_gen` module hierarchy. They are never referenced in the production `RandManager` instantiation (which uses `Share` and `AugmentedData`).

---

## Summary Risk Matrix

| # | Finding | Severity | Category |
|---|---------|----------|----------|
| 1 | DKG runtime tests disabled for `gaptos` API changes | **MEDIUM** | Test Coverage Gap |
| 2 | Randomness protocol structure sound (VUF + SHA3-256) | **LOW** | Bias Resistance ✓ |
| 3 | Secret key shares stored unencrypted in RocksDB, no `zeroize` | **MEDIUM** | Key Material Handling |
| 4 | DKG HTTP API is read-only, no influence on generation | **LOW** | API Surface ✓ |
| 5 | Disabled tests + modified `gaptos` dependency = coverage gap | **MEDIUM** | Gravity-Specific Risk |
| 6 | Epoch transition lifecycle properly managed | **LOW** | Integration ✓ |
| 6.1 | Share buffering before metadata is safe due to `retain()` | **LOW** | Protocol Correctness ✓ |
| 6.2 | `get_id()` panic on unknown peer — latent crash vector | **LOW** | Robustness |
| 6.3 | 200-round future acceptance window is bounded | **LOW** | DoS Resistance ✓ |
| 6.4 | Equivocation detected but not slashed | **INFO** | Accountability |
| 6.5 | Mock impls properly scoped to test visibility | **LOW** | Code Hygiene ✓ |

**Overall Assessment:** The randomness protocol orchestration is well-structured with proper bias resistance, ordered delivery guarantees, and clean epoch lifecycle management. The primary concerns are (a) the test coverage gap from disabled DKG integration tests against the modified `gaptos` dependency, and (b) plaintext storage of secret key shares. The core cryptographic correctness depends on the `gaptos` WVUF implementation, which is outside this repo's audit scope.

### 🛠️ Ground Review

# Ground Review (Blue Team): Code Quality & Engineering Safety Audit

## Scope

Reviewing the `aptos-core/consensus/src/rand/` implementation report for code hygiene, resource safety, concurrency correctness, and engineering best practices. Business logic and cryptographic design are out of scope.

---

## Critical Findings

### CRITICAL-1: No `zeroize` on Secret Key Material Drop

The `RandKeys` struct holds the augmented secret key (`ask`) inside an `Arc`. When the last `Arc` reference is dropped, standard Rust deallocation occurs — the memory is freed but **not zeroed**. This means:

- The secret key bytes remain in the process's heap until overwritten by a future allocation.
- A core dump, `/proc/self/mem` read, or cold-boot attack can recover the key.
- The `Arc` wrapper makes this worse: `Arc::drop` deallocates without calling `zeroize`, and cloning `Arc` means the key can live across multiple heap locations.

```rust
keys: Arc<RandKeys>  // contains ask (augmented secret key)
```

**Remediation:** Wrap the secret key in a `zeroize::Zeroizing<T>` or implement `Drop` with `Zeroize` for `RandKeys`. Consider `secrecy::Secret<T>` for additional compile-time guards against accidental logging.

---

## Warning Findings

### WARN-1: Panic in `get_id()` — Unrecoverable Crash Vector

```rust
pub fn get_id(&self, peer: &Author) -> usize {
    *self.validator.address_to_validator_index().get(peer)
        .expect("peer not found in validator index")
}
```

This is a **public method on a non-test type** that panics on invalid input. While current callers handle the precondition, this is a latent defect:

- Any new call site that forgets to pre-check will crash the node.
- In a networked context, a malformed message with an unknown `Author` could propagate to this path if input validation is missed upstream.

**Remediation:** Return `Result<usize, RandError>` or `Option<usize>`. If the panic is intentional as a debug assertion, use `debug_assert!` instead and return an error in release builds.

### WARN-2: Plaintext Key Storage in RocksDB Without Access Controls

Key pair bytes are stored in a RocksDB column family (`KeyPairSchema`) with no encryption layer:

```
RandDb (RocksDB at <db_root>/rand_db)
  └── KeyPairSchema: epoch → serialized key pair
```

No mention of:
- File permission enforcement on the `rand_db` directory
- Encryption-at-rest (e.g., RocksDB `EncryptedEnv`)
- Access logging

Any process with filesystem read access to `rand_db` can extract all historical epoch key shares.

**Remediation:** At minimum, enforce strict directory permissions (`0700`). Preferably, use an encrypted storage backend or integrate with a platform key management service. Consider purging old epoch keys after they're no longer needed.

### WARN-3: Unbounded Memory from Future Round Buffering

```rust
pub const FUTURE_ROUNDS_TO_ACCEPT: u64 = 200;
```

A malicious peer can send shares for 200 future rounds. Each share is stored per-validator, so the upper bound is `200 × N_validators` buffered shares. With a large validator set, this could be non-trivial:

- 200 rounds × 100 validators = 20,000 buffered share objects
- No backpressure mechanism or eviction policy mentioned

**Remediation:** Add a hard memory cap or LRU eviction for buffered future shares. Monitor the buffer size via metrics.

### WARN-4: `#![allow(dead_code)]` on Module Root

```rust
#![allow(dead_code)]  // in rand/mod.rs
```

This silences warnings across the entire `rand/` subtree. Dead code in a security-critical module is a maintenance hazard — it can contain stale logic that appears active during review, and it increases the attack surface.

**Remediation:** Remove the blanket allow. Apply `#[allow(dead_code)]` surgically to specific items that are intentionally unused (e.g., test utilities), or remove the dead code entirely.

### WARN-5: Disabled DKG Integration Tests — No CI Enforcement

The DKG runtime tests are disabled with no `#[ignore]` annotation or CI tracking issue — they are simply commented out or `cfg(test)` gated behind a broken API:

```
// DKG runtime tests are temporarily disabled due to API changes in gaptos
```

"Temporarily disabled" tests with no tracking mechanism tend to stay disabled permanently. This creates a silent regression window for any `gaptos` API behavioral changes.

**Remediation:** Either fix the tests or add `#[ignore]` with a tracking issue. Add a CI check that fails if `#[ignore]` tests exceed a threshold or age limit.

---

## Info Findings

### INFO-1: Equivocation Detected but Not Reported

`aug_data_store.rs` detects equivocating validators (same author, different augmented data) but only returns an error to the caller. There is no:
- Persistent evidence log
- Forwarding to a slashing/reporting subsystem
- Metric emission for monitoring

This means equivocation events are silently swallowed. Even if slashing is out of scope, the evidence should be logged at `warn!` level and emitted as a metric for operational visibility.

### INFO-2: Mock Implementations Use Unconditional `Ok(())`

`MockShare` and `MockAugData` return `Ok(())` for all verification. While scoped to `pub(super)`, this pattern is fragile:

- A module reorganization could inadvertently widen visibility.
- No compile-time guard prevents mock types from being used in production configuration.

Consider gating mocks behind `#[cfg(test)]` rather than relying solely on visibility.

### INFO-3: 300ms Hardcoded Retry Delay

The reliable broadcast layer waits 300ms before requesting missing shares. This value appears to be hardcoded rather than configurable. In high-latency geographic deployments, this may be too aggressive; in low-latency environments, it adds unnecessary delay.

### INFO-4: `Arc<RandKeys>` Shared Across Async Tasks

`RandConfig` holds `keys: Arc<RandKeys>`, and `RandConfig` is passed to async tasks for share generation. The `Arc` provides shared immutable access, which is safe for reads. However, if `RandKeys` ever gains interior mutability (e.g., key rotation within an epoch), the current sharing pattern would require a `RwLock` or similar — and no comment documents this invariant.

---

## Summary

| Severity | Count | Key Items |
|----------|-------|-----------|
| **Critical** | 1 | Secret key not zeroized on drop |
| **Warning** | 5 | Panic on unknown peer, plaintext key storage, unbounded future-round buffer, blanket `dead_code` allow, disabled integration tests |
| **Info** | 4 | Silent equivocation, mock scoping, hardcoded retry, `Arc` sharing invariant |

**Top 3 Actionable Items:**
1. Implement `Zeroize` for `RandKeys` and wrap `ask` in `Zeroizing<T>`
2. Convert `get_id()` from panic to `Result`
3. Fix or properly track the disabled DKG integration tests

### ⚔️ Intention Attacker

# Intention Attack Report: Randomness & DKG Implementation

## Red Team Adversarial Analysis

I compared the stated audit intention (comprehensive security audit of the randomness/DKG subsystem) against the actual implementation and found the following **fatal or high-impact logical vulnerabilities** that the initial audit either missed, downplayed, or mischaracterized.

---

## CRITICAL Findings

### 1. CertifiedAugData Skips Inner Delta Verification on Receipt — Quorum-Signed Malformed Deltas Are Applied Blindly

**Severity: CRITICAL**

**Location:** `aug_data_store.rs:108-119`, `types.rs:520-523`

**The Bug:** When a node receives a `CertifiedAugData` message from a peer, the verification path is:

```
RandMessage::CertifiedAugData → msg.verify() → CertifiedAugData::verify()
    → verifier.verify_multi_signatures(&self.aug_data, &self.signatures)  ← ONLY checks BLS aggregate sig
    → DOES NOT call AugmentedData::verify() / derive_apk()
```

Then in `add_certified_aug_data`, the delta is applied to the local `RandConfig` via `augment()` **without re-verifying the cryptographic well-formedness of the delta**.

**Attack Vector:** If a Byzantine quorum (>= threshold weight of colluding validators) signs a `CertifiedAugData` containing a **malformed or adversarially-crafted delta**, every honest node that receives it will:
1. Pass the aggregate signature check (the signatures are valid — the content is what's malicious)
2. Apply the poisoned delta to their local `RandConfig` via `add_certified_delta`
3. Produce incorrect randomness or crash during share generation

**Why the initial audit missed it:** The audit report (Finding 4, Risk: LOW) concluded "the API surface is purely observational" and did not trace the full `CertifiedAugData` verification chain to discover that `AugmentedData::verify()` is never called on the receiver path.

**Impact:** An adversary controlling threshold-weight of validators can poison the augmented public key state of all honest nodes, breaking the unpredictability guarantee of the entire randomness protocol for the epoch.

---

### 2. `process_reset` Does Not Clear In-Memory Share Map — Stale Shares Survive Round Resets

**Severity: CRITICAL**

**Location:** `rand_manager.rs:187-197`, `rand_store.rs`

**The Bug:** When `process_reset(ResetSignal::TargetRound(round))` is called (e.g., after state sync):

```rust
fn process_reset(&mut self, request: ResetRequest) {
    self.block_queue = BlockQueue::new();                          // blocks dropped ✓
    self.rand_store.lock().update_highest_known_round(target_round); // watermark updated
    // BUT: the in-memory shares HashMap inside RandStore is NOT cleared
}
```

The `rand_store`'s internal `HashMap<Round, RandItem>` retains shares from rounds **before the reset target**. After resetting to round N, if the node receives new block metadata for round M < N (which shouldn't happen but could during byzantine behavior or fork recovery), those stale shares could be matched against new metadata and aggregated.

**Attack Vector:** During a state sync event, a Byzantine peer could have pre-seeded shares for future rounds. After the reset, if those rounds are re-proposed with different block content, the stale shares (generated against different `RandMetadata`) survive in the store. The `retain()` pruning only fires when `add_metadata` is called — but if the metadata happens to match (same epoch+round, different block hash that isn't checked at insertion), stale shares could contribute to aggregation with mismatched randomness inputs.

**Impact:** Potential randomness bias or aggregation failure after state sync, as shares computed against one block could be aggregated with metadata for a different block at the same round.

---

## HIGH Findings

### 3. Secret Key Shares Stored in Plaintext RocksDB Without Encryption or Zeroize

**Severity: HIGH**

**Location:** `storage/db.rs`, `storage/schema.rs`, `types.rs`

**The Bug (chain of failures):**

1. **No encryption-at-rest:** `RandDb` opens RocksDB with `Options::default()` — no encryption. The `KeyPairSchema` column family stores raw `(epoch, Vec<u8>)` key bytes via BCS serialization directly to disk.

2. **No `Zeroize` on drop:** The secret key travels as `Vec<u8>` through `save_key_pair_bytes` / `get_key_pair_bytes`. Standard `Vec<u8>` does not zero memory on deallocation — the key material persists in freed heap pages.

3. **`Arc<RandKeys>` amplifies the problem:** `RandConfig` clones distribute the same `Arc<RandKeys>` to `RandStore`, `AugDataStore`, `ShareAggregateState`, and the `verification_task`. The secret key remains live in memory until **all** of these components are dropped — which only happens at epoch end.

4. **No process isolation:** Any process with read access to `<db_root>/rand_db/` can extract the secret key share, reconstruct the augmented secret key, and generate valid randomness shares for any round in the epoch.

**Impact:** Full compromise of a node's randomness contribution for the current (and potentially future) epochs. Combined with compromising threshold-many nodes, an attacker can predict or bias all randomness.

---

### 4. Equivocation in AugData Ceremony Detected But Not Punished — Free Grinding Attack

**Severity: HIGH**

**Location:** `aug_data_store.rs:93-106`, `rand_manager.rs:445-448`

**The Bug:** When a validator sends two different `AugData` payloads (equivocation), the receiving node:

```rust
pub fn add_aug_data(&mut self, data: AugData<D>) -> anyhow::Result<AugDataSignature> {
    if let Some(existing_data) = self.data.get(data.author()) {
        ensure!(existing_data == &data, "[AugDataStore] equivocate data from {}", data.author());
        // Returns Err(...) — but ONLY locally
    }
    // ...
}
```

The caller logs the error and moves on:
```rust
Err(e) => error!("[RandManager] Failed to add aug data: {}", e),
```

**Attack Vector:** A Byzantine validator can send **different `AugData` (different deltas) to different subsets of honest validators**. Each honest node only sees one version (no equivocation from its perspective), signs it, and returns a signature. The attacker collects signatures for multiple delta values, selects the one that produces the most favorable augmented public key, and broadcasts only that `CertifiedAugData`.

This is a **grinding attack on the augmentation ceremony**. The attacker gets multiple "rolls" at choosing their delta, breaking the one-shot commitment property that bias resistance depends on.

**Why the initial audit missed it:** The audit noted equivocation detection exists (Finding 6.4, Risk: INFORMATIONAL) but did not analyze the **split-brain equivocation vector** where different nodes see different data.

**Impact:** A single Byzantine validator can bias their augmented public key contribution, weakening the bias resistance of the randomness protocol proportional to the number of honest nodes they can partition.

---

### 5. `RequestShare` Messages Have Zero Verification — Amplification DoS

**Severity: HIGH**

**Location:** `network_messages.rs` (line ~47)

**The Bug:** The `RequestShare` variant's `verify()` implementation:

```rust
RandMessage::RequestShare(_) => Ok(()),  // no verification whatsoever
```

Any peer (even one not in the validator set) that can send authenticated RPC messages can spam `RequestShare` for arbitrary rounds. The recipient will:
1. Look up shares in `RandStore`
2. Re-serialize and transmit share data back
3. Consume CPU for share lookup and network bandwidth for response

**Attack Vector:** An attacker floods a validator with `RequestShare` messages for rounds [0, FUTURE_ROUNDS_TO_ACCEPT]. Each request triggers a store lookup and potential share retransmission. With `FUTURE_ROUNDS_TO_ACCEPT = 200`, the amplification is bounded but the CPU and bandwidth cost is real.

**Impact:** Network-layer DoS against the randomness subsystem. Could delay share aggregation enough to stall block finalization.

---

## MEDIUM Findings

### 6. Threshold Is `floor(total_weight / 2)`, Not Strict Majority

**Severity: MEDIUM**

**Location:** `types.rs:635-637`, test harness at line 399

**The Bug:**
```rust
let half_total_weights = weights.clone().into_iter().sum::<usize>() / 2;
let weighted_config = WeightedConfig::new(half_total_weights, weights).unwrap();
```

And the comparison:
```rust
if self.total_weight < rand_config.threshold() {  // strict less-than
```

This means aggregation triggers when `total_weight >= floor(total_weight / 2)` — exactly 50% of weight suffices, not 50%+1. For even total weights, this creates a scenario where two disjoint sets of validators, each holding exactly half the weight, could independently produce valid randomness — breaking uniqueness.

**Impact:** If the validator set weight distribution allows a 50/50 split, two conflicting randomness values could be produced for the same round. This is unlikely in practice with odd total weights but is a protocol-level correctness bug.

---

### 7. DKG Integration Tests Disabled — Silent Regression Window

**Severity: MEDIUM**

**Location:** `rand/dkg/mod.rs`

**The Bug:** All 8 DKG test modules are `#[cfg(test)]` **and** additionally disabled with a comment: *"DKG runtime tests are temporarily disabled due to API changes in gaptos"*. Combined with the `#![allow(dead_code)]` on the top-level `mod.rs`, this means:

- No automated validation that `gaptos` PVSS/WVUF APIs produce correct outputs
- No detection of serialization format changes between Aptos upstream and Gravity's fork
- The `WeightedConfig` threshold semantics (Finding 6) could have changed in `gaptos` without being caught

**Impact:** Any behavioral regression in `gaptos` cryptographic primitives would silently propagate to production.

---

## Summary: Intention vs. Implementation Gap

| Audit Intention | What Was Found | Verdict |
|---|---|---|
| DKG correctness | Core DKG is in `gaptos` (out of scope), local tests disabled | **Gap: untestable** |
| Bias resistance | AugData equivocation enables grinding (Finding 4) | **BROKEN for Byzantine validators** |
| Private key handling | Plaintext on disk, no zeroize, Arc-amplified lifetime | **HIGH risk** |
| DKG HTTP endpoint interaction | Correctly read-only | **OK** |
| Gravity-specific modifications | Disabled tests + modified deps = uncovered regression surface | **Gap** |
| Epoch transition integration | `process_reset` doesn't clear share map (Finding 2) | **CRITICAL state leak** |

**Bottom Line:** The initial audit rated overall risk as LOW-MEDIUM. This adversarial analysis elevates the assessment to **HIGH**, with two CRITICAL findings (CertifiedAugData verification bypass, stale shares surviving reset) and three HIGH findings (plaintext key storage, equivocation grinding, RequestShare DoS) that represent exploitable logical vulnerabilities in the randomness protocol's trust model.

---

## [liveness_and_leader_election] Security audit of aptos-core/consensus/src/liveness/ directory and aptos-core/co

### 🎯 Intention

Security audit of aptos-core/consensus/src/liveness/ directory and aptos-core/consensus/src/round_manager.rs for: (1) leader election algorithm correctness and resistance to stake-grinding attacks; (2) pacemaker timeout logic for liveness guarantees under adversarial conditions; (3) round state machine transitions and potential for stuck rounds; (4) the interaction between the pacemaker and gravity-sdk's custom execution pipeline; (5) any gravity-sdk modifications to upstream liveness mechanisms that could affect chain progress under Byzantine faults.

### 🔍 Discovered Implementation

# Consensus Liveness Security Audit: gravity-sdk

## Executive Summary

This audit covers the leader election, pacemaker/timeout, and round state machine in `aptos-core/consensus/src/liveness/` and `round_manager.rs`, with focus on gravity-sdk-specific modifications that could affect chain liveness or safety under Byzantine conditions.

---

## 1. Leader Election Algorithm

### Architecture

The election system is layered through a chain of wrappers:

```
UnequivocalProposerElection
  └─ CachedProposerElection
       └─ LeaderReputation  (production)
          RotatingProposer   (testing)
          RoundProposer      (testing)
```

### `LeaderReputation` — Weighted Election (`leader_reputation.rs`)

**Selection weight** = `reputation_weight × voting_power (stake)`

Three reputation tiers via `ProposerAndVoterHeuristic`:
| Tier | Weight | Condition |
|------|--------|-----------|
| Active | 100 | Proposed or voted within sliding window |
| Inactive | 10 | No recent participation |
| Failed | 1 | Failure rate > `failure_threshold_percent` |

**Election seed construction:**
```
use_root_hash=true:  SHA3-256(accumulator_root_hash ‖ epoch ‖ round)
use_root_hash=false: SHA3-256(epoch ‖ round)
```

The seed feeds into `choose_index()`, which performs weighted random selection over cumulative stake-weighted reputation scores using `next_in_range()` (SHA3-256 → u128 modular reduction).

### Stake-Grinding Resistance

**Finding 1 — `exclude_round` lag mechanism.** History is fetched at `round - exclude_round` rather than the current round, creating a deliberate gap between the state a validator can influence and the state used for election. This is the primary anti-grinding defense: a validator cannot immediately benefit from manipulating the most recent blocks because those blocks are excluded from the reputation window.

**Finding 2 — `use_root_hash` is critical for unpredictability.** When `use_root_hash = false`, the seed is `(epoch, round)` — fully predictable. When `true`, the accumulator root hash from the committed state is included, making the seed depend on the entire transaction history. However, in the **DAG anchor election adapter** (`leader_reputation_adapter.rs`), the root hash is **hardcoded to `HashValue::zero()`** with a TODO comment, meaning the DAG path effectively has a predictable seed despite `use_root_hash=true` being set.

**Finding 3 — DAG adapter chain health clamping.** `CommitHistory::get_voting_power_participation_ratio()` clamps any ratio below 0.67 to 1.0 with a `// TODO: fix this once leader reputation is fixed` comment. This suppresses chain health backpressure signals in the DAG path, meaning degraded participation won't trigger the expected block size reduction.

### Equivocation Protection (`unequivocal_proposer_election.rs`)

Tracks `(Round, HashValue)` in a mutex. Rejects:
- A proposal from a non-elected author
- A second distinct proposal from the same author in the same round
- Any proposal for a round earlier than the last seen

Logs `SecurityEvent::InvalidConsensusProposal` on rejection. The single-element tracking (only the most recent `(round, block_id)`) is sufficient because rounds are monotonically increasing.

---

## 2. Pacemaker / Timeout Logic

### No `pacemaker.rs` Exists

The traditional pacemaker is fully refactored into **`RoundState`** (`round_state.rs`).

### Exponential Backoff Formula

```
timeout = base_ms × exponent_base^min(round_index, max_exponent) × 2
```

Where `round_index` = rounds elapsed since `highest_ordered_round + 3`. The `+3` accounts for the 3-chain commit rule in HotStuff — within 3 rounds of the last commit, the base timeout is used (index=0).

**Finding 4 — Double multiplier in `setup_timeout`.** All callers pass `multiplier=1`, but internally `setup_deadline` is called with `multiplier * 2 = 2`. This means actual timeout duration is **2× the computed exponential interval**. This is intentional design (gives the network more time before triggering timeout votes) but means the configured `base_ms` does not directly represent the actual timeout — it represents half the actual timeout.

**Finding 5 — Timeout re-arms on the same round, does NOT advance.** `process_local_timeout()` re-arms the timer for the same round and returns `true`. Round advancement only happens when a `TwoChainTimeoutCertificate` is formed from a quorum of timeout votes and processed via `process_certificates()`. This is correct BFT behavior — a single node's timeout cannot unilaterally advance the round.

### Timeout Certificate (TC) Formation Flow

```
1. Timer fires → process_local_timeout(R) → returns true
2. Round manager generates nil block, signs timeout vote
3. Broadcasts timeout vote to all validators
4. PendingVotes collects timeout votes
5. Quorum reached → TwoChainTimeoutCertificate formed
6. TC broadcast via SyncInfo
7. process_certificates(SyncInfo{TC}) → new_round = TC.round + 1
8. NewRoundEvent emitted with reason=Timeout
```

**Finding 6 — Non-validator bypass.** Gravity-SDK adds an `is_validator: bool` field to `RoundState`. Non-validators:
- Never schedule timeouts
- Return `None` from `process_certificates` even when round advances
- Short-circuit `process_local_timeout` immediately

This correctly prevents observer/VFN nodes from participating in liveness signaling, but means a misconfigured validator running as non-validator would silently stop participating in consensus without any alert beyond log messages.

---

## 3. Round State Machine Transitions

### Round Advancement (`process_certificates`)

```rust
new_round = max(QC.round, TC.round) + 1
if new_round > current_round → advance
if new_round <= current_round → ignore (stale)
```

**Finding 7 — No stuck-round protection beyond exponential backoff.** If a validator cannot form a QC (network partition) or TC (< 2f+1 nodes timing out), the round will keep re-arming timeouts with exponential backoff until `max_exponent` is reached, then plateau. There is no circuit-breaker or epoch-change trigger from pure round staleness. Liveness depends entirely on eventually reaching a quorum for either QC or TC.

### Vote Back-Pressure

```rust
fn sync_only(&self) -> bool {
    self.local_config.sync_only || self.block_store.vote_back_pressure()
}
```

When `vote_back_pressure()` returns true, the node:
- Stops voting on proposals
- Only broadcasts `SyncInfo`
- Proposals encountering backpressure are re-queued via `resend_verified_proposal_to_self()` with 10ms polling

**Finding 8 — Backpressure-induced liveness risk.** If enough validators simultaneously enter vote backpressure (execution pipeline overloaded), the network could lose the ability to form QCs. The timeout path would still function (nil blocks don't require execution), but chain progress would halt until backpressure clears. The 10ms polling interval (`BACK_PRESSURE_POLLING_INTERVAL_MS`) in `resend_verified_proposal_to_self` creates a tight loop that could exacerbate CPU pressure during overload.

---

## 4. Gravity-SDK Modifications to Upstream Liveness

### Critical Modification: Proposer Sleep (`round_manager.rs:382-386`)

```rust
tokio::time::sleep(Duration::from_millis(
    std::env::var("APTOS_PROPOSER_SLEEP_MS")
        .map(|s| s.parse().unwrap())
        .unwrap_or(200),
)).await;
```

**Finding 9 — 200ms mandatory proposer delay.** Every `process_new_round_event` sleeps for 200ms (configurable via env var) before generating a proposal. This is not present in upstream Aptos.

- **Liveness impact:** Adds 200ms of latency to every round, reducing theoretical maximum throughput.
- **Safety concern:** The env var uses `.unwrap()` on parse — a malformed value will **panic the consensus task**, halting the node. This is a denial-of-service vector if the environment is attacker-controlled.
- **Interaction with timeouts:** The 200ms sleep eats into the round timeout budget. If `base_ms` is configured too low (< ~500ms accounting for the 2× multiplier), the proposer might not have enough time to generate and broadcast a proposal before the round times out.

### Critical Modification: `GravityBlockExecutor` (`gravity_state_computer.rs`)

```
commit path: BlockBufferManager.set_commit_blocks() → persist_notifier.recv()
pre_commit: no-op (returns Ok(()))
```

**Finding 10 — Synchronous persistence wait in commit path.** `commit_blocks()` calls `persist_notifier.recv()`, blocking the consensus commit path until gravity's persistence layer confirms. If the persistence layer hangs or is slow:
- The consensus thread blocks
- No new rounds can be processed
- The node will eventually timeout and be perceived as failed by peers
- Reputation penalty accumulates (failed_weight=1), reducing future election probability

This creates a tight coupling between gravity's persistence layer and consensus liveness that does not exist in upstream Aptos.

**Finding 11 — `pre_commit_block` is a no-op.** Standard Aptos uses pre-commit as a durability checkpoint. Gravity skips this entirely, pushing all durability to the `BlockBufferManager` path. If `BlockBufferManager` fails between execution and commit, blocks may need to be re-executed.

### Epoch Change Handling

```rust
VerifiedEvent::EpochChange(epoch) => {
    if !self.wait_change_epoch_flag {
        self.block_store.fast_forward_sync_by_epoch(..., epoch).await?;
        self.wait_change_epoch_flag = true;
    }
}
```

**Finding 12 — One-shot epoch change flag.** `wait_change_epoch_flag` is set to `true` after the first epoch change and never reset. If a second `EpochChange` event arrives in the same `RoundManager` lifecycle, it will be silently ignored. This is likely intentional (epoch change triggers a full restart), but if the first `fast_forward_sync_by_epoch` partially fails and returns `Ok`, the node would be stuck in a state where it believes epoch change succeeded but hasn't fully synchronized.

### Event Loop Priority Bias (`round_manager.rs:1483+`)

```
Priority 1: Shutdown
Priority 2: Delayed QC
Priority 3: Buffered proposals (batch-drained, sorted by round)
Priority 4: Payload-ready callbacks
Priority 5: Votes, OrderVotes, SyncInfo, LocalTimeout, EpochChange
```

**Finding 13 — Timeout events at lowest priority.** `LocalTimeout` events share the lowest priority bucket with votes and sync messages. Under heavy proposal load, timeout processing could be delayed, extending the effective timeout beyond the configured value. However, since the timer is managed by `TimeService` (separate from the event loop), the timeout *message* is enqueued at the correct time — only processing is delayed.

---

## 5. Summary of Findings

| # | Area | Severity | Description |
|---|------|----------|-------------|
| 1 | Leader Election | Info | `exclude_round` lag prevents immediate stake-grinding |
| 2 | Leader Election | **Medium** | DAG adapter uses `HashValue::zero()` as root hash, making election seed predictable in DAG mode |
| 3 | Leader Election | **Low** | DAG chain health ratio clamped below 0.67 → backpressure suppressed |
| 4 | Timeout | Info | Actual timeout is 2× configured `base_ms` |
| 5 | Timeout | Info | Timeout re-arms same round; TC required to advance (correct BFT) |
| 6 | Timeout | **Low** | Misconfigured validator as non-validator silently stops participating |
| 7 | Round State | Info | No stuck-round circuit breaker beyond exponential backoff plateau |
| 8 | Liveness | **Medium** | Simultaneous vote backpressure across validators can halt chain progress; 10ms polling loop may worsen CPU pressure |
| 9 | Gravity-SDK | **Medium** | 200ms proposer sleep: adds latency, `.unwrap()` on env parse can panic consensus, eats into timeout budget |
| 10 | Gravity-SDK | **High** | Synchronous `persist_notifier.recv()` in commit path — persistence layer hang blocks consensus entirely |
| 11 | Gravity-SDK | **Low** | `pre_commit_block` no-op removes upstream durability checkpoint |
| 12 | Gravity-SDK | **Low** | One-shot `wait_change_epoch_flag` — partial epoch sync failure leaves node stuck |
| 13 | Round Manager | **Low** | Timeout processing at lowest event loop priority — can be delayed under high proposal load |

### 🛠️ Ground Review

# Ground Review (Blue Team): Code Quality & Engineering Safety

## Scope

Reviewed `aptos-core/consensus/src/round_manager.rs`, `round_state.rs`, and `leader_reputation.rs` against the implementation report. Focused exclusively on code hygiene, resource management, and engineering safety — not business logic.

---

## Critical

### CRIT-1: Unguarded `.unwrap()` on env var parse — consensus panic vector
**File:** `round_manager.rs:384`
```rust
std::env::var("APTOS_PROPOSER_SLEEP_MS").map(|s| s.parse().unwrap()).unwrap_or(200)
```
The outer `.unwrap_or(200)` handles a missing env var, but the inner `.unwrap()` on `.parse()` fires **after** the env var is confirmed present. Setting `APTOS_PROPOSER_SLEEP_MS=abc` panics the consensus task. This is a denial-of-service path if an attacker can influence the process environment (container sidecar injection, shared systemd unit, CI misconfiguration).

**Fix:** Replace with `.map(|s| s.parse().unwrap_or(200)).unwrap_or(200)` or use `.and_then(|s| s.parse().ok()).unwrap_or(200)`.

### CRIT-2: Blocking `.recv()` in consensus commit path — liveness deadlock risk
**File:** `gravity_state_computer.rs` (per audit Finding 10)
`commit_blocks()` calls `persist_notifier.recv()`, which is a **synchronous channel wait** inside an async context. If the persistence layer hangs:
- The consensus task blocks indefinitely
- No new rounds process; the node is perceived as dead
- Reputation degrades to `failed_weight=1`, compounding the liveness impact

This is the single highest-severity code hygiene issue. A bounded `recv_timeout()` or `tokio::time::timeout()` wrapper is essential to prevent an unrecoverable hang.

---

## Warning

### WARN-1: Detached `tokio::spawn` with no cancellation handle — resource leak under churn
**File:** `round_manager.rs:1009-1019`
```rust
tokio::spawn(async move {
    while start.elapsed() < Duration::from_millis(timeout_ms) {
        if !block_store.vote_back_pressure() { ... break; }
        sleep(Duration::from_millis(polling_interval_ms)).await;
    }
});
```
The spawned task has no `JoinHandle` stored and no `CancellationToken`. If the `RoundManager` is dropped (epoch change, shutdown), these tasks continue polling dead `Arc<BlockStore>` references until their individual timeouts expire. Under rapid epoch transitions or heavy backpressure, many orphan tasks could accumulate.

**Fix:** Store the `JoinHandle` and abort on drop, or use a shared `CancellationToken`.

### WARN-2: 10ms polling interval — CPU-hostile under load
**File:** `round_manager.rs:96`
```rust
pub const BACK_PRESSURE_POLLING_INTERVAL_MS: u64 = 10;
```
10ms hot-loop polling inside a `tokio::spawn` generates ~100 wakeups/sec per backpressured proposal. Under load (when backpressure is most likely), this **adds** CPU contention to a system already struggling with execution throughput. A notification-based approach (e.g., `tokio::sync::Notify`) or at minimum a longer interval (50-100ms) would be significantly less hostile.

### WARN-3: `debug_assert!` for validator check — silent no-op in release builds
**File:** `round_manager.rs:751`
```rust
debug_assert!(self.is_validator(), "Not validator, cannot process local timeout: {round}");
```
In release builds, a non-validator hitting `process_local_timeout` silently proceeds past this check. The actual guard is at line 753 (`round_state.process_local_timeout` returns false for non-validators), but this depends on `RoundState.is_validator` being correctly synchronized. If it's ever inconsistent, the `debug_assert` won't catch it in production.

**Fix:** Promote to a proper `ensure!()` or early-return with a warning log.

### WARN-4: `bail!()` as normal control flow in `process_local_timeout`
**File:** `round_manager.rs:805`
```rust
bail!("Round {} timeout, broadcast to all peers", round);
```
Timeout is routine under normal BFT operation, but the function returns `Err` via `bail!`. Every caller must treat this "error" as non-fatal. The same pattern at line 759 (`sync_only` bail). Using `Result` for expected control flow obscures genuine errors in logging and error-handling paths. Any upstream caller that doesn't explicitly suppress this will log it at error severity.

### WARN-5: Hardcoded `HashValue::zero()` in DAG election adapter
**File:** `leader_reputation_adapter.rs` (per audit Finding 2)
The DAG path passes a zero hash as the root hash into the election seed, making leader election deterministic from `(epoch, round)` alone. A validator can pre-compute future leaders for all rounds in the epoch. Even if DAG mode is not production-active today, this is a latent vulnerability that should be tracked.

---

## Info

### INFO-1: One-shot `wait_change_epoch_flag` is correct but fragile
**File:** `round_manager.rs:1592-1603`
The flag is never reset, which is safe because `RoundManager` is reconstructed per epoch. However, there is no assertion or type-level enforcement that this invariant holds — if the lifecycle changes (e.g., `RoundManager` reuse for performance), this becomes a silent epoch-skip bug. A comment documenting the lifecycle assumption would help.

### INFO-2: `biased` select deprioritizes timeout processing
**File:** `round_manager.rs:1497`
The `biased;` annotation in `tokio::select!` means timeout events (priority 5) are starved when buffered proposals (priority 3) or future completions (priority 4) are continuously ready. Under sustained high throughput, effective timeout latency exceeds the configured value. This is a design tradeoff, not a bug — but it means timeout configuration must account for processing delay under load.

### INFO-3: `let _ = validator_components;` — unnecessary rebinding
**File:** `round_manager.rs:774`
```rust
let _ = validator_components;
```
This line drops a borrow but serves no functional purpose — the borrow would be dropped at end of scope regardless. Likely a leftover from a refactor. Harmless but noisy.

### INFO-4: Double-counted `vote_back_pressure()` call in `sync_only()`
**File:** `round_manager.rs:733-734`
```rust
let sync_or_not = self.local_config.sync_only || self.block_store.vote_back_pressure();
if self.block_store.vote_back_pressure() {
```
`vote_back_pressure()` is called twice — once for the boolean and once for the warning. If this method is cheap (likely an `AtomicBool` load), this is fine. If it involves any locking or computation, it should be called once and cached.

---

## Summary

| Severity | Count | Key Themes |
|----------|-------|------------|
| **Critical** | 2 | Panic on malformed env var; synchronous blocking in async commit path |
| **Warning** | 5 | Orphan tasks, CPU-hostile polling, debug-only assertion, error-as-control-flow, predictable election seed |
| **Info** | 4 | Lifecycle assumptions, priority starvation, dead code, redundant calls |

**Top recommendations:**
1. **Immediately** fix the `.unwrap()` parse in `APTOS_PROPOSER_SLEEP_MS` — trivial one-liner, eliminates a crash vector.
2. **Add a timeout** around `persist_notifier.recv()` in the commit path — this is the most dangerous liveness coupling.
3. **Replace the 10ms polling loop** with a `Notify`-based wakeup or at minimum increase the interval and add a cancellation token.

### ⚔️ Intention Attacker

# Red Team Attack Report: Consensus Liveness Security Audit — gravity-sdk

## Methodology

All findings from the implementation report were verified against source code by four independent exploration agents. Additional attack surfaces were discovered through targeted hunting for authorization gaps, TOCTOU races, state mutation failures, unbounded channels, and integer overflows. This report covers only **fatal logical vulnerabilities**.

---

## Critical Findings

### CRITICAL-1: Synchronous Persistence Blocking Creates Single-Point Consensus Kill Switch

**Location:** `gravity_state_computer.rs:86-130, 139-192`

Both `commit_blocks()` and `commit_ledger()` call `self.runtime.block_on(...)` then loop over `notifier.recv().await` for every block in the batch. The consensus thread **cannot process any events** — including timeouts — until all notifiers fire.

**Attack:**
- If `BlockBufferManager` hangs (disk I/O stall, downstream deadlock), the consensus thread is **permanently blocked** — the node cannot even emit timeout votes.
- An adversary degrading the storage layer of **f+1 validators** (not 2f+1) halts the entire chain: no QC forms (missing votes) and no TC forms (blocked nodes can't timeout-vote either).
- The `pre_commit_block` no-op (confirmed: bare `Ok(())`) removes the upstream Aptos durability checkpoint, funneling all durability through this single blocking path.

**Severity: Critical** — Violates the fundamental BFT assumption that consensus can make progress independently of execution/storage speed.

---

### CRITICAL-2: Unbounded Channel Enables Memory Exhaustion DoS

**Location:** `round_state.rs:167` — `delayed_qc_tx: UnboundedSender<DelayedQcMsg>`

The delayed QC channel is `futures_channel::mpsc::unbounded()`. Every vote triggering the delayed QC path enqueues a message with zero backpressure.

**Attack:**
- A Byzantine validator floods cryptographically valid votes triggering delayed QC processing.
- The consumer (round manager event loop, **priority 4 — lowest**) falls behind under load.
- The channel grows without bound, exhausting node memory.
- Timeout events share the same lowest-priority bucket, so the recovery mechanism is itself starved by the flood.

**Severity: Critical** — Remote memory exhaustion by any validator in the set, no rate limiting or capacity bound.

---

## High Findings

### HIGH-1: Environment Variable Parse Panic Kills Consensus Task

**Location:** `round_manager.rs:384`, `epoch_manager.rs:1479`, `consensusdb/include/reader.rs:43,47`

Four env vars use `std::env::var("X").map(|s| s.parse().unwrap()).unwrap_or(default)`. The `.unwrap()` is on `parse()` — if the var is **set but malformed** (e.g., `"20o"`), the node panics on first round.

**Affected vars:** `APTOS_PROPOSER_SLEEP_MS`, `ENABLE_QUORUM_STORE`, `FIXED_PROPOSER`

**Severity: High** — Trivial DoS if environment is attacker-influenced (container sidecar, config injection).

---

### HIGH-2: Three Silent Fallback Paths Degrade Leader Election to Predictable Selection

**Location:** `leader_reputation.rs:144, 154, 176`

Three code paths return `HashValue::zero()` as the root hash, collapsing the election seed to `SHA3-256(epoch || round)` — fully predictable:

| Path | Trigger | Log Level |
|------|---------|-----------|
| Empty event set | Event pruning | `warn!` |
| `get_accumulator_root_hash` failure | Transient DB error | `error!` |
| DB init failure | Startup race | `warn!` |

Only 1 of 3 paths logs at `error!`. The DAG adapter **permanently** uses `HashValue::zero()` (hardcoded). A validator who knows the deterministic election schedule can pre-compute and strategically time participation to maximize future election probability.

**Severity: High** — Silent, potentially persistent degradation of leader election unpredictability.

---

## Medium Findings

### MEDIUM-1: Simultaneous Vote Backpressure Creates Network-Wide Deadlock

**Location:** `block_store.rs:994-1004`

`vote_back_pressure()` is a purely local check (`ordered_round - commit_round > limit`). No cross-validator coordination. If gravity's synchronous commit path (CRITICAL-1) slows all validators simultaneously, all suppress votes → no QC → no commits → gap never shrinks → **permanent deadlock**. The 10ms polling loop (`BACK_PRESSURE_POLLING_INTERVAL_MS = 10`) further taxes CPU during overload.

**Severity: Medium** — Requires correlated slowdown, but CRITICAL-1 makes this much more likely than upstream Aptos.

---

### MEDIUM-2: TOCTOU in Proposal Generation — Leader Check Before Async Sleep

**Location:** `proposal_generator.rs:354-362`

The `last_round_generated` mutex is released immediately after the round-check. The function then enters a long async path including gravity's **200ms proposer sleep**. During this window, the round/epoch can advance. The proposal is still generated for the stale context. Safety rules are the backstop, but the TOCTOU window is 200ms+ wide.

**Severity: Medium**

---

### MEDIUM-3: Vote Inserted Before Validator-Set Membership Gate

**Location:** `round_manager.rs:1228-1277`, `round_state.rs:304-317`

`insert_vote()` mutates `pending_votes` before the verifier's signature check completes downstream. No early "is this author in the validator set?" rejection before state mutation. If the verifier is bypassed or buggy, the vote permanently occupies a slot.

**Severity: Medium**

---

### MEDIUM-4: `u32` Counter Overflow in Reputation Scoring

**Location:** `leader_reputation.rs:393-401, 518-519`

Vote/proposal counts are `u32` with unchecked `*count += 1`. The failure threshold uses `cur_failed_proposals * 100` — unchecked multiplication overflowing at ~42.9M. Counter wrap makes active validators appear inactive (weight 100→10) or failing validators appear healthy, directly manipulating leader election weights.

**Severity: Medium**

---

### MEDIUM-5: 200ms Proposer Sleep Consumes Critical Timeout Budget

**Location:** `round_manager.rs:382-386`

With the confirmed 2× multiplier in `setup_timeout`, a `base_ms` of 250 yields 500ms real timeout — 200ms (40%) consumed before any work begins. Under initial exponential backoff rounds, the proposer may timeout despite being alive, degrading liveness.

**Severity: Medium**

---

## Cascading Attack Chain

The most dangerous aspect is the **composability** of these findings:

```
Storage layer degradation (f+1 nodes)
  → CRITICAL-1: Consensus threads block on persist_notifier.recv()
  → Blocked nodes stop emitting votes AND timeout votes
  → MEDIUM-1: Remaining nodes hit vote backpressure
  → No QC forms, no TC forms
  → CRITICAL-2: Vote flood during recovery fills unbounded channel
  → Memory exhaustion on recovering nodes
  → Complete chain halt
```

This requires degrading only **f+1** validators (not 2f+1) because synchronous blocking prevents those nodes from participating in timeout certificate formation.

---

## Recommendations (Priority Order)

| Priority | Fix | Target |
|----------|-----|--------|
| **P0** | Make `commit_blocks`/`commit_ledger` non-blocking — bounded channel with timeout, allow TC participation during slow persistence | CRITICAL-1 |
| **P0** | Bound `delayed_qc_tx` channel — replace `UnboundedSender` with bounded; drop on overflow | CRITICAL-2 |
| **P1** | Replace `.unwrap()` on `parse()` with `.unwrap_or(default)` across all env var sites | HIGH-1 |
| **P1** | Promote all `HashValue::zero()` fallbacks to `error!` + metric counter; fix DAG adapter | HIGH-2 |
| **P2** | Add early validator-set membership check before `insert_vote` | MEDIUM-3 |
| **P2** | Use `checked_mul`/`saturating_add` for reputation counters | MEDIUM-4 |
| **P3** | Make proposer sleep proportional to `base_ms` or remove entirely | MEDIUM-5 |
