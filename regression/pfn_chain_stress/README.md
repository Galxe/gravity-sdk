# pfn_chain Stress Test Suite

Gravity-node TPS / stability stress harness on Docker. One entry point
(`run.sh`) parameterized by:

- **Topology** — 5-node `chain` (default) OR 3-node `simple`
- **Parallelism** — single cluster (default) OR `--parallel=N` isolated clusters
- **CPU isolation** — optional `--cpuset` to pin each cluster to a core range

…all combinations work.

## Topologies

### `chain` (default, 5 nodes)

```
                  +--- pfn1 <---+
                  |             |
   node1 <-Vfn--vfn1             +--- pfn3
                  |             |
                  +--- pfn2 <---+
```

| Service | Role         | RPC port (offset 0) | Notes |
|---------|--------------|---------------------|-------|
| node1   | genesis val. | 18545               | sole validator |
| vfn1    | VFN          | 18546               | node1's shadow fullnode |
| pfn1    | PFN          | 18547               | dials vfn1; pfn3 inbound |
| pfn2    | PFN          | 18548               | dials vfn1; pfn3 inbound |
| pfn3    | PFN leaf     | 18549               | dials pfn1 + pfn2 (redundant upstream) |

Bigger gossip surface, exercises PFN-as-upstream-for-PFN sync and redundant
upstream failover.

### `simple` (3 nodes)

```
   node1 <-Vfn-- vfn1 <--Public-- pfn1
```

| Service | Role | RPC port (offset 0) |
|---------|------|---------------------|
| node1   | val. | 18545               |
| vfn1    | VFN  | 18546               |
| pfn1    | PFN  | 18547               |

Half the containers per cluster; useful for isolating per-cluster overhead
when stacking parallel instances on one host.

## Parallel clusters

Each cluster gets a port offset (A=0, B=+10000, C=+20000, …) and its own
compose project (`pfn_stress_a`, `_b`, `_c`, …), so multiple clusters
coexist on the same host with no port collision. Each has its own genesis
(`/tmp/gravity-cluster-pfn-<topology>-<ID>/`) so accounts and nonces are
independent per-chain — same bench faucet key works for all.

With `--cpuset`, the bench's `nproc` is split into `(nproc - BENCH_RESERVE) / N`
cores per cluster; bench + system processes get the top `BENCH_RESERVE`
cores (default 6, override via env).

## All services run with `network_mode: host`

The genesis bakes peer addresses as `127.0.0.1:<port>` and the seeds block
on each PFN encodes `/ip4/127.0.0.1/...` — bridge networking would break
peer dialing. Don't change this without redoing genesis rendering.

## Quick start

```bash
cd regression/pfn_chain_stress

./run.sh                              # default: 1 cluster, chain (5 nodes), bench node1
./run.sh vfn1                         # target vfn1
./run.sh pfn3                         # target pfn3 (chain only)
./run.sh --no-bench                   # bring cluster up; no stress run
./run.sh --clean vfn1                 # wipe data volumes, fresh chain, then bench vfn1

./run.sh --topology=simple            # 3-node topology, single cluster, bench node1
./run.sh --parallel=3                 # 3 parallel 5-node chain clusters
./run.sh --parallel=3 --topology=simple --cpuset
                                      # 3 parallel 3-node clusters, each on its own
                                      # core range

./stop.sh                             # tear down single cluster (preserve data)
./stop.sh --clean                     # tear down + wipe data
./stop.sh --parallel=3 --clean        # tear down A,B,C + wipe data
./status.sh                           # container state + block heights + peer counts
./status.sh --parallel=3 --topology=simple
```

## How it works

1. **Build on host, wrap in Docker.** `run.sh` first builds `gravity_node` and
   `gravity_cli` with the `quick-release` profile on the host
   (incremental — only seconds when sources unchanged), then stages the
   binaries under `docker/gravity_node/bin/`. The Docker image is
   built with `docker/gravity_node/Dockerfile` using `--target
   runtime-host-binary`, which `COPY`s both binaries into an ubuntu:24.04
   runtime — image build is ~12s after first time.

   In-container builds from source were tried and abandoned: ~65 min
   apt install + cargo git-fetch SSL errors. Not worth it for a stress
   test that needs to iterate quickly.

2. **`setup-instance.sh <ID> <PORT_OFFSET> <TOPOLOGY>`** generates per-cluster configs:
   - Stops any host-side cluster sharing the cluster's `/tmp/gravity-cluster-...` dir.
   - Drops the appropriate `cluster.toml.<topology>.template` into
     `cluster/cluster.toml`, port-offsetting every port and rewriting `base_dir`.
   - Runs `make clean init genesis deploy` to produce identities,
     genesis, waypoint, per-node configs.
   - Copies each node's config files into `./config[-ID]/<node>/` and
     sed-rewrites absolute host paths (`/tmp/gravity-cluster-.../...`) into
     container-internal paths (`/gravity/config`, `/gravity/data`).
   - When `--parallel=N`, run.sh calls setup-instance.sh sequentially for each
     ID (cluster/Makefile is a shared workspace; concurrent renders would race
     on `cluster/output/`).

