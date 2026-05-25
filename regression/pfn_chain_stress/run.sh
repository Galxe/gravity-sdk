#!/usr/bin/env bash
# pfn_chain stress test — single OR parallel clusters, chain OR simple topology.
#
# Flow:  cargo build (host) → stage binary → docker build (wraps binary)
#        → render config(s) → up cluster(s) → run bench(es) → report
#
# Usage:
#   ./run.sh                              # default: 1 cluster, chain (5 nodes),
#                                         # bench targets node1
#   ./run.sh vfn1                         # target=vfn1 (single cluster)
#   ./run.sh --no-bench                   # bring cluster up; skip bench
#   ./run.sh --clean                      # wipe data volumes first
#
#   ./run.sh --topology=simple            # 3-node topology (drop pfn2, pfn3)
#                                         # single cluster, bench node1
#   ./run.sh --parallel=3                 # 3 parallel isolated chain clusters,
#                                         # one bench per cluster (all targeting
#                                         # their node1)
#   ./run.sh --parallel=3 --topology=simple --cpuset
#                                         # 3 parallel simple clusters with
#                                         # per-cluster cpuset isolation
#                                         # (computed from `nproc`)
#
# Combinations are orthogonal. Some legal mixes:
#   --parallel=3                          # 3 chain clusters
#   --parallel=3 --topology=simple        # 3 simple clusters
#   --cpuset --parallel=3                 # 3 chain clusters, cpuset-isolated
#
# Env overrides (see .env.example):
#   BENCH_DURATION_SECS=120
#   BENCH_TARGET_TPS=8000
#   BENCH_NUM_SENDERS=1500
#   BENCH_NUM_ACCOUNTS=10000
#   BENCH_RESERVE=6                       # cores reserved for bench+system when
#                                         # --cpuset is set
#
# Output:
#   - Single-cluster: bench runs foreground; chain TPS reported after settle.
#   - Parallel:       benches run detached in containers
#                     pfn_stress_bench_{A,B,C,...}; tail their logs.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Disable ambient proxy — adds 5x latency to localhost RPC.
unset http_proxy https_proxy HTTP_PROXY HTTPS_PROXY all_proxy ALL_PROXY no_proxy NO_PROXY

# ── Parse args ──────────────────────────────────────────────────────────────
TARGET="node1"
TARGET_SET=0
TOPOLOGY="chain"
PARALLEL=1
USE_CPUSET=0
NO_BENCH=0
CLEAN=0

usage() { sed -n '3,40p' "$0"; }

for arg in "$@"; do
    case "$arg" in
        node1|vfn1|pfn1|pfn2|pfn3)
            [[ $TARGET_SET -eq 1 ]] && { echo "[run] only one target node allowed (got $TARGET, then $arg)" >&2; exit 2; }
            TARGET="$arg"; TARGET_SET=1 ;;
        --topology=chain|--topology=simple)
            TOPOLOGY="${arg#--topology=}" ;;
        --parallel=*)
            PARALLEL="${arg#--parallel=}"
            [[ "$PARALLEL" =~ ^[1-9][0-9]*$ ]] || { echo "[run] --parallel must be a positive int"; exit 2; }
            [[ "$PARALLEL" -le 26 ]] || { echo "[run] --parallel must be <=26 (single-letter cluster IDs)"; exit 2; } ;;
        --cpuset) USE_CPUSET=1 ;;
        --no-bench) NO_BENCH=1 ;;
        --clean)    CLEAN=1 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "[run] unknown arg: $arg (use -h for usage)" >&2; exit 2 ;;
    esac
done

# Validate target against topology — simple has no pfn2/pfn3.
if [[ "$TOPOLOGY" == "simple" && ( "$TARGET" == "pfn2" || "$TARGET" == "pfn3" ) ]]; then
    echo "[run] ERROR: target=$TARGET is not available in --topology=simple" >&2
    exit 2
fi

