"""
Governance helpers for validator-set e2e cases (checklist H4).

Extracted from cluster_test_cases/fuzzy_cluster/test_epoch_switch.py so
storage_v2_fresh_sync (TC9) can reuse the one-time
"enable permissionless validator join" proposal lifecycle:
addExecutor -> createProposal -> vote -> resolve -> execute. Idempotent —
the flag is checked first, so a case can call it unconditionally.

Preconditions (the fuzzy_cluster genesis recipe):
- pool[0]'s voter is the faucet (genesis_validators[0].address == faucet
  address), providing proposer stake and voting power;
- governance_owner == faucet (addExecutor authority);
- short voting_duration_micros so a single vote resolves quickly.
"""

from __future__ import annotations

import asyncio
import logging
from typing import Optional

from eth_abi import encode
from eth_account import Account
from web3 import Web3

LOG = logging.getLogger(__name__)

# Genesis system contracts (stable addresses, chainspec gravity.rs).
GOVERNANCE = Web3.to_checksum_address("0x00000000000000000000000000000001625F3000")
STAKING = Web3.to_checksum_address("0x00000000000000000000000000000001625F2000")
VALIDATOR_MANAGER = Web3.to_checksum_address(
    "0x00000000000000000000000000000001625F2001"
)

MAX_UINT128 = (1 << 128) - 1
PROPOSAL_STATE_SUCCEEDED = 1

PROPOSAL_CREATED_TOPIC = Web3.keccak(
    text="ProposalCreated(uint64,address,address,bytes32,string)"
)


def _selector(sig: str) -> bytes:
    return Web3.keccak(text=sig)[:4]


SEL_ADD_EXECUTOR = _selector("addExecutor(address)")
SEL_CREATE_PROPOSAL = _selector("createProposal(address,address[],bytes[],string)")
SEL_VOTE = _selector("vote(address,uint64,uint128,bool)")
SEL_RESOLVE = _selector("resolve(uint64)")
SEL_EXECUTE = _selector("execute(uint64,address[],bytes[])")
SEL_GET_PROPOSAL_STATE = _selector("getProposalState(uint64)")
SEL_GET_POOL = _selector("getPool(uint256)")
SEL_SET_PERMISSIONLESS = _selector("setPermissionlessJoinEnabled(bool)")
SEL_IS_PERMISSIONLESS = _selector("isPermissionlessJoinEnabled()")


def extract_proposal_id(receipt: dict) -> Optional[int]:
    """Proposal id from a createProposal receipt's ProposalCreated event,
    or None when the event is absent. Pure over the receipt dict."""
    for log in receipt.get("logs", []):
        topics = log.get("topics") if isinstance(log, dict) else log["topics"]
        if topics and bytes(topics[0]) == bytes(PROPOSAL_CREATED_TOPIC):
            return int.from_bytes(bytes(topics[1]), "big")
    return None


def send_governance_tx(
    w3: Web3, to: str, data: bytes, sender_key: str, gas: int = 1_000_000
) -> dict:
    """Sign, send and wait out one governance transaction."""
    sender = Account.from_key(sender_key)
    tx = {
        "to": to,
        "data": data,
        "gas": gas,
        "gasPrice": w3.eth.gas_price,
        "nonce": w3.eth.get_transaction_count(sender.address),
        "chainId": w3.eth.chain_id,
        "value": 0,
    }
    signed = sender.sign_transaction(tx)
    tx_hash = w3.eth.send_raw_transaction(signed.raw_transaction)
    return w3.eth.wait_for_transaction_receipt(tx_hash, timeout=60)


def is_permissionless_join_enabled(w3: Web3) -> bool:
    raw = w3.eth.call({"to": VALIDATOR_MANAGER, "data": SEL_IS_PERMISSIONLESS})
    return bool(int.from_bytes(raw[-32:], "big"))


async def enable_permissionless_join(
    w3: Web3,
    faucet_key: str,
    faucet_address: str,
    voting_duration_s: int = 5,
):
    """One-time governance flip of permissionless validator join. Idempotent.

    ``faucet_key``/``faucet_address`` must be the governance owner and
    pool[0]'s voter (the fuzzy_cluster genesis recipe);
    ``voting_duration_s`` must match genesis voting_duration_micros.
    """
    if is_permissionless_join_enabled(w3):
        LOG.info("Permissionless join already enabled")
        return

    faucet_address = Web3.to_checksum_address(faucet_address)
    pool0_raw = w3.eth.call(
        {"to": STAKING, "data": SEL_GET_POOL + encode(["uint256"], [0])}
    )
    pool0 = Web3.to_checksum_address("0x" + pool0_raw[-20:].hex())

    receipt = send_governance_tx(
        w3,
        GOVERNANCE,
        SEL_ADD_EXECUTOR + encode(["address"], [faucet_address]),
        faucet_key,
    )
    assert receipt["status"] == 1, f"addExecutor failed: {receipt}"

    enable_call = SEL_SET_PERMISSIONLESS + encode(["bool"], [True])
    create_data = SEL_CREATE_PROPOSAL + encode(
        ["address", "address[]", "bytes[]", "string"],
        [pool0, [VALIDATOR_MANAGER], [enable_call], "e2e-permissionless-join"],
    )
    receipt = send_governance_tx(w3, GOVERNANCE, create_data, faucet_key)
    assert receipt["status"] == 1, f"createProposal failed: {receipt}"

    proposal_id = extract_proposal_id(receipt)
    assert proposal_id is not None, "ProposalCreated event not found"
    LOG.info(
        "Created governance proposal %d to enable permissionless join", proposal_id
    )

    vote_data = SEL_VOTE + encode(
        ["address", "uint64", "uint128", "bool"],
        [pool0, proposal_id, MAX_UINT128, True],
    )
    receipt = send_governance_tx(w3, GOVERNANCE, vote_data, faucet_key)
    assert receipt["status"] == 1, f"vote failed: {receipt}"

    await asyncio.sleep(voting_duration_s + 2)

    receipt = send_governance_tx(
        w3, GOVERNANCE, SEL_RESOLVE + encode(["uint64"], [proposal_id]), faucet_key
    )
    assert receipt["status"] == 1, f"resolve failed: {receipt}"
    state_raw = w3.eth.call(
        {
            "to": GOVERNANCE,
            "data": SEL_GET_PROPOSAL_STATE + encode(["uint64"], [proposal_id]),
        }
    )
    state = int.from_bytes(state_raw[-1:], "big")
    assert state == PROPOSAL_STATE_SUCCEEDED, f"proposal not SUCCEEDED: state={state}"

    exec_data = SEL_EXECUTE + encode(
        ["uint64", "address[]", "bytes[]"],
        [proposal_id, [VALIDATOR_MANAGER], [enable_call]],
    )
    receipt = send_governance_tx(w3, GOVERNANCE, exec_data, faucet_key)
    assert receipt["status"] == 1, f"execute failed: {receipt}"

    assert is_permissionless_join_enabled(
        w3
    ), "permissionless join not enabled after execute"
    LOG.info("Permissionless validator join enabled via governance")
