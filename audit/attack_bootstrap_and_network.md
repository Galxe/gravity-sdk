# attack_bootstrap_and_network

- **Agent**: intention_attacker
- **Status**: success
- **Duration**: 76420ms
- **Steps**: 1

## Report

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

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Security Audit Report: `bootstrap.rs` and `network.rs`

## | 76420ms |
