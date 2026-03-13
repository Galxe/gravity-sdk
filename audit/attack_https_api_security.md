# attack_https_api_security

- **Agent**: intention_attacker
- **Status**: success
- **Duration**: 80958ms
- **Steps**: 1

## Report

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

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Intention Attack Report — HTTP/HTTPS API Server Security A | 80957ms |
