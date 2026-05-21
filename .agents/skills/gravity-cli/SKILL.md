---
name: gravity-cli
description: Use when the user asks about `gravity_cli` command usage, validator/stake lifecycle commands, epoch or validator-set checks, DKG status, node start/stop wrappers, or wants safe command examples for Gravity private-mainnet operations.
---

# Gravity CLI

Use this skill when helping operate or explain `bin/gravity_cli` / `target/*/gravity_cli` in `gravity-sdk`.

## Safety

- Treat these as write-chain commands: `stake create`, `validator join`, `validator leave`, governance `cast send` commands that may be paired with CLI workflows.
- Treat these as local destructive commands: `unwind`, node `start`, node `stop`.
- Do not start/stop/deploy clusters unless the user explicitly asks.
- Do not put plaintext private keys in command lines or env vars. Current CLI signing flow intentionally uses stdin prompt by default, or `--kms`.
- Prefer `--output json` for scripts and `plain` for operator-facing status.
- When querying private-mainnet RPC from a restricted local environment, be ready to run with network approval or query from an internal host.

## Binary

Common paths:

```bash
./target/quick-release/gravity_cli
./target/release/gravity_cli
```

Build quick-release:

```bash
RUSTFLAGS="--cfg tokio_unstable" cargo build --profile quick-release -p gravity_cli
```

The top-level command is named `gravity-cli` in clap metadata, but repo operators often invoke the built binary as `gravity_cli`.

Before relying on a command, check the actual binary in use:

```bash
./target/quick-release/gravity_cli --help
./target/quick-release/gravity_cli <command> --help
```

The checked-in source may have newer wired commands than an older built binary under `target/`. Rebuild when source and binary help disagree.

## Global Options and Defaults

Global:

```bash
gravity_cli --profile <PROFILE> --output plain|json <command>
```

Environment/config fallbacks:

| Setting | CLI/env/config |
| --- | --- |
| profile | `--profile`, `GRAVITY_PROFILE`, `~/.gravity/config.toml active_profile` |
| RPC URL | `--rpc-url`, `GRAVITY_RPC_URL`, profile `rpc_url` |
| DKG/server URL | `--server-url`, `GRAVITY_SERVER_URL`, profile `server_url` |
| deploy path | `--deploy-path`, `GRAVITY_DEPLOY_PATH`, profile `deploy_path` |
| gas limit | `--gas-limit`, `GRAVITY_GAS_LIMIT`, profile `gas_limit` |
| gas price | `--gas-price`, `GRAVITY_GAS_PRICE`, profile `gas_price` |
| output | `--output`, `GRAVITY_OUTPUT` |

Initialize config:

```bash
gravity_cli init
gravity_cli init --non-interactive
```

`init` writes the local CLI profile config:

```text
~/.gravity/config.toml
```

Interactive `init` prompts for:

```text
Profile name
RPC URL
DKG server URL
Deploy path (optional)
```

Example config shape:

```toml
active_profile = "default"

[profiles.default]
rpc_url = "http://127.0.0.1:8545"
server_url = "127.0.0.1:1024"
deploy_path = "/path/to/node"
```

After config is initialized, commands can omit repeated flags:

```bash
gravity_cli validator list
gravity_cli epoch status
gravity_cli status
gravity_cli node start
```

`init --non-interactive` uses:

| Field | Default |
| --- | --- |
| profile | `default` |
| rpc_url | `http://127.0.0.1:8545` |
| server_url | `127.0.0.1:1024` |
| deploy_path | empty |

Use profiles for multiple environments:

```bash
gravity_cli --profile <PROFILE> validator list
GRAVITY_PROFILE=<PROFILE> gravity_cli epoch status
```

Resolution priority is:

```text
CLI flag > environment variable > selected ~/.gravity/config.toml profile
```

## Signing

On-chain commands flatten `SignerArgs`:

```bash
--kms projects/<P>/locations/<L>/keyRings/<R>/cryptoKeys/<K>/cryptoKeyVersions/<V>
```

If `--kms` is omitted, the CLI prompts:

```text
Enter private key (hex, with or without 0x prefix):
```

There is no `--private-key` flag in the current signer implementation. For non-interactive signing, use KMS.

## Command Tree

```text
genesis generate-key
genesis generate-waypoint
genesis generate-account

stake create
stake get

validator join
validator leave
validator list

epoch status
status
dkg status
dkg randomness

node start
node stop

doctor
init
completions
unwind
```

