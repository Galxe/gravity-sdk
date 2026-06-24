#!/usr/bin/env bash
# Render a Docker bridge-network devnet config that supports real network
# partition tests via `docker network disconnect`.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
SOURCE_OUT="${SOURCE_OUT:-$REPO_ROOT/cluster/output}"
BRIDGE_OUT="${BRIDGE_OUT:-$SCRIPT_DIR/cluster-output-bridge}"
BRIDGE_CONFIG_OUT="${BRIDGE_CONFIG_OUT:-$SCRIPT_DIR/config-bridge}"
BRIDGE_GENESIS_TOML="${BRIDGE_GENESIS_TOML:-$SCRIPT_DIR/genesis.bridge.toml}"

for node_id in node1 node2 node3 node4 vfn1; do
    src_dir="$SOURCE_OUT/$node_id/config"
    dst_dir="$BRIDGE_OUT/$node_id/config"
    if [[ ! -f "$src_dir/identity.yaml" || ! -f "$src_dir/identity.public.yaml" ]]; then
        echo "Missing identity files for $node_id under $src_dir." >&2
        echo "Run 'cd $REPO_ROOT/cluster && make init' first." >&2
        exit 1
    fi
    mkdir -p "$dst_dir"
    cp "$src_dir/identity.yaml" "$dst_dir/identity.yaml"
    cp "$src_dir/identity.public.yaml" "$dst_dir/identity.public.yaml"
done

# Keep the source genesis.toml authoritative, but replace validator advertised
# hosts with Docker service DNS names. These names resolve only on the bridge
# network declared by docker-compose.cluster-bridge.yaml.
awk '
    /^\[\[genesis_validators\]\]/ {
        in_validator = 1
        current_id = ""
        print
        next
    }
    in_validator && $1 == "id" && $2 == "=" {
        current_id = $3
        gsub(/"/, "", current_id)
        print
        next
    }
    in_validator && $1 == "host" && $2 == "=" && current_id ~ /^node[1-4]$/ {
        print "host = \"" current_id "\""
        next
    }
    { print }
' "$REPO_ROOT/cluster/genesis.toml" > "$BRIDGE_GENESIS_TOML"

if [[ "${BRIDGE_REGENESIS:-1}" = "1" || ! -f "$BRIDGE_OUT/genesis.json" || ! -f "$BRIDGE_OUT/waypoint.txt" ]]; then
    echo "generating bridge genesis artifacts in $BRIDGE_OUT..."
    GRAVITY_ARTIFACTS_DIR="$BRIDGE_OUT" \
        bash "$REPO_ROOT/cluster/genesis.sh" "$BRIDGE_GENESIS_TOML"
else
    echo "reusing existing bridge genesis artifacts in $BRIDGE_OUT"
fi

echo "rendering bridge container configs in $BRIDGE_CONFIG_OUT..."
CLUSTER_OUT="$BRIDGE_OUT" OUT="$BRIDGE_CONFIG_OUT" \
    "$SCRIPT_DIR/render-cluster-config.sh"

echo "done."
echo "  compose: docker-compose.cluster-bridge.yaml"
echo "  config:  $BRIDGE_CONFIG_OUT"
echo "  genesis: $BRIDGE_OUT/genesis.json"
