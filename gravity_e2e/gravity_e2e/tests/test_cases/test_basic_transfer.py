import asyncio
import logging
from typing import Dict

from ...helpers.test_helpers import RunHelper, TestResult, test_case

LOG = logging.getLogger(__name__)


@test_case
async def test_eth_transfer(run_helper: RunHelper, test_result: TestResult):
    """Test basic ETH transfer functionality"""
    LOG.info("Starting ETH transfer test")
    
    # 1. Create test accounts
    sender = await run_helper.create_test_account("sender", fund_wei=10**18)  # 1 ETH
    receiver = await run_helper.create_test_account("receiver")
    
    LOG.info(f"Sender: {sender['address']}")
    LOG.info(f"Receiver: {receiver['address']}")
    
    # 2. Get pre-transfer balances
    sender_balance_before = await run_helper.client.get_balance(sender["address"])
    receiver_balance_before = await run_helper.client.get_balance(receiver["address"])
    
    LOG.info(f"Sender balance before: {sender_balance_before / 10**18:.6f} ETH")
    LOG.info(f"Receiver balance before: {receiver_balance_before / 10**18:.6f} ETH")
    
    # 3. Execute transfer
    transfer_amount = 10**17  # 0.1 ETH
    
    # Get current gas price
    gas_price = await run_helper.client.get_gas_price()
    gas_limit = 21000
    
    # Calculate total cost
    total_cost = transfer_amount + (gas_price * gas_limit)
    
    # Get sender nonce
    nonce = await run_helper.client.get_transaction_count(sender["address"])
    
    # Build transaction
    tx_data = {
        "to": receiver["address"],
        "value": hex(transfer_amount),
        "gas": hex(gas_limit),
        "gasPrice": hex(gas_price),
        "nonce": hex(nonce),
        "chainId": hex(await run_helper.client.get_chain_id())
    }
    
    # Sign transaction
    from eth_account import Account
    signed_tx = Account.sign_transaction(
        tx_data, 
        sender["private_key"]
    )
    
    # Send transaction
    tx_hash = await run_helper.client.send_raw_transaction(signed_tx.raw_transaction)
    LOG.info(f"Transfer transaction sent: {tx_hash}")
    
    # Wait for transaction confirmation
    receipt = await run_helper.client.wait_for_transaction_receipt(tx_hash, timeout=60)
    
    if receipt["status"] != "0x1":
        raise RuntimeError(f"Transfer transaction failed: {receipt}")
    
    # 4. Verify post-transfer balances
    sender_balance_after = await run_helper.client.get_balance(sender["address"])
    receiver_balance_after = await run_helper.client.get_balance(receiver["address"])
    
    LOG.info(f"Sender balance after: {sender_balance_after / 10**18:.6f} ETH")
    LOG.info(f"Receiver balance after: {receiver_balance_after / 10**18:.6f} ETH")
    
    # Verify balance changes
    expected_receiver_balance = receiver_balance_before + transfer_amount
    expected_sender_balance = sender_balance_before - total_cost
    
    # Allow some tolerance (due to possible fee variations)
    balance_tolerance = 10**15  # 0.001 ETH
    
    if abs(receiver_balance_after - expected_receiver_balance) > balance_tolerance:
        raise RuntimeError(
            f"Receiver balance mismatch: expected {expected_receiver_balance}, "
            f"got {receiver_balance_after}"
        )
    
    if abs(sender_balance_after - expected_sender_balance) > balance_tolerance:
        raise RuntimeError(
            f"Sender balance mismatch: expected {expected_sender_balance}, "
            f"got {sender_balance_after}"
        )
    
    # Record test results
    test_result.mark_success(
        tx_hash=tx_hash,
        transfer_amount=transfer_amount,
        gas_used=int(receipt.get("gasUsed", "0x0"), 16),
        sender_final_balance=sender_balance_after,
        receiver_final_balance=receiver_balance_after
    )
    
    LOG.info("ETH transfer test completed successfully")