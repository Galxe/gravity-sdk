# attack_gravity_cli_operations

- **Agent**: intention_attacker
- **Status**: success
- **Duration**: 910113ms
- **Steps**: 1

## Report

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

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Intention Attack Report: `gravity_cli` Security Audit

##  | 270073ms |
