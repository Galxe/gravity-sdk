# Wave 7 Manual Live Oracle Soak

This manual, non-gating suite runs four equal-power Gravity validators against:

- one continuous Binance `NVDAUSDT` closed index-price kline feed (`sourceType=3`);
- one recently closed binary Polymarket market whose CTF resolution is already
  finalized on Polygon (`sourceType=6`).

The suite discovers the live inputs before deployment, then submits one Gravity
governance proposal that registers both tasks and their resolver callbacks. The
same proposal changes the epoch interval from the 60-second bootstrap value to
two hours for `E+1`. It proves that observers remain absent in proposal epoch
`E`, start in `E+1`, and reach a three-of-four JWK quorum before the soak timer
begins.

The short bootstrap epoch keeps dynamic task activation fast. The two-hour soak
epoch intentionally follows the repository's regular four-validator topology:
a 24-hour Oracle soak should exercise roughly 12 epoch transitions, rather than
also acting as a 1,440-transition consensus reconfiguration stress test.

This is operational evidence, not merge-gating CI. The deterministic Binance
and Polymarket suites remain the reproducible CI coverage.

## What Every Heartbeat Checks

- all four Gravity RPC replicas can serve one common snapshot block;
- each replica returns the same NativeOracle progress and resolver state;
- Binance delivery nonce, source position, and resolver round never regress;
- the latest resolver value maps to the exact closed one-minute bucket;
- the Binance feed and every Gravity node stay within the configured stall
  budget;
- at least three relayers have checkpointed the committed Binance nonce;
- the finalized Polymarket settlement remains immutable at nonce `1`.
- the governance-pending two-hour epoch interval is active before timing starts.

At completion, the suite fetches the exact final Binance bucket again and
compares its close with the onchain value. It also counts NativeOracle callback
successes: the Binance count must equal its final delivery nonce, while the
Polymarket settlement must have exactly one successful delivery.

For runs of at least one hour, `node4` restarts halfway through by default. The
suite requires its chain and relayer checkpoints to catch up without nonce
regression or a duplicate Polymarket delivery.

## Prerequisites

- Current quick-release `gravity_node` and `gravity_cli` binaries.
- Foundry `forge` for the resolver artifacts.
- Approved outbound access to Binance Futures testnet, Polymarket Gamma, and
  Polygon.
- `POLYGON_RPC_URL` or `POLYGON_QUICKNONE_HTTP_URL` set to an HTTPS Polygon RPC.

The Binance Futures testnet `indexPriceKlines` endpoint used here is public and
does not require an API key. It exercises a live HTTP source and the complete
validator consensus path, but its values are testnet index data and must be
labeled that way in a demo. Do not put RPC credentials in committed files. The
generated relayer mapping and source metadata live under the ignored suite
`artifacts/` directory and are removed during normal teardown.

The runner performs a global local `gravity_node` cleanup before and after the
suite. Do not run this beside another local Gravity cluster you need to keep.

## Build

From the SDK repository root:

```bash
make MODE=quick-release gravity_node gravity_cli
export PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH"
```

Export the private Polygon RPC without printing it. For a standard shell-safe
environment file, either load it normally or extract only the required entry:

```bash
export POLYGON_QUICKNONE_HTTP_URL="$(
  sed -n 's/^POLYGON_QUICKNONE_HTTP_URL=//p' /path/to/private.env
)"
```

## Required Gate: 30-Minute Burn-In

Run this first. Validator restart is intentionally disabled for the burn-in;
restart recovery belongs to the 24-hour run.

```bash
ORACLE_SOAK_DURATION_SECONDS=1800 \
ORACLE_SOAK_POLL_SECONDS=15 \
ORACLE_SOAK_STALL_TIMEOUT_SECONDS=360 \
ORACLE_SOAK_MIN_ADVANCES=24 \
ORACLE_SOAK_RESTART_AFTER_SECONDS=0 \
  ./gravity_e2e/run_test.sh \
    oracle_live_soak \
    --force-init \
    --log-cli-level=INFO
```

Do not start the 24-hour run unless this command exits successfully and the
summary reports `status: passed`.

## 24-Hour Soak

Run from a terminal multiplexer or another session that will stay alive:

```bash
ORACLE_SOAK_DURATION_SECONDS=86400 \
ORACLE_SOAK_POLL_SECONDS=15 \
ORACLE_SOAK_STALL_TIMEOUT_SECONDS=360 \
  ./gravity_e2e/run_test.sh \
    oracle_live_soak \
    --force-init \
    --log-cli-level=INFO
```

With the defaults, `node4` restarts after 12 hours. The pytest process is the
monitor, performs final assertions, writes the report, and tears down the
cluster. Do not terminate it after merely seeing the first onchain update.

## Controls

| Variable | Default | Meaning |
| --- | ---: | --- |
| `ORACLE_SOAK_DURATION_SECONDS` | `86400` | Monitored soak duration |
| `ORACLE_SOAK_POLL_SECONDS` | `15` | Onchain heartbeat interval |
| `ORACLE_SOAK_STALL_TIMEOUT_SECONDS` | `360` | Maximum feed or chain stall |
| `ORACLE_SOAK_MIN_ADVANCES` | 80% of expected minutes | Required Binance nonce advances |
| `ORACLE_SOAK_RESTART_AFTER_SECONDS` | Halfway for runs >= 1 hour | Restart time; `0` disables |
| `ORACLE_SOAK_RESTART_NODE` | `node4` | Validator selected for restart |
| `BINANCE_PRICE_FEED_BASE_URL` | `https://testnet.binancefuture.com` | Binance Futures testnet base URL |
| `BINANCE_PRICE_FEED_GRACE_MS` | `120000` | Closed-bucket safety delay |
| `POLYMARKET_GAMMA_URL` | `https://gamma-api.polymarket.com` | Market discovery API |

## Evidence

Runtime evidence remains local and ignored by Git:

- `artifacts/oracle_live_soak_heartbeat.jsonl`: one bounded checkpoint per poll;
- `artifacts/oracle_live_soak_summary.json`: final PASS/FAIL report and restart
  recovery time.

The summary is the acceptance artifact. A process that is still running has
not yet passed the soak.
