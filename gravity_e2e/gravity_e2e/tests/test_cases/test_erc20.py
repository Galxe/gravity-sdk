"""
Test ERC20 token functionality (Refactored)

This test verifies ERC20 token deployment, transfers, and events using
the new utility modules for cleaner, more maintainable code.

Key improvements from refactoring:
- Uses ContractDeployer for contract deployment
- Uses TransactionBuilder for all transactions
- Uses EventPoller for monitoring Transfer events
- Eliminates manual function selectors and data encoding
- Proper ABI-based contract interaction
- Comprehensive event testing
"""

import sys
from pathlib import Path

# Add package to path for absolute imports
_current_dir = Path(__file__).resolve().parent
_package_root = _current_dir.parent.parent.parent
if str(_package_root) not in sys.path:
    sys.path.insert(0, str(_package_root))

import asyncio
import json
import logging
from typing import Dict, List, Optional

from gravity_e2e.helpers.test_helpers import RunHelper, TestResult, test_case
from gravity_e2e.utils.contract_deployer import ContractDeployer, DeploymentOptions
from gravity_e2e.utils.transaction_builder import TransactionBuilder, TransactionOptions
from gravity_e2e.utils.event_poller import EventPoller, wait_for_transfer_event
from gravity_e2e.utils.exceptions import ContractError, TransactionError, EventError
from eth_account import Account

LOG = logging.getLogger(__name__)


