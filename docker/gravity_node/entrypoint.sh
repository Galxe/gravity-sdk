#!/usr/bin/env bash
set -euo pipefail

# Reads reth_config.json (same schema as cluster/templates/reth_config.json.tpl)
# and execs gravity_node in the foreground so docker manages its lifecycle.
#
# Expected layout (all paths inside container):
#   ${GRAVITY_CONFIG_DIR}/reth_config.json   — required
#   ${GRAVITY_CONFIG_DIR}/validator.yaml     — referenced by reth_config.json
#   ${GRAVITY_CONFIG_DIR}/identity.yaml      — referenced by validator.yaml
#   ${GRAVITY_CONFIG_DIR}/waypoint.txt       — referenced by validator.yaml
#   ${GRAVITY_CONFIG_DIR}/genesis.json       — referenced by reth_config.json (chain)
#   ${GRAVITY_CONFIG_DIR}/relayer_config.json (optional)

CONFIG_DIR="${GRAVITY_CONFIG_DIR:-/gravity/config}"
DATA_DIR="${GRAVITY_DATA_DIR:-/gravity/data}"
RETH_CONFIG="${CONFIG_DIR}/reth_config.json"

# cluster-style config paths assume these subdirs exist under $DATA_DIR.
mkdir -p "${DATA_DIR}/data" "${DATA_DIR}/consensus_log" "${DATA_DIR}/execution_logs"

if [[ "${1:-}" != "node" ]]; then
    exec /usr/local/bin/gravity_node "$@"
fi

if [[ ! -f "${RETH_CONFIG}" ]]; then
    echo "entrypoint: ${RETH_CONFIG} not found; mount config dir at ${CONFIG_DIR}" >&2
    exit 1
fi

reth_args=()
while IFS= read -r key && IFS= read -r value; do
    if [[ -z "${value}" || "${value}" == "null" ]]; then
        reth_args+=( "--${key}" )
    else
        reth_args+=( "--${key}=${value}" )
    fi
done < <(jq -r '.reth_args | to_entries[] | .key, .value' "${RETH_CONFIG}")

while IFS= read -r key && IFS= read -r value; do
    if [[ -n "${value}" && "${value}" != "null" ]]; then
        export "${key}=${value}"
    fi
done < <(jq -r '.env_vars // {} | to_entries[] | .key, .value' "${RETH_CONFIG}")

exec /usr/local/bin/gravity_node node "${reth_args[@]}"
