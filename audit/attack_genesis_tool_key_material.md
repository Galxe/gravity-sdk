# attack_genesis_tool_key_material

- **Agent**: intention_attacker
- **Status**: success
- **Duration**: 152510ms
- **Steps**: 1

## Report

# Intention Attack Report: `genesis-tool/` Security Audit

## Attack Surface: Genesis Ceremony Artifact Production & Tampering

The genesis-tool is responsible for bootstrapping the entire chain's initial state — validators, staking, governance, and system contracts. A flaw here compromises the chain from block 0.

---

## CRITICAL

### 1. `extract_runtime_bytecode` Deploys Constructor Initcode as Runtime Bytecode

**File:** `execute.rs:72-89`

The function is named "extract runtime bytecode" but **never actually extracts it**. When it detects constructor-style bytecode (leading `0x60`/`0x61`, length > 100), it logs a warning and returns the bytes unchanged:

```rust
warn!("Using constructor bytecode as runtime bytecode");
bytes  // returns constructor bytecode, NOT runtime bytecode
```

**Attack Vector:** All 21 system contracts in genesis may contain deployment initcode instead of executable contract logic. The EVM will interpret constructor preamble opcodes as contract logic, producing **completely undefined behavior** at every system contract address. A malicious actor who controls the `.hex` files can craft initcode that *appears* to deploy correctly in the genesis simulation but contains a backdoor in the actual runtime portion that is never inspected.

**Compounding factor:** `hex::decode(...).unwrap_or_default()` silently returns an empty `Vec` on decode failure — a missing or corrupted `.hex` file produces an empty-bytecode contract with **zero error signal**.

---

## HIGH

### 2. Genesis Contract Retains ~1,000,000 Phantom ETH

**File:** `execute.rs:23-45, 206`

The Genesis contract is pre-funded with `total_stake + 1,000,000 ETH` to facilitate the simulation. After execution, only `SYSTEM_CALLER` is removed from the bundle state:

```rust
bundle_state.state.remove(&SYSTEM_CALLER);
// Genesis contract's 1M ETH buffer is NEVER cleaned up
```

**Attack Vector:** The Genesis contract starts the live chain holding ~1M ETH of phantom supply that was never backed by any deposit. If the Genesis contract has any withdrawal, `selfdestruct`, or delegate-callable path, this phantom ETH becomes extractable — inflating real token supply. Even without an explicit withdrawal path, this breaks total supply accounting from genesis.

### 3. Proof-of-Possession (PoP) and Consensus Public Keys Are Never Validated

**File:** `genesis.rs:494-509`

`consensusPop` and `consensusPubkey` are parsed from hex and passed straight through to `Genesis.initialize()` with **zero cryptographic validation**:

- No check that `consensusPubkey` is a valid BLS12-381 G1/G2 point
- No check that `consensusPop` proves knowledge of the corresponding secret key
- No check for rogue-key attacks (malicious pubkey that cancels out honest validators' aggregate key)

**Attack Vector — Rogue Key Attack:** A malicious validator submits a crafted BLS public key computed as `g^x - PK_honest` (where `PK_honest` is an honest validator's key). In threshold signing, the attacker can now forge aggregate signatures without the honest validator's cooperation. The genesis tool is the **only pre-chain validation layer** — if it doesn't verify PoPs, the chain launches with a compromised validator set.

**Confirmed by config:** `genesis_config.json` shows all validators have `consensusPop: "0x"` (empty), proving this path is exercised with zero-length PoPs in practice.

---

## MEDIUM

### 4. `calculate_total_stake` Silently Wraps on U256 Overflow

**File:** `genesis.rs:527-533`

```rust
.fold(U256::ZERO, |acc, stake| acc + stake)  // unchecked addition
```

U256 `+` wraps on overflow in release builds. A crafted config with stake amounts summing past `2^256 - 1` produces a near-zero `total_stake`. This propagates to:
- `SYSTEM_CALLER` balance (insufficient to fund `Genesis.initialize`)
- `msg.value` of the initialize call (validators receive near-zero stake)
- Genesis contract balance (near-zero instead of total stake)

**Impact:** A validator set could be bootstrapped where the on-chain staking contract believes total stake is ~0, making the cost of a 2/3 majority attack essentially free.

### 5. `verify` Subcommand Hardcodes Chain ID 1337, Misses Critical Checks

**File:** `verify.rs:193`

```rust
let env = prepare_env(1337);  // hardcoded, ignores actual genesis chain ID
```

The verification step:
- Always simulates against chain ID 1337 regardless of the genesis file's actual chain ID
- Does **not** check whether bytecode is constructor vs. runtime (Finding 1)
- Does **not** check phantom balances (Finding 2)
- Does **not** validate PoPs (Finding 3)
- Does **not** verify voting power, stake amounts, or operator/owner relationships
- Has multiple verification functions commented out (`verify_jwks`, `verify_epoch_config`, `verify_randomness_config`)

**Impact:** The verify step provides a **false sense of security** — it passes on genesis files containing all of the above critical/high issues.

### 6. Pre-funded Hardhat Account #0 in `genesis_template.json`

**File:** `genesis_template.json`

Account `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266` is pre-funded. This is the default Hardhat/Anvil account derived from the **well-known mnemonic** `test test test test test test test test test test test junk`. The private key is `0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80`.

**Attack Vector:** If this template reaches a production or public testnet deployment without stripping this entry, **anyone** can immediately drain those funds. The orchestration script (`genesis.sh`) merges this template with additional allocs via `jq`, and there is no guard that removes test accounts for non-local deployments.

### 7. Default Chain ID 1337 Enables Cross-Chain Replay

**File:** `genesis.rs:69-71`

If `chainId` is omitted from the genesis config, it silently defaults to `1337`. A deployment that accidentally omits this field will share chain ID `1337` with every Hardhat/local test network, making all EIP-155 signed transactions **replayable across chains**.

---

## Attack Chain Summary

A sophisticated attacker controlling a single validator's genesis config entry could:

1. Submit a **rogue BLS public key** with empty PoP (Finding 3) → gain threshold signing capability
2. Craft stake amounts to **overflow total_stake** (Finding 4) → reduce economic security to ~0
3. Supply a **backdoored `.hex` file** that passes the broken bytecode heuristic (Finding 1) → inject arbitrary logic into a system contract
4. None of the above would be caught by `verify` (Finding 5)

The genesis ceremony produces a chain that is **compromised from block 0** with no detection mechanism.

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Intention Attack Report: `genesis-tool/` Security Audit

# | 152510ms |