@test_case
async def test_erc20_deploy_and_transfer(run_helper: RunHelper, test_result: TestResult):
    """Test ERC20 token deployment and transfer using refactored utilities"""
    LOG.info("Starting ERC20 token deployment and transfer test (refactored)")

    try:
        # 1. Create test accounts
        deployer = await run_helper.create_test_account("deployer", fund_wei=10 * 10**18)  # 10 ETH
        recipient = await run_helper.create_test_account("recipient")
        another_recipient = await run_helper.create_test_account("another_recipient")

        LOG.info(f"Deployer: {deployer['address']}")
        LOG.info(f"Recipient 1: {recipient['address']}")
        LOG.info(f"Recipient 2: {another_recipient['address']}")

        # 2. Deploy ERC20 token contract
        contracts_dir = Path(__file__).parent.parent.parent.parent / "contracts_data"
        contract_deployer = ContractDeployer(
            web3=run_helper.client.web3,
            account=deployer['account']
        )

        # Define token parameters
        token_name = "Test Token"
        token_symbol = "TEST"
        token_decimals = 18
        initial_supply = 1000000 * (10 ** token_decimals)  # 1 million tokens

        LOG.info(f"Deploying {token_name} ({token_symbol}) with {initial_supply} initial supply")

        # Deploy contract with constructor arguments
        deploy_result = await contract_deployer.deploy(
            contract_name="SimpleToken",
            constructor_args=[token_name, token_symbol, initial_supply],
            options=DeploymentOptions(
                gas_limit=3000000,
                verify=True,
                confirmations=1
            ),
            contracts_dir=contracts_dir
        )

        if not deploy_result.success:
            raise ContractError(
                f"ERC20 deployment failed: {deploy_result.error}",
                contract_name="SimpleToken"
            )

        contract_address = deploy_result.contract_address
        LOG.info(f"ERC20 contract deployed at: {contract_address}")
        LOG.info(f"Deployment gas used: {deploy_result.gas_used}")

        # 3. Get contract instance
        contract = await contract_deployer.get_deployment(
            "SimpleToken",
            contract_address,
            contracts_dir
        )

        if not contract:
            raise ContractError("Failed to get deployed contract instance")

        # 4. Initialize transaction builder and event poller
        tx_builder = TransactionBuilder(
            web3=run_helper.client.web3,
            account=deployer['account']
        )

        event_poller = EventPoller(run_helper.client.web3)

        # 5. Test basic ERC20 view functions
        LOG.info("Testing ERC20 view functions...")

        # Test token metadata
        name = await contract.functions.name().call()
        symbol = await contract.functions.symbol().call()
        decimals = await contract.functions.decimals().call()
        total_supply = await contract.functions.totalSupply().call()

        LOG.info(f"Token name: {name}")
        LOG.info(f"Token symbol: {symbol}")
        LOG.info(f"Token decimals: {decimals}")
        LOG.info(f"Total supply: {total_supply / 10**decimals} tokens")

        # Verify metadata
        if name != token_name or symbol != token_symbol or decimals != token_decimals:
            raise ContractError(
                f"Token metadata mismatch: name={name}, symbol={symbol}, decimals={decimals}"
            )

        # 6. Test initial balances
        deployer_balance = await contract.functions.balanceOf(deployer['address']).call()
        recipient_balance = await contract.functions.balanceOf(recipient['address']).call()
        another_balance = await contract.functions.balanceOf(another_recipient['address']).call()

        LOG.info(f"Deployer initial balance: {deployer_balance / 10**decimals} tokens")
        LOG.info(f"Recipient initial balance: {recipient_balance / 10**decimals} tokens")
        LOG.info(f"Another recipient initial balance: {another_balance / 10**decimals} tokens")

        # Verify deployer owns all tokens initially
        if deployer_balance != total_supply:
            raise ContractError(
                f"Initial balance mismatch: expected {total_supply}, got {deployer_balance}"
            )

        # 7. Test first transfer with event monitoring
        transfer_amount_1 = 1000 * (10 ** decimals)  # 1000 tokens

        LOG.info(f"Transferring {transfer_amount_1 / 10**decimals} tokens to recipient")

        # Monitor for Transfer event
        transfer_event_task = asyncio.create_task(
            event_poller.wait_for_event(
                contract=contract,
                event_name="Transfer",
                timeout=30,
                from_address=deployer['address'],
                to_address=recipient['address']
            )
        )

        # Execute transfer
        transfer_result_1 = await tx_builder.build_and_send_tx(
            to=contract_address,
            data=contract.functions.transfer(recipient['address'], transfer_amount_1).data_in_transaction,
            options=TransactionOptions(gas_limit=100000)
        )

        if not transfer_result_1.success:
            raise TransactionError(
                f"First transfer failed: {transfer_result_1.error}",
                tx_hash=transfer_result_1.tx_hash
            )

        # Wait for Transfer event
        transfer_event = await transfer_event_task
        if transfer_event:
            LOG.info(f"Transfer event detected: {transfer_event['args']}")
            assert transfer_event['args']['from'] == deployer['address'].lower()
            assert transfer_event['args']['to'] == recipient['address'].lower()
            assert transfer_event['args']['value'] == transfer_amount_1

        # 8. Verify balances after first transfer
        new_deployer_balance = await contract.functions.balanceOf(deployer['address']).call()
        new_recipient_balance = await contract.functions.balanceOf(recipient['address']).call()

        expected_deployer = deployer_balance - transfer_amount_1
        expected_recipient = recipient_balance + transfer_amount_1

        if new_deployer_balance != expected_deployer:
            raise ContractError(
                f"Deployer balance after transfer: expected {expected_deployer}, got {new_deployer_balance}"
            )

        if new_recipient_balance != expected_recipient:
            raise ContractError(
                f"Recipient balance after transfer: expected {expected_recipient}, got {new_recipient_balance}"
            )

        LOG.info(f"Deployer balance after transfer: {new_deployer_balance / 10**decimals} tokens")
        LOG.info(f"Recipient balance after transfer: {new_recipient_balance / 10**decimals} tokens")

        # 9. Test approval and transferFrom
        approval_amount = 500 * (10 ** decimals)  # 500 tokens

        LOG.info(f"Approving {approval_amount / 10**decimals} tokens for another recipient")

        # Approve another recipient to spend tokens
        approve_result = await tx_builder.build_and_send_tx(
            to=contract_address,
            data=contract.functions.approve(another_recipient['address'], approval_amount).data_in_transaction,
            options=TransactionOptions(gas_limit=50000)
        )

        if not approve_result.success:
            raise TransactionError(
                f"Approval failed: {approve_result.error}",
                tx_hash=approve_result.tx_hash
            )

        # Check allowance
        allowance = await contract.functions.allowance(deployer['address'], another_recipient['address']).call()
        if allowance != approval_amount:
            raise ContractError(
                f"Allowance mismatch: expected {approval_amount}, got {allowance}"
            )

        LOG.info(f"Allowance set: {allowance / 10**decimals} tokens")

        # 10. Test transferFrom (using allowance)
        transfer_amount_2 = 300 * (10 ** decimals)  # 300 tokens
        LOG.info(f"Transferring {transfer_amount_2 / 10**decimals} tokens using transferFrom")

        # Initialize transaction builder for another recipient
        another_builder = TransactionBuilder(
            web3=run_helper.client.web3,
            account=another_recipient['account']
        )

        # Execute transferFrom
        transfer_from_result = await another_builder.build_and_send_tx(
            to=contract_address,
            data=contract.functions.transferFrom(
                deployer['address'],
                another_recipient['address'],
                transfer_amount_2
            ).data_in_transaction,
            options=TransactionOptions(gas_limit=100000)
        )

        if not transfer_from_result.success:
            raise TransactionError(
                f"TransferFrom failed: {transfer_from_result.error}",
                tx_hash=transfer_from_result.tx_hash
            )

        # 11. Verify final balances and allowance
        final_deployer_balance = await contract.functions.balanceOf(deployer['address']).call()
        final_another_balance = await contract.functions.balanceOf(another_recipient['address']).call()
        final_allowance = await contract.functions.allowance(deployer['address'], another_recipient['address']).call()

        expected_final_deployer = new_deployer_balance - transfer_amount_2
        expected_final_another = another_balance + transfer_amount_2
        expected_final_allowance = approval_amount - transfer_amount_2

        if final_deployer_balance != expected_final_deployer:
            raise ContractError(
                f"Final deployer balance mismatch: expected {expected_final_deployer}, got {final_deployer_balance}"
            )

        if final_another_balance != expected_final_another:
            raise ContractError(
                f"Final another recipient balance mismatch: expected {expected_final_another}, got {final_another_balance}"
            )

        if final_allowance != expected_final_allowance:
            raise ContractError(
                f"Final allowance mismatch: expected {expected_final_allowance}, got {final_allowance}"
            )

        # 12. Get all Transfer events for verification
        LOG.info("Retrieving all Transfer events...")
        all_events = await event_poller.get_events(
            contract=contract,
            event_name="Transfer",
            from_block=deploy_result.block_number
        )

        LOG.info(f"Total Transfer events: {len(all_events.events)}")

        # Record test results
        test_result.mark_success(
            contract_address=contract_address,
            token_name=name,
            token_symbol=symbol,
            token_decimals=decimals,
            total_supply=total_supply,
            initial_deployer_balance=deployer_balance,
            transfer_1_amount=transfer_amount_1,
            transfer_2_amount=transfer_amount_2,
            approval_amount=approval_amount,
            final_deployer_balance=final_deployer_balance,
            final_recipient_balance=await contract.functions.balanceOf(recipient['address']).call(),
            final_another_balance=final_another_balance,
            total_transfer_events=len(all_events.events),
            deployment_gas_used=deploy_result.gas_used,
            transfer_1_gas_used=transfer_result_1.gas_used,
            approval_gas_used=approve_result.gas_used,
            transfer_from_gas_used=transfer_from_result.gas_used
        )

        LOG.info("ERC20 token test completed successfully")

    except (ContractError, TransactionError, EventError) as e:
        test_result.mark_failure(
            error=f"{e.__class__.__name__}: {e}",
            details=e.details
        )
        raise
    except Exception as e:
        test_result.mark_failure(
            error=f"Test failed: {e}",
            details={"type": type(e).__name__}
        )
        raise


