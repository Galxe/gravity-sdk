#!/usr/bin/env bash
set -euo pipefail

WAL_DIR="${BFT_STORAGE_WAL_PATH:-/gravity/data/data/consensus_db}"
DATABASE_DIR="${BFT_STORAGE_DATABASE_PATH:-/gravity/data/data/reth/db}"
WAL_FILE="${WAL_DIR}/000001.log"
DATABASE_FILE="${DATABASE_DIR}/state/CURRENT"
HEARTBEAT="${BFT_FIXTURE_HEARTBEAT:-/gravity/data/fixture-node.heartbeat}"

mkdir -p "${WAL_DIR}" "$(dirname "${DATABASE_FILE}")"
if [[ ! -e "${WAL_FILE}" ]]; then
    printf 'fixture-wal-original-bytes\n' > "${WAL_FILE}"
fi
if [[ ! -e "${DATABASE_FILE}" ]]; then
    printf 'fixture-database-original-bytes\n' > "${DATABASE_FILE}"
fi

if [[ ! -s "${WAL_FILE}" || ! -s "${DATABASE_FILE}" ]]; then
    echo "fixture-gravity-node: refusing to start with truncated storage" >&2
    exit 42
fi

shutdown=0
trap 'shutdown=1' TERM INT

while (( shutdown == 0 )); do
    heartbeat_tmp="${HEARTBEAT}.$$"
    printf '%s\n' "$$" > "${heartbeat_tmp}"
    mv -f "${heartbeat_tmp}" "${HEARTBEAT}"
    sleep 0.2
done

rm -f "${HEARTBEAT}"
