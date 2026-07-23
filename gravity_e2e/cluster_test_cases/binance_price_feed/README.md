# Binance Index-Kline Price Feed Integration Test

This suite proves the Gravity oracle path for stock-like price feeds using
Binance USD-M `indexPriceKlines` semantics:

1. Governance deploys `PriceFeedResolver`.
2. Governance sets the `NativeOracle` default callback for `sourceType=3`.
3. Governance registers two `OracleTaskConfig` tasks:
   - `feedId=1001`: `provider=binance_index_kline_v1&pair=NVDAUSDT`.
   - `feedId=1002`: `provider=binance_index_kline_v1&pair=TSLAUSDT`.
4. Each task is a continuous 1-minute feed anchored at the same first bucket:
   - `bucketStartMs=1783252500000` (`2026-07-05T11:55:00Z`).
   - The `binance_index_kline_v1` provider is continuous by definition, so
     delivery nonce `1` maps to that bucket.
   - Delivery nonce `3` maps to `roundId=29720877` and
     `resolvedAt=1783252679999`.
5. The suite uses a 30-second epoch so `aptos-jwk-consensus` rebuilds its
   provider list after the governance update.
6. The pytest process runs a local mock Binance server for
   `/fapi/v1/indexPriceKlines`. The mock validates `pair`, `interval`,
   `startTime`, `endTime`, and `limit`, then returns deterministic kline rows.
7. `gravity-reth` discovers `sourceType=3` as a relayer-backed task, adds the
   URI from local `relayer_config.json`, fetches the mock Binance kline through
   the `provider=binance_index_kline_v1` adapter, and publishes canonical price
   bytes through the unsupported-JWK consensus path.
8. `NativeOracle` records the payloads and calls `PriceFeedResolver`.
9. The test waits until `NativeOracle` reaches delivery nonce `3` for each
   feed, then checks `priceRounds(feedId, 29720877)`.

By default the suite does not call live Binance. It exercises the same
HTTP/JSON adapter path against a local deterministic mock so every validator
produces byte-identical payloads. For demos, set `BINANCE_PRICE_FEED_MODE=live`
to point the same tasks at Binance public market-data APIs.

This is intentionally an epoch-config test, not a dynamic request watcher test.
The current relayer-backed JWK path rebuilds observers from on-chain config at
epoch boundaries. Per-request discovery inside an epoch still needs a separate
deterministic watcher.

## Run

Build `gravity_node` and `gravity_cli`, then run from the repository root:

```bash
PATH="$CONDA_PREFIX/bin:$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh binance_price_feed --force-init
```

If you use a virtualenv instead of conda, activate it first and omit
`$CONDA_PREFIX/bin` from the `PATH` prefix:

```bash
source gravity_e2e/.venv/bin/activate
PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh binance_price_feed --force-init
```

## Run As A Live Demo Backend

For a frontend demo, run the same suite in keep-running mode and write a runtime
config JSON into the frontend repository:

```bash
PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh binance_price_feed \
    --force-init \
    --keep-running \
    --demo-config-out ../gravity_price_feed_demo_web/public/demo-config.json \
    --log-cli-level=INFO
```

In this mode the runner:

- Starts the Gravity cluster.
- Starts a long-running local Binance index-kline mock on port `18547`.
- Runs the e2e assertions.
- Leaves the cluster and mock running after success.
- Writes `public/demo-config.json` for the web app.

## Run With Live Binance Market Data

For a pure demo, use live mode. This sends outbound requests to Binance public
market-data APIs. `indexPriceKlines` does not require `BINANCE_API_KEY` or
`BINANCE_SECRET_KEY`.

```bash
BINANCE_PRICE_FEED_MODE=live \
BINANCE_PRICE_FEED_LAG_MINUTES=3 \
PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh binance_price_feed \
    --force-init \
    --keep-running \
    --demo-config-out ../gravity_price_feed_demo_web/public/demo-config.json \
    --log-cli-level=INFO
```

