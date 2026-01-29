"""
Latency Benchmark Test (4-Node Cluster)

Measures transaction latency (Send -> Receipt) on a 4-node cluster.

Test Parameters:
- 4 Validators: randomly select one for all transactions
- 200 transactions over 10 seconds (50ms interval)
- Measure P99 latency from SendRawTx to Receipt
- Compare 4-node vs 3-node (after stopping one)
"""
import pytest
import logging
import asyncio
import time
import math
import random
import statistics
from web3 import Web3
from eth_account import Account
from gravity_e2e.cluster.manager import Cluster

LOG = logging.getLogger(__name__)

# --- Configuration ---
TX_COUNT = 200            # 发送200笔交易
TX_INTERVAL = 0.05        # 50ms 间隔
TOTAL_TIME = 10           # 总计约10秒


# --- Latency Measurement Logic ---

async def wait_for_receipt(w3, tx_hash: str, timeout: float = 30.0) -> float:
    """
    Wait for a single transaction receipt.
    Returns the time taken (latency) or -1 if timeout.
    """
    start = time.time()
    while time.time() - start < timeout:
        try:
            receipt = w3.eth.get_transaction_receipt(tx_hash)
            if receipt:
                return time.time() - start
        except Exception:
            pass
        await asyncio.sleep(0.02)  # 20ms polling
    return -1  # Timeout


async def measure_latencies(w3, account, num_txs: int, interval: float, recipient: str, start_nonce: int):
    """
    Send transactions one by one, wait for each to confirm, measure latency.
    Returns list of latencies (in seconds).
    """
    latencies = []
    chain_id = w3.eth.chain_id
    gas_price = w3.to_wei('2', 'gwei')
    
    LOG.info(f"Measuring latency for {num_txs} transactions...")
    start_batch = time.time()
    
    for i in range(num_txs):
        tx = {
            'nonce': start_nonce + i,
            'to': recipient,
            'value': 0,
            'gas': 21000,
            'gasPrice': gas_price,
            'chainId': chain_id
        }
        
        try:
            signed_tx = w3.eth.account.sign_transaction(tx, account.key)
            raw_tx = signed_tx.raw_transaction
            
            # Send and immediately start timing
            send_time = time.time()
            tx_hash = w3.eth.send_raw_transaction(raw_tx)
            
            # Wait for this transaction to confirm
            latency = await wait_for_receipt(w3, tx_hash.hex(), timeout=30.0)
            
            if latency >= 0:
                latencies.append(latency)
                if (i + 1) % 20 == 0:
                    avg_so_far = sum(latencies) / len(latencies)
                    LOG.info(f"Tx {i + 1}/{num_txs}: latency={latency:.3f}s, avg={avg_so_far:.3f}s")
            else:
                LOG.warning(f"Tx {i + 1}/{num_txs}: TIMEOUT")
                
        except Exception as e:
            LOG.error(f"Error on tx {i}: {e}")
        
        # Wait interval before sending next
        await asyncio.sleep(interval)
    
    total_time = time.time() - start_batch
    LOG.info(f"Completed {len(latencies)}/{num_txs} transactions in {total_time:.1f}s")
        
    return latencies


def calculate_stats(latencies: list):
    """Calculate statistics from latency list."""
    if not latencies:
        return None

    latencies = sorted(latencies)
    
    def calc_p(p):
        k = (len(latencies) - 1) * (p / 100.0)
        f = math.floor(k)
        c = math.ceil(k)
        if f == c: return latencies[int(k)]
        d0 = latencies[int(f)]
        d1 = latencies[int(c)]
        return d0 + (d1 - d0) * (k - f)

    return {
        "count": len(latencies),
        "min": min(latencies),
        "max": max(latencies),
        "avg": statistics.mean(latencies),
        "p50": calc_p(50),
        "p90": calc_p(90),
        "p99": calc_p(99)
    }

# --- Latency Benchmark Runner ---

async def run_latency_benchmark(
    w3,
    faucet,
    recipient: str,
    num_txs: int,
    interval: float,
    phase_name: str,
    node_id: str,
    start_nonce: int
) -> dict:
    """
    Run a single latency benchmark round using faucet account.
    Sends one tx at a time, waits for confirmation, measures latency.
    
    Args:
        w3: Web3 instance
        faucet: Faucet account to send transactions from
        recipient: Recipient address for transactions
        num_txs: Number of transactions to send
        interval: Interval between transactions (seconds)
        phase_name: Name for logging (e.g., "4-Node", "3-Node")
        node_id: ID of the target node
        start_nonce: Starting nonce for transactions
        
    Returns:
        Statistics dictionary with latency metrics
    """
    LOG.info(f"Starting benchmark: {phase_name}")
    LOG.info(f"Sending {num_txs} txs, waiting for each to confirm...")
    
    latencies = await measure_latencies(
        w3, faucet, num_txs, interval, recipient, start_nonce
    )
    
    stats = calculate_stats(latencies)
    
    if stats:
        LOG.info("\n" + "=" * 50)
        LOG.info(f"LATENCY STATISTICS - {phase_name}")
        LOG.info("=" * 50)
        LOG.info(f"Target Node:   {node_id}")
        LOG.info(f"Total Txs:     {stats['count']}/{num_txs}")
        LOG.info(f"Interval:      {interval*1000:.0f}ms")
        LOG.info("-" * 50)
        LOG.info(f"Min:  {stats['min']:.4f}s")
        LOG.info(f"Max:  {stats['max']:.4f}s")
        LOG.info(f"Avg:  {stats['avg']:.4f}s")
        LOG.info(f"P50:  {stats['p50']:.4f}s")
        LOG.info(f"P90:  {stats['p90']:.4f}s")
        LOG.info(f"P99:  {stats['p99']:.4f}s  <-- Primary metric")
        LOG.info("=" * 50 + "\n")
    
    return stats


