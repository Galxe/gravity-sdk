"""
Test basic ETH transfer functionality - Pytest version

This module provides pytest-compatible versions of the basic transfer tests,
demonstrating how to use pytest fixtures and async test patterns.

Run with:
    cd gravity_e2e
    pytest gravity_e2e/tests/test_cases/test_basic_transfer_pytest.py -v
"""

import pytest
import logging

from gravity_e2e.helpers.test_helpers import RunHelper, TestResult, handle_test_exception
from gravity_e2e.utils.transaction_builder import TransactionBuilder, TransactionOptions
from gravity_e2e.utils.exceptions import TransactionError

LOG = logging.getLogger(__name__)


@pytest.mark.asyncio
async def test_eth_transfer_pytest(run_helper: RunHelper, test_result: TestResult):
    """
    Test basic ETH transfer functionality using pytest fixtures.

    This test:
    1. Creates sender and receiver accounts
    2. Funds sender from faucet
    3. Transfers ETH from sender to receiver
    4. Verifies balances
    """
    LOG.info("Starting ETH transfer test (pytest version)")

    # 1. Create test accounts
    sender = await run_helper.create_test_account("sender", fund_wei=10**18)
    receiver = await run_helper.create_test_account("receiver")

    LOG.info(f"Sender: {sender['address']}")
    LOG.info(f"Receiver: {receiver['address']}")

    # 2. Initialize transaction builder
    tx_builder = TransactionBuilder(
        web3=run_helper.client.web3,
        account=sender['account']
    )

    # 3. Get pre-transfer balances
    sender_balance_before = await run_helper.client.get_balance(sender["address"])
    receiver_balance_before = await run_helper.client.get_balance(receiver["address"])

    LOG.info(f"Sender balance before: {sender_balance_before / 10**18:.6f} ETH")
    LOG.info(f"Receiver balance before: {receiver_balance_before / 10**18:.6f} ETH")

    # 4. Execute transfer
    transfer_amount = 10**17  # 0.1 ETH

    options = TransactionOptions(
        value=transfer_amount,
        gas_limit=21000
    )

    result = await tx_builder.build_and_send_tx(
        to=receiver["address"],
        options=options,
        simulate=True
    )

    # 5. Verify transaction success
    assert result.success, f"Transfer failed: {result.error}"

    LOG.info(f"Transfer successful: {result.tx_hash}")
    LOG.info(f"Gas used: {result.gas_used}")

    # 6. Verify post-transfer balances
    sender_balance_after = await run_helper.client.get_balance(sender["address"])
    receiver_balance_after = await run_helper.client.get_balance(receiver["address"])

    LOG.info(f"Sender balance after: {sender_balance_after / 10**18:.6f} ETH")
    LOG.info(f"Receiver balance after: {receiver_balance_after / 10**18:.6f} ETH")

    # Verify receiver got the funds
    assert receiver_balance_after >= transfer_amount, \
        f"Receiver balance too low: {receiver_balance_after}"

    # Mark test success
    test_result.mark_success(
        tx_hash=result.tx_hash,
        transfer_amount=transfer_amount,
        gas_used=result.gas_used
    )

    LOG.info("ETH transfer test completed successfully")


@pytest.mark.asyncio
async def test_multiple_transfers_pytest(run_helper: RunHelper, test_result: TestResult):
    """
    Test multiple sequential ETH transfers.

    This test verifies:
    1. Nonce management across multiple transactions
    2. Balance updates after each transfer
    3. All transactions confirm successfully
    """
    LOG.info("Starting multiple transfers test (pytest version)")

    # 1. Create accounts
    sender = await run_helper.create_test_account("sender", fund_wei=5 * 10**18)
    receivers = []

    for i in range(3):
        receiver = await run_helper.create_test_account(f"receiver_{i}")
        receivers.append(receiver)

    # 2. Initialize transaction builder
    tx_builder = TransactionBuilder(
        web3=run_helper.client.web3,
        account=sender['account']
    )

    # 3. Execute transfers
    transfer_amount = 10**17  # 0.1 ETH each
    results = []

    for i, receiver in enumerate(receivers):
        LOG.info(f"Sending transfer {i+1}/3 to {receiver['address']}")

        result = await tx_builder.send_ether(
            to=receiver["address"],
            amount_wei=transfer_amount
        )

        assert result.success, f"Transfer {i+1} failed: {result.error}"
        results.append(result)
        LOG.info(f"Transfer {i+1} successful: {result.tx_hash}")

    # 4. Verify all receiver balances
    for i, receiver in enumerate(receivers):
        balance = await run_helper.client.get_balance(receiver["address"])
        assert balance >= transfer_amount, \
            f"Receiver {i} balance too low: {balance}"
        LOG.info(f"Receiver {i} balance: {balance / 10**18:.6f} ETH")

    # 5. Mark test success
    total_gas_used = sum(r.gas_used for r in results)
    test_result.mark_success(
        total_transfers=len(results),
        total_gas_used=total_gas_used,
        transaction_hashes=[r.tx_hash for r in results]
    )

    LOG.info("Multiple transfers test completed successfully")


@pytest.mark.asyncio
async def test_transfer_insufficient_funds_pytest(run_helper: RunHelper, test_result: TestResult):
    """
    Test that transfers with insufficient funds fail gracefully.

    This test verifies:
    1. Simulation detects insufficient funds
    2. Transaction reverts correctly
    3. Error handling is proper
    """
    LOG.info("Starting insufficient funds test (pytest version)")

    # 1. Create accounts with small balance
    sender = await run_helper.create_test_account("sender", fund_wei=2 * 10**17)
    receiver = await run_helper.create_test_account("receiver")

    # 2. Initialize transaction builder
    tx_builder = TransactionBuilder(
        web3=run_helper.client.web3,
        account=sender['account']
    )

    # 3. Attempt to transfer more than balance
    transfer_amount = 10**19  # 10 ETH (more than sender has)

    # 4. Build and simulate - should fail
    try:
        tx = await tx_builder.build_transaction(
            to=receiver["address"],
            options=TransactionOptions(value=transfer_amount)
        )

        sim_result = await tx_builder.simulate_transaction(tx)

        if sim_result['success']:
            # Try to send - should fail
            result = await tx_builder.send_transaction(tx)
            assert not result.success, "Transaction with insufficient funds should fail"
        else:
            LOG.info(f"Simulation correctly failed: {sim_result['error']}")

    except TransactionError as e:
        LOG.info(f"Transfer correctly failed: {e}")

    # 5. Mark test success (failure was expected)
    test_result.mark_success(
        expected_failure=True,
        transfer_attempted=transfer_amount
    )

    LOG.info("Insufficient funds test completed successfully")
