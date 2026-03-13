# attack_consensus_engine_initialization

- **Agent**: intention_attacker
- **Status**: success
- **Duration**: 101382ms
- **Steps**: 1

## Report

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

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Intention Attack Report: `crates/api/src/consensus_api.rs` | 101382ms |
