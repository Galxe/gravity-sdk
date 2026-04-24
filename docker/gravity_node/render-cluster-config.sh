#!/usr/bin/env bash
# Render per-node configs into ./config/{node1..node4,vfn1}/ using cluster/templates.
# Container-internal paths (/gravity/config, /gravity/data) are baked in; host ports
# come from cluster.toml.example defaults.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
CLUSTER_OUT="$REPO_ROOT/cluster/output"
TEMPLATES="$REPO_ROOT/cluster/templates"
OUT="$SCRIPT_DIR/config"

if [[ ! -f "$CLUSTER_OUT/genesis.json" || ! -f "$CLUSTER_OUT/waypoint.txt" ]]; then
    echo "Missing cluster output. Run 'cd $REPO_ROOT/cluster && make init && make genesis' first." >&2
    exit 1
fi

rm -rf "$OUT"
mkdir -p "$OUT"

# All paths inside the container.
export DATA_DIR=/gravity/data
export CONFIG_DIR=/gravity/config
export GENESIS_PATH=/gravity/config/genesis.json
export RELAYER_RPC_URL="${RELAYER_RPC_URL:-https://sepolia.drpc.org}"

# HARDCODED devnet defaults (open CORS, full API incl. debug namespace).
# Unlike cluster/deploy.sh, this script does NOT read cluster.toml [rpc];
# do NOT reuse for mainnet without wiring that through first — otherwise
# debug_* endpoints are exposed to any origin.
# TODO: factor into a shared snippet sourced by both scripts.
export RPC_HTTP_CORSDOMAIN="${RPC_HTTP_CORSDOMAIN:-*}"
export RPC_HTTP_API="${RPC_HTTP_API:-debug,eth,net,trace,txpool,web3,rpc}"

render_validator() {
    local node_id="$1"
    local dir="$OUT/$node_id"
    mkdir -p "$dir"

    cp "$CLUSTER_OUT/genesis.json"                "$dir/genesis.json"
    cp "$CLUSTER_OUT/waypoint.txt"                "$dir/waypoint.txt"
    cp "$CLUSTER_OUT/$node_id/config/identity.yaml" "$dir/identity.yaml"

    envsubst < "$TEMPLATES/validator.yaml.tpl"     > "$dir/validator.yaml"
    envsubst < "$TEMPLATES/reth_config.json.tpl"   > "$dir/reth_config.json"
    envsubst < "$TEMPLATES/relayer_config.json.tpl" > "$dir/relayer_config.json"

    # identity.yaml has private keys. Left world-readable for the devnet
    # topology test (host uid 1000 != container uid 10001). In mainnet,
    # chown to the container uid and chmod 600 instead.
    chmod 644 "$dir/identity.yaml"

    echo "  rendered $node_id (p2p=$P2P_PORT rpc=$RPC_PORT)"
}

render_vfn() {
    local node_id="$1"
    local dir="$OUT/$node_id"
    mkdir -p "$dir"

    cp "$CLUSTER_OUT/genesis.json"                "$dir/genesis.json"
    cp "$CLUSTER_OUT/waypoint.txt"                "$dir/waypoint.txt"
    cp "$CLUSTER_OUT/$node_id/config/identity.yaml" "$dir/identity.yaml"

    envsubst < "$TEMPLATES/validator_full_node.yaml.tpl" > "$dir/validator_full_node.yaml"
    envsubst < "$TEMPLATES/reth_config_vfn.json.tpl"     > "$dir/reth_config.json"
    envsubst < "$TEMPLATES/relayer_config.json.tpl"      > "$dir/relayer_config.json"

    chmod 644 "$dir/identity.yaml"

    echo "  rendered $node_id (vfn=$VFN_PORT rpc=$RPC_PORT)"
}

# Port allocation matches cluster/cluster.toml.example.
set_ports() {
    export HOST=127.0.0.1
    export NODE_ID="$1"
    export P2P_PORT="$2"
    export VFN_PORT="$3"
    export RPC_PORT="$4"
    export METRICS_PORT="$5"
    export INSPECTION_PORT="$6"
    export HTTPS_PORT="$7"
    export AUTHRPC_PORT="$8"
    export P2P_PORT_RETH="$9"
}

echo "rendering 4 validators + 1 vfn to $OUT..."

set_ports node1 6180 6190 8545 9001 10000 1024 8551 12024; render_validator node1
set_ports node2 6181 6191 8546 9002 10001 1025 8552 12025; render_validator node2
set_ports node3 6182 6192 8547 9003 10002 1026 8553 12026; render_validator node3
set_ports node4 6183 6193 8548 9004 10003 1027 8554 12027; render_validator node4

# vfn has no P2P_PORT (not in validator set).
set_ports vfn1 "" 6195 8550 9006 10005 1029 8566 12029; render_vfn vfn1

echo "done."
