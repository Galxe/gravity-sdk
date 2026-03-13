# attack_consensus_network_and_message_auth

- **Agent**: intention_attacker
- **Status**: success
- **Duration**: 60459ms
- **Steps**: 1

## Report

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

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Red Team Attack Report: Consensus Network Layer

## Verifi | 60459ms |
