# Deterministic Polymarket Settlement Multi-Validator E2E

This merge-gating suite proves the complete `sourceType=6` transport path
without calling a public Polygon RPC at runtime:

1. A four-validator Gravity cluster starts with equal voting power.
2. Governance deploys the resolver binding and dynamically registers one
   Polygon CTF settlement task.
3. Every validator discovers the new task and independently scans a localhost
   Polygon JSON-RPC fixture.
4. The fixture exposes the settlement at `latest` while keeping the finalized
   watermark one block behind. The test first verifies an empty scan cursor and
   no on-chain settlement.
5. Advancing the finalized watermark releases byte-identical CTF observations.
   At least three validators certify them and form a JWK voting-power quorum.
6. The execution layer records terminal nonce `1` in `NativeOracle`, invokes
   `PolymarketSettlementResolver`, and stores the expected winner and Polygon
   provenance.
7. The test verifies relayer checkpoints and identical contract state through
   all four Gravity RPC endpoints at one block.

Run from the SDK repository root after building `gravity_node` and
`gravity_cli`:

```bash
PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh \
    polymarket_settlement_multivalidator \
    --force-init \
    --log-cli-level=INFO
```

The oracle data-source fixture binds and calls localhost only. Initial builds may
still download repository dependencies. The suite contains no Polygon
credentials and does not cover prediction-market betting, live Polygon, or
product settlement.
