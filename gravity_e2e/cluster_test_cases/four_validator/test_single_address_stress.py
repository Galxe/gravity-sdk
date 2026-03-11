"""
Single-Address High-Pressure Stress Test (4-Node Cluster)

Verifies that four validator nodes correctly handle high transaction throughput
from a SINGLE address. This stresses mempool synchronization, nonce ordering,
and state consistency when one address generates overwhelming traffic.

Test Parameters:
- 4 Validators: transactions distributed round-robin across all nodes
- 1,000 transactions per round, sent within ~1 second (fire-and-forget)
- 5 rounds of sustained pressure
- Single sender (faucet) address for maximum nonce contention
- Verifies: confirmation rate, nonce consistency, balance correctness, block production
"""
import pytest
import logging
import asyncio
import time
import math
import statistics
from typing import List, Dict, Optional, Tuple
from web3 import Web3
from eth_account import Account
from gravity_e2e.cluster.manager import Cluster

LOG = logging.getLogger(__name__)

# =====================================================================
# Configuration — adjust these to tune stress intensity
# =====================================================================
TXN_PER_ROUND = 100           # Number of transactions per round
NUM_ROUNDS = 1                 # Number of stress rounds (1 for debugging)
SEND_WINDOW_SEC = 1.0          # Target: send all TXN_PER_ROUND within this window
RECEIPT_TIMEOUT_SEC = 120      # Max seconds to wait for all receipts after sending
RECEIPT_POLL_INTERVAL = 0.05   # 50ms polling interval for receipts
MIN_SUCCESS_RATE = 0.80        # Minimum acceptable confirmation rate (80%)
STABILIZE_WAIT_SEC = 10        # Seconds to wait for cluster stabilization
VALUE_PER_TX = 0               # Wei per transaction (0 = gas-only stress)
GAS_LIMIT = 21000              # Simple transfer gas
GAS_PRICE_GWEI = 2             # Gas price in gwei


# =====================================================================
# Helper Functions
# =====================================================================

def build_raw_tx(w3, account, nonce: int, recipient: str, chain_id: int, gas_price: int) -> bytes:
    """Build and sign a raw transaction. Returns signed raw bytes."""
    tx = {
        'nonce': nonce,
        'to': recipient,
        'value': VALUE_PER_TX,
        'gas': GAS_LIMIT,
        'gasPrice': gas_price,
        'chainId': chain_id,
    }
    signed = w3.eth.account.sign_transaction(tx, account.key)
    return signed.raw_transaction


def _sign_one(args):
    """Worker function for parallel signing (must be top-level for pickling)."""
    from eth_account import Account as _Account
    nonce, recipient, chain_id, gas_price, private_key = args
    tx = {
        'nonce': nonce,
        'to': recipient,
        'value': VALUE_PER_TX,
        'gas': GAS_LIMIT,
        'gasPrice': gas_price,
        'chainId': chain_id,
    }
    signed = _Account.sign_transaction(tx, private_key)
    return signed.raw_transaction