Live mode defaults to a 1-minute bucket that is at least two minutes past close
(`graceMs=120000`), generates a run-local relayer config under `artifacts/`,
registers matching task URIs, and uses:

```text
GET https://fapi.binance.com/fapi/v1/indexPriceKlines
  ?pair=<NVDAUSDT|TSLAUSDT>
  &interval=1m
  &startTime=<closed bucket start>
  &endTime=<closed bucket end>
  &limit=1
```

The live run still writes through the full Gravity path:
relayer fetch -> canonical bytes -> unsupported-JWK consensus -> `NativeOracle`
-> `PriceFeedResolver`.

If the machine is blocked by Binance production Futures API with HTTP `451`,
use the official Futures testnet market-data host for demos:

```bash
BINANCE_PRICE_FEED_MODE=live \
BINANCE_PRICE_FEED_BASE_URL=https://testnet.binancefuture.com \
BINANCE_PRICE_FEED_LAG_MINUTES=3 \
PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh binance_price_feed \
    --keep-running \
    --demo-config-out ../gravity_price_feed_demo_web/public/demo-config.json \
    --log-cli-level=INFO
```

The testnet host is still live Binance public market data, not the deterministic
local mock. A successful long-running demo should show `NativeOracle` delivery
nonces increasing as each closed 1-minute bucket is written on-chain.

Start the web dashboard in the frontend repository:

```bash
npm run dev
```

Then open the local URL printed by the dev server. The dashboard reads
`public/demo-config.json`, proxies `/rpc` to the live Gravity devnet, and should
show `provider.kind = live-binance`.

Stop the demo backend with:

```bash
bash cluster/stop.sh --config gravity_e2e/cluster_test_cases/binance_price_feed/cluster.toml
kill "$(cat gravity_e2e/cluster_test_cases/binance_price_feed/artifacts/mock_binance.pid)"
```

The `kill` command is only needed for mock mode. Live mode does not start the
local mock Binance server.

## Expected Effect

Successful logs include:

```text
Binance price feed resolved: feedId=1001 deliveryNonce=3 roundId=29720877 price=19612645000
Binance price feed resolved: feedId=1002 deliveryNonce=3 roundId=29720877 price=40117545000
PASSED
Suite binance_price_feed PASSED
All suites passed!
```

The expected price is the mock Binance kline close field scaled to 8 decimals:

```text
NVDAUSDT nonce 3 = parse_fixed_decimal("196.12645000", 8) = 19612645000
TSLAUSDT nonce 3 = parse_fixed_decimal("401.17545000", 8) = 40117545000
```

All prices use 8 decimals.

Note that `NativeOracle` delivery nonces are sequential (`1`, `2`, `3`, ...),
while resolver payload `roundId` values are time-derived Binance bucket IDs.
The relayer keeps those two concepts separate so long-running feeds can satisfy
`NativeOracle.latestNonce + 1` while still storing price rounds by market time.

The bucket origin and interval are immutable for a `feedId`. Changing either
requires a new `feedId`; the relayer rejects history that does not match the
configured nonce-to-bucket sequence. The legacy `continuous` URI parameter is
rejected because one-shot Binance price tasks are not part of this product.

## Extending Toward a Polymarket-Like Gravity Product

For Polymarket-style sports markets, keep the current split:

- `sourceType=6`: mirror finalized Polygon CTF settlements into
  `PolymarketSettlementResolver`.
- Market contracts consume resolver state and settle YES/NO or multi-outcome
  markets.

For price-index markets or perps, use this suite as the base:

- `sourceType=3`: feed equity/crypto/index price rounds into
  `PriceFeedResolver`.
- The downstream market or PerpDex contract decides how to use
  `latestPrice(feedId)`.
- The relayer adapter fetches and canonicalizes one Binance close. Product-level
  BBO, mid-price, TWAP, and risk policy should live in the consuming contract or
  in a separately versioned resolver policy.

The production version should replace the local mock with a deterministic
provider policy: local provider allowlist, closed-bucket request schedule,
source timestamps, deadline/expiry, and payload hashes bound into the signed
oracle bytes.
