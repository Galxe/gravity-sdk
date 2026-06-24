# Gravity Chaos Toolkit

This is a small, no-K8s chaos toolkit for Gravity clusters. It is intentionally
simple: process-level failures first, with a cluster-level oracle and JSONL
reports. It does not deploy, re-init, or clean node data directories.

## Scope

Implemented in this first version:

- `majority-failure`: stop validator nodes whose stake is greater than one third
  of total validator stake, assert the remaining validators stall, then recover.
- `kill-under-load`: restart one node while an optional load command, managed
  `gravity_bench`, existing `gravity_bench`, and/or built-in tx workload is
  running.
- `flap`: repeatedly stop/start one node.
- `rolling-restart`: restart validators one by one.
- `partition-kill`: partition one node, restart another, heal, then recover.
- `partition-random`: randomly isolate one quorum-safe validator, assert the
  remaining validator stake keeps advancing, heal, and repeat.
- `partition-asym`: drop only inbound or outbound peer traffic for one
  validator, then heal and check convergence.
- `partition-no-quorum-split`: split all validators into two Docker network
  islands where neither side has quorum, then heal.
- `partition-majority-minority`: split validators into a majority side with
  quorum and a bounded minority side, then heal.
- `partition-2-2` and `partition-3-1`: backward-compatible aliases for the
  generic no-quorum and majority/minority split scenarios.
- `partition-load`: run managed load while a single-node, no-quorum, or
  majority/minority partition is active, then check recovery and tx receipts.
- `delay-load-spike`: inject network delay while managed load is running.
- `partition-mixed`: longer randomized mix of single-validator isolation,
  asymmetric partitions, no-quorum splits, and majority/minority splits.
- `soak`: long steady-state test that runs oracle repeatedly, with optional
  managed load.
- Managed `gravity_bench` background pressure via `CHAOS_BENCH_ENABLE=1`.
- Built-in EVM transfer sentinel workload via `CHAOS_TX_ENABLE=1`, with JSONL
  `tx_submit` / `tx_receipt` / `tx_timeout` / `tx_error` history and post-run
  receipt persistence checks.
- `oracle.sh`: process/RPC/height-spread/no-fork/state-root/advancing-stake
  health check, plus panic/fatal/abort log scanning.
- `snapshot.sh`: collect oracle output, network state, node metadata, process
  details, and log tails without healing the system.
- `loop.sh`: randomly runs scenarios and can pause after failures.
- Basic local-host network commands: `partition`, `delay`, `loss`, `throttle`,
  `partition-asym`, `heal`, and `net-status`.
- Docker backend via `CHAOS_BACKEND=docker` or `--backend docker` for
  container stop/start/restart, Docker network partition/heal, asymmetric
  container iptables partitions, and container `tc` injection when the
  container has the required Linux capabilities.

Topology selection is stake-aware and comes from the cluster config. The
single-validator partition scenarios only choose validators whose removal still
leaves >2/3 validator stake on the majority side. Split scenarios validate that
the chosen groups match the intended quorum condition before injecting a fault.
The provided Docker compose file is a 4-validator local example; larger
clusters should provide their own compose/config, while reusing the same
scenario logic.

Not implemented yet:

- clock skew.
- DB/WAL truncation.
- twin validator / Byzantine testing.

## Usage

Use an explicit config when you do not want the default `cluster/cluster.toml`:

```bash
cd cluster/chaos
./chaos.sh --config ../cluster.toml validators
./oracle.sh --config ../cluster.toml
```

Run scenarios:

```bash
./scenarios.sh --config ../cluster.toml majority-failure
./scenarios.sh --config ../cluster.toml kill-under-load node2
./scenarios.sh --config ../cluster.toml flap node2 10 10
./scenarios.sh --config ../cluster.toml rolling-restart 30
./scenarios.sh --config ../cluster.toml partition-kill node2 node3
./scenarios.sh --config ../cluster.toml partition-asym 2 180 60 random random
./scenarios.sh --config ../cluster.toml partition-no-quorum-split 180
./scenarios.sh --config ../cluster.toml partition-majority-minority 180 random
./scenarios.sh --config ../cluster.toml partition-load random 180 60
./scenarios.sh --config ../cluster.toml delay-load-spike node2 300 200ms 50ms 60
./scenarios.sh --config ../cluster.toml soak 21600 60
```

`kill-under-load` and `soak` can start managed load for the scenario.
`CHAOS_LOAD_CMD`, `CHAOS_BENCH_ENABLE=1`, and `CHAOS_TX_ENABLE=1` are
independent, so `gravity_bench` can provide background pressure while the
built-in tx workload records sentinel transactions for correctness checks.

