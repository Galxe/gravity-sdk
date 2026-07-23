---
name: gravity-oracle-demo
description: Use when the user asks how to run, demo, debug, or restart Gravity oracle tests, especially Binance index-kline price feeds, Polymarket settlement mirrors, or the combined frontend dashboard.
---

# Gravity Oracle Demo

Use this skill for Gravity oracle demo and integration-test operations that span `gravity-sdk`,
`gravity-reth`, `gravity_chain_core_contracts`, and the demo web app.

## Safety

- Ask before outbound public API/RPC requests unless the user has already
  approved that exact live run.
- Do not commit real RPC URLs, API keys, `.env` files, or absolute user-specific
  paths in docs.
- Prefer deterministic mock E2E for CI and review. Use live mode only for demos
  or explicitly approved validation.
- Do not stop a cluster unless the user asks or the running process was started
  by the current task and is clearly part of cleanup.

## Binance Price Feed Demo

Default deterministic suite:

```bash
cd gravity-sdk
PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh binance_price_feed --force-init
```

Long-running frontend backend with the local mock:

```bash
cd gravity-sdk
PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh binance_price_feed \
    --force-init \
    --keep-running \
    --demo-config-out ../gravity_price_feed_demo_web/public/demo-config.json \
    --log-cli-level=INFO
```

Live Binance market-data demo:

```bash
cd gravity-sdk
BINANCE_PRICE_FEED_MODE=live \
BINANCE_PRICE_FEED_BASE_URL=https://testnet.binancefuture.com \
BINANCE_PRICE_FEED_LAG_MINUTES=3 \
PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh binance_price_feed \
    --keep-running \
    --demo-config-out ../gravity_price_feed_demo_web/public/demo-config.json \
    --log-cli-level=INFO
```

Notes:

- Production `https://fapi.binance.com` may return HTTP `451` from restricted
  locations. For demos, `https://testnet.binancefuture.com` exposes the same
  `indexPriceKlines` shape and worked for TSLAUSDT/NVDAUSDT.
- `indexPriceKlines` is public market data; this implementation does not need
  `BINANCE_API_KEY` or `BINANCE_SECRET_KEY`.
- Live mode computes a recently closed 1-minute bucket, then continuous tasks
  advance by delivery nonce: nonce `n` maps to bucket `start + (n - 1) * 1m`.
- The default live task uses `graceMs=120000` and a three-minute lag so the
  first requested bucket is already beyond that grace period.
- A healthy demo shows `NativeOracle` nonce and resolver `roundId` increasing
  every closed minute.

Start or refresh the frontend:

```bash
cd gravity_price_feed_demo_web
npm run dev
```

Open the URL printed by the dev server. The page reads `public/demo-config.json`
and proxies `/rpc` to the Gravity devnet.

Stop the backend:

```bash
cd gravity-sdk
bash cluster/stop.sh --config gravity_e2e/cluster_test_cases/binance_price_feed/cluster.toml
```

If mock mode is still running, also stop the local mock server:

```bash
kill "$(cat gravity_e2e/cluster_test_cases/binance_price_feed/artifacts/mock_binance.pid)"
```

## Combined Price Feed + Polymarket Dashboard

Use `oracle_demo` when the user wants one frontend showing both Binance price
feeds and a Polymarket mirror in the same local Gravity cluster.

Run the deterministic local backend:

```bash
cd gravity-sdk
PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh oracle_demo \
    --force-init \
    --keep-running \
    --demo-config-out ../gravity_price_feed_demo_web/public/demo-config.json \
    --log-cli-level=INFO
```

Then start the dashboard:

```bash
cd gravity_price_feed_demo_web
npm run dev
```

This suite sends no public internet requests. It starts:

- a local Binance `indexPriceKlines` mock for `sourceType=3`
- a local Polygon JSON-RPC mock for a Fed-style binary CTF settlement
- one Gravity cluster that records both oracle lanes

Stop it with:

```bash
cd gravity-sdk
bash cluster/stop.sh --config gravity_e2e/cluster_test_cases/oracle_demo/cluster.toml
kill "$(cat gravity_e2e/cluster_test_cases/oracle_demo/artifacts/mock_binance.pid)"
```

## Polymarket Mirror Demo

Run the offline settlement rail first:

```bash
cd gravity-sdk
PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh polymarket_mock --force-init
```

What it proves:

- `sourceType=6` relayer reads a Polygon-like CTF `ConditionResolution` log.
- Unsupported-JWK consensus carries canonical settlement bytes to
  `NativeOracle`.
- `PolymarketSettlementResolver` validates and stores the payout vector.
- A Gravity market contract settles and releases claims from that resolver.

For real Polymarket mirrors, do not infer a market from a Polygon log alone.
Start from a reviewed manifest: slug/title/rules, `conditionId`, `questionId`,
CTF address, outcome labels, `slotToOutcome`, source block range, and hashes of
raw metadata snapshots. Then register exactly one `(sourceType=6, mirrorId)` per
reviewed CTF condition.
