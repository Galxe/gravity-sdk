"""EIP-7702 (SetCode tx, type 0x04) e2e acceptance.

Verifies post-Prague behavior observable via contract execution:
  P-B1  delegated stateless call returns the target contract's value
  P-B2  delegated stateful call uses the authority's storage, not the target's
  P-B3  revocation (target = 0x0) reverts the authority to plain-EOA behavior
  P-B4  pipe-exec filter discards low-intrinsic-gas SetCode tx
  P-B5  filter admits sufficiently-gassed SetCode tx
  P-B6  self-sponsored self-delegation (tx.from == authority) does not halt the chain
"""

import asyncio
import json
import logging
from pathlib import Path

import pytest
from eth_account import Account
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils.eip7702 import build_signed_set_code_tx, sign_authorization
from gravity_e2e.utils.transaction_builder import TransactionBuilder

LOG = logging.getLogger(__name__)

CONTRACTS_DIR = Path(__file__).parent / "contracts"
ZERO_ADDR = "0x" + "00" * 20


def _load_artifact(name: str):
    with open(CONTRACTS_DIR / f"{name}.json") as f:
        a = json.load(f)
    return a["abi"], a["bytecode"]


async def _deploy(node, faucet, name: str, args=None):
    abi, bytecode = _load_artifact(name)
    tb = TransactionBuilder(node.w3, faucet)
    result = await tb.deploy_contract(bytecode=bytecode, abi=abi, args=args)
    assert result.success, f"deploy {name} failed: {result.error}"
    addr = result.tx_receipt["contractAddress"]
    return node.w3.eth.contract(address=addr, abi=abi), addr


async def _send_setcode_tx(
    node,
    sender,
    authority,
    delegate_addr: str,
    *,
    gas: int,
    chain_id: int,
):
    """Build, sign and broadcast a SetCode tx. Returns tx_hash hex string.

    Inner CALL targets `sender` (EOA self-call, no-op) so the tx exercises
    only the designator-install path. Targeting `authority` here would
    invoke the freshly-installed delegate's fallback — Delegate.sol /
    Counter.sol don't define one, and the revert would mask designator
    installation behind a tx-level failure.

    When `sender == authority` (self-sponsored self-delegation), the auth
    tuple's nonce must be `sender_nonce + 1` because the tx bumps the
    sender's nonce before the authorization list is processed. P-B6
    exercises this shape — pre-fix it panicked grevm and halted the chain.
    """
    sender_nonce = node.w3.eth.get_transaction_count(sender.address)
    if sender.address.lower() == authority.address.lower():
        auth_nonce = sender_nonce + 1
    else:
        auth_nonce = node.w3.eth.get_transaction_count(authority.address)
    auth = sign_authorization(
        authority, chain_id=chain_id, delegate=delegate_addr, nonce=auth_nonce
    )

    # Use a healthy fee — single-node devnet base fee is tiny but keep margin.
    fee = max(node.w3.eth.gas_price * 2, 10**9)

    raw = build_signed_set_code_tx(
        sender,
        chain_id=chain_id,
        nonce=sender_nonce,
        to=sender.address,
        authorization_list=[auth],
        gas=gas,
        max_fee_per_gas=fee,
        max_priority_fee_per_gas=fee,
    )
    return node.w3.eth.send_raw_transaction(raw).hex()


async def _wait_for_receipt(node, tx_hash: str, *, timeout: float = 30.0):
    deadline = asyncio.get_event_loop().time() + timeout
    while asyncio.get_event_loop().time() < deadline:
        try:
            return node.w3.eth.get_transaction_receipt(tx_hash)
        except Exception:
            await asyncio.sleep(0.5)
    return None


@pytest.mark.asyncio
async def test_p_b1_delegated_stateless_call(cluster: Cluster):
    """P-B1: after SetCode, eth_call(authority, getValue()) returns 42."""
    assert await cluster.set_full_live(timeout=60), "cluster failed to become live"
    node = cluster.get_node("node1")
    chain_id = node.w3.eth.chain_id
    faucet = cluster.faucet

    delegate, delegate_addr = await _deploy(node, faucet, "Delegate")
    authority = Account.create()
    LOG.info(f"P-B1 authority={authority.address}, delegate={delegate_addr}")

    tx_hash = await _send_setcode_tx(
        node, faucet, authority, delegate_addr, gas=200_000, chain_id=chain_id
    )
    receipt = await _wait_for_receipt(node, tx_hash)
    assert receipt is not None, "SetCode tx receipt timeout"
    assert receipt["status"] == 1, f"SetCode tx failed: {receipt}"

    # The authority now executes Delegate's code in its own context.
    authority_as_delegate = node.w3.eth.contract(address=authority.address, abi=delegate.abi)
    value = authority_as_delegate.functions.getValue().call()
    assert value == 42, f"delegated call returned {value}, expected 42"


