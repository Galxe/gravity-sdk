# attack_relayer_oracle_integration

- **Agent**: intention_attacker
- **Status**: success
- **Duration**: 69900ms
- **Steps**: 1

## Report

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

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Intention Attack Report: `bin/gravity_node/src/relayer.rs` | 69900ms |
