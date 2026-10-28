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
5. Oracle QC signature verification is active from genesis. The test checks
   that certified updates pass the SDK execution path after task registration.
6. The execution layer records source progress in `NativeOracle` and invokes
   `PriceFeedResolver`.
7. The test checks each validator's relayer state, quorum evidence, resolver
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

## Invalid Oracle QC from a proposer

The opt-in `test_z_invalid_oracle_qc.py` uses the `byzantine-test` node feature.
It registers a genuine price feed and a callback for an otherwise unregistered
source, then has node1 include an empty-signature Oracle update for that source
in a proposal that also contains a genuine Oracle update and a user transfer.
The test identifies the committed consensus block, checks that all four nodes
discard the invalid update, and compares their block hash and state roots at
the exact height. It also checks that the genuine update and transfer succeed
while the bad source has no system transaction, receipt, event, or nonce change.
It skips candidate blocks where an already delivered genuine QC is proposed
again and its Oracle transaction reverts with a stale nonce.
Removing the SDK signature guard makes the bad source record succeed and fails
the test.

To run this check locally, build the node with the test feature and opt in:

```bash
RUSTFLAGS="--cfg tokio_unstable" cargo build --bin gravity_node \
  --features byzantine-test --profile quick-release
RUSTFLAGS="--cfg tokio_unstable" cargo build --bin gravity_cli \
  --profile quick-release
GRAVITY_ORACLE_BAD_QC_E2E=1 \
PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh \
    binance_price_feed_multivalidator \
    --force-init \
    --log-cli-level=INFO
```

Without `GRAVITY_ORACLE_BAD_QC_E2E=1`, the negative test is skipped and the
ordinary price-feed test remains runnable with a production binary. If the
variable is set but the node lacks `byzantine-test`, the test fails with a
message explaining the required build feature.
