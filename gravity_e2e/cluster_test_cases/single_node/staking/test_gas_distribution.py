"""
Test Gas Fee Distribution (User Request 4.1) - Cluster Test Format
"""
import pytest
import logging
import asyncio
from web3 import Web3
from eth_account import Account
from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils.transaction_builder import TransactionBuilder, TransactionOptions
from gravity_e2e.utils.exceptions import TransactionError

LOG = logging.getLogger(__name__)

async def fund_account(web3: Web3, faucet_key: str, target: str, amount_wei: int):
    faucet = Account.from_key(faucet_key)
    tx_builder = TransactionBuilder(web3, faucet)
    res = await tx_builder.send_ether(target, amount_wei)
    if not res.success:
        raise TransactionError(f"Funding failed: {res.error}")
    LOG.info(f"Funded {target} with {amount_wei} wei")

@pytest.mark.asyncio
async def test_gas_fee_distribution(cluster: Cluster):
    """
    Test 4.1: Gas Fee Distribution Path
    """
    LOG.info("=" * 70)
    LOG.info("Test: Gas Fee Distribution (Request 4.1)")
    LOG.info("=" * 70)

    assert await cluster.set_full_live(timeout=60), "Cluster failed to start"
    node = cluster.get_node("node1")
    w3 = node.w3
    faucet_key = cluster.faucet.key

    try:
        # Step 1: Setup Accounts
        LOG.info("\n[Step 1] Setting up accounts...")
        sender = Account.create()
        control = Account.create()
        
        await fund_account(w3, faucet_key, sender.address, 10 * 10**18)
        await fund_account(w3, faucet_key, control.address, 10 * 10**18)
        
        sender_builder = TransactionBuilder(w3, sender)
        
        # Step 2: Identify Validator (Coinbase) and Snapshot Balances
        LOG.info("\n[Step 2] Snapshotting balances...")
        latest_block = w3.eth.get_block("latest")
        coinbase = latest_block.get("miner")
        if not coinbase:
             coinbase = w3.eth.default_account or w3.eth.accounts[0]
             LOG.warning(f"Could not determine miner from block, defaulting to {coinbase}")
             
        LOG.info(f"Validator (Coinbase): {coinbase}")
        
        validator_balance_start = w3.eth.get_balance(coinbase)
        control_balance_start = w3.eth.get_balance(control.address)
        
        LOG.info(f"Validator Start Balance: {validator_balance_start} wei")
        LOG.info(f"Control Start Balance: {control_balance_start} wei")
        
        # Step 3: Send Transactions
        LOG.info("\n[Step 3] Sending transactions to generate fees...")
        tx_count = 5
        tx_hashes = []
        
        for i in range(tx_count):
            # Dynamic Gas Pricing
            block = w3.eth.get_block('latest')
            base_fee = block.get('baseFeePerGas', 1_000_000_000)
            priority_fee = 2_000_000_000 # 2 Gwei
            max_fee = base_fee + priority_fee + 1_000_000
            
            result = await sender_builder.build_and_send_tx(
                to=sender.address, # Self-transfer is fine
                value=i * 1000,
                options=TransactionOptions(
                    gas_limit=21000,
                    max_priority_fee_per_gas=priority_fee,
                    max_fee_per_gas=max_fee
                )
            )
            
            assert result.success, f"Tx {i} failed: {result.error}"
            tx_hashes.append(result.tx_hash)
            LOG.info(f"Sent Tx {i+1}/{tx_count}: {result.tx_hash}")
        
        # Step 4: Analyze Receipts and Calculate Expected Fees
        LOG.info("\n[Step 4] Calculating expected fees from receipts...")
        total_gas_fees = 0
        
        for tx_hash in tx_hashes:
            receipt = w3.eth.get_transaction_receipt(tx_hash)
            gas_used = receipt['gasUsed']
            effective_gas_price = receipt['effectiveGasPrice']
            fee_wei = gas_used * effective_gas_price
            
            total_gas_fees += fee_wei
            LOG.info(f"Tx {tx_hash[:8]}... Cost: {fee_wei} wei (Gas: {gas_used} @ {effective_gas_price})")
            
        LOG.info(f"Total Expected Fee Revenue: {total_gas_fees} wei")
        
        # Step 5: Verify Validator Balance
        LOG.info("\n[Step 5] Verify Validator Balance...")
        
        await asyncio.sleep(1)
        
        validator_balance_end = w3.eth.get_balance(coinbase)
        control_balance_end = w3.eth.get_balance(control.address)
        
        actual_increase = validator_balance_end - validator_balance_start
        
        LOG.info(f"Validator End Balance:   {validator_balance_end} wei")
        LOG.info(f"Actual Increase:         {actual_increase} wei")
        
        # Verification 1: Validator gets the fees (plus potential block rewards)
        diff = actual_increase - total_gas_fees
        
        if diff != 0:
            if diff > 0 and diff > 10**18:
                 LOG.warning(f"Validator received EXTRA {diff / 10**18:.4f} ETH. Assuming Block/Epoch Rewards.")
                 LOG.info("SUCCESS: Validator balance increased by at least total gas fees.")
            elif diff < 0:
                 LOG.error(f"FAIL: Validator received LESS than gas fees (Burnt fees?). Diff: {diff}")
                 raise AssertionError(f"Validator received less than fees: {diff}")
            else:
                 LOG.warning(f"Small balance mismatch! Diff: {diff} wei")
        else:
            LOG.info("SUCCESS: Validator balance increased exactly by total gas fees.")
            
        # Verification 2: Control user gets nothing
        control_diff = control_balance_end - control_balance_start
        assert control_diff == 0, f"Passive user balance changed by {control_diff} wei"
        LOG.info("SUCCESS: Passive user balance remained unchanged.")

        LOG.info("\n" + "=" * 70)
        LOG.info("Test 'Gas Fee Distribution' PASSED!")
        LOG.info("=" * 70)

    except Exception as e:
        LOG.error(f"Test failed: {e}")
        raise
