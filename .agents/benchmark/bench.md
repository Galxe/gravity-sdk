---
description: How to run gravity_bench stress tests against a Gravity cluster
---

# Gravity Benchmark (Stress Test) Workflow

## Environment Setup

All machine-specific config is in `.env/bench.env` (gitignored). Source it before any operations:

```bash
source .env/bench.env
```

Only GCP **instance names** are stored in the env file. Internal IPs (`$DEPLOY_INTERNAL_IP`, `$BENCH_INTERNAL_IP`) and the RPC URL (`$RPC_URL`) are **resolved dynamically** from instance names via `gcloud compute instances describe` when you source the file. This avoids stale IPs if machines are recreated.

| Role | Instance Variable | Derived Variables |
|------|-------------------|-------------------|
| **Cluster (Deploy)** | `$DEPLOY_INSTANCE` | `$DEPLOY_INTERNAL_IP`, `$RPC_URL` |
| **Bench (Stress)** | `$BENCH_INSTANCE` | `$BENCH_INTERNAL_IP` |

### Check & start machines

```bash
# Check status
gcloud compute instances list --project=$GCP_PROJECT

# Start machines if TERMINATED
gcloud compute instances start $DEPLOY_INSTANCE $BENCH_INSTANCE \
    --project=$GCP_PROJECT --zone=$GCP_ZONE

# Re-source env to refresh IPs after machines start
source .env/bench.env
```

### SSH into machines
```bash
# Deploy machine
$SSH_DEPLOY "echo connected"

# Bench machine
$SSH_BENCH "echo connected"
```

> [!IMPORTANT]
> **PATH issue**: Non-interactive SSH (`--command`) does not load `.bashrc`/`.profile`. The `$SSH_CMD_PREFIX` is already included in commands below, but for manual SSH sessions, always run:
> ```bash
> source $HOME/.cargo/env && export PATH=$HOME/.foundry/bin:$PATH
> ```

> [!NOTE]
> For local testing, both roles can be the same machine (`127.0.0.1`). For remote deployment, ensure the bench machine can reach the cluster's RPC ports (`$RPC_URL`).

---

## Prerequisites

- **Rust** toolchain (with `cargo`) — to compile gravity_bench
- **Python 3** with `web3`, `py-solc-x` — install system-wide: `pip3 install --break-system-packages web3 py-solc-x`
- **jq** — for TOML/JSON config parsing
- **forge** — gravity_bench setup compiles contracts
- **gravity_bench** repo — auto-cloned by `faucet.sh` to `external/gravity_bench`, or manually specify location
- **python symlink** — `sudo ln -sf /usr/bin/python3 /usr/bin/python` (gravity_bench calls `python`, not `python3`)

---

## 1. Prepare the Cluster

Ensure a Gravity cluster is up and running on the deploy machine. Refer to the `/build-and-test` workflow (Sections 1-2).

```bash
$SSH_DEPLOY "$SSH_CMD_PREFIX && cd $DEPLOY_SDK_DIR && \
    git checkout main && git pull origin main && \
    RUSTFLAGS='--cfg tokio_unstable' cargo build --bin gravity_node --profile quick-release"

$SSH_DEPLOY "$SSH_CMD_PREFIX && cd $DEPLOY_SDK_DIR/cluster && \
    cp example/1_node/genesis.toml . && cp example/1_node/cluster.toml . && \
    make init && make genesis && make deploy_start && make status"
```

---

## 2. Configure gravity_bench Dependency

In `cluster.toml`, specify the bench repo source:

```toml
[dependencies.gravity_bench]
repo = "https://github.com/Galxe/gravity_bench.git"
ref = "main"
```

The repo is auto-cloned to `external/gravity_bench` on first faucet run.

---

## 3. Run Stress Test

> [!NOTE]
> **No separate faucet step needed.** `gravity_bench` automatically handles account creation and funding (faucet) as part of its startup. The `[faucet]` and `[accounts]` sections in `bench_config.toml` control this behavior.
>
> (`make faucet` in the cluster Makefile exists for the E2E test workflow, not for bench.)

### 3.1 Write Bench Config

On the bench machine, generate `bench_config.toml`:

```bash
$SSH_BENCH "cat > $BENCH_DIR/bench_config.toml << EOF
contract_config_path = \"deploy.json\"
target_tps = $BENCH_TARGET_TPS
num_tokens = $BENCH_NUM_TOKENS
enable_swap_token = false
address_pool_type = \"random\"

nodes = [
    { rpc_url = \"$RPC_URL\", chain_id = $CHAIN_ID },
]

[faucet]
private_key = \"$FAUCET_PRIVATE_KEY\"
faucet_level = 10
wait_duration_secs = 2
fauce_eth_balance = \"100000000000000000000000000000000\"

[accounts]
num_accounts = $FAUCET_NUM_ACCOUNTS

[performance]
num_senders = $BENCH_NUM_SENDERS
max_pool_size = $BENCH_MAX_POOL_SIZE
duration_secs = $BENCH_DURATION_SECS
EOF"
```