# Load .env if present. Provides defaults; pre-set env wins, so
# `BENCH_TARGET_TPS=12000 ./run.sh` behaves as users expect even with a
# saved .env. Lines starting with '#' and blank lines are ignored.
if [[ -f .env ]]; then
    while IFS='=' read -r k v; do
        [[ -z "$k" || "$k" =~ ^[[:space:]]*# ]] && continue
        # Strip surrounding quotes from value if present.
        v="${v%\"}"; v="${v#\"}"; v="${v%\'}"; v="${v#\'}"
        # `${!k+x}` is non-empty iff $k is already set (even if empty).
        [[ -z "${!k+x}" ]] && export "$k=$v"
    done < .env
fi

BENCH_DURATION_SECS="${BENCH_DURATION_SECS:-120}"
BENCH_TARGET_TPS="${BENCH_TARGET_TPS:-8000}"
BENCH_NUM_SENDERS="${BENCH_NUM_SENDERS:-1500}"
BENCH_NUM_ACCOUNTS="${BENCH_NUM_ACCOUNTS:-10000}"
BENCH_RESERVE="${BENCH_RESERVE:-6}"

echo "[run] topology=$TOPOLOGY parallel=$PARALLEL cpuset=$USE_CPUSET target=$TARGET clean=$CLEAN bench=$([[ $NO_BENCH -eq 0 ]] && echo yes || echo no)"

# ── Helpers ─────────────────────────────────────────────────────────────────

# Generate cluster IDs: PARALLEL=1 → ("single"); PARALLEL=N → ("A" "B" "C" ...)
cluster_ids() {
    if [[ "$PARALLEL" -eq 1 ]]; then
        echo "single"
    else
        # A, B, ..., up to PARALLEL letters
        local i
        for (( i=0; i<PARALLEL; i++ )); do
            printf '%s ' "$(printf "\\$(printf '%03o' $((65+i)))")"
        done
    fi
}

# Per-cluster port offset. Parallel clusters use 10000, 20000, … to ensure
# disjoint bands (max metrics-to-inspection spread is well under 1000).
port_offset_for() {
    local id="$1"
    if [[ "$id" == "single" ]]; then
        echo 0
    else
        # A→0, B→10000, C→20000, …
        local letter_ord=$(printf '%d' "'$id")
        echo $(( (letter_ord - 65) * 10000 ))
    fi
}

# RPC port for a (cluster, node) pair = base + offset.
rpc_port_for() {
    local id="$1" node="$2"
    local offset; offset="$(port_offset_for "$id")"
    case "$node" in
        node1) echo $((18545 + offset)) ;;
        vfn1)  echo $((18546 + offset)) ;;
        pfn1)  echo $((18547 + offset)) ;;
        pfn2)  echo $((18548 + offset)) ;;
        pfn3)  echo $((18549 + offset)) ;;
        *) echo "[run] BUG: unknown node $node" >&2; exit 1 ;;
    esac
}

compose_project() {
    local id="$1"
    if [[ "$id" == "single" ]]; then
        echo "pfn_stress"
    else
        local lc; lc="$(echo "$id" | tr 'A-Z' 'a-z')"
        echo "pfn_stress_${lc}"
    fi
}

config_dir() {
    local id="$1"
    if [[ "$id" == "single" ]]; then
        echo "./config"
    else
        echo "./config-${id}"
    fi
}

bench_dir() {
    local id="$1"
    if [[ "$id" == "single" ]]; then
        echo "./bench"
    else
        echo "./bench-${id}"
    fi
}

# Services to bring up depend on topology.
if [[ "$TOPOLOGY" == "chain" ]]; then
    NODE_SERVICES=(node1 vfn1 pfn1 pfn2 pfn3)
else
    NODE_SERVICES=(node1 vfn1 pfn1)
fi

# ── 1. Build host binary + stage into Docker ctx ───────────────────────────
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
HOST_BINARY="$REPO_ROOT/target/quick-release/gravity_node"
STAGED_BINARY="$REPO_ROOT/docker/gravity_node/bin/gravity_node"

echo "[run] cargo build gravity_node (host, profile=quick-release)…"
(cd "$REPO_ROOT" && RUSTFLAGS="--cfg tokio_unstable" \
    cargo build --bin gravity_node --profile quick-release) \
    || { echo "[run] cargo build failed"; exit 1; }

mkdir -p "$REPO_ROOT/docker/gravity_node/bin"
if ! cmp -s "$HOST_BINARY" "$STAGED_BINARY" 2>/dev/null; then
    cp -f "$HOST_BINARY" "$STAGED_BINARY"
    echo "[run] staged binary -> $STAGED_BINARY ($(stat -c %s "$STAGED_BINARY") bytes)"
else
    echo "[run] staged binary already up-to-date"
fi

