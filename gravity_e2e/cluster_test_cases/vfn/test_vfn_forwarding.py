"""
VFN Transaction Forwarding Test

Verifies that a VFN (Validator Full Node) can:
1. Accept and forward transactions to the validator
2. Return correct transaction receipts
3. Reflect state changes (balance updates) consistently
"""

import pytest
import logging
from eth_account import Account
from web3 import Web3
from gravity_e2e.cluster.manager import Cluster, NodeState
from gravity_e2e.utils.transaction_builder import TransactionBuilder

LOG = logging.getLogger(__name__)


@pytest.mark.asyncio
async def test_vfn_connectivity(cluster: Cluster):
    """Verify VFN is running and syncing blocks."""
    LOG.info("Testing VFN connectivity...")

    assert await cluster.set_full_live(timeout=60), "Cluster failed to become fully live"

    vfn = cluster.get_node("vfn1")
    assert vfn is not None, "vfn1 not found in cluster config"

    block = vfn.get_block_number()
    LOG.info(f"VFN block height: {block}")
    assert block >= 0, "VFN should report a valid block height"


@pytest.mark.asyncio
async def test_vfn_forward_transaction(cluster: Cluster):
    """
    Send a transaction via the VFN RPC endpoint and verify:
    - VFN accepts the transaction
    - Transaction gets included in a block (forwarded to validator)
    - Receipt is obtainable from the VFN
    - Balance change is reflected on both VFN and validator
    """
    LOG.info("Testing VFN transaction forwarding...")

    assert await cluster.set_full_live(timeout=60), "Cluster failed to become fully live"

    validator = cluster.get_node("node1")
    vfn = cluster.get_node("vfn1")
    assert validator is not None, "node1 (validator) not found"
    assert vfn is not None, "vfn1 not found"

    # Use the faucet account as sender
    sender = cluster.faucet
    assert sender, "Faucet not configured"
    LOG.info(f"Sender (faucet): {sender.address}")

    # Create a fresh receiver
    receiver = Account.create()
    LOG.info(f"Receiver: {receiver.address}")

    amount_wei = Web3.to_wei(1, "ether")

    # Verify initial balance is 0 on both VFN and validator
    assert vfn.w3.eth.get_balance(receiver.address) == 0, "Receiver should start with 0 on VFN"
    assert validator.w3.eth.get_balance(receiver.address) == 0, "Receiver should start with 0 on validator"

    # Build and send transaction through VFN
    tb = TransactionBuilder(vfn.w3, sender)
    result = await tb.send_ether(receiver.address, amount_wei)
    assert result.success, f"Transaction via VFN failed: {result.error}"

    LOG.info(f"Tx hash: {result.tx_hash}")
    LOG.info(f"Block number: {result.block_number}")
    LOG.info(f"Gas used: {result.gas_used}")

    # Verify receipt fields
    assert result.tx_receipt is not None, "Should have a receipt"
    assert result.block_number is not None and result.block_number > 0, "Should be in a valid block"
    assert result.tx_receipt["status"] == 1, "Transaction should succeed (status=1)"

    # Verify balance on VFN
    vfn_balance = vfn.w3.eth.get_balance(receiver.address)
    assert vfn_balance == amount_wei, f"VFN balance mismatch: expected {amount_wei}, got {vfn_balance}"
    LOG.info(f"VFN shows receiver balance: {Web3.from_wei(vfn_balance, 'ether')} ETH")

    # Verify balance on validator (should be consistent)
    val_balance = validator.w3.eth.get_balance(receiver.address)
    assert val_balance == amount_wei, f"Validator balance mismatch: expected {amount_wei}, got {val_balance}"
    LOG.info(f"Validator shows receiver balance: {Web3.from_wei(val_balance, 'ether')} ETH")

    LOG.info("VFN transaction forwarding verified successfully!")


@pytest.mark.asyncio
async def test_vfn_receipt_from_validator_tx(cluster: Cluster):
    """
    Send a transaction via VALIDATOR, then query the receipt from VFN.
    Verifies VFN syncs committed blocks and can serve receipts for
    transactions it didn't originally receive.
    """
    LOG.info("Testing receipt query from VFN for validator-submitted tx...")

    assert await cluster.set_full_live(timeout=60), "Cluster failed to become fully live"

    validator = cluster.get_node("node1")
    vfn = cluster.get_node("vfn1")

    sender = cluster.faucet
    receiver = Account.create()
    amount_wei = Web3.to_wei(0.5, "ether")

    # Send via validator
    tb = TransactionBuilder(validator.w3, sender)
    result = await tb.send_ether(receiver.address, amount_wei)
    assert result.success, f"Validator tx failed: {result.error}"
    LOG.info(f"Tx sent via validator, hash: {result.tx_hash}")

    # Query receipt from VFN
    import time
    receipt = None
    for _ in range(30):
        try:
            receipt = vfn.w3.eth.get_transaction_receipt(result.tx_hash)
            if receipt:
                break
        except Exception:
            pass
        time.sleep(1)

    assert receipt is not None, "VFN should be able to serve receipt for validator tx"
    assert receipt["status"] == 1, "Transaction should succeed"
    LOG.info(f"VFN returned receipt for validator tx at block {receipt['blockNumber']}")

    # Verify balance consistency
    vfn_balance = vfn.w3.eth.get_balance(receiver.address)
    assert vfn_balance == amount_wei, f"VFN balance mismatch after validator tx"
    LOG.info("VFN correctly synced validator-submitted transaction!")