```bash
CHAOS_LOAD_CMD='gravity_bench --config /path/to/bench_config.toml' \
  ./scenarios.sh --config ../cluster.toml kill-under-load node2

CHAOS_LOAD_CMD='gravity_bench --config /path/to/bench_config.toml' \
  ./scenarios.sh --config ../cluster.toml soak 21600 60
```

Use `CHAOS_BENCH_ENABLE=1` to let the scenario start and stop
`external/gravity_bench` itself. By default it passes `--recover`, so prepare
accounts/contracts first or set `CHAOS_BENCH_RECOVER=0` for a full bench setup
run. Set `CHAOS_BENCH_CONFIG` explicitly unless a generated
`faucet_bench_config.toml` is already present under `GRAVITY_ARTIFACTS_DIR` or
`cluster/output`. The bench config should set `performance.duration_secs` to
`0` or to a value longer than the chaos scenario; otherwise the scenario treats
an early bench exit as a failure.

```bash
CHAOS_BENCH_ENABLE=1 \
CHAOS_BENCH_CONFIG=/path/to/bench_config.toml \
  ./scenarios.sh --config ../cluster.toml soak 21600 60

CHAOS_BENCH_ENABLE=1 \
CHAOS_BENCH_CONFIG=/path/to/bench_config.toml \
CHAOS_BENCH_LOG_FILE=/tmp/gravity-bench.log \
  ./scenarios.sh --config ../cluster.toml kill-under-load node2
```

Use the built-in minimal EVM transfer workload as the correctness sentinel. It
signs simple transfers with funded private keys, records submitted and confirmed
transactions to a history JSONL file, then checks confirmed receipts still
exist on the canonical chain after the scenario.

```bash
CHAOS_TX_ENABLE=1 \
CHAOS_TX_PRIVATE_KEYS=0x... \
CHAOS_TX_HISTORY_FILE=/tmp/gravity-chaos-history.jsonl \
  ./scenarios.sh --config ../cluster.toml kill-under-load node2

CHAOS_TX_ENABLE=1 \
CHAOS_TX_HISTORY_FILE=/tmp/gravity-chaos-history.jsonl \
  ./scenarios.sh --config ../cluster.toml soak 3600 60
```

Recommended combined mode:

```bash
CHAOS_BENCH_ENABLE=1 \
CHAOS_BENCH_CONFIG=/path/to/bench_config.toml \
CHAOS_TX_ENABLE=1 \
CHAOS_TX_HISTORY_FILE=/tmp/gravity-chaos-history.jsonl \
  ./scenarios.sh --config ../cluster.toml soak 21600 60
```

The workload can discover keys from `CHAOS_TX_PRIVATE_KEYS`,
`CHAOS_TX_ACCOUNTS_FILE`, `GRAVITY_ARTIFACTS_DIR/accounts.csv`, `[faucet_init]`
in the cluster config, or `[genesis.faucet]` in a nearby genesis TOML. Tune it
with `CHAOS_TX_INTERVAL`, `CHAOS_TX_MAX_ACCOUNTS`, `CHAOS_TX_NODES`,
`CHAOS_TX_GAS_PRICE_WEI`, `CHAOS_TX_RECEIPT_TIMEOUT`, and
`CHAOS_TX_FAIL_ON_TIMEOUT`.
For `partition-load`, when `CHAOS_TX_ENABLE=1` and
`CHAOS_TX_RECEIPT_TIMEOUT` is not set, the scenario automatically stretches the
receipt timeout to `hold_s + recover_s + 60` so transactions submitted during a
temporary no-quorum partition have time to confirm after heal.

Run the pieces directly:

```bash
python3 lib/tx_workload.py \
  --config ../cluster.toml \
  --history-file /tmp/gravity-chaos-history.jsonl \
  --max-txs 10

python3 lib/receipt_checker.py \
  --config ../cluster.toml \
  --history-file /tmp/gravity-chaos-history.jsonl \
  --require-txs
```

`receipt_checker.py` now reports both receipt canonicality and tx-history
health. `tx_timeout`, `tx_error`, missing terminal events, missing workload stop,
or a workload stop status of `fail` are hard failures by default. A
`tx_interrupted` at scenario shutdown is reported as `inconclusive` unless
`CHAOS_TX_FAIL_INTERRUPTED=1` is set. Set `CHAOS_TX_FAIL_ON_INCONCLUSIVE=1` when
an inconclusive tx history should fail the whole scenario.

Reports are written as JSONL. Override the path with:

```bash
CHAOS_REPORT_FILE=/tmp/gravity-chaos.jsonl \
  ./scenarios.sh --config ../cluster.toml rolling-restart

./report.sh /tmp/gravity-chaos.jsonl
```

Network commands affect the machine where they are run:

```bash
./chaos.sh --config ../cluster.toml partition node2
./chaos.sh --config ../cluster.toml partition-asym node2 in
./chaos.sh --config ../cluster.toml partition-asym node2 out
./chaos.sh --config ../cluster.toml net-status
./chaos.sh --config ../cluster.toml heal

CHAOS_NET_IFACE=eth0 ./chaos.sh --config ../cluster.toml delay 200ms 50ms
CHAOS_NET_IFACE=eth0 ./chaos.sh --config ../cluster.toml loss 5%
CHAOS_NET_IFACE=eth0 ./chaos.sh --config ../cluster.toml throttle 1mbit
```

For localhost multi-node clusters, `partition <node>` drops the target node's
configured listener ports. This is useful as a simple local approximation, but
it is not as strong as per-node network namespaces or running the command on a
dedicated VM/container for the target node.

Docker backend:

```bash
CHAOS_BACKEND=docker ./chaos.sh --config ../cluster.toml restart node2
CHAOS_BACKEND=docker ./chaos.sh --config ../cluster.toml partition node2
CHAOS_BACKEND=docker ./chaos.sh --config ../cluster.toml partition-asym node2 in
CHAOS_BACKEND=docker ./chaos.sh --config ../cluster.toml heal node2
CHAOS_BACKEND=docker ./chaos.sh --config ../cluster.toml net-status node2
CHAOS_BACKEND=docker ./scenarios.sh --config ../cluster.toml rolling-restart 30
```

Docker network disconnect based partitioning requires containers attached to a
regular Docker bridge or user-defined network. It does not work for containers
created with `network_mode: host`; in that topology Docker refuses to disconnect
the host network, so `partition` exits with an error instead of reporting a
false partition.

For the local 4-validator Docker partition topology, render and start the bridge
compose file first:

```bash
cd docker/gravity_node
./render-cluster-bridge-config.sh
GRAVITY_IMAGE=gravity_node IMAGE_TAG=local-chaos-entrypoint \
  docker compose -f docker-compose.cluster-bridge.yaml up -d

cd ../..
CHAOS_BACKEND=docker ./cluster/chaos/chaos.sh \
  --config docker/gravity_node/cluster.bridge.toml partition node4
CHAOS_BACKEND=docker ./cluster/chaos/chaos.sh \
  --config docker/gravity_node/cluster.bridge.toml heal node4

CHAOS_BACKEND=docker ./cluster/chaos/scenarios.sh \
  --config docker/gravity_node/cluster.bridge.toml partition-random 3 180 60
CHAOS_BACKEND=docker ./cluster/chaos/scenarios.sh \
  --config docker/gravity_node/cluster.bridge.toml partition-asym 2 180 60 random random
CHAOS_BACKEND=docker ./cluster/chaos/scenarios.sh \
  --config docker/gravity_node/cluster.bridge.toml partition-no-quorum-split 180
CHAOS_BACKEND=docker ./cluster/chaos/scenarios.sh \
  --config docker/gravity_node/cluster.bridge.toml partition-majority-minority 180 random
CHAOS_BACKEND=docker CHAOS_TX_ENABLE=1 ./cluster/chaos/scenarios.sh \
  --config docker/gravity_node/cluster.bridge.toml partition-load majority-minority 180 60
CHAOS_BACKEND=docker CHAOS_TX_ENABLE=1 ./cluster/chaos/scenarios.sh \
  --config docker/gravity_node/cluster.bridge.toml delay-load-spike node2 300 200ms 50ms 60
CHAOS_BACKEND=docker ./cluster/chaos/scenarios.sh \
  --config docker/gravity_node/cluster.bridge.toml partition-mixed 1800 180 60
```

For Docker backend, `oracle.sh` and `scenarios.sh` default
`CHAOS_DOCKER_RPC_NETWORK=gravity-chaos` and sample RPC through Docker DNS
using a short-lived `curlimages/curl` container. This avoids false failures from
Docker Desktop host-port forwarding after `docker network disconnect/connect`.
Panic/fatal/abort log scanning also runs inside the node containers for Docker
backend, so the check does not depend on host-mounted log paths. To avoid
stale historical matches from earlier runs, Docker log scanning checks the
latest `CHAOS_DOCKER_LOG_TAIL_LINES` lines per log file; default `2000`.

Container names are resolved in this order:

1. exact node id, such as `node2`;
2. `CHAOS_DOCKER_PREFIX + node id`;
3. fuzzy compose-style names containing the node id, such as
   `gravity-node2-1`.

For Docker `partition-asym`, the command installs an iptables chain inside the
target container. For Docker `delay/loss/throttle`, the command runs `tc` inside
the container:

```bash
CHAOS_BACKEND=docker ./chaos.sh --config ../cluster.toml delay node2 200ms 50ms
CHAOS_BACKEND=docker ./chaos.sh --config ../cluster.toml loss node2 5%
CHAOS_BACKEND=docker ./chaos.sh --config ../cluster.toml throttle node2 1mbit
```