# Build gravity_node image (shared by all clusters).
# Always invoke `docker build` — BuildKit's COPY layer is keyed on the binary's
# hash, so a no-op build is fast (~1s) when the staged binary hasn't changed,
# and a real rebuild (~10s) happens only when it has. A previous version gated
# this on `docker image inspect` succeeding, which silently kept the stale
# image baked from the first-ever run even after `cargo build` produced a new
# binary — every subsequent cluster ran old code.
echo "[run] building gravity_node:pfn-stress (cached if binary unchanged)…"
DOCKER_BUILDKIT=1 docker build \
    -t gravity_node:pfn-stress \
    -f "$REPO_ROOT/docker/gravity_node/Dockerfile" \
    --target runtime-host-binary \
    "$REPO_ROOT" \
    || { echo "[run] gravity_node image build failed"; exit 1; }

if [[ "$NO_BENCH" -eq 0 ]]; then
    # Bench image — build only if missing. Don't trigger a from-source rebuild
    # on every run (docker.io auth occasionally times out on flaky IPv6).
    if ! docker image inspect gravity_bench:pfn-stress >/dev/null 2>&1; then
        echo "[run] gravity_bench:pfn-stress not found — building once…"
        DOCKER_BUILDKIT=1 docker build \
            -t gravity_bench:pfn-stress \
            -f "$REPO_ROOT/external/gravity_bench/Dockerfile" \
            "$REPO_ROOT/external/gravity_bench" \
            || { echo "[run] bench image build failed"; exit 1; }
    fi
fi

# ── 2. Compute cpuset bands (if --cpuset) ───────────────────────────────────
declare -A CPUSET
BENCH_CPUSET=""
if [[ "$USE_CPUSET" -eq 1 ]]; then
    total_cores=$(nproc)
    usable=$(( total_cores - BENCH_RESERVE ))
    per=$(( usable / PARALLEL ))
    if [[ $per -lt 2 ]]; then
        echo "[run] ERROR: $total_cores cores too few for $PARALLEL clusters + $BENCH_RESERVE reserve" >&2
        exit 1
    fi
    cur=0
    for id in $(cluster_ids); do
        end=$(( cur + per - 1 ))
        CPUSET[$id]="${cur}-${end}"
        cur=$(( end + 1 ))
    done
    BENCH_CPUSET="${cur}-$(( total_cores - 1 ))"
    echo "[run] cpuset: machine=$total_cores cores; $per per cluster; bench/system on $BENCH_CPUSET"
    for id in $(cluster_ids); do
        echo "  cluster $id -> cores ${CPUSET[$id]}"
    done
fi

# ── 3. Bring down any previous instances ────────────────────────────────────
for id in $(cluster_ids); do
    proj="$(compose_project "$id")"
    cfg="$(config_dir "$id")"
    if [[ $CLEAN -eq 1 ]]; then
        COMPOSE_PROJECT_NAME="$proj" CONFIG_DIR="$cfg" \
            docker compose down -v --remove-orphans 2>/dev/null || true
    else
        COMPOSE_PROJECT_NAME="$proj" CONFIG_DIR="$cfg" \
            docker compose down --remove-orphans 2>/dev/null || true
    fi
done

# ── 4. Render configs sequentially (cluster/Makefile is a shared workspace) ──
for id in $(cluster_ids); do
    offset="$(port_offset_for "$id")"
    echo "[run] setup-instance.sh $id offset=$offset topology=$TOPOLOGY"
    ./setup-instance.sh "$id" "$offset" "$TOPOLOGY"
done

# ── 5. Start clusters (in parallel for PARALLEL>1) ──────────────────────────
echo "[run] starting cluster(s)…"
for id in $(cluster_ids); do
    proj="$(compose_project "$id")"
    cfg="$(config_dir "$id")"
    cset="${CPUSET[$id]:-}"
    (
        COMPOSE_PROJECT_NAME="$proj" \
        CONFIG_DIR="$cfg" \
        CLUSTER_CPUSET="$cset" \
        docker compose up -d "${NODE_SERVICES[@]}" 2>&1 | sed "s/^/[$id] /"
    ) &
done
wait
echo "[run] all clusters issued 'up -d'."

# ── 6. Wait for chain progress on each cluster ──────────────────────────────
deadline=$(( $(date +%s) + 240 ))
for id in $(cluster_ids); do
    port="$(rpc_port_for "$id" node1)"
    echo -n "[run] waiting cluster $id node1 (port $port)…"
    while [[ $(date +%s) -lt $deadline ]]; do
        # `|| true` swallows curl exit 7 (connection refused) during the
        # node-is-still-coming-up window; pipefail would otherwise trip set -e.
        bn=$(curl -s --noproxy '*' -m 2 -X POST -H 'Content-Type: application/json' \
            --data '{"jsonrpc":"2.0","method":"eth_blockNumber","id":1}' \
            "http://127.0.0.1:$port" 2>/dev/null \
            | sed -n 's/.*"result":"\(0x[0-9a-fA-F]*\)".*/\1/p' || true)
        if [[ -n "$bn" && "$bn" != "0x0" ]]; then
            echo " block=$bn"
            break
        fi
        sleep 3
    done