Availability notes:

- `doctor` is wired in current source (`command.rs` / `main.rs`). If `gravity_cli --help` does not show `doctor`, the binary is stale; rebuild `gravity_cli`.
- `bin/gravity_cli/src/qs_db.rs` contains a `find-batches` implementation draft, but it is not wired into the current top-level clap command in `command.rs/main.rs`. Current `gravity_cli --help` does not expose `qs-db`, and `gravity_cli qs-db --help` fails with `unrecognized subcommand`.

## Genesis Commands

Generate identity:

```bash
gravity_cli genesis generate-key \
  --output-file <identity.yaml>
```

Optional deterministic test seed:

```bash
gravity_cli genesis generate-key \
  --output-file <identity.yaml> \
  --random-seed <64_HEX_CHARS>
```

Generated identity includes:

```yaml
account_address: ...
account_private_key: ...
consensus_private_key: ...
network_private_key: ...
consensus_public_key: ...
consensus_pop: ...
network_public_key: ...
```

Generate waypoint:

```bash
gravity_cli genesis generate-waypoint \
  --input-file <validator_set.json> \
  --output-file <waypoint.txt>
```

Generate EVM account:

```bash
gravity_cli genesis generate-account \
  --output-file <account.yaml>
```

## Stake Commands

Create StakePool:

```bash
gravity_cli stake create \
  --rpc-url <RPC_URL> \
  --stake-amount <AMOUNT_IN_ETH> \
  --lockup-duration <SECONDS> \
  --gas-limit <GAS_LIMIT>
```

The signing key is prompted on stdin unless `--kms` is passed.

Important defaults:

| Option | Default |
| --- | --- |
| `--lockup-duration` | `2592000` seconds |
| `--gas-limit` | `2000000` |
| `--gas-price` | `100000000000` wei |

When the chain enforces a minimum lockup, avoid setting lockup exactly at the minimum. Use a buffer because the CLI computes `lockedUntil` from latest block timestamp and the transaction may execute in a later block.

Query pools by owner:

```bash
gravity_cli stake get \
  --rpc-url <RPC_URL> \
  --owner <EVM_OWNER_ADDRESS>
```

Useful range controls:

```bash
gravity_cli stake get \
  --rpc-url <RPC_URL> \
  --owner <EVM_OWNER_ADDRESS> \
  --from-block auto \
  --to-block latest \
  --show-voting-power true
```

`auto` scans back up to 90,000 blocks to avoid reth log-range limits.

## Validator Commands

Join:

```bash
gravity_cli validator join \
  --rpc-url <RPC_URL> \
  --stake-pool <STAKE_POOL_ADDRESS> \
  --consensus-public-key <BLS_PUBLIC_KEY_HEX> \
  --consensus-pop <BLS_POP_HEX> \
  --network-public-key <X25519_PUBLIC_KEY_HEX> \
  --validator-network-address /dns/<VALIDATOR_HOST>/tcp/6180 \
  --fullnode-network-address /dns/<VALIDATOR_HOST>/tcp/6190 \
  --moniker <NODE_ID> \
  --gas-limit <GAS_LIMIT>
```

Join validates:

- `stake_pool` is a valid StakePool.
- pool has voting power.
- validator registration exists or can be created.
- network addresses are `/ip4/.../tcp/...` or `/dns/.../tcp/...`.

The CLI expands network addresses to include:

```text
/noise-ik/<network_public_key>/handshake/0
```

Leave:

```bash
gravity_cli validator leave \
  --rpc-url <RPC_URL> \
  --stake-pool <STAKE_POOL_ADDRESS> \
  --gas-limit <GAS_LIMIT>
```

Leave state behavior:

| Current | After leave |
| --- | --- |
| `PENDING_ACTIVE` | `INACTIVE` |
| `ACTIVE` | `PENDING_INACTIVE`, then `INACTIVE` next epoch |
| `PENDING_INACTIVE` | no-op |
| `INACTIVE` | no-op |

List validators:

```bash
gravity_cli validator list --rpc-url <RPC_URL>
gravity_cli --output json validator list --rpc-url <RPC_URL>
```

Output includes:

- current epoch
- total voting power
- active validators
- pending active validators
- pending inactive validators

## Epoch and Status Commands

Epoch status:

```bash
gravity_cli epoch status --rpc-url <RPC_URL>
gravity_cli --output json epoch status --rpc-url <RPC_URL>
```

Reads:

- `Reconfiguration.currentEpoch()`
- `Reconfiguration.lastReconfigurationTime()`
- `EpochConfig.epochIntervalMicros()`
- latest block timestamp

Combined status:

```bash
gravity_cli status --rpc-url <RPC_URL>
gravity_cli status --server-url <SERVER_URL>
gravity_cli status --rpc-url <RPC_URL> --server-url <SERVER_URL>
gravity_cli --output json status --rpc-url <RPC_URL> --server-url <SERVER_URL>
```

`status` can combine epoch info, validator summary, and DKG info. At least one of `--rpc-url` or `--server-url` is required.

## DKG Commands

DKG status:

```bash
gravity_cli dkg status --server-url <SERVER_URL>
gravity_cli --output json dkg status --server-url <SERVER_URL>
```

The URL may omit `http://`; the CLI normalizes it.

Randomness by block:

```bash
gravity_cli dkg randomness \
  --server-url <SERVER_URL> \
  --block-number <BLOCK_NUMBER>
```

Endpoints used:

```text
GET /dkg/status
GET /dkg/randomness/<block_number>
```

## Node Lifecycle

Start:

```bash
gravity_cli node start --deploy-path <NODE_DEPLOY_DIR>
```

Stop:

```bash
gravity_cli node stop --deploy-path <NODE_DEPLOY_DIR>
```

These invoke:

```text
<deploy-path>/script/start.sh
<deploy-path>/script/stop.sh
```

`node start` checks `script/node.pid` before starting and errors on an already-running or zombie PID file.

## Doctor

`doctor` is available only in binaries built from source that includes the `DoctorCommand` top-level subcommand. Verify with:

```bash
gravity_cli doctor --help
```

Run diagnostics:

```bash
gravity_cli doctor \
  --rpc-url <RPC_URL> \
  --server-url <SERVER_URL> \
  --deploy-path <NODE_DEPLOY_DIR>
```

Skip TCP bind checks:

```bash
gravity_cli doctor --skip-ports ...
```

Use JSON:

```bash
gravity_cli --output json doctor ...
```

Doctor checks config, RPC/server connectivity, deployment path, scripts, logs, and port conflicts.

## Unwind

`unwind` is destructive. Use only when the user explicitly asks to modify local consensus DB state.

```bash
gravity_cli unwind \
  --consensus-db-path <NODE>/data/consensus_db \
  --target <BLOCK_NUMBER>
```

It deletes consensus data with `block_number > target`, cleans unused QuorumStore batches, and may remove `secure.json` and `rand_db` under the data dir.

## Completions

```bash
gravity_cli completions bash
gravity_cli completions zsh
gravity_cli completions fish
gravity_cli completions powershell
gravity_cli completions elvish
```

## Common Operator Recipes

Validator join verification:

```bash
gravity_cli validator list --rpc-url <RPC_URL>
gravity_cli epoch status --rpc-url <RPC_URL>
gravity_cli status --rpc-url <RPC_URL>
```

StakePool lookup:

```bash
gravity_cli stake get --rpc-url <RPC_URL> --owner <OWNER_ADDRESS>
```

Wait for leave:

```bash
gravity_cli validator leave --rpc-url <RPC_URL> --stake-pool <POOL>
gravity_cli validator list --rpc-url <RPC_URL>
gravity_cli epoch status --rpc-url <RPC_URL>
```

Local node health:

```bash
gravity_cli doctor --rpc-url <RPC_URL> --deploy-path <NODE_DEPLOY_DIR>
tail -n 200 <NODE_DEPLOY_DIR>/consensus_log/*.log
ls -lh <NODE_DEPLOY_DIR>/execution_logs
```

## Common Pitfalls

- Do not pass `--private-key`; current on-chain commands use stdin prompt or `--kms`.
- Use `--output json` globally before the command, for example `gravity_cli --output json validator list ...`.
- `stake get --from-block auto` only scans the latest 90,000 blocks.
- `validator join` requires StakePool operator signing, not just any funded EVM account.
- `validator join` requires the StakePool to be whitelisted through governance.
- `validator leave` does not unstake or withdraw funds; it only changes validator-set membership.
- Do not run `node start` / `node stop` unless the user explicitly asks.
- Do not use `unwind` unless the user explicitly asks for destructive DB surgery.
