#!/usr/bin/env bash
# Render one cluster instance.
#
# Args:
#   $1 = ID            (single | A | B | C | ...) — distinguishes parallel
#                       instances; "single" yields the base layout (no -ID suffix)
#   $2 = PORT_OFFSET   (integer added to every port in cluster.toml)
#   $3 = TOPOLOGY      (chain | simple)
#
# Effects (idempotent — wipes & rewrites):
#   - rewrites cluster/cluster.toml (port-offset + base_dir)
#   - runs cluster/Makefile clean→init→genesis→deploy to produce identities,
#     genesis.json, waypoint.txt, and per-node configs under
#     /tmp/gravity-cluster-pfn-{chain,simple}-${ID}/
#   - rehosts per-node configs into ./config[-ID]/<node>/ with container-side
#     path remap (host /tmp/... -> /gravity/{config,data})
#
# NB: cluster/Makefile is a shared workspace; setup-instance.sh must be
# called sequentially for ID=A, B, C — concurrent calls would race on
# cluster/output/. (Bringing the clusters UP can still happen in parallel
# once all setups complete.)

set -euo pipefail

ID="${1:?usage: $0 <ID> <PORT_OFFSET> <TOPOLOGY>}"
PORT_OFFSET="${2:?usage: $0 <ID> <PORT_OFFSET> <TOPOLOGY>}"
TOPOLOGY="${3:?usage: $0 <ID> <PORT_OFFSET> <TOPOLOGY>}"

case "$TOPOLOGY" in
    chain|simple) ;;
    *) echo "[setup] ERROR: TOPOLOGY must be chain|simple (got $TOPOLOGY)" >&2; exit 2 ;;
esac

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
CLUSTER_DIR="$REPO_ROOT/cluster"
SRC_TOML="$SCRIPT_DIR/cluster.toml.${TOPOLOGY}.template"

# Host base dir: "single" -> /tmp/gravity-cluster-pfn-<topo> ; parallel -> with -ID suffix.
# Output config dir: "single" -> ./config ; parallel -> ./config-${ID}.
if [[ "$ID" == "single" ]]; then
    HOST_BASE_DIR="/tmp/gravity-cluster-pfn-${TOPOLOGY}"
    OUT_CONFIG_DIR="$SCRIPT_DIR/config"
else
    HOST_BASE_DIR="/tmp/gravity-cluster-pfn-${TOPOLOGY}-${ID}"
    OUT_CONFIG_DIR="$SCRIPT_DIR/config-${ID}"
fi

if [[ "$TOPOLOGY" == "chain" ]]; then
    NODES=(node1 vfn1 pfn1 pfn2 pfn3)
else
    NODES=(node1 vfn1 pfn1)
fi

log() { printf '[setup-%s] %s\n' "$ID" "$*" >&2; }
die() { printf '[setup-%s] ERROR: %s\n' "$ID" "$*" >&2; exit 1; }

[[ -f "$SRC_TOML" ]] || die "missing cluster template: $SRC_TOML"

# ── 1. Generate per-instance cluster.toml (port-offset all ports + base_dir) ──
log "topology=$TOPOLOGY offset=$PORT_OFFSET base_dir=$HOST_BASE_DIR"

python3 - "$SRC_TOML" "$CLUSTER_DIR/cluster.toml" "$PORT_OFFSET" "$HOST_BASE_DIR" <<'PY'
import re, sys
src, dst, offset_s, base_dir = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
offset = int(offset_s)
text = open(src).read()

def patch_port(m):
    key, val = m.group(1), int(m.group(2))
    return f"{key} = {val + offset}"

text = re.sub(
    r'\b(validator_port|vfn_port|rpc_port|metrics_port|inspection_port|https_port|authrpc_port|public_port|reth_p2p_port)\s*=\s*(\d+)',
    patch_port, text,
)
text = re.sub(r'base_dir\s*=\s*"[^"]*"', f'base_dir = "{base_dir}"', text)
open(dst, 'w').write(text)
PY

# Sanity: rpc_port should now be (18545 + offset).
expected_rpc=$((18545 + PORT_OFFSET))
grep -qE "^rpc_port = $expected_rpc\b" "$CLUSTER_DIR/cluster.toml" \
    || die "port substitution failed (expected $expected_rpc)"
log "rpc_port substitution verified ($expected_rpc)"

# ── 2. Stop & wipe stale host base_dir ────────────────────────────────────
if pgrep -f "$HOST_BASE_DIR" >/dev/null 2>&1; then
    log "killing leftover gravity_node processes on $HOST_BASE_DIR"
    pkill -TERM -f "$HOST_BASE_DIR" >/dev/null 2>&1 || true
    sleep 2
    pkill -KILL -f "$HOST_BASE_DIR" >/dev/null 2>&1 || true
fi
if [[ -d "$HOST_BASE_DIR" ]]; then
    log "wiping stale $HOST_BASE_DIR (so deploy.sh runs non-interactively)"
    rm -rf "$HOST_BASE_DIR"
fi

# ── 3. Run cluster/Makefile (clean → init → genesis → deploy) ─────────────
log "running cluster/Makefile…"
(cd "$CLUSTER_DIR" && make clean >/dev/null && make init >/dev/null \
    && make genesis >/dev/null && make deploy >/dev/null) \
    || die "cluster/Makefile failed; see cluster/ output"

[[ -f "$HOST_BASE_DIR/genesis.json" ]] || die "Makefile did not produce $HOST_BASE_DIR/genesis.json"

# ── 4. Rehost per-node configs ─────────────────────────────────────────────
# Path remap (host -> container):
#   $HOST_BASE_DIR/genesis.json              -> /gravity/config/genesis.json
#   $HOST_BASE_DIR/<node>/config/*           -> /gravity/config/*
#   $HOST_BASE_DIR/<node>/data/*             -> /gravity/data/*
#   $HOST_BASE_DIR/<node>/consensus_log      -> /gravity/data/consensus_log
#   $HOST_BASE_DIR/<node>/execution_logs     -> /gravity/data/execution_logs
rm -rf "$OUT_CONFIG_DIR"
mkdir -p "$OUT_CONFIG_DIR"

for node in "${NODES[@]}"; do
    src="$HOST_BASE_DIR/$node/config"
    dst="$OUT_CONFIG_DIR/$node"
    [[ -d "$src" ]] || die "rendered config missing: $src"

    mkdir -p "$dst"
    find "$src" -maxdepth 1 -type f -exec cp {} "$dst/" \;
    cp -f "$HOST_BASE_DIR/genesis.json" "$dst/genesis.json"

    # Order matters: rewrite longer prefixes first so /data doesn't clobber
    # /consensus_log / /execution_logs.
    sed -i \
        -e "s|$HOST_BASE_DIR/genesis.json|/gravity/config/genesis.json|g" \
        -e "s|$HOST_BASE_DIR/$node/consensus_log|/gravity/data/consensus_log|g" \
        -e "s|$HOST_BASE_DIR/$node/execution_logs|/gravity/data/execution_logs|g" \
        -e "s|$HOST_BASE_DIR/$node/config|/gravity/config|g" \
        -e "s|$HOST_BASE_DIR/$node/data|/gravity/data|g" \
        "$dst"/*.json "$dst"/*.yaml 2>/dev/null || true

    # identity.yaml has private keys; container uid 10001 != host uid, so
    # it must be world-readable. Devnet only — do NOT reuse for mainnet.
    chmod 644 "$dst"/*.yaml 2>/dev/null || true
done

log "rendered $OUT_CONFIG_DIR (${#NODES[@]} nodes, node1 rpc=$expected_rpc)"
