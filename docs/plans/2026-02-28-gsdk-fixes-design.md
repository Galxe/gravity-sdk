# GSDK Security Fixes Design (Round 2)

Date: 2026-02-28

## GSDK-006: Panic-Inducing `unwrap()` in Consensus-Critical Execution Loop

**Problem:** `start_execution()`, `start_commit_vote()`, and `start_commit()` in `reth_cli.rs` contain 15+ bare `.unwrap()` calls on fallible operations (transaction decoding, signer recovery, DB reads, vector indexing). A single malformed transaction or transient DB error crashes the entire validator node. An attacker who can inject a malformed transaction into the ordering layer could DoS all validators simultaneously.

**Fix:** Replace `.unwrap()` calls with proper error handling:
1. `TransactionSigned::decode_2718().unwrap()` / `recover_signer().unwrap()` → skip the transaction with `warn!()` if malformed.
2. `senders[idx].unwrap()` / `transactions[idx].unwrap()` → filter out `None` entries and log skipped transactions.
3. `provider.recover_block_number().unwrap()` → propagate the error up via `?` (the functions already return `Result`).
4. `exec_blocks.unwrap()` after partial error check → merge the error branch to handle all errors before unwrapping.
5. `block_ids.last().unwrap()` → guard with `is_empty()` check (already present but unwrap follows).
6. `.set_state().await.unwrap()` / `.wait_for_block_persistence().await.unwrap()` → propagate via `?`.

**Files:** `bin/gravity_node/src/reth_cli.rs`

## GSDK-007: Panic in Relayer `get_last_state` on Missing Cached Result

**Problem:** `RelayerWrapper::get_last_state()` panics when `should_block_poll` returns `true` but no cached result exists. The panic message says "No cached result for uri" but the comment above says "fall through to poll". This is a logic bug: the panic should be a fallthrough to `poll_and_update_state()`.

**Fix:** Replace `panic!()` at L280 with:
```rust
warn!("No cached result for uri: {uri}, falling through to poll");
// Fall through to poll below
```
Remove the `return` path and let execution continue to `poll_and_update_state()`.

**Files:** `bin/gravity_node/src/relayer.rs`

## GSDK-008: Unvalidated Signer Recovery in Mempool External Transaction Ingestion

**Problem:** `add_external_txn()` calls `txn.recover_signer().unwrap()` on externally-submitted transactions. A crafted transaction with an invalid signature that passes `decode_2718` but fails `recover_signer` crashes the validator.

**Fix:** Replace `.unwrap()` with error handling:
```rust
let signer = match txn.recover_signer() {
    Some(s) => s,
    None => {
        tracing::error!("Failed to recover signer for external transaction");
        return false;
    }
};
```

**Files:** `bin/gravity_node/src/mempool.rs`

## GSDK-009: Verbose Internal Error Messages in HTTP API Responses

**Problem:** Error responses in `consensus.rs` and `dkg.rs` include `format!("{e:?}")` which leaks internal type names, schema names, and potentially file paths to API consumers.

**Fix:** Create a consistent error response pattern:
1. Log the detailed error server-side with `error!()`.
2. Return generic messages to clients: "Internal server error", "Resource not found", "Service unavailable".
3. Factor out into a helper function that takes a log-level error and returns a sanitized response.

**Files:** `crates/api/src/https/consensus.rs`, `crates/api/src/https/dkg.rs`

**Review Comments** reviewer: Lightman; state: Accepted;

## GSDK-010: `GLOBAL_CONFIG_STORAGE` `.unwrap()` Crashes Relayer During Startup Race

**Problem:** `GLOBAL_CONFIG_STORAGE.get().unwrap()` in `get_oracle_source_states()` panics if called before the storage is initialized. This can happen during node startup if the relayer is activated before the config storage is ready.

**Fix:** Replace `.get().unwrap()` with:
```rust
let config_storage = match GLOBAL_CONFIG_STORAGE.get() {
    Some(cs) => cs,
    None => {
        warn!("GLOBAL_CONFIG_STORAGE not yet initialized, returning empty oracle states");
        return vec![];
    }
};
```