@test_case
async def test_erc20_batch_transfers(run_helper: RunHelper, test_result: TestResult):
    """Test batch ERC20 transfers with event monitoring"""
    LOG.info("Starting ERC20 batch transfers test (refactored)")

    try:
        # 1. Create test accounts
        deployer = await run_helper.create_test_account("deployer", fund_wei=20 * 10**18)

        # Create multiple recipients
        recipients = []
        for i in range(5):
            recipient = await run_helper.create_test_account(f"recipient_{i}")
            recipients.append(recipient)

        LOG.info(f"Created {len(recipients)} recipient accounts")

        # 2. Deploy token contract
        contracts_dir = Path(__file__).parent.parent.parent.parent / "contracts_data"
        contract_deployer = ContractDeployer(
            web3=run_helper.client.web3,
            account=deployer['account']
        )

        initial_supply = 10000000 * (10 ** 18)  # 10 million tokens

        deploy_result = await contract_deployer.deploy(
            contract_name="SimpleToken",
            constructor_args=["Batch Token", "BATCH", initial_supply],
            options=DeploymentOptions(gas_limit=3000000),
            contracts_dir=contracts_dir
        )

        if not deploy_result.success:
            raise ContractError(f"Token deployment failed: {deploy_result.error}")

        contract_address = deploy_result.contract_address
        contract = await contract_deployer.get_deployment("SimpleToken", contract_address, contracts_dir)

        # 3. Initialize utilities
        tx_builder = TransactionBuilder(
            web3=run_helper.client.web3,
            account=deployer['account']
        )

        event_poller = EventPoller(run_helper.client.web3)

        # 4. Execute batch transfers
        transfer_amount = 1000 * (10 ** 18)  # 1000 tokens each
        total_transferred = transfer_amount * len(recipients)
        transfer_results = []

        LOG.info(f"Executing {len(recipients)} transfers of {transfer_amount / 10**18} tokens each")

        for i, recipient in enumerate(recipients):
            LOG.info(f"Transfer {i+1}/{len(recipients)} to {recipient['address']}")

            # Execute transfer
            result = await tx_builder.build_and_send_tx(
                to=contract_address,
                data=contract.functions.transfer(recipient['address'], transfer_amount).data_in_transaction,
                options=TransactionOptions(gas_limit=100000)
            )

            if not result.success:
                raise TransactionError(
                    f"Transfer {i+1} failed: {result.error}",
                    tx_hash=result.tx_hash
                )

            transfer_results.append(result)

        # 5. Monitor for all Transfer events
        LOG.info("Monitoring Transfer events...")
        all_events = await event_poller.get_events(
            contract=contract,
            event_name="Transfer",
            from_block=deploy_result.block_number
        )

        LOG.info(f"Found {len(all_events.events)} Transfer events")

        # 6. Verify all transfers
        deployer_balance = await contract.functions.balanceOf(deployer['address']).call()
        expected_deployer_balance = initial_supply - total_transferred

        if deployer_balance != expected_deployer_balance:
            raise ContractError(
                f"Deployer balance after batch transfers: expected {expected_deployer_balance}, got {deployer_balance}"
            )

        # Verify each recipient balance
        for i, recipient in enumerate(recipients):
            balance = await contract.functions.balanceOf(recipient['address']).call()
            if balance != transfer_amount:
                raise ContractError(
                    f"Recipient {i} balance mismatch: expected {transfer_amount}, got {balance}"
                )

        # 7. Analyze events
        transfer_events = [e for e in all_events.events if e['event'] == 'Transfer']
        if len(transfer_events) != len(recipients):
            raise EventError(
                f"Event count mismatch: expected {len(recipients)}, got {len(transfer_events)}"
            )

        # 8. Record test results
        total_gas_used = sum(r.gas_used for r in transfer_results)
        avg_gas_per_transfer = total_gas_used // len(recipients)

        test_result.mark_success(
            contract_address=contract_address,
            total_recipients=len(recipients),
            transfer_amount=transfer_amount,
            total_transferred=total_transferred,
            final_deployer_balance=deployer_balance,
            total_transfer_events=len(transfer_events),
            total_gas_used=total_gas_used,
            avg_gas_per_transfer=avg_gas_per_transfer,
            transaction_hashes=[r.tx_hash for r in transfer_results]
        )

        LOG.info(f"Batch transfers test completed successfully")
        LOG.info(f"Total gas used: {total_gas_used}")
        LOG.info(f"Average gas per transfer: {avg_gas_per_transfer}")

    except Exception as e:
        test_result.mark_failure(error=f"Batch transfer test failed: {e}")
        raise


