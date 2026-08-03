# Four-Validator Live Polygon Polymarket E2E

This public-network suite discovers a recently resolved binary Polymarket from
the Gamma API, verifies its finalized CTF `ConditionResolution` log through a
configured Polygon RPC, and then starts four Gravity validators.

The test adds the mirror task through governance in epoch `E`. In `E+1`, all
validators start the dynamic observer. At least three equal-power validators
independently certify the same real Polygon log and form a JWK quorum; a slower
validator may instead fast-forward from the committed nonce. The test then
checks all four RPC replicas, the `NativeOracle` write, resolver callback, and
binary-market claim.

The Polygon RPC URL is written only to the ignored
`artifacts/relayer_config.live.json` file and is removed during normal cleanup.
This suite is excluded from default E2E runs.

Required environment:

```bash
export POLYGON_RPC_URL="..."
```

Run:

```bash
PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH" \
  ./gravity_e2e/run_test.sh \
    polymarket_live_dynamic_mirror \
    --force-init \
    --log-cli-level=INFO
```

Set `GRAVITY_E2E_SKIP_GLOBAL_PKILL=1` when an unrelated local Gravity node
must remain alive. The runner will still stop this suite's four-node cluster.