@pytest.mark.asyncio
async def test_p_b2_delegated_stateful_call_uses_authority_storage(cluster: Cluster):
    """P-B2: delegated set/get reads/writes authority's storage, not the target's."""
    node = cluster.get_node("node1")
    chain_id = node.w3.eth.chain_id
    faucet = cluster.faucet

    counter, counter_addr = await _deploy(node, faucet, "Counter")
    authority = Account.create()
    LOG.info(f"P-B2 authority={authority.address}, counter={counter_addr}")

    # Install the delegation.
    tx_hash = await _send_setcode_tx(
        node, faucet, authority, counter_addr, gas=200_000, chain_id=chain_id
    )
    receipt = await _wait_for_receipt(node, tx_hash)
    assert receipt is not None and receipt["status"] == 1

    authority_view = node.w3.eth.contract(address=authority.address, abi=counter.abi)

    # Initial state: both 0.
    assert authority_view.functions.get().call() == 0
    assert counter.functions.get().call() == 0

    # Call set(7) on the authority (faucet pays gas).
    tb = TransactionBuilder(node.w3, faucet)
    set_tx = authority_view.functions.set(7).build_transaction(
        {"from": faucet.address, "gas": 200_000, "nonce": node.w3.eth.get_transaction_count(faucet.address)}
    )
    set_result = await tb.send_transaction(set_tx, wait_for_receipt=True)
    assert set_result.success, f"set(7) failed: {set_result.error}"

    # Authority's storage now holds 7; the target contract is unchanged.
    assert authority_view.functions.get().call() == 7
    assert counter.functions.get().call() == 0, "target storage should not change"


@pytest.mark.asyncio
async def test_p_b3_revoke_delegation(cluster: Cluster):
    """P-B3: SetCode with target=0x0 reverts the authority to plain EOA."""
    node = cluster.get_node("node1")
    chain_id = node.w3.eth.chain_id
    faucet = cluster.faucet

    delegate, delegate_addr = await _deploy(node, faucet, "Delegate")
    authority = Account.create()

    # Install delegation.
    tx_hash = await _send_setcode_tx(
        node, faucet, authority, delegate_addr, gas=200_000, chain_id=chain_id
    )
    receipt = await _wait_for_receipt(node, tx_hash)
    assert receipt is not None and receipt["status"] == 1

    authority_view = node.w3.eth.contract(address=authority.address, abi=delegate.abi)
    assert authority_view.functions.getValue().call() == 42, "delegation should be active"

    # Revoke.
    revoke_hash = await _send_setcode_tx(
        node, faucet, authority, ZERO_ADDR, gas=200_000, chain_id=chain_id
    )
    revoke_receipt = await _wait_for_receipt(node, revoke_hash)
    assert revoke_receipt is not None and revoke_receipt["status"] == 1

    # After revocation, calling getValue() on the now-plain EOA should not return 42.
    # Either it raises (no code) or returns empty bytes that Web3 interprets as call failure.
    raised = False
    try:
        result = authority_view.functions.getValue().call()
        # If it didn't raise, it must not be 42 (e.g. revert→empty returndata decoded as 0).
        assert result != 42, f"delegation still active after revoke: got {result}"
    except Exception as exc:
        raised = True
        LOG.info(f"P-B3 expected error on revoked authority: {type(exc).__name__}")
    LOG.info(f"P-B3 revocation observable (raised={raised})")


@pytest.mark.asyncio
async def test_p_b4_low_intrinsic_gas_dropped(cluster: Cluster):
    """P-B4: SetCode tx with gas < 21000 + 25000*N(auths) is silently dropped."""
    node = cluster.get_node("node1")
    chain_id = node.w3.eth.chain_id
    faucet = cluster.faucet

    delegate, delegate_addr = await _deploy(node, faucet, "Delegate")
    authority = Account.create()

    # 1 auth → floor = 21000 + 25000 = 46000. Send 45999.
    try:
        tx_hash = await _send_setcode_tx(
            node, faucet, authority, delegate_addr, gas=45_999, chain_id=chain_id
        )
        LOG.info(f"P-B4 low-gas tx submitted: {tx_hash}; expecting silent drop")
        receipt = await _wait_for_receipt(node, tx_hash, timeout=15.0)
        assert receipt is None, f"low-gas SetCode unexpectedly mined: {receipt}"
    except Exception as exc:
        # Pool may reject at submission time; that's also acceptable.
        LOG.info(f"P-B4 pool rejected at submission: {type(exc).__name__}: {exc}")

    # Stronger oracle: delegation must not have been installed.
    authority_view = node.w3.eth.contract(address=authority.address, abi=delegate.abi)
    delegation_installed = False
    try:
        v = authority_view.functions.getValue().call()
        delegation_installed = (v == 42)
    except Exception:
        delegation_installed = False
    assert not delegation_installed, "low-gas tx should not have installed delegation"


