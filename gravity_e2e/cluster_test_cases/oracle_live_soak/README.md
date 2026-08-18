# Wave 7 Binance Price Feed Live Soak

This manual, non-gating suite runs four equal-power Gravity validators against
three continuous Binance Futures testnet closed index-price kline feeds:

- `NVDAUSDT` (`sourceType=3`, feed ID `1001`);
- `BTCUSDT` (`sourceType=3`, feed ID `1002`);
- `ETHUSDT` (`sourceType=3`, feed ID `1003`).

Before enabling those feeds, the suite exercises the Gamma testnet
hardfork. Genesis is generated from the packed-V1 contracts, then the
`NativeOracle` and `OracleTaskConfig` runtimes are replaced with the exact
pre-fork bytes signed into `genesis/testnet/genesis.json`. At the first block
whose timestamp is at least `config.gammaTime`, gravity-reth must atomically install both packed-V1
runtimes without changing either account's balance, nonce, or storage root.

Each callback uses packed Price Feed V1: a canonical 32-byte big-endian body
containing `version`, `feedId`, `roundId`, `resolvedAtMs`, fixed-8 `price`, and
zero `flags`. The resolver exposes the latest round as
`(uint32 roundId, uint48 resolvedAtMs, uint96 price)`.

The suite submits one Gravity governance proposal that registers the three
price tasks and the `PriceFeedResolver` callback. The same proposal changes the
epoch interval from the 60-second bootstrap value to two hours for `E+1`. It
proves that observers remain absent in proposal epoch `E`, start in `E+1`, and
reach a three-of-four JWK quorum before the soak timer begins.

This is the initial Binance-only launch gate tracked by gravity-audit #1093.
The suite does not enable source type 6, deploy or register a Polymarket
resolver, create a Polymarket task or callback, require Polygon credentials, or
write a `gravity://6/...` validator mapping. The merged deterministic
Polymarket transport suite remains separate regression coverage for a possible
future activation.

The short bootstrap epoch keeps dynamic task activation fast. The two-hour soak
epoch intentionally follows the repository's regular four-validator topology,
so a long Oracle soak does not also become a minute-by-minute reconfiguration
stress test.

## What Every Heartbeat Checks

- all four Gravity RPC replicas expose the same block hash at the common
  confirmed height `min(latest node heights) - 16`;
- each replica returns the same NativeOracle progress and PriceFeedResolver
  state at that exact EIP-1898 canonical block hash;
- bounded whole-snapshot retries tolerate a transient execution-view lag, while
  a state difference that persists across the retry window fails the soak;
- every price snapshot uses a progress/resolver/progress seqlock read;
- each feed's delivery nonce, source position, and resolver round never regress;
- each latest resolver value maps to the pair's exact closed one-minute bucket;
- all three feeds and every Gravity node stay within the stall budget;
- at least three relayers have checkpointed each committed Binance nonce;
- the governance-pending two-hour epoch interval is active before timing starts.

At completion, the suite fetches each exact final Binance bucket again and
compares its close with the corresponding onchain value. Each feed's
NativeOracle callback count must equal its independent final delivery nonce.
Observed price changes remain informational because timely identical index
closes are valid Oracle updates.

## Gamma Startup Gate

Before deploying the resolver or submitting governance, the test captures each
adjacent state while it is still inside the RPC proof window, waits until all
four validators pass the discovered activation block plus 16 confirmations,
and then confirms that both captured
block hashes remain canonical:

- the last block with `timestamp < gammaTime` has the frozen testnet pre-fork codehashes;
- the first block with `timestamp >= gammaTime` has the frozen packed-V1 codehashes;
- all four RPC replicas agree on both canonical block hashes and account
  proofs;
- `eth_getCode` and `eth_getProof.codeHash` agree for every observation;
- balance, nonce, and full `storageHash` are unchanged across activation.

The suite-local reth template enables a 64-block historical proof window for
this evidence capture. Production defaults and shared E2E templates remain
unchanged.

Only after this gate passes does the suite deploy `PriceFeedResolver`, register
the three Binance tasks, wait for JWK quorum, and begin the configured soak.
This ordering proves that the live price-feed path is running on contracts
installed by the hardfork rather than contracts already present at genesis.

For runs of at least one hour, `node4` becomes eligible to restart halfway
through by default. A focused run can configure multiple restart times. The
runner defers each restart while the chain is within five minutes of an epoch
boundary, then requires RPC, block height, and all three relayer checkpoints to
catch up without nonce regression.

## Prerequisites

- Current quick-release `gravity_node` and `gravity_cli` binaries.
- Foundry `forge` for the PriceFeedResolver artifact.
- Approved outbound access to Binance Futures testnet.

The Binance Futures testnet `indexPriceKlines` endpoint is public and does not
require an API key. Each validator independently fetches all three pairs. The
values are testnet index data and must be labeled that way in any demo. The
generated relayer mapping and source metadata live under the ignored suite
`artifacts/` directory and are removed during normal teardown.

The runner performs a global local `gravity_node` cleanup before and after the
suite. Do not run it beside another local Gravity cluster that must stay alive.

## Build

From the SDK repository root:

