# OracleV1 Rolling Binary Upgrade E2E

This manual suite proves the safe deployment order for the OracleV1 system
contract hardfork:

1. Start four equal-power validators on a pre-OracleV1 `gravity_node`.
2. Replace `node1` through `node4` one at a time while the chain remains below
   `oracleV1Block`.
3. After every replacement, require RPC recovery, block catch-up, continued
   block production, a common canonical checkpoint, and exact running-binary
   hashes for the mixed old/new cluster.
4. Require all four validators to run the new binary before activation.
5. At `oracleV1Block`, verify the exact NativeOracle and OracleTaskConfig
   pre/post codehashes on all replicas while balance, nonce, and storage root
   remain unchanged.
6. Restart one upgraded validator after activation and require it to replay and
   rejoin the same canonical chain.

The suite uses a 30-minute epoch and requires at least ten minutes of epoch
headroom before every stop/start. It also asserts the rollout, activation, and
post-fork restart all stay in one epoch. Epoch-transition restart recovery is a
separate safety property and must not obscure the OracleV1 rolling result.

An old binary must never remain in the validator set at activation: it does not
execute the OracleV1 migration and would diverge from upgraded validators.

## Prepare Binaries

Build the baseline from the SDK commit immediately before the OracleV1 reth pin
and build the candidate from this branch. Keep the resulting files at stable,
different paths. Limit Rust parallelism on memory-constrained hosts:

```bash
export CARGO_BUILD_JOBS=2
export CARGO_INCREMENTAL=0
export MALLOC_ARENA_MAX=2

RUSTFLAGS='--cfg tokio_unstable' \
  cargo build -j2 --profile quick-release -p gravity_node
```

The old baseline for this PR is SDK `90c78afdb5`, which pins gravity-reth
`4372a103a42a862593fa8221814b3bcc8d47d0aa`. The candidate pins gravity-reth
`d0f59d5b664ee68cf1816e77e91bf6457fca1ae9`.

## Run

```bash
export GRAVITY_OLD_BINARY=/stable/path/old/gravity_node
export GRAVITY_NEW_BINARY=/stable/path/new/gravity_node

./gravity_e2e/run_test.sh \
  oracle_v1_rolling_upgrade \
  --force-init \
  --log-cli-level=INFO
```

The default activation block is 1600. A shorter manual smoke may override it,
provided there is enough headroom to finish all four replacements:

```bash
ORACLE_V1_ROLLING_ACTIVATION_BLOCK=800 \
ORACLE_V1_ROLLING_MIN_BLOCKS_PER_NODE=128 \
  ./gravity_e2e/run_test.sh \
    oracle_v1_rolling_upgrade \
    --force-init \
    --log-cli-level=INFO
```

`ORACLE_V1_ROLLING_BLOCK_TIMEOUT_SECONDS` defaults to 900 seconds and bounds
each catch-up or activation wait. Increase it for slower CI workers rather than
lowering the activation block until the four replacements lose safe headroom.

The ignored `artifacts/oracle_v1_rolling_upgrade_summary.json` records every
binary replacement, canonical checkpoint, activation snapshot, storage guard,
and post-fork restart. The suite deploys an empty relayer mapping and makes no
external data-provider request.
