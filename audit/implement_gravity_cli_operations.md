# implement_gravity_cli_operations

- **Agent**: implementation_analyzer
- **Status**: success
- **Duration**: 195209ms
- **Steps**: 1

## Report

# Implementation Analysis: `gravity_cli` Binary

## Files/Contracts Involved

| File | Description |
|------|-------------|
| `bin/gravity_cli/src/main.rs` | Entry point; parses CLI args via clap and dispatches to subcommand handlers |
| `bin/gravity_cli/src/command.rs` | Defines top-level `Command` struct, `SubCommands` enum, `Executable` trait, and version info |
| `bin/gravity_cli/src/contract.rs` | Solidity ABI bindings for `ValidatorManagement` and `Staking` system contracts; hardcoded addresses |
| `bin/gravity_cli/src/util.rs` | ETH/wei conversion helpers (`format_ether`, `parse_ether`) |
| `bin/gravity_cli/src/stake/mod.rs` | Module root for `Create` and `Get` staking subcommands |
| `bin/gravity_cli/src/stake/create.rs` | Creates a new StakePool via on-chain transaction |
| `bin/gravity_cli/src/stake/get.rs` | Queries PoolCreated events to list StakePools by owner |
| `bin/gravity_cli/src/validator/mod.rs` | Module root for `Join`, `Leave`, `List` validator subcommands |
| `bin/gravity_cli/src/validator/join.rs` | Registers a validator and joins the validator set |
| `bin/gravity_cli/src/validator/leave.rs` | Leaves the validator set |
| `bin/gravity_cli/src/validator/list.rs` | Read-only listing of all validators as JSON |
| `bin/gravity_cli/src/dkg/mod.rs` | Module root for `Status` and `Randomness` DKG subcommands |
| `bin/gravity_cli/src/dkg/randomness.rs` | Queries DKG randomness for a block via HTTP |
| `bin/gravity_cli/src/dkg/status.rs` | Queries DKG status via HTTP |
| `bin/gravity_cli/src/genesis/mod.rs` | Module root for `GenerateKey`, `GenerateWaypoint`, `GenerateAccount` |
| `bin/gravity_cli/src/genesis/key.rs` | Generates full validator identity (BLS12-381 + X25519 + Ed25519 keys) |
| `bin/gravity_cli/src/genesis/account.rs` | Generates a standard Ethereum secp256k1 account |
| `bin/gravity_cli/src/genesis/waypoint.rs` | Generates a waypoint from genesis validator config JSON |
| `bin/gravity_cli/src/node/start.rs` | Starts a node by executing `script/start.sh` |
| `bin/gravity_cli/src/node/stop.rs` | Stops a node by executing `script/stop.sh` |

---

## Execution Path: Entry Point

1. `main()` calls `Command::parse()` (clap derives CLI parsing).
2. Pattern-matches on `SubCommands` enum variants: `Genesis`, `Validator`, `Stake`, `Node`, `Dkg`.
3. Each variant is further matched to its sub-subcommand, calling `.execute()` on the resolved struct.
4. If `execute()` returns `Err`, the error is printed to stderr and the process exits with code 1.

---

## Key Functions by Module

### 1. `command.rs` — CLI Structure

- **`Command`**: Top-level clap `Parser` struct. Name = `"gravity-cli"`, version derived from `build_information!()` macro via `OnceLock` statics.
- **`SubCommands`**: Enum with variants `Genesis`, `Validator`, `Stake`, `Node`, `Dkg`.
- **`Executable` trait**: Single method `fn execute(self) -> Result<(), anyhow::Error>`.
- **`short_version()`**: Returns package version string from build info.
- **`long_version()`**: Returns all build info key-value pairs joined by newlines.

### 2. `contract.rs` — ABI Bindings

- **`VALIDATOR_MANAGER_ADDRESS`**: Hardcoded to `0x00000000000000000000000000000001625F2001`.
- **`STAKING_ADDRESS`**: Hardcoded to `0x00000000000000000000000000000001625F2000`.
- **`alloy_sol_macro::sol!` block**: Defines Solidity-compatible types:
  - `ValidatorStatus` enum: `INACTIVE(0)`, `PENDING_ACTIVE(1)`, `ACTIVE(2)`, `PENDING_INACTIVE(3)`.
  - `ValidatorConsensusInfo` struct: validator, consensusPubkey, consensusPop, votingPower, validatorIndex, networkAddresses, fullnodeAddresses.
  - `ValidatorRecord` struct: full validator record with moniker, status, bond, feeRecipient, pendingFeeRecipient, stakingPool, validatorIndex.
  - `ValidatorManagement` contract: `registerValidator`, `joinValidatorSet`, `leaveValidatorSet`, `rotateConsensusKey`, `setFeeRecipient`, plus view functions and events.
  - `Staking` contract: `createPool` (payable), `isPool`, `getPoolVotingPower`, `getPoolVotingPowerNow`, `getPoolOperator`, `getPoolOwner`, `getPoolLockedUntil`, `getPoolActiveStake`, `getPoolCount`, `getPool`, `getAllPools`, plus `PoolCreated` event.