### 3.2 Bench Config Parameters

| Parameter | Description |
|-----------|-------------|
| `target_tps` | Target transactions per second |
| `num_tokens` | Number of ERC20 tokens to deploy (**must be ≥ 1**, 0 causes division-by-zero) |
| `enable_swap_token` | Enable Uniswap swap test mode |
| `num_senders` | Concurrent sender tasks inside TxnConsumer |
| `max_pool_size` | Max capacity of the transaction pool |
| `duration_secs` | Bench duration in seconds (0 = run until all txns sent) |
| `faucet_level` | Tree-based distribution depth (10 → cascade 1→10→100) |

### 3.3 Build & Run

```bash
# Build
$SSH_BENCH "$SSH_CMD_PREFIX && cd $BENCH_DIR && \
    git pull origin main && cargo build --release"

# Run (tee stdout to capture log filename for report generation)
$SSH_BENCH "$SSH_CMD_PREFIX && cd $BENCH_DIR && \
    cargo run --release" 2>&1 | tee /tmp/bench_stdout.log
```

> [!TIP]
> `gravity_bench` prints `Logging to file: "./log.<timestamp>.log"` to stdout. We `tee` it so step 5 can extract the exact log filename.

---

## 4. Reading Results & Key Metrics

`gravity_bench` outputs periodic snapshots to both stdout and a log file (`./log.<timestamp>.log`). There are **two tables** to focus on:

### 4.1 Transaction Tracker Table (periodic, every ~2s)

```
┌─────────────────┬───────────┬───────────────┬────────┐
│ Metric          ┆ Value     ┆ Metric        ┆ Value  │
╞═════════════════╪═══════════╪═══════════════╪════════╡
│ Progress        ┆ 1.2M/1.2M ┆ TPS           ┆ 9553.2 │
│ Pending Txns    ┆ 5242      ┆ Avg Latency   ┆ 0.5s   │
│ Timed Out Txns  ┆ 0         ┆ Success%      ┆ 99.3%  │
│ Send Failures   ┆ 0         ┆ Exec Failures ┆ 0      │
│ Pool Pending    ┆ 4187      ┆ Pool Queued   ┆ 0      │
│ Ready Accounts  ┆ 1405      ┆ Processing    ┆ 1668   │
└─────────────────┴───────────┴───────────────┴────────┘
```

| Field | Meaning | Health Indicator |
|-------|---------|------------------|
| **TPS** | **Sliding-window** throughput: confirmed receipts in the last 17 seconds ÷ 17. NOT total/elapsed. | Close to `target_tps` = good |
| **Avg Latency** | Time from `eth_sendRawTransaction` to receipt confirmation (rolling window of last 1000 txns) | < 1s = good, > 5s = bottleneck |
| **Success%** | `total_resolved / total_produced × 100`. Includes both faucet and bench phases. | > 99% = healthy |
| **Pending Txns** | Sent but not yet confirmed. Grows if chain can't keep up. | Stable or decreasing = good |
| **Timed Out Txns** | Sent but no receipt after 120s (`RETRY_TIMEOUT`). Absolute timeout is 10 min. | 0 = ideal |
| **Send Failures** | RPC `eth_sendRawTransaction` returned error (not NonceTooLow). | 0 = ideal |
| **Exec Failures** | Txn included in block but reverted (`receipt.status() == false`). | 0 = ideal |
| **Pool Pending/Queued** | Node's mempool state via `txpool_status` RPC. | Queued=0 means nonces are sequential |
| **Progress** | `total_resolved / total_produced` — cumulative across ALL plans (faucet + bench). | Reaches target before `duration_secs` = good |

> [!TIP]
> **Bottleneck Analysis**: The report includes a raw time series of (TPS, Pending Txns, Pool Pending, Pool Queued) for every bench-phase snapshot. Use this to determine the bottleneck:
> - **Chain bottleneck**: Pending Txns keeps growing or stays high; Pool Pending consistently ≥ `max_pool_size / 2` (~80% of time)
> - **Pressure client bottleneck**: Pool Pending oscillates widely, frequently dropping to 0; Pending Txns doesn't grow
> - **Nonce issues**: Pool Queued > 0 indicates nonce gaps
> - **Data quality**: If fewer than 50 data points, extend `duration_secs` for more reliable analysis

