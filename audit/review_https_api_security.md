# review_https_api_security

- **Agent**: ground_reviewer
- **Status**: success
- **Duration**: 78657ms
- **Steps**: 1

## Report

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

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Ground Review (Blue Team) — HTTP/HTTPS API Server

## Crit | 78657ms |
