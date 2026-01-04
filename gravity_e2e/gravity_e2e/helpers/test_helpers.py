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
- Unified exception handling patterns

Usage:
    # Create test helper
    helper = RunHelper(client, "/tmp/test_output", faucet_account)

    # Create and fund test account
    account = await helper.create_test_account("test_user", fund_wei=10**18)

    # Initialize test result
    result = TestResult("my_test")

    # Mark test success with details
    result.mark_success(tx_hash="0x123...", gas_used=21000)

    # Use unified exception handling
    with handle_test_exception(test_result):
        # test code here
        pass
"""

import asyncio
import logging
from contextlib import contextmanager
from typing import Any, Coroutine, Dict, Optional, TypeVar
from eth_account import Account

from ..core.client.gravity_client import GravityClient
from ..utils.exceptions import (
    GravityE2EError,
    TransactionError,
    ContractError,
    EventError,
)

LOG = logging.getLogger(__name__)

T = TypeVar('T')


def _handle_exception(test_result: 'TestResult', e: Exception) -> None:
    """Internal helper to handle exceptions and mark test result."""
    if isinstance(e, (TransactionError, ContractError, EventError)):
        # Framework-specific exceptions with details
        test_result.mark_failure(
            error=f"{e.__class__.__name__}: {e}",
            details=e.details if hasattr(e, 'details') else {}
        )
    elif isinstance(e, GravityE2EError):
        # Base framework exception
        test_result.mark_failure(
            error=f"{e.__class__.__name__}: {e}",
            details=e.details if hasattr(e, 'details') else {}
        )
    else:
        # Generic exception
        test_result.mark_failure(
            error=f"Test failed: {e}",
            details={"type": type(e).__name__}
        )


@contextmanager
def handle_test_exception(test_result: 'TestResult', reraise: bool = True):
    """
    Context manager for unified exception handling in tests.

    This ensures consistent error handling and result marking across all tests.

    Args:
        test_result: TestResult instance to mark on failure
        reraise: Whether to re-raise the exception after handling

    Usage:
        with handle_test_exception(test_result):
            # your test code
            await do_something()
    """
    try:
        yield
    except Exception as e:
        _handle_exception(test_result, e)
        if reraise:
            raise


async def handle_test_exception_async(
    test_result: 'TestResult',
    coro: Coroutine[Any, Any, T],
    reraise: bool = True
) -> Optional[T]:
    """
    Async version of exception handling for tests.

    Args:
        test_result: TestResult instance to mark on failure
        coro: Coroutine to execute
        reraise: Whether to re-raise the exception after handling

    Returns:
        Result of the coroutine if successful

    Usage:
        result = await handle_test_exception_async(test_result, do_something())
    """
    try:
        return await coro
    except Exception as e:
        _handle_exception(test_result, e)
        if reraise:
            raise
    return None


class TestResult:  # noqa: N801
    """
    Standardized test result tracking and reporting.

    This class provides a consistent way to track test execution results,
    including success/failure status, error messages, timing, and custom
    test-specific details.

    Note: __test__ = False tells pytest not to collect this class as a test.

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

    __test__ = False  # Tell pytest not to collect this class as a test

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


def mark_as_testcase(func):
    """
    Test case decorator for marking test functions.

    This decorator is compatible with pytest - it simply returns the function unchanged.
    When running with pytest, the function receives test_result from the fixture.
    When running standalone, the function can be called with explicit parameters.

    Usage:
        @mark_as_testcase
        async def test_something(run_helper: RunHelper, test_result: TestResult):
            # test code
            pass
    """
    import functools

    @functools.wraps(func)
    async def wrapper(*args, **kwargs):
        # If test_result is not provided, create one
        if 'test_result' not in kwargs:
            import time
            test_name = kwargs.get('test_name') or func.__name__
            result = TestResult(test_name)
            result.start_time = time.time()

            try:
                await func(*args, test_result=result, **kwargs)
                result.end_time = time.time()
                result.set_duration(result.end_time - result.start_time)

                if not result.success and result.error is None:
                    result.mark_success()

            except Exception as e:
                result.end_time = time.time()
                result.set_duration(result.end_time - result.start_time)
                if result.error is None:
                    result.mark_failure(str(e))
                raise

            return result
        else:
            # test_result is provided (e.g., by pytest fixture)
            return await func(*args, **kwargs)

    return wrapper


# Alias for backward compatibility
test_case = mark_as_testcase
testcase = mark_as_testcase