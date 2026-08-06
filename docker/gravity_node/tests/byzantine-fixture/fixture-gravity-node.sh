#!/usr/bin/env bash
set -euo pipefail

CONTROL_PATH="${BFT_BYZANTINE_CONTROL_PATH:-/run/bft-node/byzantine-control.json}"
EVIDENCE_PATH="${BFT_BYZANTINE_EVIDENCE_PATH:-/run/bft-node/byzantine-evidence.json}"
shutdown=0
last_fault=""

trap 'shutdown=1' TERM INT

while (( shutdown == 0 )); do
    if [[ -f "${CONTROL_PATH}" ]] \
        && [[ "$(jq -r '.active // false' "${CONTROL_PATH}")" == "true" ]]; then
        fault_id="$(jq -r '.faultId' "${CONTROL_PATH}")"
        if [[ "${fault_id}" != "${last_fault}" ]]; then
            node_id="$(jq -r '.nodeId' "${CONTROL_PATH}")"
            temporary="${EVIDENCE_PATH}.$$"
            jq -cn \
                --arg faultId "${fault_id}" \
                --arg nodeId "${node_id}" \
                '{schemaVersion: 1,
                  faultId: $faultId,
                  behavior: "equivocation",
                  nodeId: $nodeId,
                  protocolEffect: {
                    observed: true,
                    behavior: "equivocation",
                    eventCount: 1,
                    epoch: 7,
                    round: 11,
                    distinctMessageCount: 2,
                    recipientGroupCount: 2,
                    firstMessageId: "0x01",
                    secondMessageId: "0x02",
                    firstRecipientCount: 2,
                    secondRecipientCount: 2
                  }}' > "${temporary}"
            mv -f -- "${temporary}" "${EVIDENCE_PATH}"
            last_fault="${fault_id}"
        fi
    fi
    sleep 0.1
done