- **`status_from_u8(value: u8) -> ValidatorStatus`**: Maps 0–3 to enum variants; anything else maps to `__Invalid`.

### 3. `util.rs` — Conversion Helpers

- **`format_ether(wei: U256) -> String`**: Converts wei to ETH decimal string. If the wei string has ≤18 digits, it produces `0.{zero-padded}{digits}`. Otherwise splits at position `len-18`. Trailing zeros are trimmed from the decimal portion.
- **`parse_ether(eth_amount: &str) -> Result<U256>`**: Splits on `'.'`. Integer-only input gets 18 zeros appended. Fractional part is right-padded with zeros to 18 digits. Rejects inputs with >1 decimal point or >18 fractional digits.

### 4. `stake/create.rs` — CreateCommand

**Parameters**: `--rpc-url`, `--private-key`, `--gas-limit` (default 2,000,000), `--gas-price` (default 20), `--stake-amount`, `--lockup-duration` (default 2,592,000 = 30 days).

**Execution path**:
1. Strips `0x` prefix from private key, hex-decodes to bytes, constructs `SigningKey` → `PrivateKeySigner`.
2. Builds an `alloy` HTTP provider with the signer wallet.
3. Fetches chain ID and wallet balance (printed).
4. Parses `stake_amount` via `parse_ether()` to get wei value.
5. Fetches latest block timestamp; computes `locked_until = (timestamp + lockup_duration) * 1_000_000` (microseconds).
6. Constructs `Staking::createPoolCall` with `owner = staker = operator = voter = wallet_address`.
7. Sends transaction with `value = stake_wei` to `STAKING_ADDRESS`.
8. Waits for 2 confirmations (60s timeout).
9. Fetches receipt, parses `PoolCreated` event logs to extract the new pool address.

**State changes**: Creates a new StakePool on-chain. The sent ETH value becomes the pool's initial stake. All four roles (owner, staker, operator, voter) are set to the signing wallet's address.

### 5. `stake/get.rs` — GetCommand

**Parameters**: `--rpc-url`, `--owner`, `--from-block` (default `"auto"`), `--to-block` (default `"latest"`), `--show-voting-power` (default `true`).

**Execution path**:
1. Parses owner address, zero-pads to 32 bytes for topic filtering.
2. Creates an unauthenticated provider (no wallet needed — read-only).
3. Resolves block range: `"auto"` or `"earliest"` → `latest_block - 90_000` (saturating subtraction); `"latest"` → `Latest`; otherwise → parsed number.
4. Builds an event `Filter` for `STAKING_ADDRESS` with `topic0 = POOL_CREATED_EVENT_SIGNATURE` (hardcoded keccak256 `0x45d43f0d...`) and `topic3 = owner`.
5. Calls `provider.get_logs(&filter)`.
6. For each log, extracts pool address from `topics[2]` (last 20 bytes of 32-byte topic).
7. If `show_voting_power` is true, calls `Staking::getPoolVotingPowerNow` for each pool via `eth_call`; on failure, shows `"N/A"`.

**State changes**: None (read-only).

### 6. `validator/join.rs` — JoinCommand

**Parameters**: `--rpc-url`, `--private-key`, `--gas-limit` (default 2,000,000), `--gas-price` (default 20), `--stake-pool`, `--moniker` (default `"Gravity1"`), `--consensus-public-key`, `--consensus-pop`, `--network-public-key`, `--validator-network-address`, `--fullnode-network-address`.

**Execution path**:
1. Decodes private key → signer → provider (same pattern as create).
2. Calls `Staking::isPool(stake_pool)` to validate the pool exists.
3. Calls `Staking::getPoolVotingPowerNow(stake_pool)` to display voting power.
4. Calls `ValidatorManagement::isValidator(stake_pool)` to check if already registered.
5. **If not registered**, validates inputs:
   - Moniker: ≤31 bytes
   - Consensus public key: exactly 96 hex chars (48 bytes, BLS12-381)
   - Network public key: exactly 64 hex chars (32 bytes)
   - Consensus PoP: exactly 192 hex chars (96 bytes, BLS signature)
   - Network addresses: must match `/ip4/{host}/tcp/{port}` format
   - Full addresses constructed by appending `/noise-ik/{network_pk}/handshake/0`