3. **`docker compose up -d <services>`** brings up nodes per cluster.
   - chain topology: all 5 services (node1 vfn1 pfn1 pfn2 pfn3)
   - simple topology: only 3 (node1 vfn1 pfn1)
   - With `--parallel`, all clusters bring up concurrently.

4. **Bench launch** depends on parallelism:
   - Single cluster: `docker compose run --rm bench` in the foreground;
     chain-level TPS is reported after a 15s settle by
     `tools/analyze_chain_tps.sh` (override with `ANALYZER=/path/to/script`).
   - Parallel: each bench launched detached as
     `pfn_stress_bench_{a,b,c,...}`; tail their logs via
     `docker logs --tail 5 <container>`.

   Deployed contract addresses persist in `./bench[-ID]/deploy.json` for
   reuse across runs (deleted automatically when the chain is reset).

## Tuning

Copy `.env.example` to `.env` and edit:

| Var                   | Default | Effect |
|-----------------------|---------|--------|
| `BENCH_DURATION_SECS` | 120     | Wall time of the bench run (excluding faucet/deploy, ~6-10 min). |
| `BENCH_TARGET_TPS`    | 8000    | Bench's submit-rate cap (it backs off if pool fills). |
| `BENCH_NUM_SENDERS`   | 1500    | Concurrent senders. Below ~500 caps TPS; above ~3000 may overwhelm RPC. |
| `BENCH_NUM_ACCOUNTS`  | 10000   | Distinct EOAs the bench rotates through. |
| `BENCH_RESERVE`       | 6       | Cores reserved for bench+system when `--cpuset` is set. |
| `RUST_LOG`            | info    | Per-node log level. `debug` is very chatty. |

## Reference baselines

Measured on a clean host with bench defaults (8K target TPS, 1500 senders,
10K accts, 120 s). "Sustained chain TPS" = `eth_getBlockByNumber` derived
TPS over the steady-state window (post-ramp, pre-cooldown).

### Single cluster (`./run.sh`)

| Topology | Target | Sustained TPS | Notes |
|----------|--------|---------------|-------|
| chain    | node1  | ~5,800        | Validator direct; baseline. |
| chain    | vfn1   | ~4,800        | One hop. |
| chain    | pfn3   | ~3,300        | Three hops; gossip-bound. |
| simple   | node1  | ~5,800        | No PFN-to-PFN gossip; same val ceiling. |

### Parallel clusters (`./run.sh --parallel=3`)

Aggregate throughput across all 3 clusters' node1 bench targets.

| Topology | `--cpuset` | Aggregate sustained TPS | Notes |
|----------|------------|-------------------------|-------|
| chain    | off        | ~9,400                  | 3× 5-node clusters; ~3,100/cluster. CPU-bound — gossip overhead per cluster dominates. |
| chain    | on         | ~10,200                 | Cpuset reduces cross-cluster cache contention. |
| simple   | off        | ~14,000                 | ~4,700/cluster; fewer gossip hops free up CPU. |
| simple   | on         | ~14,500                 | Marginal gain over no-cpuset for simple. |

(Numbers are from a 48-core / 256GB testbed; absolute scale will differ.
The shape of the chart — `simple > chain` in parallel, `cpuset` helping
chain more than simple — is what matters.)

## Caveats

- **Conflicts with host cluster:** `setup-instance.sh` stops any process
  bound to the cluster's `/tmp/gravity-cluster-pfn-<topology>-<ID>` dir.
  If a host-side `cluster/start.sh` cluster needs to coexist, change
  `base_dir` in the appropriate `cluster.toml.<topology>.template`.
- **Devnet only:** `identity.yaml` (private keys) is left world-readable
  so the container's uid 10001 can read it. Do NOT reuse this layout
  for mainnet.
- **Bench's `log_path` is ignored:** the bench logs to
  `/tmp/log.<unix-ts>.log` inside the container; `docker compose run --rm`
  removes that on exit. To capture, drop `--rm` or stream with
  `docker compose logs -f bench`. Parallel benches use `docker run -d`
  (not compose), so logs are reachable via `docker logs <container>`.
- **TPS analyzer optional:** chain-level TPS is computed by
  `tools/analyze_chain_tps.sh` (bundled). If you set `ANALYZER=` to a path
  that isn't executable, the calculation is silently skipped; the bench's
  own throughput report still prints.
- **Faucet phase is slow:** with `BENCH_NUM_ACCOUNTS=10000` and 2 tokens,
  the multi-level cascade takes ~6-10 min before stress begins. Reduce
  `BENCH_NUM_ACCOUNTS` to speed up iteration.
- **Sequential render:** parallel cluster bring-up is parallel, but
  `setup-instance.sh` must run sequentially (cluster/Makefile shares
  `cluster/output/`). Setup time is ~30 s per cluster.
