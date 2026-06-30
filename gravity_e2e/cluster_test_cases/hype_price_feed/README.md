# Hype/HIP-3 Price Feed Oracle PoC

This local-only suite proves the Gravity oracle path for stock-like price feeds:

1. Governance deploys `MultiSourceOracleResolver`.
2. Governance sets the `NativeOracle` default callback for `sourceType=3`.
3. Governance registers two `OracleTaskConfig` tasks:
   - `feedId=1001`: Hype/HIP-3-style NVDA/USD round.
   - `feedId=1002`: Hype/HIP-3-style GOOGL/USD round.
4. The suite uses a 30-second epoch so `aptos-jwk-consensus` rebuilds its provider list after the governance update.
5. `gravity-reth` discovers `sourceType=3` as a relayer-backed task, adds the URI from local `relayer_config.json`, and publishes the canonical price bytes through the unsupported-JWK consensus path.
6. `NativeOracle` records the payloads and calls `MultiSourceOracleResolver`.
7. The resolver stores `latestPrice(feedId)` and `priceRounds(feedId, roundId)`.

The suite does not call a live Hyperliquid endpoint. The URI carries deterministic
multi-source observations so every validator produces byte-identical payloads.
Live Hype/HIP-3 fetching should be tested separately with a mock `/info` server
or run manually with validator-local relayer config.

This is intentionally an epoch-config test, not a dynamic request watcher test.
The current relayer-backed JWK path rebuilds observers from on-chain config at
epoch boundaries. Per-request discovery inside an epoch still needs a separate
deterministic watcher.

## Run

Build `gravity_node` and `gravity_cli`, then run from the repository root:

```bash
PATH="$CONDA_PREFIX/bin:$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh hype_price_feed --force-init
```

If you use a virtualenv instead of conda, activate it first and omit
`$CONDA_PREFIX/bin` from the `PATH` prefix:

```bash
source gravity_e2e/.venv/bin/activate
PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh hype_price_feed --force-init
```

## Expected Effect

Successful logs include:

```text
Hype price feed resolved: feedId=1001 roundId=1 price=19538000000
Hype price feed resolved: feedId=1002 roundId=1 price=35364400000
PASSED
Suite hype_price_feed PASSED
All suites passed!
```

The expected price math uses weighted mean aggregation:

```text
NVDA  = (19538000000 * 3 + 19540000000 + 19536000000) / 5 = 19538000000
GOOGL = (35364000000 * 3 + 35370000000 + 35360000000) / 5 = 35364400000
```

All prices use 8 decimals.

## Extending Toward a Polymarket-Like Gravity Product

For Polymarket-style sports markets, keep the current split:

- `sourceType=6`: mirror finalized Polygon CTF settlements into `PolymarketSettlementResolver`.
- Market contracts consume resolver state and settle YES/NO or multi-outcome markets.

For price-index markets or perps, use this suite as the base:

- `sourceType=3`: feed equity/crypto/index price rounds into `MultiSourceOracleResolver`.
- The downstream market or PerpDex contract decides how to use `latestPrice(feedId)`.
- The relayer adapter should only fetch and canonicalize source observations. Product-level BBO, mid-price, TWAP, risk, and weighting policy should live in the consuming contract or in a separately versioned resolver policy.

The production version should replace static observations with a deterministic
source adapter policy: local provider allowlist, request/round schedule,
source timestamps, deadline/expiry, and payload hashes bound into the signed
oracle bytes.
