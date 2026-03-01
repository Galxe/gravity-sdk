# Security Audit Report — gravity-sdk (Round 2)

**Date:** 2026-02-28
**Scope:** Gravity-specific code in `crates/api/`, `crates/block-buffer-manager/`, `bin/gravity_node/`, `bin/sentinel/`
**Repository:** https://github.com/Galxe/gravity-sdk
**Release:** gravity-testnet-v1.0.0 (post-Round 1 fixes)
**Skills Used:** `security-auditor`, `security-scanning-security-sast`, `security-scanning-security-hardening`, `threat-modeling-expert`, `stride-analysis-patterns`, `memory-safety-patterns`

---

## Summary

| Severity | Findings | Status |
|----------|----------|--------|
| HIGH | 3 | Open |
| MEDIUM | 5 | Open |
| LOW | 3 | Open |
| **Total** | **11** | **All open** |

---

## HIGH Severity (3)

### GSDK-006: Panic-Inducing `unwrap()` in Consensus-Critical Execution Loop

**File:** `bin/gravity_node/src/reth_cli.rs:130-131, 201-202, 270, 305, 311, 372, 387, 392-399, 407, 413, 417, 420`
**Issue:** The `start_execution()`, `start_commit_vote()`, and `start_commit()` methods — the three main consensus-critical event loops — are peppered with bare `.unwrap()` calls. For example:
- `TransactionSigned::decode_2718(&mut slice).unwrap()` (L130) — a malformed transaction from the ordering layer panics the node.
- `txn.recover_signer().unwrap()` (L131) — a transaction with an unrecoverable signer panics the node.
- `senders.into_iter().map(|x| x.unwrap()).collect()` (L201-202) — if any sender or transaction is `None`, the node crashes.
- `self.provider.recover_block_number().unwrap()` (L270, L372, L413) — DB read failures crash the node.
- `exec_blocks.unwrap()` (L305) after an `if let Err` check that only handles epoch-change errors — any other error variant skips the check and panics on the next line.
- Multiple `block_ids.last().unwrap()` (L311, L392, L397, L399) — empty vector panics.

A single malformed transaction or transient database error causes the entire validator node to crash, potentially halting the chain if enough validators are affected simultaneously. An attacker who can inject a malformed transaction into the ordering layer (e.g., via a compromised mempool peer) could DoS all validators.

**Recommended Fix:** Replace all `.unwrap()` calls in these critical loops with proper error handling (`.map_err()`, `?`, or `match`). Log the error and skip the problematic block/transaction rather than crashing the validator.

---

### GSDK-007: Panic in Relayer `get_last_state` on Missing Cached Result

**File:** `bin/gravity_node/src/relayer.rs:280`
**Issue:** In `RelayerWrapper::get_last_state()`, when `should_block_poll` returns `true` but no cached result exists, the code calls `panic!("No cached result for uri: {uri}")`. This is a logic bug: the `should_block_poll` condition can be `true` (because `fetched_nonce > onchain_nonce`) while `last_result` is `None` if state was partially initialized. A panic in the relayer disrupts oracle data polling, potentially stalling on-chain oracle updates for all validators.

**Recommended Fix:** Replace the `panic!()` with a warning log and fall through to `poll_and_update_state` instead of crashing. This is the defensive intended behavior indicated by the comment "No cached result available, fall through to poll".

---

### GSDK-008: Unvalidated Signer Recovery in Mempool External Transaction Ingestion

**File:** `bin/gravity_node/src/mempool.rs:187`
**Issue:** In `add_external_txn()`, `txn.recover_signer().unwrap()` panics if signer recovery fails. External transactions come from untrusted network peers. A maliciously crafted transaction with an invalid signature that passes `decode_2718` but fails `recover_signer` would crash the validator node. This is directly exploitable by any network participant that can submit transactions.

**Recommended Fix:** Replace `.unwrap()` with error handling. Return `false` and log the error when signer recovery fails, similar to the `decode_2718` error handling already in place.

---

## MEDIUM Severity (5)

### GSDK-009: Verbose Internal Error Messages in HTTP API Responses

**File:** `crates/api/src/https/consensus.rs:92-93, 125, 175, 277, 314, 323`, `crates/api/src/https/dkg.rs:100, 145, 149, 161, 234, 238`
**Issue:** Error responses include full Rust debug-formatted error messages via `format!("{e:?}")`. These messages leak internal implementation details including:
- Database schema names (`EpochByBlockNumberSchema`, `LedgerInfoSchema`)
- Internal type names (`ValidatorSet`, `DKGState`)
- Stack trace fragments and memory addresses
- Database path information

An attacker can use these error details to fingerprint the exact software version, identify internal data structures, and craft targeted exploits.

**Recommended Fix:** Return generic error messages to clients (e.g., "internal server error", "not found") and log the detailed error server-side only. Create a consistent error response helper that strips internal details.

---

### GSDK-010: `GLOBAL_CONFIG_STORAGE` `.unwrap()` Crashes Relayer During Startup Race

**File:** `bin/gravity_node/src/relayer.rs:110`
**Issue:** `GLOBAL_CONFIG_STORAGE.get().unwrap()` in `get_oracle_source_states()` panics if the global config storage hasn't been initialized yet. During node startup, there's a race condition: the relayer's `add_uri()` or `get_last_state()` can be called before `GLOBAL_CONFIG_STORAGE` is initialized. A crash during startup can cause a restart loop if the initialization order is not deterministic.

**Recommended Fix:** Use `.get()` with a match/`if let` guard, returning an empty `Vec` with a warning log if the config storage isn't available yet (consistent with other fallible operations in the same function).

