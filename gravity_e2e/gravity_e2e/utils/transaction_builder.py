"""
Transaction builder utility for Gravity E2E Test Framework

This module provides a unified interface for building and signing blockchain
transactions across different test scenarios.

Design Notes:
- Supports both EIP-1559 and legacy transaction types
- Handles gas estimation and optimization
- Provides transaction simulation before sending
- Integrates with retry mechanisms for reliability
- Type hints for better IDE support
- Uses run_in_executor for synchronous Web3 calls to avoid blocking
"""

import asyncio
import functools
import logging
import time
from concurrent.futures import ThreadPoolExecutor
from typing import Any, Callable, Dict, List, Optional, Tuple, TypeVar, Union
from dataclasses import dataclass
from datetime import datetime
from web3 import Web3
from web3.types import TxParams, TxReceipt, Wei
from eth_account.signers.local import LocalAccount
from .exceptions import TransactionError
from .async_retry import AsyncRetry

LOG = logging.getLogger(__name__)

T = TypeVar('T')

# Shared thread pool for Web3 sync calls
_web3_executor = ThreadPoolExecutor(max_workers=4, thread_name_prefix="web3_sync_")


async def run_sync(func: Callable[..., T], *args, **kwargs) -> T:
    """
    Run a synchronous function in a thread pool to avoid blocking the event loop.

    Args:
        func: Synchronous function to run
        *args: Positional arguments for func
        **kwargs: Keyword arguments for func

    Returns:
        Result of func
    """
    loop = asyncio.get_running_loop()
    partial_func = functools.partial(func, *args, **kwargs)
    return await loop.run_in_executor(_web3_executor, partial_func)


@dataclass
class TransactionOptions:
    """Options for transaction construction"""
    gas_limit: Optional[int] = None
    max_fee_per_gas: Optional[int] = None  # For EIP-1559
    max_priority_fee_per_gas: Optional[int] = None  # For EIP-1559
    gas_price: Optional[int] = None  # For legacy transactions
    nonce: Optional[int] = None
    value: int = 0
    chain_id: Optional[int] = None
    tx_type: Optional[int] = None  # 0 for legacy, 1 for access list, 2 for EIP-1559


@dataclass
class TransactionResult:
    """Result of a transaction"""
    tx_hash: str
    tx_receipt: Optional[TxReceipt] = None
    success: bool = False
    error: Optional[str] = None
    gas_used: Optional[int] = None
    block_number: Optional[int] = None
    timestamp: Optional[datetime] = None


