# Deterministic Binance Multi-Validator E2E

This merge-gating suite proves the complete price-feed path without public
network access:

1. A four-validator Gravity cluster starts with equal voting power.
2. The test deploys `PriceFeedResolver` and uses governance to register two
   `sourceType=3` tasks plus the resolver callback.
3. Every validator observes the tasks after the next epoch and independently
   fetches exact closed 1-minute index-price buckets from a local Binance
   `indexPriceKlines` fixture.
4. At least three validators certify byte-identical payloads and form a JWK
   voting-power quorum.
5. The execution layer records source progress in `NativeOracle` and invokes
   `PriceFeedResolver`.
6. The test checks each validator's relayer state, quorum evidence, resolver
   values, and identical state from all four RPC endpoints at one block.

Run from the SDK repository root after building `gravity_node` and
`gravity_cli`:

```bash
PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh \
    binance_price_feed_multivalidator \
    --force-init \
    --log-cli-level=INFO
```

No Binance credentials are used. The test only binds and calls localhost. Live
Binance coverage belongs in a separate manual, non-gating suite.
