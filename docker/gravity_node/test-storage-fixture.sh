#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
SUFFIX="${BFT_STORAGE_TEST_SUFFIX:-$$}"
IMAGE="gravity-node-storage-fixture:${SUFFIX}"
NORMAL_IMAGE="gravity-node-storage-normal:${SUFFIX}"
CONTAINER="gravity-storage-fixture-${SUFFIX}"
VOLUME="gravity-storage-fixture-${SUFFIX}"
CONFIG_DIR=""

cleanup() {
    docker rm -f "${CONTAINER}" >/dev/null 2>&1 || true
    docker volume rm "${VOLUME}" >/dev/null 2>&1 || true
    if [[ -n "${CONFIG_DIR}" ]]; then
        rm -rf -- "${CONFIG_DIR}"
    fi
    if [[ "${BFT_STORAGE_TEST_KEEP_IMAGES:-0}" != "1" ]]; then
        docker image rm -f "${IMAGE}" "${NORMAL_IMAGE}" >/dev/null 2>&1 || true
    fi
}
trap cleanup EXIT

fail() {
    echo "storage fixture test: $*" >&2
    exit 1
}

wait_for_node() {
    local attempts=100

    while (( attempts > 0 )); do
        if docker exec -u 0 "${CONTAINER}" sh -c \
            'test -s /run/bft-node/node.pid && test -s /gravity/data/fixture-node.heartbeat' \
            >/dev/null 2>&1; then
            return
        fi
        attempts=$((attempts - 1))
        sleep 0.2
    done
    fail "fixture node did not become ready"
}

wait_for_fault_exit() {
    local attempts=50

    while (( attempts > 0 )); do
        if docker exec -u 0 "${CONTAINER}" test ! -e /run/bft-node/node.pid \
            >/dev/null 2>&1; then
            return
        fi
        attempts=$((attempts - 1))
        sleep 0.2
    done
    fail "fixture node did not exit after storage truncation"
}

assert_json() {
    local document="$1"
    local expression="$2"

    jq -e "${expression}" <<<"${document}" >/dev/null \
        || fail "JSON assertion failed: ${expression}; document=${document}"
}

build_args=(
    --build-arg HOST_BINARY=docker/gravity_node/tests/storage-fixture/fixture-gravity-node.sh
    --build-arg HOST_CLI_BINARY=docker/gravity_node/tests/storage-fixture/fixture-gravity-cli.sh
    -f docker/gravity_node/Dockerfile
)

CONFIG_DIR="$(mktemp -d "${SCRIPT_DIR}/tests/storage-fixture/config.XXXXXX")"
cat > "${CONFIG_DIR}/reth_config.json" <<'EOF'
{
  "reth_args": {},
  "env_vars": {}
}
EOF

docker build --load "${build_args[@]}" \
    --target runtime-host-binary-storage-test \
    -t "${IMAGE}" "${REPO_ROOT}"
docker build --load "${build_args[@]}" \
    --target runtime-host-binary \
    -t "${NORMAL_IMAGE}" "${REPO_ROOT}"

docker run --rm --entrypoint /bin/sh "${NORMAL_IMAGE}" \
    -c 'test ! -e /usr/local/bin/bft-storage' \
    || fail "normal runtime unexpectedly contains the destructive hook"

if docker run --rm --entrypoint /usr/local/bin/bft-storage "${IMAGE}" \
    read disabled-fixture >/dev/null 2>&1; then
    fail "storage hook ran without explicit disposable-data authorization"
fi

docker volume create "${VOLUME}" >/dev/null
docker run -d \
    --name "${CONTAINER}" \
    --mount "type=volume,src=${VOLUME},dst=/gravity/data" \
    --mount "type=bind,src=${CONFIG_DIR},dst=/gravity/config,readonly" \
    -e BFT_STORAGE_FIXTURE_ENABLED=1 \
    -e BFT_STORAGE_DISPOSABLE_DATA=I_UNDERSTAND_THIS_DATA_WILL_BE_DESTROYED \
    -e BFT_STORAGE_WAL_PATH=/gravity/data/data/consensus_db \
    -e BFT_STORAGE_DATABASE_PATH=/gravity/data/data/reth/db \
    -e BFT_STORAGE_DATABASE_MUTATION_FILE=state/CURRENT \
    -e BFT_STORAGE_BACKUP_RESERVE_MIB=1 \
    -e BFT_STORAGE_INJECT_START_TIMEOUT_SECONDS=2 \
    -e BFT_STORAGE_INJECT_STABLE_SECONDS=1 \
    -e BFT_STORAGE_HEAL_START_TIMEOUT_SECONDS=10 \
    -e BFT_STORAGE_HEAL_STABLE_SECONDS=1 \
    "${IMAGE}" >/dev/null

