#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
SUFFIX="${BFT_BYZANTINE_TEST_SUFFIX:-$$}"
IMAGE="gravity-node-byzantine-fixture:${SUFFIX}"
NORMAL_IMAGE="gravity-node-byzantine-normal:${SUFFIX}"
CONTAINER="gravity-byzantine-fixture-${SUFFIX}"

cleanup() {
    docker rm -f "${CONTAINER}" >/dev/null 2>&1 || true
    if [[ "${BFT_BYZANTINE_TEST_KEEP_IMAGES:-0}" != "1" ]]; then
        docker image rm -f "${IMAGE}" "${NORMAL_IMAGE}" >/dev/null 2>&1 || true
    fi
}
trap cleanup EXIT

fail() {
    echo "Byzantine fixture test: $*" >&2
    exit 1
}

assert_json() {
    local document="$1"
    local expression="$2"

    jq -e "${expression}" <<<"${document}" >/dev/null \
        || fail "JSON assertion failed: ${expression}; document=${document}"
}

build_args=(
    --build-arg HOST_BINARY=docker/gravity_node/tests/byzantine-fixture/fixture-gravity-node.sh
    --build-arg HOST_CLI_BINARY=docker/gravity_node/tests/storage-fixture/fixture-gravity-cli.sh
    -f docker/gravity_node/Dockerfile
)

docker build --load "${build_args[@]}" \
    --target runtime-host-binary-byzantine-test \
    -t "${IMAGE}" "${REPO_ROOT}"
docker build --load "${build_args[@]}" \
    --target runtime-host-binary \
    -t "${NORMAL_IMAGE}" "${REPO_ROOT}"

docker run --rm --entrypoint /bin/sh "${NORMAL_IMAGE}" \
    -c 'test ! -e /usr/local/bin/bft-byzantine' \
    || fail "normal runtime unexpectedly contains the Byzantine hook"

if docker run --rm --user 0 --entrypoint /usr/local/bin/bft-byzantine \
    "${IMAGE}" read disabled-fixture >/dev/null 2>&1; then
    fail "Byzantine hook ran without explicit runtime authorization"
fi

docker run -d \
    --name "${CONTAINER}" \
    --entrypoint /usr/local/bin/gravity_node \
    -e BFT_BYZANTINE_FIXTURE_ENABLED=1 \
    "${IMAGE}" node >/dev/null

baseline="$(docker exec -u 0 "${CONTAINER}" /usr/local/bin/bft-byzantine \
    read equivocation-1)"
assert_json "${baseline}" \
    '(.active | not) and .capabilities == ["equivocation"] and (.protocolEffect.observed | not)'

if docker exec -u 0 "${CONTAINER}" /usr/local/bin/bft-byzantine \
    inject double-sign unsupported-1 validator-1 >/dev/null 2>&1; then
    fail "hook accepted an unsupported double-sign behavior"
fi

docker exec -u 0 "${CONTAINER}" /usr/local/bin/bft-byzantine \
    inject equivocation equivocation-1 validator-1

attempts=100
while (( attempts > 0 )); do
    active="$(docker exec -u 0 "${CONTAINER}" /usr/local/bin/bft-byzantine \
        read equivocation-1)"
    if jq -e '
        .active and
        .protocolEffect.observed and
        .protocolEffect.eventCount >= 1 and
        .protocolEffect.distinctMessageCount >= 2 and
        .protocolEffect.recipientGroupCount >= 2
      ' <<<"${active}" >/dev/null; then
        break
    fi
    attempts=$((attempts - 1))
    sleep 0.1
done
(( attempts > 0 )) || fail "fixture node did not publish protocol-effect evidence"

docker exec -u 0 "${CONTAINER}" /usr/local/bin/bft-byzantine \
    heal equivocation-1
healed="$(docker exec -u 0 "${CONTAINER}" /usr/local/bin/bft-byzantine \
    read equivocation-1)"
assert_json "${healed}" \
    '(.active | not) and .protocolEffect.observed and .protocolEffect.distinctMessageCount == 2'

echo "Byzantine fixture test: injection, evidence, and recovery passed"
