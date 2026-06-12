"""EIP-7702 SetCode (type 0x04) transaction helpers.

Requires eth-account >= 0.13.6.
"""

from typing import Any, Dict, List, Optional

from eth_account import Account
from eth_account.signers.local import LocalAccount


def sign_authorization(
    signer: LocalAccount,
    *,
    chain_id: int,
    delegate: str,
    nonce: int,
) -> Dict[str, Any]:
    """Sign a single EIP-7702 authorization tuple.

    Returns the RPC-structured dict (camelCase keys) ready to embed in
    a SetCode transaction's authorizationList.

    `delegate` is the contract address whose code will be delegated to
    the signer's EOA. Use 0x00..00 to revoke an active delegation.
    """
    signed = signer.sign_authorization(
        {"chainId": chain_id, "address": delegate, "nonce": nonce}
    )
    return {
        "chainId": signed.chain_id,
        "address": signed.address,
        "nonce": signed.nonce,
        "yParity": signed.y_parity,
        "r": signed.r,
        "s": signed.s,
    }


def build_signed_set_code_tx(
    sender: LocalAccount,
    *,
    chain_id: int,
    nonce: int,
    to: str,
    authorization_list: List[Dict[str, Any]],
    gas: int,
    max_fee_per_gas: int,
    max_priority_fee_per_gas: int,
    value: int = 0,
    data: bytes = b"",
    access_list: Optional[List[Dict[str, Any]]] = None,
) -> bytes:
    """Build and sign a type-0x04 SetCode transaction.

    Returns the raw RLP-encoded transaction bytes (starts with 0x04).
    Caller can send via `eth_sendRawTransaction`.
    """
    tx = {
        "type": 4,
        "chainId": chain_id,
        "nonce": nonce,
        "to": to,
        "value": value,
        "data": data,
        "gas": gas,
        "maxFeePerGas": max_fee_per_gas,
        "maxPriorityFeePerGas": max_priority_fee_per_gas,
        "accessList": access_list or [],
        "authorizationList": authorization_list,
    }
    signed = sender.sign_transaction(tx)
    return bytes(signed.raw_transaction)
