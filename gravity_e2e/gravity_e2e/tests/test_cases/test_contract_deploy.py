"""
Test contract deployment and interaction (Refactored)

This test verifies that contract deployment and interaction work correctly
using the new utility modules for cleaner, more maintainable code.

Key improvements from refactoring:
- Uses ContractDeployer for deployment
- Uses TransactionBuilder for transactions
- Uses EventPoller for monitoring events
- Eliminates duplicate contract deployment code
- More readable and maintainable
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
from typing import Dict, Optional

from gravity_e2e.helpers.test_helpers import RunHelper, TestResult, test_case
from gravity_e2e.utils.contract_deployer import ContractDeployer, DeploymentOptions
from gravity_e2e.utils.transaction_builder import TransactionBuilder, TransactionOptions
from gravity_e2e.utils.event_poller import EventPoller
from gravity_e2e.utils.exceptions import ContractError, TransactionError
from eth_account import Account

LOG = logging.getLogger(__name__)


@test_case
async def test_simple_storage_deploy(run_helper: RunHelper, test_result: TestResult):
    """Test SimpleStorage contract deployment and interaction using refactored utilities"""
    LOG.info("Starting SimpleStorage contract deployment test (refactored)")

    try:
        # 1. Create test account
        deployer = await run_helper.create_test_account("deployer", fund_wei=5 * 10**18)  # 5 ETH
        LOG.info(f"Deployer: {deployer['address']}")

        # 2. Initialize contract deployer
        contracts_dir = Path(__file__).parent.parent.parent.parent / "contracts_data"
        contract_deployer = ContractDeployer(
            web3=run_helper.client.web3,
            account=deployer['account']
        )

        # 3. Check for existing deployment
        cached_deployment = contract_deployer.get_cached_deployment("SimpleStorage")
        contract_address = None
        deployment_tx_hash = None
        deployment_gas_used = None

        if cached_deployment:
            contract_address = cached_deployment.get('address')
            LOG.info(f"Found cached deployment: {contract_address}")

            # Verify contract still exists
            contract = await contract_deployer.get_deployment("SimpleStorage", contract_address)
            if contract:
                LOG.info("Using existing deployed contract")
                deployment_tx_hash = cached_deployment.get('transaction_hash')
                deployment_gas_used = cached_deployment.get('gas_used')
            else:
                LOG.warning("Cached deployment not found on chain, deploying new contract")
                contract_address = None

        # 4. Deploy new contract if needed
        if not contract_address:
            LOG.info("Deploying new SimpleStorage contract...")

            # Define deployment options
            options = DeploymentOptions(
                gas_limit=200000,
                confirmations=1,
                verify=True
            )

            # Deploy contract
            result = await contract_deployer.deploy(
                contract_name="SimpleStorage",
                options=options,
                contracts_dir=contracts_dir
            )

            if not result.success:
                raise ContractError(
                    f"Contract deployment failed: {result.error}",
                    contract_name="SimpleStorage"
                )

            contract_address = result.contract_address
            deployment_tx_hash = result.transaction_hash
            deployment_gas_used = result.gas_used

            LOG.info(f"Contract deployed at: {contract_address}")
            LOG.info(f"Deployment tx: {deployment_tx_hash}")
            LOG.info(f"Gas used: {deployment_gas_used}")

        # 5. Get contract instance for interaction
        contract = await contract_deployer.get_deployment("SimpleStorage", contract_address)
        if not contract:
            raise ContractError(
                f"Failed to get contract instance at {contract_address}"
            )

        # 6. Initialize transaction builder for interactions
        tx_builder = TransactionBuilder(
            web3=run_helper.client.web3,
            account=deployer['account']
        )

        # 7. Test contract interaction - setValue
        test_value = 12345
        LOG.info(f"Setting value to {test_value}")

        # Build setValue transaction
        set_value_result = await tx_builder.build_and_send_tx(
            to=contract_address,
            data=contract.functions.setValue(test_value).data_in_transaction,
            options=TransactionOptions(gas_limit=50000)
        )

        if not set_value_result.success:
            raise TransactionError(
                f"setValue transaction failed: {set_value_result.error}",
                tx_hash=set_value_result.tx_hash
            )

        LOG.info(f"setValue tx: {set_value_result.tx_hash}")

        # 8. Test contract interaction - getValue
        LOG.info("Reading value from contract")

        # Call view function
        stored_value = await contract.functions.getValue().call()
        LOG.info(f"Retrieved value: {stored_value}")

        if stored_value != test_value:
            raise TransactionError(
                f"Value mismatch: expected {test_value}, got {stored_value}",
                contract_address=contract_address
            )

        # 9. Test with event polling (if events exist)
        # SimpleStorage typically doesn't emit events, but we can monitor for transaction events
        event_poller = EventPoller(run_helper.client.web3)

        # Record test results
        test_result.mark_success(
            contract_address=contract_address,
            deployment_tx_hash=deployment_tx_hash,
            deployment_gas_used=deployment_gas_used,
            test_value=test_value,
            retrieved_value=stored_value,
            set_value_tx_hash=set_value_result.tx_hash,
            gas_used_set_value=set_value_result.gas_used
        )

        LOG.info("SimpleStorage contract test completed successfully")

    except ContractError as e:
        test_result.mark_failure(
            error=f"Contract error: {e}",
            details={
                "contract_name": e.details.get("contract_name"),
                "contract_address": e.details.get("contract_address"),
                "method": e.details.get("method")
            }
        )
        raise
    except TransactionError as e:
        test_result.mark_failure(
            error=f"Transaction error: {e}",
            details={
                "tx_hash": e.details.get("tx_hash"),
                "from_address": e.details.get("from_address"),
                "to_address": e.details.get("to_address")
            }
        )
        raise
    except Exception as e:
        test_result.mark_failure(
            error=f"Test failed: {e}",
            details={"type": type(e).__name__}
        )
        raise


@test_case
async def test_contract_with_constructor(run_helper: RunHelper, test_result: TestResult):
    """Test contract deployment with constructor arguments"""
    LOG.info("Starting contract deployment with constructor test (refactored)")

    try:
        # 1. Create test account
        deployer = await run_helper.create_test_account("deployer", fund_wei=5 * 10**18)

        # 2. Initialize contract deployer
        contracts_dir = Path(__file__).parent.parent.parent.parent / "contracts_data"
        contract_deployer = ContractDeployer(
            web3=run_helper.client.web3,
            account=deployer['account']
        )

        # 3. Deploy SimpleToken contract with constructor arguments
        token_name = "Test Token"
        token_symbol = "TEST"
        initial_supply = 1000000 * 10**18  # 1 million tokens

        LOG.info(f"Deploying {token_name} ({token_symbol}) with supply {initial_supply}")

        result = await contract_deployer.deploy(
            contract_name="SimpleToken",
            constructor_args=[token_name, token_symbol, initial_supply],
            options=DeploymentOptions(gas_limit=2000000),
            contracts_dir=contracts_dir
        )

        if not result.success:
            raise ContractError(
                f"Token deployment failed: {result.error}",
                contract_name="SimpleToken"
            )

        LOG.info(f"Token deployed at: {result.contract_address}")

        # 4. Verify contract state
        contract = await contract_deployer.get_deployment(
            "SimpleToken",
            result.contract_address,
            contracts_dir
        )

        if not contract:
            raise ContractError("Failed to get deployed token contract")

        # Check token metadata
        name = await contract.functions.name().call()
        symbol = await contract.functions.symbol().call()
        total_supply = await contract.functions.totalSupply().call()

        if name != token_name or symbol != token_symbol or total_supply != initial_supply:
            raise ContractError(
                f"Contract state mismatch: name={name}, symbol={symbol}, supply={total_supply}"
            )

        # 5. Test token transfer
        recipient = await run_helper.create_test_account("token_recipient")
        transfer_amount = 100 * 10**18  # 100 tokens

        # First approve (if needed)
        # Then transfer
        tx_builder = TransactionBuilder(
            web3=run_helper.client.web3,
            account=deployer['account']
        )

        transfer_result = await tx_builder.build_and_send_tx(
            to=result.contract_address,
            data=contract.functions.transfer(recipient['address'], transfer_amount).data_in_transaction,
            options=TransactionOptions(gas_limit=100000)
        )

        if not transfer_result.success:
            raise TransactionError(
                f"Token transfer failed: {transfer_result.error}",
                tx_hash=transfer_result.tx_hash
            )

        # Verify recipient balance
        recipient_balance = await contract.functions.balanceOf(recipient['address']).call()
        if recipient_balance != transfer_amount:
            raise TransactionError(
                f"Recipient balance mismatch: expected {transfer_amount}, got {recipient_balance}"
            )

        # Record test results
        test_result.mark_success(
            contract_address=result.contract_address,
            token_name=name,
            token_symbol=symbol,
            total_supply=total_supply,
            transfer_amount=transfer_amount,
            recipient_balance=recipient_balance,
            deployment_gas_used=result.gas_used,
            transfer_gas_used=transfer_result.gas_used
        )

        LOG.info("Contract deployment with constructor test completed successfully")

    except Exception as e:
        test_result.mark_failure(error=f"Test failed: {e}")
        raise


@test_case
async def test_contract_deployment_with_retry(run_helper: RunHelper, test_result: TestResult):
    """Test contract deployment with retry mechanism"""
    LOG.info("Starting contract deployment with retry test (refactored)")

    try:
        # 1. Create test account
        deployer = await run_helper.create_test_account("deployer", fund_wei=5 * 10**18)

        # 2. Initialize contract deployer with custom retry config
        from gravity_e2e.utils.async_retry import AsyncRetry

        retry_config = AsyncRetry(
            max_retries=3,
            base_delay=1.0,
            max_delay=10.0,
            jitter=True
        )

        contract_deployer = ContractDeployer(
            web3=run_helper.client.web3,
            account=deployer['account'],
            retry_config=retry_config
        )

        # 3. Deploy contract with retry
        contracts_dir = Path(__file__).parent.parent.parent.parent / "contracts_data"

        LOG.info("Deploying contract with retry mechanism")
        result = await contract_deployer.deploy(
            contract_name="SimpleStorage",
            contracts_dir=contracts_dir,
            options=DeploymentOptions(
                gas_limit=200000,
                timeout=60
            )
        )

        if not result.success:
            raise ContractError(
                f"Contract deployment failed after retries: {result.error}"
            )

        LOG.info(f"Contract deployed successfully: {result.contract_address}")

        # 4. Test interaction with retry
        tx_builder = TransactionBuilder(
            web3=run_helper.client.web3,
            account=deployer['account'],
            retry_config=retry_config
        )

        # Simulate network issues by using a low timeout (this is just a test of retry mechanism)
        test_values = [100, 200, 300]
        tx_hashes = []

        for i, value in enumerate(test_values):
            LOG.info(f"Setting value {i+1}: {value}")

            # Get contract
            contract = await contract_deployer.get_deployment(
                "SimpleStorage",
                result.contract_address
            )

            set_result = await tx_builder.build_and_send_tx(
                to=result.contract_address,
                data=contract.functions.setValue(value).data_in_transaction,
                options=TransactionOptions(gas_limit=50000)
            )

            if not set_result.success:
                raise TransactionError(
                    f"setValue {i+1} failed: {set_result.error}"
                )

            tx_hashes.append(set_result.tx_hash)

        # 5. Verify final value
        final_value = await contract.functions.getValue().call()
        if final_value != test_values[-1]:
            raise TransactionError(
                f"Final value mismatch: expected {test_values[-1]}, got {final_value}"
            )

        # Record test results
        test_result.mark_success(
            contract_address=result.contract_address,
            deployment_time=result.deploy_time,
            final_value=final_value,
            total_transactions=len(tx_hashes),
            transaction_hashes=tx_hashes
        )

        LOG.info("Contract deployment with retry test completed successfully")

    except Exception as e:
        test_result.mark_failure(error=f"Test failed: {e}")
        raise