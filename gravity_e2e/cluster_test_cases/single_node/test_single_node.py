import pytest
import logging
from gravity_e2e.cluster.manager import Cluster, NodeState

LOG = logging.getLogger(__name__)

@pytest.mark.asyncio
async def test_single_node_connectivity(cluster: Cluster):
    """Verify single node is running and responsive using Cluster fixture."""
    LOG.info("Testing connectivity to single node...")
    
    # 1. Use Declarative API to ensure node is live
    # (Runner typically starts it, but this ensures it's reachable)
    assert await cluster.set_full_live(timeout=30), "Cluster failed to become fully live"
    
    node = cluster.get_node("node1")
    assert node, "node1 not found in cluster config"
    
    # 2. Check block progress
    current_height = await node.get_block_number()
    LOG.info(f"Connected to {node.id}! Current block: {current_height}")
    
    assert isinstance(current_height, int)
    assert current_height >= 0
