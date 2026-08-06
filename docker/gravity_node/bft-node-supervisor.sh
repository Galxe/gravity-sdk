#!/usr/bin/env bash
set -uo pipefail

ENTRYPOINT="${BFT_NODE_ENTRYPOINT:-/usr/local/bin/entrypoint.sh}"
RUN_DIR="${BFT_NODE_SUPERVISOR_DIR:-/run/bft-node}"
NODE_PID_FILE="${RUN_DIR}/node.pid"
SUPERVISOR_PID_FILE="${RUN_DIR}/supervisor.pid"
MAINTENANCE_FILE="${RUN_DIR}/maintenance"
FAULT_FILE="${RUN_DIR}/storage-fault"
DATA_ROOT="${BFT_STORAGE_DATA_ROOT:-/gravity/data}"
PERSISTENT_FAULT_FILE="${DATA_ROOT}/.bft-storage-active"
RESTART_DELAY="${BFT_NODE_RESTART_DELAY_SECONDS:-1}"

if [[ "${1:-}" != "node" ]]; then
    exec "${ENTRYPOINT}" "$@"
fi

if [[ ! "${RESTART_DELAY}" =~ ^[0-9]+([.][0-9]+)?$ ]]; then
    echo "bft-node-supervisor: invalid restart delay: ${RESTART_DELAY}" >&2
    exit 2
fi

mkdir -p "${RUN_DIR}"
umask 0027
printf '%s\n' "$$" > "${SUPERVISOR_PID_FILE}"
rm -f "${NODE_PID_FILE}"

child_pid=""
shutting_down=0

remove_pid_file() {
    local recorded=""

    if [[ -f "${NODE_PID_FILE}" ]]; then
        recorded="$(<"${NODE_PID_FILE}")"
    fi
    if [[ -z "${child_pid}" || "${recorded}" == "${child_pid}" ]]; then
        rm -f "${NODE_PID_FILE}"
    fi
}

request_shutdown() {
    shutting_down=1
    if [[ "${child_pid}" =~ ^[0-9]+$ ]] && kill -0 "${child_pid}" 2>/dev/null; then
        kill -TERM "${child_pid}" 2>/dev/null || true
    fi
}

cleanup() {
    remove_pid_file
    rm -f "${SUPERVISOR_PID_FILE}"
}

trap request_shutdown TERM INT
trap cleanup EXIT

if [[ -e "${PERSISTENT_FAULT_FILE}" ]]; then
    echo "bft-node-supervisor: persistent storage fault found; waiting for heal" >&2
    while [[ -e "${PERSISTENT_FAULT_FILE}" ]] && (( shutting_down == 0 )); do
        sleep 0.2
    done
fi

while (( shutting_down == 0 )); do
    while [[ -e "${MAINTENANCE_FILE}" ]] && (( shutting_down == 0 )); do
        sleep 0.2
    done
    (( shutting_down == 0 )) || break

    "${ENTRYPOINT}" "$@" &
    child_pid=$!
    pid_tmp="${NODE_PID_FILE}.$$"
    printf '%s\n' "${child_pid}" > "${pid_tmp}"
    mv -f "${pid_tmp}" "${NODE_PID_FILE}"

    wait "${child_pid}"
    child_status=$?

    if (( shutting_down != 0 )) && kill -0 "${child_pid}" 2>/dev/null; then
        wait "${child_pid}" 2>/dev/null || true
    fi
    remove_pid_file
    child_pid=""

    (( shutting_down == 0 )) || break

    if [[ -e "${FAULT_FILE}" || -e "${PERSISTENT_FAULT_FILE}" ]]; then
        echo "bft-node-supervisor: node exited during an active storage fault; waiting for heal" >&2
        while [[ -e "${FAULT_FILE}" || -e "${PERSISTENT_FAULT_FILE}" ]] \
              && (( shutting_down == 0 )); do
            sleep 0.2
        done
        continue
    fi

    if [[ ! -e "${MAINTENANCE_FILE}" ]]; then
        echo "bft-node-supervisor: node exited with status ${child_status}; restarting" >&2
        sleep "${RESTART_DELAY}"
    fi
done

exit 0