6. Sends `ValidatorManagement::registerValidator` transaction (BCS-encodes network/fullnode addresses). Waits for 2 confirmations, checks for `ValidatorRegistered` event.
7. Calls `ValidatorManagement::getValidator(stake_pool)`, decodes full `ValidatorRecord`, prints all fields.
8. **If status is `INACTIVE`**, sends `ValidatorManagement::joinValidatorSet(stake_pool)`. Waits for 2 confirmations, checks for `ValidatorJoinRequested` event.
9. Final status check: expects `PENDING_ACTIVE` or `ACTIVE`.

**State changes**: Registers a new validator record on-chain (if not already registered) and transitions it from `INACTIVE` to `PENDING_ACTIVE`.

### 7. `validator/leave.rs` — LeaveCommand

**Parameters**: `--rpc-url`, `--private-key`, `--gas-limit` (default 2,000,000), `--gas-price` (default 20), `--stake-pool`.

**Execution path**:
1. Decodes private key → signer → provider.
2. Calls `ValidatorManagement::isValidator(stake_pool)`; errors if not registered.
3. Calls `ValidatorManagement::getValidator(stake_pool)`, decodes `ValidatorRecord`.
4. Status checks:
   - `PENDING_ACTIVE` or `ACTIVE`: proceeds to leave.
   - `PENDING_INACTIVE`: prints "already leaving", returns `Ok(())`.
   - `INACTIVE`: prints "already left", returns `Ok(())`.
   - Any other: returns error.
5. Sends `ValidatorManagement::leaveValidatorSet(stake_pool)`. Waits for 2 confirmations, checks for `ValidatorLeaveRequested` event.
6. Final status check: expects `PENDING_INACTIVE` or `INACTIVE`.

**State changes**: Transitions the validator from `ACTIVE`/`PENDING_ACTIVE` to `PENDING_INACTIVE`.

### 8. `validator/list.rs` — ListCommand

**Parameters**: `--rpc-url` only.

**Execution path**:
1. Creates an unauthenticated provider (no wallet — read-only).
2. Makes 6 `eth_call` queries to `VALIDATOR_MANAGER_ADDRESS`:
   - `getCurrentEpoch()`
   - `getTotalVotingPower()`
   - `getActiveValidatorCount()`
   - `getActiveValidators()`
   - `getPendingActiveValidators()`
   - `getPendingInactiveValidators()`
3. Converts results to `SerializableValidatorSet` (with nested `SerializableValidatorInfo` entries).
4. `convert_validator_info()` helper: hex-encodes consensus pubkey, BCS-decodes network/fullnode addresses (falls back to hex on failure), formats voting power via `format_ether`.
5. Outputs the entire set as pretty-printed JSON to stdout.

**State changes**: None (read-only).

### 9. `dkg/randomness.rs` — RandomnessCommand

**Parameters**: `--server-url`, `--block-number`.

**Execution path**:
1. `normalize_url()`: trims trailing `/`, prepends `http://` if no scheme present.
2. Builds a `reqwest::Client` with `danger_accept_invalid_certs(true)` and `danger_accept_invalid_hostnames(true)`.
3. Sends GET to `{base_url}/dkg/randomness/{block_number}`.
4. On non-success HTTP status, attempts to parse an `ErrorResponse` JSON body.
5. On success, parses `RandomnessResponse` JSON, prints `block_number: randomness_hex`.

**State changes**: None (read-only HTTP query).

### 10. `dkg/status.rs` — StatusCommand

**Parameters**: `--server-url`.

**Execution path**:
1. `normalize_url()`: same logic as randomness.
2. Builds a `reqwest::Client` with `danger_accept_invalid_certs(true)` and `danger_accept_invalid_hostnames(true)`.
3. Sends GET to `{base_url}/dkg/status`.
4. On success, parses `DKGStatusResponse` JSON with fields: `epoch`, `round`, `block_number`, `participating_nodes`.
5. Prints each field.

**State changes**: None (read-only HTTP query).

### 11. `genesis/key.rs` — GenerateKey

**Parameters**: `--random-seed` (optional, 64 hex chars), `--output-file`.