---

### GSDK-011: No Rate Limiting or Request Size Limits on HTTP/HTTPS Endpoints

**File:** `crates/api/src/https/mod.rs:107-126`
**Issue:** The HTTP and HTTPS servers have no rate limiting middleware, no request body size limits, and no connection limits. An attacker can:
1. **DoS via request flooding:** Unlimited requests per second to any endpoint.
2. **Memory exhaustion via large payloads:** The `/tx/submit_tx` POST endpoint accepts arbitrary-size JSON bodies. Axum's default body limit is generous; a multi-GB JSON payload could exhaust memory.
3. **Connection exhaustion:** Unlimited concurrent connections can exhaust file descriptors.

Database-backed endpoints like `/consensus/block/:epoch/:round` perform full epoch scans (`get_range`) which are expensive — repeated requests could CPU-starve the validator.

**Recommended Fix:** Add `tower::limit::RateLimitLayer` or axum's built-in rate limiting, `axum::extract::DefaultBodyLimit` for request size limits, and connection limits on the listener.

---

### GSDK-012: Sentinel Webhook URLs Not Validated for SSRF

**File:** `bin/sentinel/src/notifier.rs:34, 50`, `bin/sentinel/src/config.rs:10-15`
**Issue:** The `feishu_webhook` and `slack_webhook` URLs in `AlertingConfig` are passed directly to `reqwest::Client::post()` without any validation. Unlike the `ProbeConfig.url` which was fixed in GSDK-004/005, the webhook URLs are never checked against the SSRF blocklist. An attacker who modifies `sentinel.toml` (same threat model as GSDK-004) can set webhook URLs to internal endpoints or cloud metadata URLs. Since alert messages include log content, this could exfiltrate sensitive data.

**Recommended Fix:** Apply the same `validate_probe_url()` validation to webhook URLs during `Config::load()`.

---

### GSDK-013: `ensure_https` Middleware Ineffective for Plain TCP Connections

**File:** `crates/api/src/https/mod.rs:34-39, 126`
**Issue:** The `ensure_https` middleware checks `req.uri().scheme_str() != Some("https")`. However, when running behind `axum_server::bind()` (plain HTTP, the fallback path on L146-151), the request URI scheme is typically `None` (not `"http"`) for connections received on plain TCP sockets. Axum/hyper does not populate the scheme for incoming requests. This means:
1. The `https_routes` protected by `ensure_https` may incorrectly reject HTTPS requests if the scheme isn't populated (defensive but potentially breaking).
2. More critically, in the HTTP fallback path, both `https_routes` and `http_routes` are merged into the same router — the middleware layer on `https_routes` may not block HTTP access as intended if the URI scheme isn't set.

When TLS certificates are not configured (`None`), all routes including the "HTTPS-only" consensus/DKG endpoints are served over plain HTTP, and the `ensure_https` middleware may silently pass depending on the runtime URI population behavior.

**Recommended Fix:** Instead of relying on URI scheme inspection, either (a) create separate routers for HTTP and HTTPS listeners on different ports, or (b) use a TLS-presence check via connection info (e.g., `ConnectInfo` extension) to determine if TLS is active.

---

## LOW Severity (3)

### GSDK-014: Address Parse `unwrap()` Can Crash Server on Invalid Bind Address

**File:** `crates/api/src/https/mod.rs:127`
**Issue:** `self.address.parse().unwrap()` panics if the configured bind address is malformed. While this is a configuration error (not remotely exploitable), it provides no diagnostic message — the process crashes with a generic unwrap panic rather than a clear error about the invalid address.

**Recommended Fix:** Use `.parse().expect("Invalid bind address: {}")` or return a `Result` from `serve()`.

---

### GSDK-015: ReDoS Risk via User-Provided Regex in Sentinel Whitelist

**File:** `bin/sentinel/src/whitelist.rs:29-33`
**Issue:** The whitelist CSV file accepts arbitrary regex patterns via `Regex::new(pattern_str)`. A maliciously crafted regex (e.g., `(a+)+$`) can cause catastrophic backtracking when matched against log lines, effectively freezing the sentinel monitoring process. While the whitelist CSV is a local file (requiring filesystem access), the same threat model as GSDK-004 applies (compromised CI/CD, shared config management).

**Recommended Fix:** Use `regex::RegexBuilder::new().size_limit(1 << 20).build()` to limit compiled regex size, and wrap the `is_match` call in a timeout or use `regex::Regex` with the default safety limits (which already mitigates most ReDoS). Alternatively, document that only literal strings are supported and always escape the pattern.

---

### GSDK-016: Glob Pattern Injection in Sentinel File Monitoring

**File:** `bin/sentinel/src/watcher.rs:26`, `bin/sentinel/src/config.rs:31`
**Issue:** The `file_patterns` field in `MonitoringConfig` is passed directly to `glob()`. A glob pattern like `/**/*` would recursively enumerate the entire filesystem, causing high I/O and potential DoS of the sentinel process. While this requires config file write access (same threat model as GSDK-004), it could be used to slow down or stall error monitoring, allowing an attacker to delay detection of other attacks.

**Recommended Fix:** Validate glob patterns at `Config::load()` time: reject patterns that start with `/` or contain `..`, and limit the depth of recursion.

---

## Commits

(No commits — all findings are open)

## Design Documents

- [Round 2 Security Fixes Design](../plans/2026-02-28-gsdk-fixes-design.md)
- [Round 1 Security Audit Report](./2026-02-23-security-audit-report.md)
- [Round 1 Security Fixes Design](../plans/2026-02-23-gsdk-fixes-design.md)
