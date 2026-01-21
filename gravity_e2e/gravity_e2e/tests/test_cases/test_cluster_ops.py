import asyncio
import logging
from ..test_registry import register_test
from ...helpers.test_helpers import test_case
from ...cluster.manager import Cluster, NodeState

LOG = logging.getLogger(__name__)


@register_test("cluster_lifecycle", suite="cluster_ops", self_managed=True)
@test_case
async def test_cluster_lifecycle(cluster: Cluster, run_helper, test_result):
    """
    Verify cluster start/stop lifecycle using declarative API.
    """
    LOG.info("Starting Cluster Lifecycle Test (Declarative)")
    
    # 1. Declarative: Set all nodes to RUNNING
    LOG.info("Step 1: set_full_live()")
    if not await cluster.set_full_live(timeout=60):
        test_result.error = "Failed to bring all nodes to RUNNING"
        return
    
    # Verify all are running
    live_nodes = await cluster.get_live_nodes()
    LOG.info(f"Live nodes: {[n.id for n in live_nodes]}")
    if len(live_nodes) != len(cluster.nodes):
        test_result.error = f"Expected {len(cluster.nodes)} live nodes, got {len(live_nodes)}"
        return
    
    # 2. Declarative: Stop all nodes
    LOG.info("Step 2: set_all_stopped()")
    if not await cluster.set_all_stopped(timeout=60):
        test_result.error = "Failed to stop all nodes"
        return
    
    # Verify all stopped
    dead_nodes = await cluster.get_dead_nodes()
    LOG.info(f"Dead nodes: {[n.id for n in dead_nodes]}")
    if len(dead_nodes) != len(cluster.nodes):
        test_result.error = f"Expected {len(cluster.nodes)} dead nodes, got {len(dead_nodes)}"
        return
    
    # 3. Declarative: Restore all nodes
    LOG.info("Step 3: set_full_live() again (restore)")
    if not await cluster.set_full_live(timeout=60):
        test_result.error = "Failed to restore all nodes to RUNNING"
        return
    
    LOG.info("Cluster lifecycle test passed!")
    test_result.mark_success()


@register_test("cluster_fault_tolerance", suite="cluster_ops", self_managed=True)
@test_case
async def test_fault_tolerance(cluster: Cluster, run_helper, test_result):
    """
    Verify block production continues when a validator is stopped.
    Uses declarative API for state management.
    """
    # 1. Declarative: Ensure all nodes are RUNNING
    LOG.info("Step 1: set_full_live()")
    if not await cluster.set_full_live(timeout=60):
        test_result.error = "Failed to bring all nodes to RUNNING"
        return
    
    # Log current state
    for node_id, node in cluster.nodes.items():
        state, height = await node.get_state()
        LOG.info(f"  {node_id}: {state.name}, block={height}")
    
    node1 = cluster.get_node("node1")
    node2 = cluster.get_node("node2")
    
    if not node1 or not node2:
        test_result.error = "Test requires node1 and node2"
        return

    # 2. Record baseline height
    _, start_height = await node2.get_state()
    LOG.info(f"Baseline height from node2: {start_height}")
    
    # 3. Declarative: Stop Node 1
    LOG.info("Step 2: set_node('node1', STOPPED)")
    if not await cluster.set_node("node1", NodeState.STOPPED, timeout=30):
        test_result.error = "Failed to stop node1"
        return
    
    # Verify node1 is STOPPED
    n1_state, _ = await node1.get_state()
    LOG.info(f"  node1: {n1_state.name}")
    
    # 4. Wait for block production
    LOG.info("Waiting 10s for block production...")
    await asyncio.sleep(10)
    
    # 5. Check liveness on remaining nodes
    _, current_height = await node2.get_state()
    LOG.info(f"Current height from node2: {current_height}")
    
    if current_height <= start_height:
        test_result.error = f"Chain halted! Height unchanged at {current_height}"
        await cluster.set_all_stopped()
        return
    
    LOG.info(f"Chain progressed: {start_height} -> {current_height}")
        
    # 6. Declarative: Restart node1 for sync check
    LOG.info("Step 3: set_node('node1', RUNNING)")
    if not await cluster.set_node("node1", NodeState.RUNNING, timeout=30):
        test_result.error = "Failed to restart node1"
        await cluster.set_all_stopped()
        return
    
    # 7. Wait for sync
    LOG.info("Waiting 10s for node1 to sync...")
    await asyncio.sleep(10)
    
    # 8. Verify sync
    _, node1_height = await node1.get_state()
    _, node2_height = await node2.get_state()
    LOG.info(f"Node1 height: {node1_height}, Node2 height: {node2_height}")
    
    gap = abs(node2_height - node1_height)
    if gap > 5:
        test_result.error = f"Node1 failed to sync. Gap: {gap}"
        await cluster.set_all_stopped()
        return
    
    LOG.info("Fault tolerance test passed!")
    await cluster.set_all_stopped()
    test_result.mark_success()