**Execution path**:
1. `key_generator()`: If `--random-seed` provided, strips `0x`, hex-decodes to `[u8; 32]`, creates `KeyGen::from_seed(seed_slice)`. Otherwise creates `KeyGen::from_os_rng()`.
2. Generates three key types:
   - **X25519 network key**: `key_gen.generate_x25519_private_key()`
   - **BLS12-381 consensus key**: `key_gen.generate_bls12381_private_key()`
   - **Ed25519 account key**: `key_gen.generate_ed25519_private_key()`
3. Derives `account_address` from consensus public key via SHA3-256 (`tiny_keccak::Sha3::v256`). Also prints the last 20 bytes as an ETH-format address.
4. Creates `ProofOfPossession` from the consensus private key.
5. Serializes all keys/addresses as a `ValidatorIndentity` struct (note: typo in "Indentity") to YAML, writes to `--output-file`.

**State changes**: Writes a YAML file containing `account_address`, `account_private_key`, `consensus_private_key`, `network_private_key`, `consensus_public_key`, `consensus_pop`, `network_public_key`.

### 12. `genesis/account.rs` — GenerateAccount

**Parameters**: `--output-file`.

**Execution path**:
1. `generate_eth_account()`:
   - Generates a random secp256k1 signing key via `SigningKey::random(&mut OsRng)`.
   - Derives the uncompressed public key via `verifying_key().to_sec1_bytes()`.
   - Strips the `0x04` prefix byte.
   - Hashes the remaining 64 bytes with Keccak-256.
   - Takes the last 20 bytes of the hash as the Ethereum address (prefixed with `0x`).
2. Serializes `Account { private_key, public_key, address }` to YAML, writes to `--output-file`.

**State changes**: Writes a YAML file containing `private_key`, `public_key`, `address`.

### 13. `genesis/waypoint.rs` — GenerateWaypoint

**Parameters**: `--input-file` (JSON), `--output-file`.

**Execution path**:
1. `load_genesis_config()`: Reads JSON file, deserializes to `GenesisConfig { validators: Vec<ValidatorEntry> }`. Each `ValidatorEntry` has `consensusPubkey`, `networkAddresses`, `fullnodeAddresses`, `votingPower`.
2. `generate_validator_set()`: For each validator:
   - Strips `0x` from consensus pubkey, hex-decodes, constructs `bls12381::PublicKey`.
   - Derives `AccountAddress` from consensus pubkey bytes via SHA3-256 (same derivation as `genesis/key.rs`).
   - Parses `votingPower` as `u128` (wei), divides by `10^18` to convert to ether, casts to `u64`.
   - Creates `ValidatorConfig` with BCS-encoded network/fullnode addresses (each wrapped in a `Vec` before BCS encoding).
   - Creates `ValidatorInfo` with account address, voting power, config, and empty vec for extra data.
   - Collects into `ValidatorSet`.
3. `generate_waypoint()`: Creates `LedgerInfoWithSignatures::genesis()` with the accumulator placeholder hash and the validator set, then calls `Waypoint::new_epoch_boundary()`.
4. Writes the waypoint string to `--output-file`.

**State changes**: Writes a waypoint string to a file.

### 14. `node/start.rs` — StartCommand

**Parameters**: `--deploy-path`.

**Execution path**:
1. Constructs paths: `{deploy_path}/script/start.sh` and `{deploy_path}/script/node.pid`.
2. Checks that `start.sh` exists.
3. `check_pid_file()`: If PID file exists, reads the PID, runs `ps -p {pid}` to check if the process is alive. Returns error whether the process is running ("already running") or not ("zombie PID file").
4. Executes `bash start.sh` with `current_dir` set to `deploy_path`, using `status()` (waits for script exit, not output).
5. Sleeps 500ms, then checks if PID file was created and prints the PID.

**State changes**: Spawns a node process via shell script. Does not directly write PID file (script does).

### 15. `node/stop.rs` — StopCommand

**Parameters**: `--deploy-path`.

**Execution path**:
1. Constructs path `{deploy_path}/script/stop.sh`.
2. Checks that `stop.sh` exists.
3. Executes `bash stop.sh` with `current_dir` set to `deploy_path`, using `output()` (captures stdout/stderr).
4. On failure, prints stderr. On success, prints stdout if non-empty.

**State changes**: Stops a node process via shell script.

---

## External Dependencies