> [!TIP]
> **How TPS is calculated** (from [`txn_tracker.rs`](file:///home/kenji/galxe/gravity-sdk-dev/external/gravity_bench/src/actors/monitor/txn_tracker.rs#L674-L687)):
> ```
> TPS = count(resolved_txn_timestamps in last 17s) / 17
> ```
> It's a **17-second sliding window** (`TPS_WINDOW = 17s`) of confirmed receipts, reported every ~2 seconds by the Monitor actor. This means:
> - TPS reflects the **instantaneous confirmation rate**, not a cumulative average
> - During faucet phase, TPS will be low (single digits) — this is normal
> - The TPS you care about is the one reported **after faucet completes** and the bench loop starts

> [!IMPORTANT]
> **Faucet vs Bench phase**: The flow in `main.rs` is **sequential** — ETH faucet (blocking) → token faucets (blocking) → bench loop. However, **TPS/Progress/Success% are global counters** that accumulate across ALL phases. To read the actual bench TPS:
> - Look for the log line `"bench erc20 transfer"` (or `"bench uniswap"`) — everything **after** this is the real bench phase
> - The TPS snapshots after that point reflect actual stress test throughput
> - `Progress` will include faucet txns in the denominator, so it may show a head start

### 4.2 RPC Stats Table (at end of bench)

```
┌───────────────────────────┬──────┬───────────┬────────┬──────────────┬─────────────┐
│ RPC Method                ┆ Sent ┆ Succeeded ┆ Failed ┆ Success Rate ┆ Avg Latency │
╞═══════════════════════════╪══════╪═══════════╪════════╪══════════════╪═════════════╡
│ eth_sendRawTransaction    ┆ 1.2M ┆ 1.2M      ┆ 0      ┆ 100.0%       ┆ 3.2ms       │
│ eth_getTransactionReceipt ┆ 1514 ┆ 1514      ┆ 0      ┆ 100.0%       ┆ 1.3ms       │
│ txpool_status             ┆ 189  ┆ 189       ┆ 0      ┆ 100.0%       ┆ 1.0ms       │
└───────────────────────────┴──────┴───────────┴────────┴──────────────┴─────────────┘
```

This shows **RPC-level** performance. Key things to check:
- `eth_sendRawTransaction` Failed=0: the bench machine can reach the node and txns are accepted
- Avg Latency: measures RPC round-trip, not on-chain confirmation. 1-5ms is normal.

### 4.3 Faucet Phase

Before the stress test, the faucet distributes ETH + ERC20 tokens to all accounts. You'll see low single-digit TPS during this phase — **this is normal**. The real benchmark starts after `Ready Accounts` reaches `num_accounts`.

---

## 5. Record Commit IDs & Generate Report

After each bench run, collect commit IDs and generate a report with **bench-phase only** TPS:

```bash
# 1. Extract log filename from bench stdout (captured by tee in step 3.3)
BENCH_LOG_NAME=$(grep -oP 'Logging to file: "\K[^"]+' /tmp/bench_stdout.log)
echo "Log file: $BENCH_LOG_NAME"

# 2. Collect commit IDs
SDK_COMMIT=$($SSH_DEPLOY "$SSH_CMD_PREFIX && cd $DEPLOY_SDK_DIR && git log -1 --format='%H'")
BENCH_COMMIT=$($SSH_BENCH "$SSH_CMD_PREFIX && cd $BENCH_DIR && git log -1 --format='%H'")

# 3. Download log locally
$SSH_BENCH "cat $BENCH_DIR/$BENCH_LOG_NAME" > /tmp/bench_latest.log

# 4. Generate report (bench-phase TPS only, faucet excluded)
bash .agents/benchmark/gen_report.sh "$SDK_COMMIT" "$BENCH_COMMIT" /tmp/bench_latest.log

# 5. Archive log to bench_reports/ for future auditing
cp /tmp/bench_latest.log bench_reports/"$(basename $BENCH_LOG_NAME)"
echo "Log archived to bench_reports/$(basename $BENCH_LOG_NAME)"
```

Reports are saved to `bench_reports/bench_report_<timestamp>.md`. The raw log is archived alongside as `bench_reports/log.<timestamp>.log`. The script ([`gen_report.sh`](.agents/benchmark/gen_report.sh)):
- Extracts TPS values only **after** the `"bench erc20 transfer"` log line (faucet excluded)
- Calculates avg/min/max TPS across all bench-phase snapshots
- Includes the last 50 lines as a detailed snapshot


---

## 6. Shutdown

> [!CAUTION]
> **Always stop machines after bench is done** to avoid unnecessary GCP costs.

```bash
gcloud compute instances stop $DEPLOY_INSTANCE $BENCH_INSTANCE \
    --project=$GCP_PROJECT --zone=$GCP_ZONE
```

---

## Troubleshooting

| Problem | Solution |
|---------|----------|
| `cargo: command not found` | SSH non-interactive shell doesn't load PATH. Run `source $HOME/.cargo/env` first |
| `python: command not found` | Only `python3` exists. Create symlink: `sudo ln -sf /usr/bin/python3 /usr/bin/python` |
| `No module named 'web3'` / `'solcx'` | Install system-wide: `pip3 install --break-system-packages -r requirements.txt` |
| `num_tokens = 0` causes panic | `erc20_transfer.rs` division by zero. Set `num_tokens ≥ 1` |
| `gravity_bench` clone fails | Check network/SSH keys, or manually clone to `external/gravity_bench` |
| Faucet reports chain_id not found | Ensure `output/genesis.json` exists (run `make genesis` first) |
| Faucet insufficient balance | Check genesis allocation for the faucet private key address |
| RPC connection refused | Confirm nodes are running and bench machine can reach `$RPC_URL` |
| `setup.sh` not found | gravity_bench repo version might be wrong, check `ref` config |
