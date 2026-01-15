"""
Test basic ETH transfer functionality

This test verifies that ETH transfers work correctly using the
utility modules for cleaner, more maintainable code.

Run with:
    cd gravity_e2e
    pytest gravity_e2e/tests/test_cases/test_basic_transfer.py -v
"""

import logging

from gravity_e2e.helpers.test_helpers import RunHelper, TestResult, handle_test_exception, test_case
from gravity_e2e.utils.transaction_builder import TransactionBuilder, TransactionOptions
from gravity_e2e.utils.exceptions import TransactionError

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

    # 2. Initialize transaction builder with sender's account
    tx_builder = TransactionBuilder(
        web3=run_helper.client.web3,
        account=sender['account']
    )

    # 3. Get pre-transfer balances
    sender_balance_before = await run_helper.client.get_balance(sender["address"])
    receiver_balance_before = await run_helper.client.get_balance(receiver["address"])

    LOG.info(f"Sender balance before: {sender_balance_before / 10**18:.6f} ETH")
    LOG.info(f"Receiver balance before: {receiver_balance_before / 10**18:.6f} ETH")

    # 4. Execute transfer using transaction builder
    transfer_amount = 10**17  # 0.1 ETH

    # Define transaction options
    options = TransactionOptions(
        value=transfer_amount,
        gas_limit=21000
    )

    # Build and send transaction
    result = await tx_builder.build_and_send_tx(
        to=receiver["address"],
        options=options,
        simulate=True  # Simulate before sending
    )

    # Check if transaction was successful
    assert result.success, f"Transfer transaction failed: {result.error}"

    LOG.info(f"Transfer successful: {result.tx_hash}")
    LOG.info(f"Gas used: {result.gas_used}")

    # 5. Verify post-transfer balances
    sender_balance_after = await run_helper.client.get_balance(sender["address"])
    receiver_balance_after = await run_helper.client.get_balance(receiver["address"])

    LOG.info(f"Sender balance after: {sender_balance_after / 10**18:.6f} ETH")
    LOG.info(f"Receiver balance after: {receiver_balance_after / 10**18:.6f} ETH")

    # Verify balance changes
    total_cost = result.gas_used * (await run_helper.client.get_gas_price()) + transfer_amount
    expected_receiver_balance = receiver_balance_before + transfer_amount
    expected_sender_balance = sender_balance_before - total_cost

    # Allow some tolerance (due to possible fee variations)
    balance_tolerance = 10**15  # 0.001 ETH

    assert abs(receiver_balance_after - expected_receiver_balance) <= balance_tolerance, \
        f"Receiver balance mismatch: expected {expected_receiver_balance}, got {receiver_balance_after}"

    assert abs(sender_balance_after - expected_sender_balance) <= balance_tolerance, \
        f"Sender balance mismatch: expected {expected_sender_balance}, got {sender_balance_after}"

    # Record test results
    test_result.mark_success(
        tx_hash=result.tx_hash,
        transfer_amount=transfer_amount,
        gas_used=result.gas_used,
        sender_final_balance=sender_balance_after,
        receiver_final_balance=receiver_balance_after,
        total_cost=total_cost
    )

    LOG.info("ETH transfer test completed successfully")


@test_case
async def test_multiple_transfers(run_helper: RunHelper, test_result: TestResult):
    """Test multiple ETH transfers to verify nonce management"""
    LOG.info("Starting multiple transfers test")

    # 1. Create accounts
    sender = await run_helper.create_test_account("sender", fund_wei=5 * 10**18)  # 5 ETH
    receivers = []

    # Create 3 receiver accounts
    for i in range(3):
        receiver = await run_helper.create_test_account(f"receiver_{i}")
        receivers.append(receiver)

    # 2. Initialize transaction builder
    tx_builder = TransactionBuilder(
        web3=run_helper.client.web3,
        account=sender['account']
    )

    # 3. Send transfers sequentially
    transfer_amount = 10**17  # 0.1 ETH each
    results = []

    for i, receiver in enumerate(receivers):
        LOG.info(f"Sending transfer {i+1}/3 to {receiver['address']}")

        # Build and send transaction
        result = await tx_builder.send_ether(
            to=receiver["address"],
            amount_wei=transfer_amount
        )

        assert result.success, f"Transfer {i+1} failed: {result.error}"

        results.append(result)
        LOG.info(f"Transfer {i+1} successful: {result.tx_hash}")

    # 4. Verify all transactions succeeded
    total_gas_used = sum(r.gas_used for r in results)

    # 5. Verify final receiver balances
    for i, receiver in enumerate(receivers):
        balance = await run_helper.client.get_balance(receiver["address"])
        LOG.info(f"Receiver {i} balance: {balance / 10**18:.6f} ETH")

        assert balance >= transfer_amount, f"Receiver {i} received insufficient funds"

    # Record test results
    test_result.mark_success(
        total_transfers=len(results),
        total_amount=transfer_amount * len(receivers),
        total_gas_used=total_gas_used,
        transaction_hashes=[r.tx_hash for r in results]
    )

    LOG.info("Multiple transfers test completed successfully")


@test_case
async def test_transfer_with_insufficient_funds(run_helper: RunHelper, test_result: TestResult):
    """Test that transfers with insufficient funds fail gracefully"""
    LOG.info("Starting insufficient funds test")

    # 1. Create accounts
    sender = await run_helper.create_test_account("sender", fund_wei=2 * 10**17)  # 0.2 ETH
    receiver = await run_helper.create_test_account("receiver")

    # 2. Initialize transaction builder
    tx_builder = TransactionBuilder(
        web3=run_helper.client.web3,
        account=sender['account']
    )

    # 3. Attempt to transfer more than balance
    transfer_amount = 10**19  # 10 ETH (more than sender has)

    # Build transaction (but don't send)
    try:
        tx = await tx_builder.build_transaction(
            to=receiver["address"],
            options=TransactionOptions(value=transfer_amount)
        )

        # Simulate transaction to check if it would fail
        sim_result = await tx_builder.simulate_transaction(tx)

        if sim_result['success']:
            # If simulation succeeds, try to send (should fail)
            result = await tx_builder.send_transaction(tx)

            if result.success:
                pytest.fail("Transaction with insufficient funds unexpectedly succeeded")
            else:
                LOG.info(f"Transaction correctly failed: {result.error}")
        else:
            LOG.info(f"Transaction simulation correctly failed: {sim_result['error']}")

    except TransactionError as e:
        # Expected failure
        LOG.info(f"Transfer correctly failed with error: {e}")

    # Record test success (failure was expected)
    test_result.mark_success(
        transfer_attempted=transfer_amount,
        sender_balance=await run_helper.client.get_balance(sender["address"]),
        expected_failure=True
    )

    LOG.info("Insufficient funds test completed successfully")
