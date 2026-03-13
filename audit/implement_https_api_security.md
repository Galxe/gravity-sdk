# implement_https_api_security

- **Agent**: implementation_analyzer
- **Status**: success
- **Duration**: 154240ms
- **Steps**: 1

## Report

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

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # HTTP/HTTPS API Server — Implementation Analysis

## Files  | 154240ms |
