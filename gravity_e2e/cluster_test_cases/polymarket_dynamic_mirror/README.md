# Dynamic Polymarket Mirror E2E

This suite starts Gravity without any `sourceType=6` task in genesis, then
adds two Polymarket mirrors through governance while the chain remains live.

It verifies:

1. a task written during epoch `E` is stored on-chain but is not polled in `E`;
2. the JWK epoch manager creates its observer after the transition to `E+1`;
3. only a finalized Polygon `ConditionResolution` is submitted;
4. the payload reaches JWK consensus, `NativeOracle`, and the settlement resolver;
5. replaying the same Polygon log does not advance the oracle nonce;
6. a second mirror added later follows the same next-epoch lifecycle.

The two complete Gravity URIs are intentionally present in
`relayer_config.json`. This suite proves dynamic on-chain task discovery, not
dynamic RPC credential discovery. A production watcher should resolve a stable
local alias such as `polygon-mainnet` instead of requiring every condition URI
to be preconfigured.

Run locally:

```bash
./gravity_e2e/run_test.sh polymarket_dynamic_mirror --force-init
```

The shared runner currently performs a global `pkill -9 gravity_node` before
and after a suite. Run this command only when no unrelated local Gravity node
must remain alive.