wait_for_node

baseline="$(docker exec -u 0 "${CONTAINER}" /usr/local/bin/bft-storage \
    read wal-1 wal validator-1)"
assert_json "${baseline}" '.active == false and .restored == false'

wal_before="$(docker exec -u 0 "${CONTAINER}" sha256sum \
    /gravity/data/data/consensus_db/000001.log | awk '{print $1}')"
wal_active="$(docker exec -u 0 "${CONTAINER}" /usr/local/bin/bft-storage \
    inject wal wal-1 validator-1)"
assert_json "${wal_active}" \
    '.active and .backupReady and .mutationApplied and (.restored | not)'
wal_read="$(docker exec -u 0 "${CONTAINER}" /usr/local/bin/bft-storage \
    read wal-1 wal validator-1)"
assert_json "${wal_read}" \
    '.active and .backupReady and .mutationApplied and (.restored | not)'
docker exec -u 0 "${CONTAINER}" test ! -s \
    /gravity/data/data/consensus_db/000001.log
wait_for_fault_exit
docker inspect --format '{{.State.Running}}' "${CONTAINER}" | grep -qx true \
    || fail "container exited with the corrupted fixture node"

# A whole-container restart loses /run but must retain the volume-backed fault
# marker. The replacement supervisor stays available for read/heal without
# entering a restart storm against the corrupted node.
docker restart "${CONTAINER}" >/dev/null
wait_for_fault_exit
docker exec -u 0 "${CONTAINER}" test -e /gravity/data/.bft-storage-active
wal_after_restart="$(docker exec -u 0 "${CONTAINER}" /usr/local/bin/bft-storage \
    read wal-1 wal validator-1)"
assert_json "${wal_after_restart}" \
    '.active and .backupReady and .mutationApplied and (.restored | not)'

wal_healed="$(docker exec -u 0 "${CONTAINER}" /usr/local/bin/bft-storage \
    heal wal-1)"
assert_json "${wal_healed}" \
    '(.active | not) and .backupReady and .mutationApplied and .restored and .nodeRunning'
wait_for_node
wal_after="$(docker exec -u 0 "${CONTAINER}" sha256sum \
    /gravity/data/data/consensus_db/000001.log | awk '{print $1}')"
[[ "${wal_after}" == "${wal_before}" ]] || fail "WAL bytes were not restored"

wal_healed_again="$(docker exec -u 0 "${CONTAINER}" /usr/local/bin/bft-storage \
    heal wal-1)"
assert_json "${wal_healed_again}" '(.active | not) and .restored'

database_before="$(docker exec -u 0 "${CONTAINER}" sha256sum \
    /gravity/data/data/reth/db/state/CURRENT | awk '{print $1}')"
database_active="$(docker exec -u 0 "${CONTAINER}" /usr/local/bin/bft-storage \
    inject database database-1 pfn-1)"
assert_json "${database_active}" \
    '.active and .backupReady and .mutationApplied and (.restored | not)'
database_read="$(docker exec -u 0 "${CONTAINER}" /usr/local/bin/bft-storage \
    read database-1 database pfn-1)"
assert_json "${database_read}" \
    '.active and .backupReady and .mutationApplied and (.restored | not)'
docker exec -u 0 "${CONTAINER}" test ! -s \
    /gravity/data/data/reth/db/state/CURRENT
wait_for_fault_exit

database_healed="$(docker exec -u 0 "${CONTAINER}" /usr/local/bin/bft-storage \
    heal database-1)"
assert_json "${database_healed}" \
    '(.active | not) and .backupReady and .mutationApplied and .restored and .nodeRunning'
wait_for_node
database_after="$(docker exec -u 0 "${CONTAINER}" sha256sum \
    /gravity/data/data/reth/db/state/CURRENT | awk '{print $1}')"
[[ "${database_after}" == "${database_before}" ]] \
    || fail "database bytes were not restored"

docker exec -u 0 "${CONTAINER}" test ! -d \
    /gravity/data/.bft-storage/backups/database-1

echo "storage fixture test: WAL and database injection/recovery passed"
