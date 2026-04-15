#!/usr/bin/env bash
# Standalone manual verification for the distinct_roles e2e fixture.
# Assumes a single-node cluster from cluster_test_cases/distinct_roles/ is
# already running (e.g. after `python3 runner.py --case distinct_roles`) and
# the RPC is reachable at $RPC_URL.
#
# This is a minimal sanity script. The authoritative tests live in
# test_distinct_roles.py; prefer those for CI.

set -euo pipefail

RPC_URL="${RPC_URL:-http://127.0.0.1:8550}"

STAKING_ADDR="0x00000000000000000000000000000001625f2000"
VALIDATOR_MANAGER_ADDR="0x00000000000000000000000000000001625f2001"

# anvil well-known test accounts
FAUCET_KEY="0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
FAUCET_ADDR="0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
OPERATOR_KEY="0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d"
OPERATOR_ADDR="0x70997970C51812dc3A010C7d01b50e0d17dc79C8"
OWNER_KEY="0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a"
OWNER_ADDR="0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC"
STAKER_KEY="0x7c852118294e51e653712a81e05800f419141751be58f605c371e15141b007a6"
STAKER_ADDR="0x90F79bf6EB2c4f870365E785982E1f101E93b906"

pass=0
fail=0

report() {
    local label="$1" ok="$2"
    if [[ "$ok" == "1" ]]; then
        echo "  [PASS] $label"
        pass=$((pass + 1))
    else
        echo "  [FAIL] $label"
        fail=$((fail + 1))
    fi
}

require_cast() {
    if ! command -v cast >/dev/null; then
        echo "cast (foundry) is required. Install foundry first." >&2
        exit 2
    fi
}

main() {
    require_cast

    echo "[distinct_roles] RPC = $RPC_URL"
    echo "[distinct_roles] Block number: $(cast block-number --rpc-url "$RPC_URL")"

    echo "[distinct_roles] Looking up genesis StakePool (index 0)..."
    local pool
    pool=$(cast call --rpc-url "$RPC_URL" "$STAKING_ADDR" "getPool(uint256)(address)" 0)
    echo "  pool = $pool"

    local on_chain_op on_chain_staker
    on_chain_op=$(cast call --rpc-url "$RPC_URL" "$pool" "getOperator()(address)")
    on_chain_staker=$(cast call --rpc-url "$RPC_URL" "$pool" "getStaker()(address)")
    echo "  operator on chain = $on_chain_op"
    echo "  staker   on chain = $on_chain_staker"

    [[ "${on_chain_op,,}" == "${OPERATOR_ADDR,,}" ]] \
        && report "genesis operator == anvil[1]" 1 \
        || report "genesis operator == anvil[1]" 0
    [[ "${on_chain_staker,,}" == "${STAKER_ADDR,,}" ]] \
        && report "genesis staker   == anvil[3]" 1 \
        || report "genesis staker   == anvil[3]" 0

    echo "[distinct_roles] Fanning out 2 ETH to operator/owner/staker from faucet..."
    for target in "$OPERATOR_ADDR" "$OWNER_ADDR" "$STAKER_ADDR"; do
        cast send --rpc-url "$RPC_URL" --private-key "$FAUCET_KEY" \
            --value 2ether "$target" >/dev/null
    done

    echo "[distinct_roles] setFeeRecipient gating..."
    local recipient="0x000000000000000000000000000000000000c0de"
    if cast send --rpc-url "$RPC_URL" --private-key "$OWNER_KEY" \
        "$VALIDATOR_MANAGER_ADDR" "setFeeRecipient(address,address)" "$pool" "$recipient" \
        >/dev/null 2>&1; then
        report "owner.setFeeRecipient rejected" 0
    else
        report "owner.setFeeRecipient rejected" 1
    fi
    if cast send --rpc-url "$RPC_URL" --private-key "$STAKER_KEY" \
        "$VALIDATOR_MANAGER_ADDR" "setFeeRecipient(address,address)" "$pool" "$recipient" \
        >/dev/null 2>&1; then
        report "staker.setFeeRecipient rejected" 0
    else
        report "staker.setFeeRecipient rejected" 1
    fi
    if cast send --rpc-url "$RPC_URL" --private-key "$OPERATOR_KEY" \
        "$VALIDATOR_MANAGER_ADDR" "setFeeRecipient(address,address)" "$pool" "$recipient" \
        >/dev/null 2>&1; then
        report "operator.setFeeRecipient accepted" 1
    else
        report "operator.setFeeRecipient accepted" 0
    fi

    echo "[distinct_roles] addStake gating..."
    if cast send --rpc-url "$RPC_URL" --private-key "$OPERATOR_KEY" \
        --value 1ether "$pool" "addStake()" >/dev/null 2>&1; then
        report "operator.addStake rejected" 0
    else
        report "operator.addStake rejected" 1
    fi
    if cast send --rpc-url "$RPC_URL" --private-key "$OWNER_KEY" \
        --value 1ether "$pool" "addStake()" >/dev/null 2>&1; then
        report "owner.addStake rejected" 0
    else
        report "owner.addStake rejected" 1
    fi
    if cast send --rpc-url "$RPC_URL" --private-key "$STAKER_KEY" \
        --value 1ether "$pool" "addStake()" >/dev/null 2>&1; then
        report "staker.addStake accepted" 1
    else
        report "staker.addStake accepted" 0
    fi

    echo "[distinct_roles] setStaker gating (Ownable)..."
    local rot_staker="0x000000000000000000000000000000000000beef"
    if cast send --rpc-url "$RPC_URL" --private-key "$STAKER_KEY" \
        "$pool" "setStaker(address)" "$rot_staker" >/dev/null 2>&1; then
        report "staker.setStaker rejected" 0
    else
        report "staker.setStaker rejected" 1
    fi
    if cast send --rpc-url "$RPC_URL" --private-key "$OPERATOR_KEY" \
        "$pool" "setStaker(address)" "$rot_staker" >/dev/null 2>&1; then
        report "operator.setStaker rejected" 0
    else
        report "operator.setStaker rejected" 1
    fi
    if cast send --rpc-url "$RPC_URL" --private-key "$OWNER_KEY" \
        "$pool" "setStaker(address)" "$rot_staker" >/dev/null 2>&1; then
        report "owner.setStaker accepted" 1
    else
        report "owner.setStaker accepted" 0
    fi

    echo
    echo "[distinct_roles] $pass passed, $fail failed"
    [[ "$fail" == "0" ]]
}

main "$@"
