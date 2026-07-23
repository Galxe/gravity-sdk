# Combined Oracle Dashboard Demo

This suite runs both production-candidate oracle paths in one local Gravity cluster so the frontend can
show both lanes at once:

- `sourceType=3` Binance index-kline price feed with a deterministic local mock.
- `sourceType=6` Polymarket CTF settlement mirror with a deterministic local
  Polygon JSON-RPC mock.

No public Binance, Polygon, or Polymarket request is sent by this suite.

## What It Proves

1. Governance deploys/configures `PriceFeedResolver` for two price
   feeds: `NVDAUSDT` and `TSLAUSDT`.
2. Governance deploys/configures `PolymarketSettlementResolver` and a
   `PolymarketBinaryMarket` for a Fed-style YES/NO market.
3. The relayer fetches both mocked external sources and routes their canonical
   bytes through the unsupported-JWK oracle consensus path.
4. `NativeOracle` records both `sourceType=3` and `sourceType=6` payloads.
5. The price resolver stores index rounds and the binary market settles from the
   mirrored Polymarket payout vector.
6. The suite can write one `demo-config.json` that the web dashboard renders as
   both a price-feed lane and a Polymarket-mirror lane.

## Run

From the repository root:

```bash
PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh oracle_demo --force-init
```

## Run As A Frontend Backend

```bash
PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh oracle_demo \
    --force-init \
    --keep-running \
    --demo-config-out ../gravity_price_feed_demo_web/public/demo-config.json \
    --log-cli-level=INFO
```

Then start the dashboard:

```bash
cd ../gravity_price_feed_demo_web
npm run dev
```

Open the URL printed by the dev server. The dashboard uses `/rpc` to proxy to
the running Gravity devnet.

## Stop

```bash
bash cluster/stop.sh --config gravity_e2e/cluster_test_cases/oracle_demo/cluster.toml
kill "$(cat gravity_e2e/cluster_test_cases/oracle_demo/artifacts/mock_binance.pid)"
```

The mock Polygon server is owned by the e2e runner process; it exits when the
runner exits. That is fine for the dashboard because the Polymarket settlement
has already been written to `NativeOracle` before the suite passes. The Binance
mock stays alive in `--keep-running` mode so continuous price buckets can keep
advancing.

## Expected Logs

```text
Price feed resolved: feedId=1001 roundId=29720877 price=19612645000
Price feed resolved: feedId=1002 roundId=29720877 price=40117545000
Released mock binary Polymarket settlement: payout=[1, 0]
Combined oracle demo resolved: NVDA/TSLA roundId=29720877, polymarket marketId=1 YES claimable=300000000000000000000
PASSED
```

## Product Mapping

The Polymarket lane intentionally uses `PolymarketBinaryMarket` because this is
the cleanest contract shape for a Fed-rate YES/NO market or a single-match
"Team A wins?" mirror:

```text
Polymarket CTF slot 0 -> Gravity YES
Polymarket CTF slot 1 -> Gravity NO
```

Do not assume this slot order for real Polymarket markets. Production mirrors
must use a reviewed manifest with frozen labels, rules, CTF token ids, and
`slotToOutcome`.