**Files:** `bin/gravity_node/src/relayer.rs`

## GSDK-011: No Rate Limiting or Request Size Limits on HTTP/HTTPS Endpoints

**Problem:** The HTTP/HTTPS server has no rate limiting, body size limits, or connection limits. An attacker can flood endpoints or send oversized payloads to exhaust resources.

**Fix:** Add middleware layers to the router:
1. `axum::extract::DefaultBodyLimit::max(1_048_576)` (1 MB max request body)
2. `tower::limit::RateLimitLayer::new(100, Duration::from_secs(1))` (100 req/s)
3. `tower::limit::ConcurrencyLimitLayer::new(256)` (max concurrent requests)

**Files:** `crates/api/src/https/mod.rs`

**Review Comments** reviewer: Lightman; state: Accepted;

## GSDK-012: Sentinel Webhook URLs Not Validated for SSRF

**Problem:** `feishu_webhook` and `slack_webhook` in `AlertingConfig` are used directly in `reqwest::Client::post()` without SSRF validation. Same threat model as GSDK-004 but the fix was not applied to webhook URLs.

**Fix:** In `Config::load()`, after deserializing, call `validate_probe_url()` on each webhook URL:
```rust
if let Some(feishu) = &config.alerting.feishu_webhook {
    if !feishu.is_empty() {
        validate_probe_url(feishu)?;
    }
}
if let Some(slack) = &config.alerting.slack_webhook {
    if !slack.is_empty() {
        validate_probe_url(slack)?;
    }
}
```

**Files:** `bin/sentinel/src/config.rs`

## GSDK-013: `ensure_https` Middleware Ineffective for Plain TCP Connections

**Problem:** `ensure_https` checks `req.uri().scheme_str()` but Axum/hyper does not populate the URI scheme for incoming TCP connections. When TLS is not configured (cert/key are `None`), the fallback path serves all routes (including "HTTPS-only" ones) over plain HTTP. The middleware may not reject these requests because `scheme_str()` returns `None` (not `"http"`).

**Fix:** Separate HTTP and HTTPS routes onto different listeners:
1. When TLS is configured: bind `https_routes` to the TLS listener and `http_routes` to a plaintext listener (or only serve HTTPS).
2. When TLS is NOT configured: do not register the sensitive `https_routes` at all. Log a startup warning that consensus/DKG endpoints are disabled without TLS.

**Alternative:** If a single port is required, check `ConnectInfo` or a custom `Extension` set by the TLS acceptor to detect whether the connection is encrypted.

**Files:** `crates/api/src/https/mod.rs`

**Review Comments** reviewer: Lightman; state: Accepted;

## GSDK-014: Address Parse `unwrap()` Can Crash Server on Invalid Bind Address

**Problem:** `self.address.parse().unwrap()` at L127 panics with no diagnostic message if the configured address is malformed.

**Fix:** Replace with `.parse().unwrap_or_else(|e| panic!("Invalid bind address '{}': {e}", self.address))` or return a `Result`.

**Files:** `crates/api/src/https/mod.rs`

## GSDK-015: ReDoS Risk via User-Provided Regex in Sentinel Whitelist

**Problem:** Whitelist CSV accepts arbitrary regex patterns. A crafted evil regex can freeze the sentinel via catastrophic backtracking.

**Fix:** Use `RegexBuilder::new(pattern_str).size_limit(1 << 20).dfa_size_limit(1 << 20).build()` to limit compiled regex complexity. Log a warning and skip the rule if compilation exceeds the size limit.

**Files:** `bin/sentinel/src/whitelist.rs`

## GSDK-016: Glob Pattern Injection in Sentinel File Monitoring

**Problem:** `file_patterns` config field is passed directly to `glob()`. A pattern like `/**/*` recursively scans the entire filesystem.

**Fix:** Validate patterns in `Config::load()`:
1. Reject patterns containing `..`
2. Reject patterns starting with `/` (require relative paths)
3. Add a maximum depth limit for glob results

**Files:** `bin/sentinel/src/config.rs`, `bin/sentinel/src/watcher.rs`