done

if [[ $NO_BENCH -eq 1 ]]; then
    echo "[run] --no-bench set; cluster(s) up. Stop with ./stop.sh"
    exit 0
fi

# ── 7. Render & launch bench(es) ────────────────────────────────────────────
export TARGET_CHAIN_ID="${TARGET_CHAIN_ID:-1337}"
export BENCH_DURATION_SECS BENCH_TARGET_TPS BENCH_NUM_SENDERS BENCH_NUM_ACCOUNTS

start_ts=$(date +%s)

if [[ "$PARALLEL" -eq 1 ]]; then
    # ── Single-cluster: bench runs in foreground; we report TPS after settle.
    id="single"
    bdir="$(bench_dir "$id")"
    cfg="$(config_dir "$id")"
    proj="$(compose_project "$id")"
    mkdir -p "$bdir"
    rm -f "$bdir/deploy.json"  # force redeploy against this run's fresh chain

    export TARGET_RPC_URL="http://127.0.0.1:$(rpc_port_for "$id" "$TARGET")"
    envsubst < "$SCRIPT_DIR/bench.toml.tpl" > "$bdir/bench.toml"

    echo "[run] bench rendered → $bdir/bench.toml (target=$TARGET_RPC_URL)"
    echo "[run] launching gravity_bench (this can take ~${BENCH_DURATION_SECS}s + ~60s faucet/deploy)"

    COMPOSE_PROJECT_NAME="$proj" CONFIG_DIR="$cfg" BENCH_DIR="$bdir" \
        docker compose run --rm bench
    bench_exit=$?
    end_ts=$(date +%s)
    echo "[run] bench finished (exit=$bench_exit, wall=$((end_ts-start_ts))s)"

    # Optional chain-level TPS analysis. Defaults to the bundled analyzer
    # under tools/; override with ANALYZER=/path/to/script for a custom one.
    sleep 15
    ANALYZER="${ANALYZER:-$SCRIPT_DIR/tools/analyze_chain_tps.sh}"
    if [[ -x "$ANALYZER" ]]; then
        win_start=$((start_ts + 30))
        win_end=$((end_ts - 10))
        last_node="${NODE_SERVICES[-1]}"
        analyze_port="$(rpc_port_for "$id" "$last_node")"
        echo "[run] computing sustained chain TPS via $ANALYZER (window $win_start..$win_end)"
        bash "$ANALYZER" "http://127.0.0.1:$analyze_port" "$win_start" "$win_end" || true
    fi
else
    # ── Parallel: launch all benches detached so they run concurrently ──────
    for id in $(cluster_ids); do
        bdir="$(bench_dir "$id")"
        mkdir -p "$bdir"
        rm -f "$bdir/deploy.json"

        TARGET_RPC_URL="http://127.0.0.1:$(rpc_port_for "$id" "$TARGET")" \
        envsubst < "$SCRIPT_DIR/bench.toml.tpl" > "$bdir/bench.toml"
    done

    echo "[run] launching $PARALLEL benches in parallel at $(date)"
    date +%s > /tmp/pfn-stress-parallel-start.ts

    declare -A BENCH_NAME
    for id in $(cluster_ids); do
        bdir="$(bench_dir "$id")"
        lc="$(echo "$id" | tr 'A-Z' 'a-z')"
        name="pfn_stress_bench_${lc}"
        BENCH_NAME[$id]="$name"

        docker rm -f "$name" >/dev/null 2>&1 || true

        cpu_flag=()
        if [[ -n "$BENCH_CPUSET" ]]; then
            cpu_flag=(--cpuset-cpus "$BENCH_CPUSET")
        fi

        docker run -d --rm --name "$name" \
            --network host \
            "${cpu_flag[@]}" \
            -v "$SCRIPT_DIR/${bdir#./}:/tmp/bench" \
            -w /tmp \
            gravity_bench:pfn-stress \
            --config /tmp/bench/bench.toml >/dev/null
        echo "[run]   bench-$id launched as $name (target $TARGET on $(rpc_port_for "$id" "$TARGET"))"
    done

    echo "[run] benches running detached. Monitor with:"
    echo "  for c in ${BENCH_NAME[*]}; do docker logs --tail 5 \$c; done"
    echo "  ./status.sh   # cluster health"
fi

echo "[run] done."
