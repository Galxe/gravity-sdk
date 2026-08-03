# Rolling Oracle Activation E2E

This suite proves the operational rollout sequence for new relayer-backed
Oracle source types without enabling them on partially upgraded validators:

1. Start four equal-power validators on an old `gravity_node` binary.
2. Process a sourceType `0` bridge event on the all-old cluster.
3. Upgrade one validator at a time. After each replacement, process another
   bridge event while old and new binaries coexist.
4. After all four validators run the new binary, use governance to add
   production-data Binance sourceType `3` tasks and one finalized
   Polygon/Polymarket sourceType `6` task.
5. Prove the tasks are visible immediately but their nonces remain zero in the
   proposal epoch.
6. Cross the epoch boundary, require three independent relayer checkpoints and
   three-validator JWK quorum evidence, then verify the exact Binance index
   prices and Polymarket settlement on all four RPC replicas.
7. Process one more bridge event and restart a validator which lagged the
   Polymarket fetch, proving it fast-forwards from the committed onchain nonce
   and persists the recovered source state.

Genesis deliberately contains only the historical sourceType `0` task. The
sourceType `3` and `6` registrations come entirely from
`OracleTaskConfig.setTask` governance calls after every binary has been
replaced.

## Prerequisites

- An executable old binary built from the intended production baseline.
- An executable new binary built from the Oracle branch under test.
- Both binaries must be consensus-compatible with the fixed-size
  `OracleSourceState` used by the pinned genesis contracts. A transition from
  the legacy variable-size state requires its own version-gated hardfork test.
- `POLYGON_RPC_URL` or `POLYGON_QUICKNONE_HTTP_URL`.
- Network egress that can access Polygon, Polymarket Gamma, and Binance's
  official public-data archive. No Binance API key is required.

## Run

```bash
export GRAVITY_OLD_BINARY=/path/to/old/gravity_node
export GRAVITY_NEW_BINARY=/path/to/new/gravity_node
export POLYGON_RPC_URL='https://polygon-rpc.example'
export GRAVITY_E2E_SKIP_GLOBAL_PKILL=1
export ORACLE_ROLLING_BINANCE_MODE=official-archive

./gravity_e2e/run_test.sh oracle_rolling_activation_live --force-init
```

`GRAVITY_E2E_SKIP_GLOBAL_PKILL=1` prevents the runner from globally killing
unrelated `gravity_node` processes. The suite still stops its own four nodes
through its cluster configuration.

Optional controls:

```bash
export BINANCE_PRICE_FEED_ARCHIVE_DATE=2026-07-27
export BINANCE_PRICE_FEED_ARCHIVE_BASE_URL=https://data.binance.vision
export BINANCE_PRICE_FEED_GRACE_MS=120000
export POLYMARKET_GAMMA_URL=https://gamma-api.polymarket.com
```

`official-archive` downloads Binance's published daily `indexPriceKlines` ZIPs for
both pairs, records each archive SHA-256, selects one common closed minute,
and serves only those exact rows through a localhost protocol adapter. The
four validators still fetch and certify independently. Prices come from
Binance production data; the local adapter is not a generated-price mock.
When no date is pinned, the hook tries the previous seven UTC dates and uses
the newest date available for both pairs.

To exercise Binance's production REST endpoint directly instead:

```bash
export ORACLE_ROLLING_BINANCE_MODE=production-rest
export BINANCE_PRICE_FEED_BASE_URL=https://fapi.binance.com
export BINANCE_PRICE_FEED_BUCKET_START_MS=1785200000000
export BINANCE_PRICE_FEED_GRACE_MS=120000
```

The REST mode never falls back silently. It requires an egress region accepted
by Binance and fails on HTTP 451 otherwise.

The generated relayer configuration and selected live market metadata are
written under the ignored suite `artifacts/` directory and removed at teardown.
Do not commit RPC URLs, credentials, proxy settings, or generated metadata.