```bash
CARGO_BUILD_JOBS=2 make -j2 MODE=quick-release gravity_node gravity_cli
export PATH="$HOME/.foundry/bin:$PWD/target/quick-release:$PATH"
```

## Quick Burn-In

Use a short no-restart run after changing task activation, payload handling, or
the price resolver:

```bash
ORACLE_SOAK_DURATION_SECONDS=1800 \
ORACLE_SOAK_POLL_SECONDS=15 \
ORACLE_SOAK_STALL_TIMEOUT_SECONDS=900 \
ORACLE_SOAK_MIN_ADVANCES=24 \
ORACLE_SOAK_RESTART_AFTER_SECONDS=0 \
  ./gravity_e2e/run_test.sh \
    oracle_live_soak \
    --force-init \
    --log-cli-level=INFO
```

## Four-Hour Two-Restart Gate

This is the focused release-profile stability run. It schedules node4 restarts
one and three hours into the monitored four-hour period, away from the expected
two-hour epoch boundaries:

```bash
ORACLE_SOAK_DURATION_SECONDS=14400 \
ORACLE_SOAK_POLL_SECONDS=15 \
ORACLE_SOAK_STALL_TIMEOUT_SECONDS=900 \
ORACLE_SOAK_MIN_ADVANCES=192 \
ORACLE_SOAK_RESTART_SCHEDULE_SECONDS=3600,10800 \
  ./gravity_e2e/run_test.sh \
    oracle_live_soak \
    --force-init \
    --log-cli-level=INFO
```

The summary must contain two entries in `restartRecoveries`; the final snapshot
must still show all four replicas, every relayer checkpoint caught up, and every
price callback count equal to its source nonce.

## Two-Hour Packed V1 Gate

This focused compatibility run validates the packed contracts/reth/SDK stack
and restarts `node4` at the midpoint:

```bash
ORACLE_SOAK_DURATION_SECONDS=7200 \
ORACLE_SOAK_POLL_SECONDS=15 \
ORACLE_SOAK_STALL_TIMEOUT_SECONDS=900 \
ORACLE_SOAK_MIN_ADVANCES=96 \
ORACLE_SOAK_RESTART_AFTER_SECONDS=3600 \
  ./gravity_e2e/run_test.sh \
    oracle_live_soak \
    --force-init \
    --log-cli-level=INFO
```

The final summary must report `status: passed`, one restart recovery, at least
96 advances for every feed, four converged replicas, and callback counts equal
to final source nonces.

## Optional 24-Hour Soak

Run from a terminal multiplexer or another session that will stay alive:

```bash
ORACLE_SOAK_DURATION_SECONDS=86400 \
ORACLE_SOAK_POLL_SECONDS=15 \
ORACLE_SOAK_STALL_TIMEOUT_SECONDS=900 \
  ./gravity_e2e/run_test.sh \
    oracle_live_soak \
    --force-init \
    --log-cli-level=INFO
```

With the defaults, `node4` becomes eligible to restart after 12 hours and runs
as soon as the five-minute epoch-transition guard is open. The pytest process
is the monitor, performs final assertions, writes the report, and tears down the
cluster. A long-lived node can spend more than the harness's usual 30 seconds
reopening its persisted databases, so this suite gives restart RPC recovery a
three-minute window while still failing if the process exits.

## Controls

| Variable | Default | Meaning |
| --- | ---: | --- |
| `ORACLE_SOAK_DURATION_SECONDS` | `86400` | Monitored soak duration |
| `ORACLE_SOAK_POLL_SECONDS` | `15` | Onchain heartbeat interval |
| `ORACLE_SOAK_STALL_TIMEOUT_SECONDS` | `900` | Maximum feed or chain stall |
| `ORACLE_SOAK_MIN_ADVANCES` | 80% of expected minutes | Required nonce advances for each Binance feed |
| `ORACLE_SOAK_RESTART_AFTER_SECONDS` | Halfway for runs >= 1 hour | Earliest restart time; `0` disables |
| `ORACLE_SOAK_RESTART_SCHEDULE_SECONDS` | unset | Strictly increasing comma-separated restart times; mutually exclusive with `ORACLE_SOAK_RESTART_AFTER_SECONDS` |
| `ORACLE_SOAK_RESTART_NODE` | `node4` | Validator selected for restart |
| `ORACLE_SOAK_RESTART_RPC_TIMEOUT_SECONDS` | `180` | RPC recovery window for the restarted validator |
| `BINANCE_PRICE_FEED_BASE_URL` | `https://testnet.binancefuture.com` | Binance Futures testnet base URL |
| `BINANCE_PRICE_FEED_GRACE_MS` | `120000` | Closed-bucket safety delay |

## Evidence

Runtime evidence remains local and ignored by Git:

- `artifacts/oracle_live_soak_heartbeat.jsonl`: one bounded checkpoint per poll;
- `artifacts/oracle_live_soak_summary.json`: final PASS/FAIL report and restart
  recovery times.

The summary is the acceptance artifact. A process that is still running has not
yet passed the soak. Its `gammaHardfork` section records both canonical
blocks and the pre/post account proofs used by the startup gate.