class TransactionBuilder:
    """
    Utility class for building, signing, and sending transactions.

    Provides a consistent interface for transaction operations across
    different test scenarios while handling common edge cases.
    """

    def __init__(
        self,
        web3: Web3,
        account: LocalAccount,
        default_options: Optional[TransactionOptions] = None,
        retry_config: Optional[AsyncRetry] = None
    ):
        """
        Initialize transaction builder.

        Args:
            web3: Web3 instance for blockchain interaction
            account: Account to sign transactions with
            default_options: Default transaction options
            retry_config: Retry configuration for sending transactions
        """
        self.web3 = web3
        self.account = account
        self.default_options = default_options or TransactionOptions()
        self.retry = retry_config or AsyncRetry(
            max_retries=3,
            base_delay=1.0,
            max_delay=30.0
        )

        # Cache for nonces
        self._nonce_cache: Dict[str, int] = {}
        self._last_nonce_update = None

    async def get_nonce(self, refresh: bool = False) -> int:
        """
        Get the next nonce for the account.

        Args:
            refresh: Force refresh nonce from blockchain

        Returns:
            Next nonce to use
        """
        address = self.account.address

        # Check cache
        if not refresh and address in self._nonce_cache:
            # Use cached nonce if recent (within 30 seconds)
            if self._last_nonce_update and \
               (datetime.now() - self._last_nonce_update).seconds < 30:
                return self._nonce_cache[address]

        try:
            # Get pending transaction count (run sync call in executor)
            pending_count = await run_sync(
                self.web3.eth.get_transaction_count,
                address,
                'pending'
            )

            # Update cache
            self._nonce_cache[address] = pending_count
            self._last_nonce_update = datetime.now()

            return pending_count

        except Exception as e:
            raise TransactionError(
                f"Failed to get nonce for {address}",
                from_address=address,
                cause=e
            )

    async def estimate_gas(
        self,
        transaction: TxParams,
        padding: float = 1.2
    ) -> int:
        """
        Estimate gas required for a transaction.

        Args:
            transaction: Transaction to estimate gas for
            padding: Multiplier to add padding to estimate

        Returns:
            Estimated gas limit with padding
        """
        try:
            # Create a copy for estimation
            tx_copy = transaction.copy()

            # Ensure from address is set
            if 'from' not in tx_copy:
                tx_copy['from'] = self.account.address

            # Remove gas-related fields for estimation
            tx_copy.pop('gas', None)
            tx_copy.pop('gasLimit', None)
            tx_copy.pop('maxFeePerGas', None)
            tx_copy.pop('maxPriorityFeePerGas', None)
            tx_copy.pop('gasPrice', None)

            # Estimate gas (run sync call in executor)
            gas_estimate = await run_sync(
                self.web3.eth.estimate_gas,
                tx_copy
            )

            # Apply padding
            gas_limit = int(gas_estimate * padding)

            # Apply minimum gas limit
            gas_limit = max(gas_limit, 21000)

            LOG.debug(f"Gas estimate: {gas_estimate} -> {gas_limit} (with padding)")
            return gas_limit

        except Exception as e:
            raise TransactionError(
                f"Gas estimation failed: {e}",
                from_address=self.account.address,
                to_address=transaction.get('to'),
                cause=e
            )

    async def build_transaction(
        self,
        to: str,
        data: Optional[str] = None,
        options: Optional[TransactionOptions] = None,
        **kwargs
    ) -> TxParams:
        """
        Build a transaction with proper defaults.

        Args:
            to: Recipient address
            data: Transaction data (calldata)
            options: Transaction options to override defaults
            **kwargs: Additional transaction parameters

        Returns:
            Complete transaction dictionary
        """
        # Merge options with defaults
        opts = self.default_options
        if options:
            opts = TransactionOptions(
                gas_limit=options.gas_limit or opts.gas_limit,
                max_fee_per_gas=options.max_fee_per_gas or opts.max_fee_per_gas,
                max_priority_fee_per_gas=options.max_priority_fee_per_gas or opts.max_priority_fee_per_gas,
                gas_price=options.gas_price or opts.gas_price,
                nonce=options.nonce or opts.nonce,
                value=options.value or opts.value,
                chain_id=options.chain_id or opts.chain_id,
                tx_type=options.tx_type or opts.tx_type
            )

        # Build transaction
        tx: TxParams = {
            'to': to,
            'value': Wei(opts.value),
            'data': data or '0x'
        }

        # Add chain ID if not set
        if opts.chain_id:
            tx['chainId'] = opts.chain_id
        else:
            # Get from network (run sync property access in executor)
            try:
                chain_id = await run_sync(lambda: self.web3.eth.chain_id)
                tx['chainId'] = chain_id
            except Exception as e:
                LOG.warning(f"Could not get chain ID: {e}")

        # Add nonce
        if opts.nonce is not None:
            tx['nonce'] = opts.nonce
        else:
            tx['nonce'] = await self.get_nonce()

        # Handle different transaction types
        if opts.tx_type == 2 or (opts.tx_type is None and opts.max_fee_per_gas):
            # EIP-1559 transaction
            if opts.max_fee_per_gas:
                tx['maxFeePerGas'] = Wei(opts.max_fee_per_gas)
            if opts.max_priority_fee_per_gas:
                tx['maxPriorityFeePerGas'] = Wei(opts.max_priority_fee_per_gas)

            # Estimate gas if not provided
            if opts.gas_limit:
                tx['gas'] = Wei(opts.gas_limit)
            else:
                tx['gas'] = Wei(await self.estimate_gas(tx))

        else:
            # Legacy transaction
            if opts.gas_price:
                tx['gasPrice'] = Wei(opts.gas_price)
            else:
                # Get current gas price (run sync property access in executor)
                try:
                    gas_price = await run_sync(lambda: self.web3.eth.gas_price)
                    tx['gasPrice'] = gas_price
                except Exception as e:
                    LOG.warning(f"Could not get gas price: {e}")
                    tx['gasPrice'] = Wei(20000000000)  # 20 gwei fallback

            # Estimate gas if not provided
            if opts.gas_limit:
                tx['gas'] = Wei(opts.gas_limit)
            else:
                tx['gas'] = Wei(await self.estimate_gas(tx))

        # Add any additional parameters
        tx.update(kwargs)

        return tx

    async def simulate_transaction(
        self,
        transaction: TxParams,
        block_identifier: Optional[Union[int, str]] = 'latest'
    ) -> Dict[str, Any]:
        """
        Simulate a transaction without executing it.

        Args:
            transaction: Transaction to simulate
            block_identifier: Block number or tag to simulate against

        Returns:
            Simulation result
        """
        try:
            # Ensure from address is set
            if 'from' not in transaction:
                transaction['from'] = self.account.address

            # Call eth_call to simulate (run sync call in executor)
            result = await run_sync(
                self.web3.eth.call,
                transaction,
                block_identifier
            )

            return {
                'success': True,
                'result': result.hex(),
                'gas_used': await self.estimate_gas(transaction)
            }

        except Exception as e:
            return {
                'success': False,
                'error': str(e),
                'gas_used': await self.estimate_gas(transaction) if 'gas' not in transaction else None
            }

    def sign_transaction(self, transaction: TxParams) -> Tuple[str, bytes]:
        """
        Sign a transaction with the account's private key.

        Args:
            transaction: Transaction to sign

        Returns:
            Tuple of (raw transaction hex, signed transaction bytes)
        """
        try:
            # Ensure from address matches account
            if 'from' in transaction and transaction['from'] != self.account.address:
                raise TransactionError(
                    f"Transaction from address {transaction['from']} does not match account address {self.account.address}"
                )

            # Sign transaction
            signed_tx = self.account.sign_transaction(transaction)

            # Support both old and new web3.py API
            raw_tx = getattr(signed_tx, 'raw_transaction', None) or getattr(signed_tx, 'rawTransaction', None)
            return raw_tx.hex(), raw_tx

        except Exception as e:
            raise TransactionError(
                f"Failed to sign transaction: {e}",
                from_address=self.account.address,
                cause=e
            )

    async def send_transaction(
        self,
        transaction: TxParams,
        wait_for_receipt: bool = True,
        timeout: float = 120.0,
        poll_latency: float = 0.5
    ) -> TransactionResult:
        """
        Send a transaction to the blockchain.

        Args:
            transaction: Transaction to send
            wait_for_receipt: Whether to wait for transaction receipt
            timeout: Timeout for waiting for receipt
            poll_latency: Polling interval for receipt

        Returns:
            TransactionResult with receipt information
        """
        # Sign transaction
        raw_tx_hex, _ = self.sign_transaction(transaction)

        try:
            # Send raw transaction (run sync call in executor)
            tx_hash = await run_sync(
                self.web3.eth.send_raw_transaction,
                raw_tx_hex
            )

            result = TransactionResult(
                tx_hash=tx_hash.hex(),
                timestamp=datetime.now()
            )

            # Update nonce cache if we used a specific nonce
            if 'nonce' in transaction:
                self._nonce_cache[self.account.address] = transaction['nonce'] + 1

            # Wait for receipt if requested
            if wait_for_receipt:
                LOG.info(f"Waiting for transaction receipt: {tx_hash.hex()}")

                receipt = await self._wait_for_receipt(
                    tx_hash,
                    timeout=timeout,
                    poll_latency=poll_latency
                )

                result.tx_receipt = receipt
                result.success = bool(receipt.status)
                result.gas_used = receipt.gasUsed
                result.block_number = receipt.blockNumber

                if result.success:
                    LOG.info(f"Transaction successful: {tx_hash.hex()}")
                else:
                    LOG.error(f"Transaction failed: {tx_hash.hex()}")

            return result

        except Exception as e:
            raise TransactionError(
                f"Failed to send transaction: {e}",
                tx_hash=tx_hash.hex() if 'tx_hash' in locals() else None,
                from_address=self.account.address,
                to_address=transaction.get('to'),
                cause=e
            )

    async def _wait_for_receipt(
        self,
        tx_hash,
        timeout: float,
        poll_latency: float
    ) -> TxReceipt:
        """
        Wait for transaction receipt with timeout.

        Args:
            tx_hash: Transaction hash to wait for
            timeout: Maximum time to wait
            poll_latency: Polling interval

        Returns:
            Transaction receipt
        """
        from web3.exceptions import TransactionNotFound

        start_time = time.time()
        tx_hash_hex = tx_hash.hex() if hasattr(tx_hash, 'hex') else str(tx_hash)

        while True:
            try:
                # Run sync call in executor
                receipt = await run_sync(
                    self.web3.eth.get_transaction_receipt,
                    tx_hash
                )
                if receipt is not None:
                    return receipt

            except TransactionNotFound:
                # Transaction not yet mined, continue polling
                pass
            except Exception as e:
                # Check if transaction is pending (legacy error message check)
                if "transaction not found" not in str(e).lower() and "not found" not in str(e).lower():
                    raise

            # Check timeout
            if time.time() - start_time > timeout:
                raise TransactionError(
                    f"Transaction receipt timeout after {timeout}s",
                    tx_hash=tx_hash_hex
                )

            # Wait before polling again
            await asyncio.sleep(poll_latency)

    async def build_and_send_tx(
        self,
        to: str,
        data: Optional[str] = None,
        options: Optional[TransactionOptions] = None,
        wait_for_receipt: bool = True,
        simulate: bool = False,
        **kwargs
    ) -> TransactionResult:
        """
        Build and send a transaction in one call.

        Args:
            to: Recipient address
            data: Transaction data (calldata)
            options: Transaction options
            wait_for_receipt: Whether to wait for receipt
            simulate: Whether to simulate before sending
            **kwargs: Additional transaction parameters

        Returns:
            TransactionResult with execution details
        """
        # Build transaction
        tx = await self.build_transaction(
            to=to,
            data=data,
            options=options,
            **kwargs
        )

        # Simulate if requested
        if simulate:
            LOG.info("Simulating transaction...")
            sim_result = await self.simulate_transaction(tx)

            if not sim_result['success']:
                raise TransactionError(
                    f"Transaction simulation failed: {sim_result['error']}",
                    from_address=self.account.address,
                    to_address=to
                )

            LOG.info(f"Simulation successful. Gas usage: {sim_result['gas_used']}")

        # Send transaction
        return await self.send_transaction(
            tx,
            wait_for_receipt=wait_for_receipt
        )

    async def send_ether(
        self,
        to: str,
        amount_wei: int,
        **kwargs
    ) -> TransactionResult:
        """
        Send ether to an address.

        Args:
            to: Recipient address
            amount_wei: Amount to send in wei
            **kwargs: Additional transaction options

        Returns:
            TransactionResult
        """
        options = TransactionOptions(value=amount_wei)

        return await self.build_and_send_tx(
            to=to,
            options=options,
            **kwargs
        )

    async def deploy_contract(
        self,
        bytecode: str,
        abi: List[Dict],
        args: Optional[List] = None,
        options: Optional[TransactionOptions] = None,
        **kwargs
    ) -> TransactionResult:
        """
        Deploy a contract.

        Args:
            bytecode: Contract bytecode
            abi: Contract ABI
            args: Constructor arguments
            options: Transaction options
            **kwargs: Additional parameters

        Returns:
            TransactionResult with contract address in receipt
        """
        from web3.contract import Contract

        # Create contract instance
        contract = self.web3.eth.contract(abi=abi, bytecode=bytecode)

        # Build constructor data
        if args:
            constructor = contract.constructor(*args)
        else:
            constructor = contract.constructor()
        
        # Get constructor data using build_transaction
        tx = constructor.build_transaction({'from': self.account.address, 'gas': 0, 'gasPrice': 0})
        data = tx.get('data', '0x')

        # Deploy transaction (to field is None for contract deployment)
        return await self.build_and_send_tx(
            to=None,  # Contract deployment
            data=data,
            options=options,
            **kwargs
        )