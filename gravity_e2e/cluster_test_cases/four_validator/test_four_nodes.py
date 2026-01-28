import pytest
import logging
import asyncio
from gravity_e2e.cluster.manager import Cluster

LOG = logging.getLogger(__name__)

@pytest.mark.asyncio
async def test_four_node_connectivity(cluster: Cluster):
    """Verify all four nodes are running and responsive using Cluster fixture."""
    LOG.info("Testing connectivity to four validator cluster...")
    
    # 1. Ensure all nodes are live
    assert await cluster.set_full_live(timeout=60), "Cluster failed to become fully live"
    
    assert len(cluster.nodes) == 4, f"Expected 4 nodes, got {len(cluster.nodes)}"
    
    # 2. Verify each node individually
    for node_id, node in cluster.nodes.items():
        try:
            height = node.get_block_number()
            LOG.info(f"{node_id} connected at port {node.rpc_port}! Height: {height}")
            assert height >= 0
        except Exception as e:
            LOG.error(f"Failed to connect to {node_id}: {e}")
            raise

    # 3. Verify consensus (all nodes advancing)
    LOG.info("Verifying block production...")
    await asyncio.sleep(2)
    assert await cluster.check_block_increasing(timeout=30), "Block production halted"
    LOG.info("Block production verified.")

@pytest.mark.asyncio
async def test_faucet_transfer_propagation(cluster: Cluster):
    """Verify faucet transfer propagates across the cluster (Send on Node1, Check on Node2)."""
    from eth_account import Account
    from web3 import Web3
    from gravity_e2e.utils.transaction_builder import TransactionBuilder

    LOG.info("Testing faucet transfer propagation...")
    
    sender = cluster.faucet
    assert sender, "Faucet not configured"
    receiver = Account.create()
    
    # Use node's web3 directly
    node1 = cluster.get_node("node1")
    node2 = cluster.get_node("node2")
    
    # Send from Node 1
    tb = TransactionBuilder(node1.w3, sender)
    amount = Web3.to_wei(0.1, 'ether')
    
    LOG.info(f"Sending {amount} wei from {sender.address} to {receiver.address} via {node1.id}")
    result = await tb.send_ether(receiver.address, amount)
    assert result.success, f"Transfer failed: {result.error}"
    
    # Verify on Node 2 (Polled check for propagation)
    verifier = node2 if node2 else node1
    LOG.info(f"Verifying balance on {verifier.id}...")
    
    import time
    start = time.time()
    balance = 0
    while time.time() - start < 10:
        balance = verifier.w3.eth.get_balance(receiver.address)
        if balance == amount:
            break
        await asyncio.sleep(1)
        
    assert balance == amount, f"Balance mismatch on verifier node. Expected {amount}, got {balance}"
    LOG.info("Propagation verified!")