| Dependency | Usage |
|------------|-------|
| `clap` | CLI argument parsing with derive macros |
| `alloy_primitives`, `alloy_provider`, `alloy_rpc_types`, `alloy_signer`, `alloy_signer_local`, `alloy_sol_types`, `alloy_sol_macro` | Ethereum JSON-RPC provider, transaction construction, ABI encoding/decoding, signing |
| `k256` | secp256k1 key generation (ETH accounts) |
| `sha3` (Keccak256) | Ethereum address derivation |
| `tiny_keccak` (SHA3-256) | Validator account address derivation from consensus pubkey |
| `gaptos::aptos_crypto` | BLS12-381 keys, X25519 keys, Ed25519 keys, ProofOfPossession |
| `gaptos::aptos_keygen` | `KeyGen` (seeded or OS-RNG based key generation) |
| `gaptos::aptos_types` | `AccountAddress`, `ValidatorSet`, `ValidatorConfig`, `ValidatorInfo`, `Waypoint`, `LedgerInfoWithSignatures` |
| `bcs` | Binary Canonical Serialization for network addresses |
| `reqwest` | HTTP client for DKG API queries |
| `tokio` | Async runtime (created per command via `Runtime::new()`) |
| `serde`, `serde_yaml`, `serde_json` | Serialization/deserialization for YAML/JSON output |
| `hex` | Hex encoding/decoding throughout |
| `build_info` | Build metadata for `--version` |
| `anyhow` | Error handling |
| `rand_core::OsRng` | Cryptographic RNG for account generation |

---

## Access Control Summary

| Command | Authentication | On-chain Writes |
|---------|---------------|-----------------|
| `stake create` | Private key required (signs tx) | Creates StakePool, transfers ETH |
| `stake get` | None (read-only) | None |
| `validator join` | Private key required (signs tx) | Registers validator, joins set |
| `validator leave` | Private key required (signs tx) | Leaves validator set |
| `validator list` | None (read-only) | None |
| `dkg randomness` | None | None (HTTP GET) |
| `dkg status` | None | None (HTTP GET) |
| `genesis generate-key` | None | None (writes local file) |
| `genesis generate-account` | None | None (writes local file) |
| `genesis generate-waypoint` | None | None (writes local file) |
| `node start` | Filesystem access | Executes shell script |
| `node stop` | Filesystem access | Executes shell script |

All on-chain authorization is delegated to the smart contracts (`ValidatorManagement` and `Staking`) which enforce their own access control based on the transaction sender's relationship to the stake pool roles. The CLI itself does not enforce any role-based access beyond requiring the private key to sign transactions.

---

## Notable Implementation Details

1. **Private key handling**: All commands accepting `--private-key` strip the `0x` prefix, hex-decode to raw bytes, construct a `SigningKey`, then a `PrivateKeySigner`. The private key is passed as a plaintext CLI argument (visible in process listings).

2. **DKG HTTP clients**: Both `randomness.rs` and `status.rs` build `reqwest::Client` with `danger_accept_invalid_certs(true)` and `danger_accept_invalid_hostnames(true)`, disabling TLS certificate validation entirely.

3. **Timestamp unit conversion**: `stake/create.rs` computes `locked_until = (current_timestamp + lockup_duration) * 1_000_000` converting from seconds to microseconds. The contract's `lockedUntil` parameter is a `u64`, so the maximum representable value is ~18.4 × 10^18 microseconds (~584,942 years from epoch).

4. **Voting power conversion**: `genesis/waypoint.rs` parses voting power as `u128` (wei) and performs integer division by `10^18`, then casts to `u64`. This truncates fractional ether amounts.

5. **Address derivation**: Two distinct derivation schemes exist:
   - **Validator account address**: SHA3-256 hash of BLS12-381 consensus public key bytes (used in `genesis/key.rs` and `genesis/waypoint.rs`)
   - **Ethereum account address**: Keccak-256 hash of uncompressed secp256k1 public key, last 20 bytes (used in `genesis/account.rs`)

6. **Node start/stop**: These commands delegate entirely to external shell scripts (`start.sh`/`stop.sh`). The `deploy_path` is user-provided and used directly in `PathBuf::from()` and as `current_dir` for subprocess execution.

7. **Transaction confirmation**: All write operations wait for 2 block confirmations with a 60-second timeout before fetching receipts and parsing events.

8. **`format_ether` edge case**: When the decimal portion is all zeros after trimming, the `trim_end_matches('.')` call removes the trailing dot, producing output like `"1"` instead of `"1.0"` or `"1."`.

## Step Logs

| Step | Action | Input | Output | Duration |
|------|--------|-------|--------|----------|
| 1 | llm_query | Entry File: ./bin/gravity_node/src/main.rs
User Intent: Comp | # Implementation Analysis: `gravity_cli` Binary

## Files/Co | 195208ms |