@pytest.mark.asyncio
async def test_p_b5_sufficient_intrinsic_gas_accepted(cluster: Cluster):
    """P-B5: SetCode tx at floor (21000 + 25000) is admitted and observable."""
    node = cluster.get_node("node1")
    chain_id = node.w3.eth.chain_id
    faucet = cluster.faucet

    delegate, delegate_addr = await _deploy(node, faucet, "Delegate")
    authority = Account.create()

    # Floor is 46000 for 1 auth, but the *call* itself (executing Delegate.getValue
    # on the authority during tx execution? — no, SetCode tx body is empty by default).
    # The intrinsic floor 46000 covers tx + 1 auth. Use a healthy margin to avoid
    # exec OOG (tx data + setCode flow). 100k is well above floor and well below
    # any sensible block limit.
    tx_hash = await _send_setcode_tx(
        node, faucet, authority, delegate_addr, gas=100_000, chain_id=chain_id
    )
    receipt = await _wait_for_receipt(node, tx_hash)
    assert receipt is not None, "SetCode tx receipt timeout"
    assert receipt["status"] == 1, f"SetCode tx failed: {receipt}"

    authority_view = node.w3.eth.contract(address=authority.address, abi=delegate.abi)
    assert authority_view.functions.getValue().call() == 42


@pytest.mark.asyncio
async def test_p_b6_self_sponsored_self_delegation(cluster: Cluster):
    """P-B6: self-sponsored self-delegation (tx.from listed as own authority).

    Regression for Galxe/grevm#102 / gravity-reth#345: pre-fix, grevm's
    StateAsyncCommit asserted on the caller-nonce relation and panicked when
    a type-4 SetCode tx had `tx.from` in its own authorizationList. This is
    the exact `simple-bench --transfer-type eip7702` pattern that halted
    Gravity testnet at block 1400868 on 2026-06-09. The chain-halt oracle
    here is "receipt arrives" — pre-fix the validator panicked and no
    further blocks were produced, so the receipt would never come back.
    """
    node = cluster.get_node("node1")
    chain_id = node.w3.eth.chain_id
    faucet = cluster.faucet

    delegate, delegate_addr = await _deploy(node, faucet, "Delegate")

    self_signer = Account.create()
    LOG.info(f"P-B6 self_signer={self_signer.address}, delegate={delegate_addr}")

    # Fund the self-signer so it can pay for the SetCode tx itself.
    # gas=200k * fee≈1 gwei ≈ 2e-4 ETH; 1 ETH is comfortable headroom.
    tb = TransactionBuilder(node.w3, faucet)
    fund = await tb.send_ether(to=self_signer.address, amount_wei=10**18)
    assert fund.success, f"funding self_signer failed: {fund.error}"

    # tx.from == authority — pre-fix this panicked grevm in async_commit.
    tx_hash = await _send_setcode_tx(
        node, self_signer, self_signer, delegate_addr, gas=200_000, chain_id=chain_id
    )
    receipt = await _wait_for_receipt(node, tx_hash, timeout=60.0)
    assert receipt is not None, (
        "self-sponsored SetCode tx receipt timeout — chain may have halted "
        "(regression of grevm self-auth panic)"
    )
    assert receipt["status"] == 1, f"self-sponsored SetCode tx failed: {receipt}"

    # Delegation is observable on the self-signer's address.
    self_signer_view = node.w3.eth.contract(address=self_signer.address, abi=delegate.abi)
    assert self_signer_view.functions.getValue().call() == 42

    # Chain progression oracle: another block must be produced after the tx.
    head_after_tx = receipt["blockNumber"]
    deadline = asyncio.get_event_loop().time() + 30.0
    while asyncio.get_event_loop().time() < deadline:
        if node.w3.eth.block_number > head_after_tx:
            return
        await asyncio.sleep(0.5)
    raise AssertionError(
        f"no new block after self-sponsored SetCode (head stuck at {head_after_tx}) "
        "— chain may have halted"
    )