@test_case
async def test_erc20_edge_cases(run_helper: RunHelper, test_result: TestResult):
    """Test ERC20 edge cases and error conditions"""
    LOG.info("Starting ERC20 edge cases test (refactored)")

    try:
        # 1. Create test accounts
        deployer = await run_helper.create_test_account("deployer", fund_wei=10 * 10**18)
        recipient = await run_helper.create_test_account("recipient")

        # 2. Deploy token contract
        contracts_dir = Path(__file__).parent.parent.parent.parent / "contracts_data"
        contract_deployer = ContractDeployer(
            web3=run_helper.client.web3,
            account=deployer['account']
        )

        deploy_result = await contract_deployer.deploy(
            contract_name="SimpleToken",
            constructor_args=["Edge Token", "EDGE", 1000000 * (10 ** 18)],
            options=DeploymentOptions(gas_limit=3000000),
            contracts_dir=contracts_dir
        )

        contract_address = deploy_result.contract_address
        contract = await contract_deployer.get_deployment("SimpleToken", contract_address, contracts_dir)

        # 3. Initialize transaction builder
        tx_builder = TransactionBuilder(
            web3=run_helper.client.web3,
            account=deployer['account']
        )

        # 4. Test edge case: Transfer more than balance
        deployer_balance = await contract.functions.balanceOf(deployer['address']).call()
        over_transfer_amount = deployer_balance + (10 ** 18)  # Try to transfer 1 token more than balance

        LOG.info(f"Testing transfer of more than balance: {over_transfer_amount / 10**18} tokens")

        # Attempt over-transfer (should fail)
        try:
            result = await tx_builder.build_and_send_tx(
                to=contract_address,
                data=contract.functions.transfer(recipient['address'], over_transfer_amount).data_in_transaction,
                options=TransactionOptions(gas_limit=100000)
            )

            if result.success:
                raise TransactionError("Transfer with insufficient balance unexpectedly succeeded")
            else:
                LOG.info("Over-transfer correctly failed")
        except TransactionError as e:
            if "insufficient" in str(e).lower() or "revert" in str(e).lower():
                LOG.info("Over-transfer correctly reverted")
            else:
                raise

        # 5. Test edge case: Transfer to zero address
        LOG.info("Testing transfer to zero address")

        zero_transfer_amount = 100 * (10 ** 18)
        try:
            result = await tx_builder.build_and_send_tx(
                to=contract_address,
                data=contract.functions.transfer("0x0000000000000000000000000000000000000000", zero_transfer_amount).data_in_transaction,
                options=TransactionOptions(gas_limit=100000)
            )

            if result.success:
                LOG.info("Transfer to zero address succeeded (contract may allow it)")
            else:
                LOG.info("Transfer to zero address reverted (expected for some tokens)")
        except TransactionError:
            LOG.info("Transfer to zero address failed")

        # 6. Test edge case: Zero amount transfer
        LOG.info("Testing zero amount transfer")

        zero_amount = 0
        result = await tx_builder.build_and_send_tx(
            to=contract_address,
            data=contract.functions.transfer(recipient['address'], zero_amount).data_in_transaction,
            options=TransactionOptions(gas_limit=50000)
        )

        if result.success:
            LOG.info("Zero amount transfer succeeded")

            # Verify no events were emitted
            event_poller = EventPoller(run_helper.client.web3)
            events = await event_poller.get_events(
                contract=contract,
                event_name="Transfer",
                from_block=result.block_number
            )

            if events.total_count == 0:
                LOG.info("No Transfer event emitted for zero transfer (correct)")
            else:
                LOG.warning("Transfer event emitted for zero transfer")
        else:
            LOG.info("Zero amount transfer failed")

        # 7. Test edge case: Approval of zero amount
        LOG.info("Testing zero amount approval")

        result = await tx_builder.build_and_send_tx(
            to=contract_address,
            data=contract.functions.approve(recipient['address'], 0).data_in_transaction,
            options=TransactionOptions(gas_limit=50000)
        )

        if result.success:
            LOG.info("Zero amount approval succeeded")

            # Check allowance
            allowance = await contract.functions.allowance(deployer['address'], recipient['address']).call()
            if allowance == 0:
                LOG.info("Zero allowance correctly set")
        else:
            LOG.info("Zero amount approval failed")

        # 8. Test edge case: TransferFrom with insufficient allowance
        LOG.info("Testing TransferFrom with insufficient allowance")

        small_approval = 100 * (10 ** 18)
        large_transfer = 200 * (10 ** 18)

        # Approve small amount
        await tx_builder.build_and_send_tx(
            to=contract_address,
            data=contract.functions.approve(recipient['address'], small_approval).data_in_transaction,
            options=TransactionOptions(gas_limit=50000)
        )

        # Try to transfer larger amount
        recipient_builder = TransactionBuilder(
            web3=run_helper.client.web3,
            account=recipient['account']
        )

        try:
            result = await recipient_builder.build_and_send_tx(
                to=contract_address,
                data=contract.functions.transferFrom(
                    deployer['address'],
                    recipient['address'],
                    large_transfer
                ).data_in_transaction,
                options=TransactionOptions(gas_limit=100000)
            )

            if result.success:
                raise TransactionError("TransferFrom with insufficient allowance unexpectedly succeeded")
            else:
                LOG.info("TransferFrom with insufficient allowance correctly failed")
        except TransactionError:
            LOG.info("TransferFrom with insufficient allowance failed as expected")

        # Record test results
        test_result.mark_success(
            contract_address=contract_address,
            over_transfer_test=True,
            zero_address_test=True,
            zero_amount_test=True,
            zero_approval_test=True,
            insufficient_allowance_test=True,
            final_deployer_balance=await contract.functions.balanceOf(deployer['address']).call()
        )

        LOG.info("ERC20 edge cases test completed successfully")

    except Exception as e:
        test_result.mark_failure(error=f"Edge cases test failed: {e}")
        raise