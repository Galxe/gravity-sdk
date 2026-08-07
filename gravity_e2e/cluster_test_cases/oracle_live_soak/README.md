# Wave 7 Manual Live Oracle Soak

This manual, non-gating suite runs four equal-power Gravity validators against:

- three continuous Binance `NVDAUSDT`, `BTCUSDT`, and `ETHUSDT` closed
  index-price kline feeds (`sourceType=3`, feed IDs `1001` through `1003`);
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

- all four Gravity RPC replicas expose the same block hash at the common
  confirmed height `min(latest node heights) - 16`;
- each replica returns the same NativeOracle progress and resolver state at
  that exact EIP-1898 canonical block hash, outside number-to-state view
  transitions near the execution head;
- a transient cross-replica execution-view lag retries the entire canonical
  snapshot for up to five seconds; a state difference that persists across the
  bounded retry window still fails the soak;
- each price snapshot uses a progress/resolver/progress seqlock read so an RPC
  view transition between cross-contract calls is retried, not mistaken for
  inconsistent Oracle state;
- every Binance feed's delivery nonce, source position, and resolver round never
  regress independently;
- every latest resolver value maps to that pair's exact closed one-minute
  bucket;
- all three Binance feeds and every Gravity node stay within the configured
  stall budget;
- at least three relayers have checkpointed each committed Binance nonce;
- the finalized Polymarket settlement remains immutable at nonce `1`.
- the governance-pending two-hour epoch interval is active before timing starts.

At completion, the suite fetches each exact final Binance bucket again and
compares its close with the corresponding onchain value. It also counts
NativeOracle callback successes: each pair's count must equal its independent
final delivery nonce, while the Polymarket settlement must have exactly one
successful delivery. Heartbeats and the final summary report observed price
changes per pair; price changes are informational because timely identical
index closes are valid Oracle updates.

For runs of at least one hour, `node4` becomes eligible to restart halfway
through by default. A focused regression can configure multiple restart times.
The runner defers each restart while the chain is within five minutes of an
epoch boundary, then requires the node's chain and relayer checkpoints to catch
up without nonce regression or a duplicate Polymarket delivery. This keeps the
Oracle restart check independent from a simultaneous epoch reconfiguration.

## Prerequisites

- Current quick-release `gravity_node` and `gravity_cli` binaries.
- Foundry `forge` for the resolver artifacts.
- Approved outbound access to Binance Futures testnet, Polymarket Gamma, and
  Polygon.
- `POLYGON_RPC_URL` or `POLYGON_QUICKNONE_HTTP_URL` set to an HTTPS Polygon RPC.

The Binance Futures testnet `indexPriceKlines` endpoint used here is public and
does not require an API key. Each validator independently fetches all three
pairs. This exercises multiple live HTTP tasks and the complete validator
consensus path, but the values are testnet index data and must be labeled that
way in a demo. Do not put RPC credentials in committed files. The generated
relayer mapping and source metadata live under the ignored suite `artifacts/`
directory and are removed during normal teardown.

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

## Focused Multi-Restart Regression

Use this after changing validator startup, persistence, JWK consensus, or
relayer recovery. It restarts `node4` three times while the same live Binance
and Polygon tasks continue running:

```bash
ORACLE_SOAK_DURATION_SECONDS=1200 \
ORACLE_SOAK_POLL_SECONDS=15 \
ORACLE_SOAK_STALL_TIMEOUT_SECONDS=360 \
ORACLE_SOAK_MIN_ADVANCES=15 \
ORACLE_SOAK_RESTART_SCHEDULE_SECONDS=360,720,960 \
  ./gravity_e2e/run_test.sh \
    oracle_live_soak \
    --force-init \
    --log-cli-level=INFO
```

The schedule is measured from the start of the monitored soak, after task
activation and initial quorum. A scheduled restart can run later than its
threshold when the epoch guard is closed. The summary must contain three
entries in `restartRecoveries`, and the final snapshot must still show all four
replicas and every relayer checkpoint caught up.

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

With the defaults, `node4` becomes eligible to restart after 12 hours and runs
as soon as it is outside the five-minute epoch-transition guard. The pytest
process is the monitor, performs final assertions, writes the report, and tears
down the cluster. Do not terminate it after merely seeing the first onchain
update. A long-lived node can spend more than the cluster harness's usual 30
seconds reopening its persisted databases, so this suite gives restart RPC
recovery a three-minute window while still failing immediately if the process
exits.

## Controls

| Variable | Default | Meaning |
| --- | ---: | --- |
| `ORACLE_SOAK_DURATION_SECONDS` | `86400` | Monitored soak duration |
| `ORACLE_SOAK_POLL_SECONDS` | `15` | Onchain heartbeat interval |
| `ORACLE_SOAK_STALL_TIMEOUT_SECONDS` | `360` | Maximum feed or chain stall |
| `ORACLE_SOAK_MIN_ADVANCES` | 80% of expected minutes | Required nonce advances for each Binance feed |
| `ORACLE_SOAK_RESTART_AFTER_SECONDS` | Halfway for runs >= 1 hour | Earliest restart time; `0` disables |
| `ORACLE_SOAK_RESTART_SCHEDULE_SECONDS` | unset | Strictly increasing comma-separated restart times; mutually exclusive with `ORACLE_SOAK_RESTART_AFTER_SECONDS` |
| `ORACLE_SOAK_RESTART_NODE` | `node4` | Validator selected for restart |
| `ORACLE_SOAK_RESTART_RPC_TIMEOUT_SECONDS` | `180` | RPC recovery window for the restarted validator |
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