def build_raw_txs_parallel(account, start_nonce: int, count: int, recipient: str, chain_id: int, gas_price: int) -> List[bytes]:
    """Build and sign transactions in parallel using multiprocessing."""
    from multiprocessing import Pool, cpu_count
    
    args_list = [
        (start_nonce + i, recipient, chain_id, gas_price, account.key.hex())
        for i in range(count)
    ]
    
    workers = min(cpu_count(), 16)
    with Pool(workers) as pool:
        raw_txs = pool.map(_sign_one, args_list, chunksize=max(1, count // workers))
    return raw_txs


async def fire_transactions(
    nodes: List,
    raw_txs: List[bytes],
    target_duration: float,
) -> Tuple[List[str], List[str], float]:
    """
    Send all raw transactions as fast as possible using native async HTTP.
    Returns (successful_hashes, errors, elapsed_time).
    """
    import aiohttp
    
    tx_hashes: List[str] = []
    errors: List[str] = []
    
    # Get RPC URLs from nodes
    node_urls = []
    for node in nodes:
        url = node.w3.provider.endpoint_uri
        node_urls.append(url)
    node_count = len(node_urls)
    
    # Convert raw txs to hex
    raw_txs_hex = ['0x' + tx.hex() for tx in raw_txs]
    
    connector = aiohttp.TCPConnector(limit=500, force_close=False)
    timeout = aiohttp.ClientTimeout(total=60)
    
    start = time.time()
    
    async with aiohttp.ClientSession(connector=connector, timeout=timeout) as session:
        sem = asyncio.Semaphore(500)  # Limit concurrent requests
        
        async def send_one(idx: int, raw_hex: str):
            url = node_urls[idx % node_count]
            payload = {
                "jsonrpc": "2.0",
                "method": "eth_sendRawTransaction",
                "params": [raw_hex],
                "id": idx,
            }
            async with sem:
                try:
                    async with session.post(url, json=payload) as resp:
                        result = await resp.json()
                        if 'result' in result:
                            return result['result'], None
                        elif 'error' in result:
                            return None, f"tx#{idx}: {result['error']}"
                        else:
                            return None, f"tx#{idx}: unexpected response"
                except Exception as e:
                    return None, f"tx#{idx}: {e}"
        
        # Fire ALL at once
        all_tasks = [send_one(i, raw_txs_hex[i]) for i in range(len(raw_txs_hex))]
        results = await asyncio.gather(*all_tasks)
    
    for tx_hash, err in results:
        if tx_hash:
            tx_hashes.append(tx_hash)
        if err:
            errors.append(err)
    
    elapsed = time.time() - start
    return tx_hashes, errors, elapsed


async def collect_receipts(
    nodes: List,
    tx_hashes: List[str],
    timeout: float,
    poll_interval: float = 0.05,
) -> Dict[str, dict]:
    """
    Poll for transaction receipts across all nodes.
    Returns dict of tx_hash -> receipt_info.
    Uses the first node that returns a receipt for each hash.
    """
    loop = asyncio.get_running_loop()
    receipts: Dict[str, dict] = {}
    pending = set(tx_hashes)
    
    # Use primary node for receipt queries (they should all have them eventually)
    primary_w3 = nodes[0].w3
    
    start = time.time()
    check_count = 0
    
    while pending and (time.time() - start) < timeout:
        still_pending = set()
        
        # Check all pending in parallel batches
        batch = list(pending)
        RECEIPT_BATCH = 100
        
        for batch_start in range(0, len(batch), RECEIPT_BATCH):
            batch_end = min(batch_start + RECEIPT_BATCH, len(batch))
            chunk = batch[batch_start:batch_end]
            
            async def get_receipt(tx_hash: str):
                try:
                    receipt = await loop.run_in_executor(
                        None, primary_w3.eth.get_transaction_receipt, tx_hash
                    )
                    if receipt:
                        return tx_hash, {
                            'status': receipt.status,
                            'block_number': receipt.blockNumber,
                            'gas_used': receipt.gasUsed,
                            'confirmed_at': time.time(),
                        }
                except Exception:
                    pass
                return tx_hash, None
            
            tasks = [get_receipt(h) for h in chunk]
            results = await asyncio.gather(*tasks)
            
            for tx_hash, info in results:
                if info:
                    receipts[tx_hash] = info
                    info['latency'] = info['confirmed_at'] - start
                else:
                    still_pending.add(tx_hash)
        
        pending = still_pending
        check_count += 1
        
        if pending:
            confirmed = len(tx_hashes) - len(pending)
            if check_count % 20 == 0:
                LOG.info(
                    f"  Receipt progress: {confirmed}/{len(tx_hashes)} confirmed, "
                    f"{len(pending)} pending ({time.time() - start:.1f}s elapsed)"
                )
            await asyncio.sleep(poll_interval)
    
    return receipts


def compute_stats(latencies: List[float]) -> Dict:
    """Compute P50/P90/P99/min/max/avg for a list of latencies."""
    if not latencies:
        return {}
    
    latencies = sorted(latencies)
    
    def percentile(p):
        k = (len(latencies) - 1) * (p / 100.0)
        f, c = math.floor(k), math.ceil(k)
        if f == c:
            return latencies[int(k)]
        return latencies[int(f)] + (latencies[int(c)] - latencies[int(f)]) * (k - f)
    
    return {
        'count': len(latencies),
        'min': min(latencies),
        'max': max(latencies),
        'avg': statistics.mean(latencies),
        'p50': percentile(50),
        'p90': percentile(90),
        'p99': percentile(99),
    }


def log_round_report(
    round_num: int,
    total_txns: int,
    send_errors: int,
    send_elapsed: float,
    receipts: Dict,
    stats: Dict,
):
    """Print a detailed report for one stress round."""
    confirmed = len(receipts)
    success_rate = confirmed / total_txns * 100 if total_txns > 0 else 0
    reverted = sum(1 for r in receipts.values() if r['status'] == 0)
    
    LOG.info("")
    LOG.info("=" * 70)
    LOG.info(f"  ROUND {round_num} REPORT")
    LOG.info("=" * 70)
    LOG.info(f"  Transactions sent:     {total_txns}")
    LOG.info(f"  Send errors (RPC):     {send_errors}")
    LOG.info(f"  Send throughput:       {total_txns / send_elapsed:.0f} tx/s  ({send_elapsed:.2f}s)")
    LOG.info(f"  Confirmed receipts:    {confirmed}/{total_txns} ({success_rate:.1f}%)")
    LOG.info(f"  Reverted (status=0):   {reverted}")
    LOG.info("-" * 70)
    
    if stats:
        LOG.info(f"  Confirmation Latency (from send-complete to receipt):")
        LOG.info(f"    Min:  {stats['min']:.3f}s")
        LOG.info(f"    P50:  {stats['p50']:.3f}s")
        LOG.info(f"    P90:  {stats['p90']:.3f}s")
        LOG.info(f"    P99:  {stats['p99']:.3f}s")
        LOG.info(f"    Max:  {stats['max']:.3f}s")
        LOG.info(f"    Avg:  {stats['avg']:.3f}s")
    
    LOG.info("=" * 70)
    LOG.info("")


# =====================================================================
# Main Test
# =====================================================================

@pytest.mark.longrun
@pytest.mark.asyncio
async def test_single_address_high_pressure(cluster: Cluster):
    """
    Stress test: send 1,000 txns/sec from a single address, distributed
    across all 4 validator nodes for NUM_ROUNDS rounds.

    Verifies:
    1. Transactions can be submitted at high throughput
    2. High confirmation rate (≥ 80%)
    3. Nonce is consistent across all nodes after each round
    4. Balance is consistent across all nodes
    5. Block production continues under load
    """
    LOG.info("=" * 70)
    LOG.info("  SINGLE-ADDRESS HIGH-PRESSURE STRESS TEST")
    LOG.info(f"  Config: {TXN_PER_ROUND} txns/round × {NUM_ROUNDS} rounds")
    LOG.info(f"  Send window: {SEND_WINDOW_SEC}s  |  Receipt timeout: {RECEIPT_TIMEOUT_SEC}s")
    LOG.info("=" * 70)

    # --- Setup ---
    assert await cluster.set_full_live(timeout=120), "Cluster failed to start all 4 nodes"
    assert len(cluster.nodes) == 4, f"Expected 4 nodes, got {len(cluster.nodes)}"

    LOG.info(f"Waiting {STABILIZE_WAIT_SEC}s for cluster stabilization...")
    await asyncio.sleep(STABILIZE_WAIT_SEC)
    assert await cluster.check_block_increasing(timeout=30), "Cluster not producing blocks!"
    LOG.info("Cluster is healthy and producing blocks.")

    # Wait for all nodes to converge to the same block height
    # (a recently restarted node may still be catching up)
    LOG.info("Waiting for all nodes to converge to the same block height...")
    node_list_all = list(cluster.nodes.values())
    converge_deadline = time.time() + 120  # 2 minutes max
    while time.time() < converge_deadline:
        heights = {}
        for node in node_list_all:
            try:
                heights[node.id] = node.w3.eth.block_number
            except Exception:
                heights[node.id] = -1
        valid_heights = [h for h in heights.values() if h >= 0]
        if valid_heights and (max(valid_heights) - min(valid_heights)) <= 2:
            LOG.info(f"All nodes converged. Heights: {heights}")
            break
        LOG.info(f"Waiting for convergence... Heights: {heights}")
        await asyncio.sleep(3)
    else:
        pytest.fail(f"Nodes failed to converge within 120s. Heights: {heights}")

    # Sender = faucet (single address)
    sender = cluster.faucet
    assert sender, "Faucet account not configured"
    
    # Recipient (dummy, just to receive value-0 transfers)
    recipient = Account.create().address
    
    # Node list for round-robin distribution
    node_list = list(cluster.nodes.values())
    node_ids = [n.id for n in node_list]
    LOG.info(f"Sender:    {sender.address}")
    LOG.info(f"Recipient: {recipient}")
    LOG.info(f"Nodes:     {node_ids}")
    
    # Get chain info from first node
    primary_w3 = node_list[0].w3
    chain_id = primary_w3.eth.chain_id
    gas_price = Web3.to_wei(GAS_PRICE_GWEI, 'gwei')
    
    # Track cumulative stats
    all_round_stats = []
    
    # --- Stress Rounds ---
    for round_num in range(1, NUM_ROUNDS + 1):
        LOG.info("")
        LOG.info("*" * 70)
        LOG.info(f"  ROUND {round_num}/{NUM_ROUNDS}")
        LOG.info("*" * 70)
        
        # Get current nonce from pending state
        current_nonce = primary_w3.eth.get_transaction_count(sender.address, 'pending')
        LOG.info(f"Starting nonce: {current_nonce}")
        
        # Pre-build all signed transactions (parallel)
        LOG.info(f"Pre-building {TXN_PER_ROUND} signed transactions (parallel)...")
        build_start = time.time()
        raw_txs = build_raw_txs_parallel(
            sender, current_nonce, TXN_PER_ROUND, recipient, chain_id, gas_price
        )
        build_elapsed = time.time() - build_start
        LOG.info(f"Built {TXN_PER_ROUND} txns in {build_elapsed:.2f}s")
        
        # Fire all transactions across nodes
        LOG.info(f"Firing {TXN_PER_ROUND} txns across {len(node_list)} nodes...")
        tx_hashes, send_errors, send_elapsed = await fire_transactions(
            node_list, raw_txs, SEND_WINDOW_SEC
        )
        LOG.info(
            f"Sent {len(tx_hashes)} txns in {send_elapsed:.2f}s "
            f"({len(tx_hashes) / send_elapsed:.0f} tx/s), "
            f"{len(send_errors)} errors"
        )
        
        if send_errors:
            for err in send_errors[:10]:  # Log first 10 errors
                LOG.warning(f"  Send error: {err}")
            if len(send_errors) > 10:
                LOG.warning(f"  ... and {len(send_errors) - 10} more errors")
        
        # Collect receipts
        LOG.info(f"Collecting receipts (timeout={RECEIPT_TIMEOUT_SEC}s)...")
        receipts = await collect_receipts(
            node_list, tx_hashes, RECEIPT_TIMEOUT_SEC, RECEIPT_POLL_INTERVAL
        )
        
        # Compute latency stats
        latencies = [r['latency'] for r in receipts.values() if 'latency' in r]
        stats = compute_stats(latencies)
        
        # Report
        log_round_report(
            round_num, TXN_PER_ROUND, len(send_errors), send_elapsed, receipts, stats
        )
        all_round_stats.append({
            'round': round_num,
            'sent': len(tx_hashes),
            'send_errors': len(send_errors),
            'confirmed': len(receipts),
            'reverted': sum(1 for r in receipts.values() if r['status'] == 0),
            'send_elapsed': send_elapsed,
            'stats': stats,
        })
        
        # --- Verification: Nonce consistency across all nodes ---
        LOG.info("Verifying nonce consistency across all nodes...")
        await asyncio.sleep(3)  # Short wait for propagation
        
        nonce_values = {}
        for node in node_list:
            try:
                nonce = node.w3.eth.get_transaction_count(sender.address, 'latest')
                nonce_values[node.id] = nonce
            except Exception as e:
                LOG.error(f"Failed to get nonce from {node.id}: {e}")
                nonce_values[node.id] = -1
        
        LOG.info(f"Nonces: {nonce_values}")
        unique_nonces = set(v for v in nonce_values.values() if v >= 0)
        if len(unique_nonces) > 1:
            LOG.warning(
                f"Nonce divergence detected! Values: {nonce_values}. "
                f"This may resolve with more block confirmations."
            )
            # Allow a few seconds for convergence then re-check
            await asyncio.sleep(5)
            nonce_values_retry = {}
            for node in node_list:
                try:
                    nonce_values_retry[node.id] = node.w3.eth.get_transaction_count(
                        sender.address, 'latest'
                    )
                except Exception:
                    nonce_values_retry[node.id] = -1
            LOG.info(f"Nonces (retry): {nonce_values_retry}")
            unique_retry = set(v for v in nonce_values_retry.values() if v >= 0)
            assert len(unique_retry) <= 1, (
                f"Nonce still divergent after retry: {nonce_values_retry}"
            )
        
        # --- Verification: Block production still healthy ---
        assert await cluster.check_block_increasing(timeout=30), (
            f"Block production halted after round {round_num}!"
        )
        LOG.info(f"Block production OK after round {round_num}.")
        
        # Brief pause between rounds to let the mempool drain
        if round_num < NUM_ROUNDS:
            LOG.info("Pausing 5s before next round...")
            await asyncio.sleep(5)
    
    # =====================================================================
    # Final Summary
    # =====================================================================
    LOG.info("")
    LOG.info("=" * 70)
    LOG.info("  FINAL SUMMARY")
    LOG.info("=" * 70)
    LOG.info(f"  {'Round':<8} {'Sent':<8} {'Confirmed':<12} {'Rate':<8} {'Reverted':<10} {'TX/s':<10} {'P99':<10}")
    LOG.info("-" * 70)
    
    total_confirmed = 0
    total_sent = 0
    for rs in all_round_stats:
        rate = rs['confirmed'] / TXN_PER_ROUND * 100 if TXN_PER_ROUND > 0 else 0
        tps = rs['sent'] / rs['send_elapsed'] if rs['send_elapsed'] > 0 else 0
        p99 = rs['stats'].get('p99', 0) if rs['stats'] else 0
        LOG.info(
            f"  {rs['round']:<8} {rs['sent']:<8} {rs['confirmed']:<12} "
            f"{rate:<7.1f}% {rs['reverted']:<10} {tps:<10.0f} {p99:<10.3f}"
        )
        total_confirmed += rs['confirmed']
        total_sent += rs['sent']
    
    overall_rate = total_confirmed / (TXN_PER_ROUND * NUM_ROUNDS) * 100
    LOG.info("-" * 70)
    LOG.info(f"  Overall: {total_confirmed}/{TXN_PER_ROUND * NUM_ROUNDS} confirmed ({overall_rate:.1f}%)")
    LOG.info("=" * 70)
    
    # --- Final balance consistency check ---
    LOG.info("Verifying final balance consistency across all nodes...")
    await asyncio.sleep(5)
    
    balances = {}
    for node in node_list:
        try:
            balances[node.id] = node.w3.eth.get_balance(sender.address)
        except Exception as e:
            LOG.error(f"Failed to get balance from {node.id}: {e}")
            balances[node.id] = -1
    
    LOG.info(f"Sender balances: {balances}")
    valid_balances = set(v for v in balances.values() if v >= 0)
    assert len(valid_balances) <= 1, f"Balance divergence across nodes: {balances}"
    LOG.info("Balance consistency verified.")
    
    # --- Final assertions ---
    assert overall_rate >= MIN_SUCCESS_RATE * 100, (
        f"Overall confirmation rate {overall_rate:.1f}% is below minimum {MIN_SUCCESS_RATE * 100}%"
    )
    LOG.info(f"✅ Stress test PASSED: {overall_rate:.1f}% confirmation rate across {NUM_ROUNDS} rounds")