Those commands require `iptables` or `tc` inside the image and container
permissions such as `NET_ADMIN`. If those are unavailable, use Docker network
disconnect/connect based `partition/heal` first. Docker loop defaults exclude
`partition-asym` and `delay-load-spike`; set
`CHAOS_DOCKER_ENABLE_NET_TOOLS_SCENARIOS=1` to include them when the image has
the required tools.

Collect a failure snapshot:

```bash
./snapshot.sh --config ../cluster.toml --out /tmp/gravity-chaos-snapshot
```

Run a bounded weighted random loop:

```bash
LOOP_MAX_ROUNDS=10 LOOP_INTERVAL=60 \
  ./loop.sh --config ../cluster.toml
```

Dry-run the loop planner without touching the cluster:

```bash
LOOP_DRY_RUN=1 LOOP_MAX_ROUNDS=5 LOOP_INTERVAL=0 \
LOOP_SCENARIO_WEIGHTS='rolling-restart=1,partition-asym=1,delay-load-spike=1' \
  ./loop.sh --config ../cluster.toml
```

Customize scenario weights with a comma-separated `name=weight` list. Weight
`0` disables a scenario.

```bash
CHAOS_BACKEND=docker \
CHAOS_TX_ENABLE=1 \
LOOP_SCENARIO_WEIGHTS='partition-random=10,partition-no-quorum-split=10,partition-majority-minority=10,partition-load=20' \
LOOP_MAX_ROUNDS=20 \
LOOP_INTERVAL=120 \
  ./loop.sh --config docker/gravity_node/cluster.bridge.toml
```

`loop.sh` writes structured JSONL rows for `round_start`, `round_pass`,
`round_fail`, `paused`, and final `summary`. Each round records the selected
scenario, target, fault, assertions, command, duration, exit code, and snapshot
path when available. Scenarios also write `result=phase` rows for important
sub-steps such as `heal`, `recovery-oracle`, and `receipt-check`, including
phase duration and pass/fail status. `report.sh` summarizes loop pass rate,
round duration percentiles, and phase duration percentiles.

Write a standalone loop summary JSON:

```bash
LOOP_SUMMARY_FILE=/tmp/gravity-loop-summary.json \
LOOP_MAX_ROUNDS=10 \
  ./loop.sh --config ../cluster.toml
```

When `PAUSE_ON_FAILURE=1`, `LOOP_FREEZE_ON_FAILURE` defaults to `1`. The loop
passes `CHAOS_FREEZE_ON_FAILURE=1` to scenarios, asking them to preserve network
fault state after a failure snapshot instead of auto-healing when possible.
Send `USR1` to resume after inspection:

```bash
kill -USR1 <loop-pid>
```

Default timings are intentionally longer than smoke tests:

- `SAMPLE_INTERVAL=10` for oracle sampling.
- `COMMON_DEPTH=5` for common-height hash/state checks.
- `LOG_SINCE=<unix-seconds>` limits panic/fatal/abort log scanning to files
  modified after that time. Scenario scripts set this to scenario start time.
- Panic scanning looks for panic event evidence such as `thread ... panicked`,
  `panicked at`, `panic occurred`, `fatal`, `abort`, and segmentation faults.
  Stack frames or source paths containing names like `std::panic::catch_unwind`
  are not treated as panic by themselves; they can appear in ordinary error
  backtraces.
- `RECOVERY_TIMEOUT=300` and `RECOVERY_INTERVAL=10` for scenario recovery.
- `STALL_HOLD=60` for `majority-failure`.
- `flap` defaults to `10` cycles with `10s` intervals.
- `rolling-restart` defaults to `30s` between stop/start.
- `partition-random` defaults to `3` rounds, `180s` partition hold, and `60s`
  recovery.
- `partition-no-quorum-split` defaults to a `180s` split hold.
- `partition-mixed` defaults to `1800s` total duration, `180s` partition hold,
  and `60s` recovery.
- `loop.sh` defaults to `60s` between rounds and writes a final summary row.
- `soak` defaults to `21600s` (6 hours), with oracle checks every `60s`.
- Built-in tx workload defaults to one transfer per second, `100 gwei`
  gas price, `30s` receipt timeout, and at most 8 loaded accounts.

## Safety Rules

- `chaos.sh kill/start/restart` call `cluster/stop.sh` and `cluster/start.sh`.
- The toolkit does not call deploy, init, clean, or `make restart`.
- `majority-failure` selects victims by validator stake, not node count.
- iptables rules are added only to the `GRAVITY_CHAOS` chain; `heal` flushes
  that chain instead of flushing host firewall rules globally.
- `tc` commands use a fixed toolkit handle and `heal` removes it only when that
  handle is present.
- Docker backend does not create containers. Start a multi-container Gravity
  topology first, then run chaos against it.
