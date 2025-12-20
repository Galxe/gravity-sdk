"""
Gravity E2E Test Framework Helpers

This module provides core helper classes for test execution, including test
result tracking and account management utilities.

Design Notes:
- TestResult class for standardized test outcome reporting
- RunHelper class for common test operations
- Automatic account creation and funding
- Integration with faucet for test ETH distribution
- Type-safe account information handling

Usage:
    # Create test helper
    helper = RunHelper(client, "/tmp/test_output", faucet_account)

    # Create and fund test account
    account = await helper.create_test_account("test_user", fund_wei=10**18)

    # Initialize test result
    result = TestResult("my_test")

    # Mark test success with details
    result.mark_success(tx_hash="0x123...", gas_used=21000)
"""

import asyncio
import logging
from typing import Dict, Optional
from eth_account import Account
from web3 import Web3

from .account_manager import TestAccountManager
from ..core.client.gravity_client import GravityClient

LOG = logging.getLogger(__name__)


class TestResult:
    """
    Standardized test result tracking and reporting.

    This class provides a consistent way to track test execution results,
    including success/failure status, error messages, timing, and custom
    test-specific details.

    Attributes:
        test_name: Name/identifier of the test
        success: Whether the test passed (True) or failed (False)
        error: Error message if test failed
        start_time: Test start timestamp
        end_time: Test end timestamp
        details: Dictionary of test-specific metrics and data

    Example:
        result = TestResult("token_transfer")
        result.mark_success(
            tx_hash="0xabc123",
            gas_used=50000,
            amount=1000
        )
        print(result.to_dict())
    """

    def __init__(self, test_name: str):
        """
        Initialize test result.

        Args:
            test_name: Unique identifier for the test
        """
        self.test_name = test_name
        self.success = False
        self.error = None
        self.start_time = None
        self.end_time = None
        self.details = {}

    def mark_success(self, **details):
        """
        Mark the test as successful with optional details.

        Args:
            **details: Test-specific metrics and data (e.g., tx_hash, gas_used)
        """
        self.success = True
        if details:
            self.details.update(details)

    def mark_failure(self, error: str, **details):
        """
        Mark the test as failed with error message and optional details.

        Args:
            error: Description of what went wrong
            **details: Additional context (e.g., failed_operation, expected_vs_actual)
        """
        self.success = False
        self.error = error
        if details:
            self.details.update(details)

    def set_duration(self, duration: float):
        """
        Set the test execution duration.

        Args:
            duration: Test duration in seconds
        """
        self.details["duration"] = duration

    def to_dict(self):
        """
        Convert test result to dictionary for JSON serialization.

        Returns:
            Dictionary representation suitable for saving to file
        """
        return {
            "test_name": self.test_name,
            "success": self.success,
            "error": self.error,
            "start_time": self.start_time,
            "end_time": self.end_time,
            "details": self.details
        }


class RunHelper:
    """
    Helper class for test execution operations.

    Provides common functionality needed during test execution, including
    account creation, funding, and interaction with the blockchain client.

    Attributes:
        client: GravityClient instance for blockchain interactions
        working_dir: Directory for test outputs and temporary files
        faucet_account: Account used for funding test accounts

    Example:
        # Initialize helper
        helper = RunHelper(client, "/tmp/tests", faucet_account)

        # Create funded test account
        account = await helper.create_test_account(
            name="alice",
            fund_wei=10**18  # 1 ETH
        )

        # Use account address for transactions
        print(f"Created account: {account['address']}")
    """

    def __init__(self, client: GravityClient, working_dir: str, faucet_account: Optional[Dict] = None):
        """
        Initialize run helper.

        Args:
            client: GravityClient for blockchain communication
            working_dir: Directory path for test outputs
            faucet_account: Optional faucet account with ETH for funding tests
        """
        self.client = client
        self.working_dir = working_dir
        self.faucet_account = faucet_account
        
    async def create_test_account(self, name: str, fund_wei: Optional[int] = None) -> Dict:
        """Create and optionally fund test account
        
        Args:
            name: Account name
            fund_wei: Funding amount in wei, None means no funding
            
        Returns:
            Account information dictionary
        """
        from eth_account import Account
        
        account = Account.create()
        account_info = {
            "name": name,
            "address": account.address,
            "private_key": account.key.hex(),
            "account": account
        }
        
        # Log account information
        LOG.info(f"Created test account '{name}':")
        LOG.info(f"  Address: {account.address}")
        LOG.info(f"  Private Key: {account.key.hex()}")
        
        # Fund account if needed and faucet is configured
        if fund_wei and fund_wei > 0 and self.faucet_account:
            await self._fund_account(account_info, fund_wei)
            
        return account_info
        
    async def _fund_account(self, account: Dict, amount_wei: int, confirmations: int = 1):
        """Fund test account using faucet account

        Args:
            account: Account information
            amount_wei: Funding amount in wei
            confirmations: Number of confirmations to wait

        Returns:
            Transaction receipt
        """
        try:
            # Import here to avoid circular dependency
            from ..utils.transaction_builder import TransactionBuilder, TransactionOptions
            from eth_account import Account

            # Create transaction builder with faucet account
            faucet_account_obj = Account.from_key(self.faucet_account["private_key"])
            tx_builder = TransactionBuilder(
                web3=self.client.web3,
                account=faucet_account_obj
            )

            # Send ETH transfer
            result = await tx_builder.send_ether(
                to=account["address"],
                amount_wei=amount_wei
            )

            # Wait for additional confirmations if needed
            if confirmations > 1 and result.success:
                await self._wait_for_confirmations(
                    result.tx_hash,
                    confirmations - 1,
                    result.block_number
                )

            LOG.info(f"Funded account '{account['name']}' with {amount_wei / 10**18:.6f} ETH")
            return result.tx_receipt

        except Exception as e:
            LOG.error(f"Failed to fund account: {e}")
            raise

    async def _wait_for_confirmations(self, tx_hash: str, additional_confirmations: int, current_block: int):
        """Wait for additional block confirmations"""
        target_block = current_block + additional_confirmations

        while int(await self.client.get_block_number()) < target_block:
            await asyncio.sleep(0.5)


def test_case(func):
    """Test case decorator"""
    async def wrapper(*args, **kwargs):
        test_name = kwargs.get('test_name') or func.__name__
        result = TestResult(test_name)
        result.start_time = asyncio.get_event_loop().time()
        result.end_time = result.start_time  # Initialize end_time
        
        try:
            await func(*args, **kwargs, test_result=result)
            result.end_time = asyncio.get_event_loop().time()
            result.set_duration(result.end_time - result.start_time)
            result.mark_success()
        except Exception as e:
            result.end_time = asyncio.get_event_loop().time()
            result.set_duration(result.end_time - result.start_time)
            result.mark_failure(str(e))
            raise
            
        return result
    
    return wrapper