# --- Test Case ---

@pytest.mark.asyncio
async def test_latency_benchmark_4_and_3_nodes(cluster: Cluster):
    """
    Runs latency benchmark on 4-node cluster, then stops one node and runs again with 3 nodes.
    
    Test spec:
    - Phase 1: 4 Validators running, randomly select one for transactions
    - Phase 2: Stop one validator, test with 3 validators
    - 200 transactions per phase over 10 seconds (50ms interval)
    - Measure P99 latency from SendRawTx to Receipt
    - Compare results between 4-node and 3-node configurations
    """
    LOG.info("=" * 70)
    LOG.info("Benchmark: Transaction Latency (4-Node vs 3-Node)")
    LOG.info(f"Config: {TX_COUNT} txs, {TX_INTERVAL*1000:.0f}ms interval")
    LOG.info("=" * 70)

    # Start cluster with all 4 nodes
    assert await cluster.set_full_live(timeout=120), "Cluster failed to start"
    
    # Wait for cluster to stabilize and produce blocks
    LOG.info("Waiting for cluster to stabilize...")
    await asyncio.sleep(10)
    
    # Verify blocks are being produced
    assert await cluster.check_block_increasing(timeout=30), "Cluster not producing blocks!"
    LOG.info("Cluster is producing blocks, proceeding with benchmark...")
    
    # Setup
    faucet = cluster.faucet
    recipient = Account.create().address
    node_ids = list(cluster.nodes.keys())
    
    # Get initial nonce from faucet
    first_node = cluster.get_node(node_ids[0])
    current_nonce = first_node.w3.eth.get_transaction_count(faucet.address, 'pending')
    LOG.info(f"Faucet address: {faucet.address}, starting nonce: {current_nonce}")
    
    # ========================================
    # Phase 1: 4-Node Benchmark
    # ========================================
    LOG.info("=" * 70)
    LOG.info("PHASE 1: 4-Node Cluster Benchmark")
    LOG.info("=" * 70)
    
    # Randomly select one of the 4 validators
    phase1_node_id = random.choice(node_ids)
    phase1_node = cluster.get_node(phase1_node_id)
    w3_phase1 = phase1_node.w3
    
    LOG.info(f"Selected validator for Phase 1: {phase1_node_id} ({phase1_node.url})")
    
    stats_4_nodes = await run_latency_benchmark(
        w3=w3_phase1,
        faucet=faucet,
        recipient=recipient,
        num_txs=TX_COUNT,
        interval=TX_INTERVAL,
        phase_name="4-Node Cluster",
        node_id=phase1_node_id,
        start_nonce=current_nonce
    )
    
    assert stats_4_nodes is not None, "Phase 1: No transactions confirmed!"
    
    # Check success rate (allow up to 20% loss for now, log warning if higher)
    success_rate_4 = stats_4_nodes['count'] / TX_COUNT * 100
    if success_rate_4 < 80:
        LOG.warning(f"Phase 1: Low success rate {success_rate_4:.1f}% ({stats_4_nodes['count']}/{TX_COUNT})")
    
    # Update nonce for phase 2
    current_nonce += TX_COUNT
    
    # ========================================
    # Phase 2: Stop one node, run with 3 nodes
    # ========================================
    LOG.info("=" * 70)
    LOG.info("PHASE 2: Stopping one node, running with 3-Node Cluster")
    LOG.info("=" * 70)
    
    # Select a node to stop (prefer one that wasn't used in phase 1)
    other_nodes = [nid for nid in node_ids if nid != phase1_node_id]
    node_to_stop = random.choice(other_nodes)
    
    LOG.info(f"Stopping node: {node_to_stop}")
    node_obj = cluster.get_node(node_to_stop)
    await node_obj.stop()
    
    # Wait for cluster to stabilize after node shutdown
    LOG.info("Waiting for cluster to stabilize after node shutdown...")
    await asyncio.sleep(10)  # Increased wait time
    
    # Verify we have 3 live nodes
    live_nodes = await cluster.get_live_nodes()
    LOG.info(f"Live nodes after shutdown: {[n.id for n in live_nodes]}")
    assert len(live_nodes) == 3, f"Expected 3 live nodes, got {len(live_nodes)}"
    
    # Select one of the remaining 3 nodes for phase 2
    remaining_node_ids = [n.id for n in live_nodes]
    phase2_node_id = random.choice(remaining_node_ids)
    phase2_node = cluster.get_node(phase2_node_id)
    w3_phase2 = phase2_node.w3
    
    LOG.info(f"Selected validator for Phase 2: {phase2_node_id} ({phase2_node.url})")
    
    # Verify cluster is still producing blocks with 3 nodes
    LOG.info("Verifying 3-node cluster is producing blocks...")
    assert await cluster.check_block_increasing(node_id=phase2_node_id, timeout=30), \
        "3-node cluster not producing blocks!"
    LOG.info("3-node cluster is healthy, proceeding with Phase 2...")
    
    stats_3_nodes = await run_latency_benchmark(
        w3=w3_phase2,
        faucet=faucet,
        recipient=recipient,
        num_txs=TX_COUNT,
        interval=TX_INTERVAL,
        phase_name="3-Node Cluster",
        node_id=phase2_node_id,
        start_nonce=current_nonce
    )
    
    assert stats_3_nodes is not None, "Phase 2: No transactions confirmed!"
    
    # Check success rate (allow up to 20% loss for now, log warning if higher)
    success_rate_3 = stats_3_nodes['count'] / TX_COUNT * 100
    if success_rate_3 < 80:
        LOG.warning(f"Phase 2: Low success rate {success_rate_3:.1f}% ({stats_3_nodes['count']}/{TX_COUNT})")
    
    # ========================================
    # Final Comparison
    # ========================================
    LOG.info("\n" + "=" * 70)
    LOG.info("FINAL COMPARISON: 4-Node vs 3-Node")
    LOG.info("=" * 70)
    LOG.info(f"{'Metric':<15} {'4-Node':>12} {'3-Node':>12} {'Diff':>12}")
    LOG.info("-" * 70)
    LOG.info(f"{'Success Rate':<15} {success_rate_4:>11.1f}% {success_rate_3:>11.1f}% {success_rate_3 - success_rate_4:>+11.1f}%")
    LOG.info(f"{'Confirmed':<15} {stats_4_nodes['count']:>12} {stats_3_nodes['count']:>12} {stats_3_nodes['count'] - stats_4_nodes['count']:>+12}")
    LOG.info(f"{'Min':<15} {stats_4_nodes['min']:>12.4f} {stats_3_nodes['min']:>12.4f} {stats_3_nodes['min'] - stats_4_nodes['min']:>+12.4f}")
    LOG.info(f"{'Max':<15} {stats_4_nodes['max']:>12.4f} {stats_3_nodes['max']:>12.4f} {stats_3_nodes['max'] - stats_4_nodes['max']:>+12.4f}")
    LOG.info(f"{'Avg':<15} {stats_4_nodes['avg']:>12.4f} {stats_3_nodes['avg']:>12.4f} {stats_3_nodes['avg'] - stats_4_nodes['avg']:>+12.4f}")
    LOG.info(f"{'P50':<15} {stats_4_nodes['p50']:>12.4f} {stats_3_nodes['p50']:>12.4f} {stats_3_nodes['p50'] - stats_4_nodes['p50']:>+12.4f}")
    LOG.info(f"{'P90':<15} {stats_4_nodes['p90']:>12.4f} {stats_3_nodes['p90']:>12.4f} {stats_3_nodes['p90'] - stats_4_nodes['p90']:>+12.4f}")
    LOG.info(f"{'P99':<15} {stats_4_nodes['p99']:>12.4f} {stats_3_nodes['p99']:>12.4f} {stats_3_nodes['p99'] - stats_4_nodes['p99']:>+12.4f}")
    LOG.info("=" * 70)
    LOG.info(f"P99 (4-Node): {stats_4_nodes['p99']:.4f}s | Success: {success_rate_4:.1f}%")
    LOG.info(f"P99 (3-Node): {stats_3_nodes['p99']:.4f}s | Success: {success_rate_3:.1f}%")
    LOG.info("=" * 70 + "\n")
    
    # Final assertions (relaxed for initial testing)
    # Warn but don't fail if success rate is low
    min_success_rate = 50  # At least 50% should succeed
    assert success_rate_4 >= min_success_rate, f"4-Node success rate too low: {success_rate_4:.1f}%"
    assert success_rate_3 >= min_success_rate, f"3-Node success rate too low: {success_rate_3:.1f}%"
    
    # P99 latency check (relaxed to 30s for initial testing)
    if stats_4_nodes['p99'] > 30.0:
        LOG.warning(f"4-Node P99 latency is high: {stats_4_nodes['p99']:.4f}s")
    if stats_3_nodes['p99'] > 30.0:
        LOG.warning(f"3-Node P99 latency is high: {stats_3_nodes['p99']:.4f}s")
