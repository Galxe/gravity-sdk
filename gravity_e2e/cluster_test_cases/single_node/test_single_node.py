import pytest
import logging
from eth_account import Account
from web3 import Web3
from gravity_e2e.cluster.manager import Cluster, NodeState
from gravity_e2e.utils.transaction_builder import TransactionBuilder

LOG = logging.getLogger(__name__)

@pytest.mark.asyncio
async def test_single_node_connectivity(cluster: Cluster):
    """Verify single node is running and responsive using Cluster fixture."""
    LOG.info("Testing connectivity to single node...")
    
    # 1. Use Declarative API to ensure node is live
    assert await cluster.set_full_live(timeout=30), "Cluster failed to become fully live"
    
    node = cluster.get_node("node1")
    assert node, "node1 not found in cluster config"
    
    # 2. Check block progress
    current_height = node.get_block_number()
    LOG.info(f"Connected to {node.id}! Current block: {current_height}")
    
    assert isinstance(current_height, int)
    assert current_height >= 0

@pytest.mark.asyncio
async def test_faucet_transfer(cluster: Cluster):
    """Verify faucet functionality by sending funds to a random address."""
    LOG.info("Testing faucet transfer...")
    
    sender = cluster.faucet
    assert sender, "Faucet not configured"
    LOG.info(f"Faucet Address: {sender.address}")
    
    # Use node's web3 directly
    node = cluster.get_node("node1")
    
    # Setup Receiver
    receiver = Account.create()
    LOG.info(f"Receiver Address: {receiver.address}")
    
    # Build & Send
    tb = TransactionBuilder(node.w3, sender)
    amount_wei = Web3.to_wei(1, 'ether')
    
    initial_balance = node.w3.eth.get_balance(receiver.address)
    assert initial_balance == 0
    
    result = await tb.send_ether(receiver.address, amount_wei)
    assert result.success, f"Transfer failed: {result.error}"
    
    # Verify
    new_balance = node.w3.eth.get_balance(receiver.address)
    assert new_balance == amount_wei
    LOG.info("Faucet transfer verified successfully!")
