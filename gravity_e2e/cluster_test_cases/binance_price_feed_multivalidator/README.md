# Four-Validator Live Binance E2E

The public-network gate for four Binance observers, at least three equal-power
certifiers per feed, JWK voting-power quorum, onchain execution, and four-RPC
replication. A slower validator may fast-forward from the committed nonce. It
never accepts the local Binance mock and is excluded from the default suite
run.

Run:

```bash
BINANCE_PRICE_FEED_MODE=live \
BINANCE_PRICE_FEED_BASE_URL=https://testnet.binancefuture.com \
BINANCE_PRICE_FEED_LAG_MINUTES=4 \
PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh \
    binance_price_feed_multivalidator \
    --force-init \
    --log-cli-level=INFO
```

The suite supports `--keep-running --demo-config-out <path>` for the live
frontend. See [`../../docs/ORACLE_E2E.md`](../../docs/ORACLE_E2E.md) for
approval requirements, assertions, replay parameters, revisions, and the last
verified result.
