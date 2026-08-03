# Four-Validator Live Oracle Soak

This manual public-network suite runs four equal-power validators against a
continuous Binance NVDA index-price feed and one real finalized Polymarket
settlement. Neither task exists at genesis. One Gravity governance proposal:

1. registers the NVDA `sourceType=3` task and its resolver callback;
2. registers a `sourceType=6` Polymarket mirror and callback;
3. registers the corresponding Gravity settlement mirror.

Observers must remain absent in proposal epoch `E`. They start in `E+1`, and
at least three validators must certify both sources and form JWK quorum. The
test then verifies the resolver settlement and enters the configured soak
period. Binary-market accounting remains covered by the focused Polymarket
suite.

The default run lasts 24 hours. Every heartbeat checks:

- all four Gravity RPC replicas are connected and agree at one block;
- NVDA source progress is monotonic and matches `latestPrice`;
- the feed has not stalled for more than six minutes;
- at least three relayer state files reached the committed NVDA nonce;
- the Polymarket progress and settlement remain immutable;
- all four Gravity block heights continue advancing.

For runs of at least one hour, `node4` restarts halfway through by default and
must catch up. At completion, the final onchain NVDA price is compared with the
exact Binance `indexPriceKlines` close, and the Polymarket source must have
exactly one successful delivery.

## Prerequisites

- A current `gravity_node` and `gravity_cli` quick-release build.
- Approved outbound access to `https://fapi.binance.com`, the Polymarket Gamma
  API, and the configured Polygon RPC.
- `POLYGON_RPC_URL` or `POLYGON_QUICKNONE_HTTP_URL`.
- An egress region eligible for Binance production futures data.

The public Binance endpoint does not require an API key. The suite rejects
local, private-address, and non-HTTPS Binance base URLs so it cannot silently
become a mock test.

## Run For One Day

Run this from a terminal multiplexer or another session that will not be
suspended with the interactive shell:

```bash
export POLYGON_RPC_URL="..."
export BINANCE_PRICE_FEED_BASE_URL=https://fapi.binance.com
export ORACLE_SOAK_DURATION_SECONDS=86400
export GRAVITY_E2E_SKIP_GLOBAL_PKILL=1
export PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH"

./gravity_e2e/run_test.sh \
  oracle_live_soak \
  --force-init \
  --log-cli-level=INFO
```

Do not add `--keep-running`: the pytest process itself is the 24-hour monitor
and performs the final assertions and cleanup.

## Short Validation

A three-minute development run can validate setup and at least one new minute:

```bash
ORACLE_SOAK_DURATION_SECONDS=180 \
ORACLE_SOAK_POLL_SECONDS=10 \
ORACLE_SOAK_STALL_TIMEOUT_SECONDS=180 \
ORACLE_SOAK_MIN_ADVANCES=1 \
ORACLE_SOAK_RESTART_AFTER_SECONDS=0 \
  ./gravity_e2e/run_test.sh \
    oracle_live_soak \
    --force-init \
    --log-cli-level=INFO
```

## Controls And Evidence

| Variable | Default | Meaning |
| --- | ---: | --- |
| `ORACLE_SOAK_DURATION_SECONDS` | `86400` | Monitored soak duration |
| `ORACLE_SOAK_POLL_SECONDS` | `15` | Onchain heartbeat interval |
| `ORACLE_SOAK_STALL_TIMEOUT_SECONDS` | `360` | Maximum price or chain stall |
| `ORACLE_SOAK_MIN_ADVANCES` | 80% of expected minutes | Required NVDA nonce advances |
| `ORACLE_SOAK_RESTART_AFTER_SECONDS` | Halfway for runs >= 1 hour | Restart time; `0` disables |
| `ORACLE_SOAK_RESTART_NODE` | `node4` | Validator selected for restart |

Runtime evidence is written to ignored files:

- `artifacts/oracle_live_soak_heartbeat.jsonl`
- `artifacts/oracle_live_soak_summary.json`

The generated relayer configuration can contain an RPC credential and is also
ignored. Normal teardown removes that configuration and source metadata while
retaining heartbeat and summary evidence.
