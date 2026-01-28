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
            height = await node.get_block_number()
            LOG.info(f"{node_id} connected at port {node.rpc_port}! Height: {height}")
            assert height >= 0
        except Exception as e:
            LOG.error(f"Failed to connect to {node_id}: {e}")
            raise

    # 3. Verify consensus (all nodes advancing)
    LOG.info("Verifying block production...")
    # Wait for 1 block
    await asyncio.sleep(2)
    assert await cluster.check_block_increasing(timeout=30), "Block production halted"
    LOG.info("Block production verified.")

@pytest.mark.asyncio
async def test_faucet_transfer_propagation(cluster: Cluster):
    """Verify faucet transfer propagates across the cluster (Send on Node1, Check on Node2)."""
    from web3 import Web3
    from eth_account import Account
    from gravity_e2e.utils.transaction_builder import TransactionBuilder

    LOG.info("Testing faucet transfer propagation...")
    
    faucet_cfg = cluster.faucet
    assert faucet_cfg, "Faucet config not found"
    
    # Setup Web3 on Node 1 (Sender)
    node1 = cluster.get_node("node1")
    web3_1 = Web3(Web3.HTTPProvider(node1.url))
    
    # Setup Web3 on Node 2 (Verifier)
    node2 = cluster.get_node("node2")
    if not node2:
        LOG.warning("Node2 not found, falling back to verification on Node1")
        web3_2 = web3_1
    else:
        web3_2 = Web3(Web3.HTTPProvider(node2.url))

    sender = Account.from_key(faucet_cfg["private_key"])
    receiver = Account.create()
    
    # Send from Node 1
    tb = TransactionBuilder(web3_1, sender)
    amount = Web3.to_wei(0.1, 'ether')
    
    LOG.info(f"Sending {amount} wei from {sender.address} to {receiver.address} via {node1.id}")
    result = await tb.send_ether(receiver.address, amount)
    assert result.success, f"Transfer failed: {result.error}"
    
    # Verify on Node 2 (Polled check for propagation)
    LOG.info(f"Verifying balance on {node2.id if node2 else node1.id}...")
    
    import time
    start = time.time()
    balance = 0
    while time.time() - start < 10:
        balance = web3_2.eth.get_balance(receiver.address)
        if balance == amount:
            break
        await asyncio.sleep(1)
        
    assert balance == amount, f"Balance mismatch on verifier node. Expected {amount}, got {balance}"
    LOG.info("Propagation verified!")
