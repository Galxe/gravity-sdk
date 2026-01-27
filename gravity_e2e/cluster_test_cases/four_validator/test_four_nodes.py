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
