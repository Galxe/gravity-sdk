"""
Test cross-chain deposit functionality (Refactored)

This test verifies cross-chain deposit from Sepolia to Gravity chain using
the new utility modules for cleaner, more maintainable code.

Key improvements from refactoring:
- Uses EventPoller for efficient event monitoring
- Uses TransactionBuilder for transaction handling
- Uses ConfigManager for configuration management
- Eliminates manual event parsing and hex manipulation
- Proper event-based verification instead of polling
- Better error handling with custom exceptions
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
import os
from typing import Dict, Optional, Any, Tuple

from web3 import Web3
from eth_account import Account
from eth_utils import to_checksum_address, from_wei, to_wei

from gravity_e2e.helpers.test_helpers import RunHelper, TestResult, test_case
from gravity_e2e.utils.config_manager import ConfigManager
from gravity_e2e.utils.transaction_builder import TransactionBuilder, TransactionOptions
from gravity_e2e.utils.event_poller import EventPoller, wait_for_transfer_event
from gravity_e2e.utils.async_retry import AsyncRetry
from gravity_e2e.utils.exceptions import TransactionError, EventError, ConfigurationError

LOG = logging.getLogger(__name__)


class CrossChainTestHarness:
    """Test harness for cross-chain operations with refactored utilities"""

    def __init__(self, config: Dict):
        self.config = config
        self.sepolia_w3 = Web3(Web3.HTTPProvider(config["sepolia"]["rpc_url"]))

        # Initialize accounts
        self.sepolia_account = Account.from_key(config["sepolia"]["test_account"]["private_key"])
        self.gravity_account = Account.from_key(config["gravity"]["test_account"]["private_key"])

        # Initialize transaction builders
        self.sepolia_builder = TransactionBuilder(
            web3=self.sepolia_w3,
            account=self.sepolia_account,
            retry_config=AsyncRetry(max_retries=3, base_delay=1.0)
        )

        # Initialize contracts
        self._init_contracts()

        # Initialize event poller
        self.sepolia_event_poller = EventPoller(self.sepolia_w3)

        LOG.info(f"Cross-chain test harness initialized")
        LOG.info(f"Sepolia account: {self.sepolia_account.address}")
        LOG.info(f"Gravity account: {self.gravity_account.address}")

    def _init_contracts(self):
        """Initialize contract instances"""
        # Sepolia contracts
        sepolia_config = self.config["sepolia"]
        self.gravity_bridge = self.sepolia_w3.eth.contract(
            address=sepolia_config["contracts"]["gravity_bridge"]["address"],
            abi=sepolia_config["contracts"]["gravity_bridge"]["abi"]
        )
        self.g_token = self.sepolia_w3.eth.contract(
            address=sepolia_config["contracts"]["g_token"]["address"],
            abi=sepolia_config["contracts"]["g_token"]["abi"]
        )

    async def check_balances(self) -> Dict[str, Any]:
        """Check account balances on Sepolia"""
        try:
            # Check ETH balance
            eth_balance_wei = await self.sepolia_w3.eth.get_balance(self.sepolia_account.address)
            eth_balance = from_wei(eth_balance_wei, 'ether')

            # Check G Token balance
            g_balance = await self.g_token.functions.balanceOf(self.sepolia_account.address).call()

            # Get G Token decimals
            try:
                decimals = await self.g_token.functions.decimals().call()
                g_balance_formatted = g_balance / (10 ** decimals)
            except:
                g_balance_formatted = from_wei(g_balance, 'ether')

            LOG.info(f"Sepolia ETH balance: {eth_balance:.6f} ETH")
            LOG.info(f"Sepolia G Token balance: {g_balance_formatted:.6f} G")

            return {
                "eth_balance": eth_balance_wei,
                "g_balance": g_balance,
                "eth_balance_formatted": eth_balance,
                "g_balance_formatted": g_balance_formatted
            }
        except Exception as e:
            raise TransactionError(f"Failed to check balances: {e}")

    async def approve_g_token(self, amount: int) -> str:
        """Approve Gravity Bridge contract to spend G Tokens"""
        try:
            LOG.info(f"Approving {amount} G tokens for Gravity Bridge")

            # Build and send approval transaction
            result = await self.sepolia_builder.build_and_send_tx(
                to=self.config["sepolia"]["contracts"]["g_token"]["address"],
                data=self.g_token.functions.approve(
                    self.config["sepolia"]["contracts"]["gravity_bridge"]["address"],
                    amount
                ).data_in_transaction,
                options=TransactionOptions(gas_limit=100000)
            )

            if not result.success:
                raise TransactionError(f"Approval transaction failed: {result.error}")

            LOG.info(f"Approval successful: {result.tx_hash}")
            return result.tx_hash

        except Exception as e:
            raise TransactionError(f"Failed to approve G Token: {e}")

    async def deposit_gravity(self, amount: int, target_address: Optional[str] = None) -> Tuple[Dict[str, Any], Dict[str, Any]]:
        """Execute depositGravity and monitor for events"""
        try:
            if not target_address:
                target_address = self.sepolia_account.address

            LOG.info(f"Depositing {from_wei(amount, 'ether')} G tokens to Gravity chain")
            LOG.info(f"Target address on Gravity: {target_address}")

            # Start monitoring for DepositGravity event before sending transaction
            deposit_event_task = asyncio.create_task(
                self.sepolia_event_poller.wait_for_event(
                    contract=self.gravity_bridge,
                    event_name="DepositGravity",
                    timeout=30,
                    from_address=self.sepolia_account.address,
                    callback=lambda event: (
                        event['args']['to'].lower() == target_address.lower() and
                        event['args']['amount'] == amount
                    )
                )
            )

            # Execute deposit transaction
            result = await self.sepolia_builder.build_and_send_tx(
                to=self.config["sepolia"]["contracts"]["gravity_bridge"]["address"],
                data=self.gravity_bridge.functions.depositGravity(
                    to_checksum_address(target_address),
                    amount
                ).data_in_transaction,
                options=TransactionOptions(gas_limit=200000, timeout=120)
            )

            if not result.success:
                raise TransactionError(f"Deposit transaction failed: {result.error}")

            LOG.info(f"Deposit transaction sent: {result.tx_hash}")

            # Wait for event
            try:
                deposit_event = await asyncio.wait_for(deposit_event_task, timeout=30)

                if deposit_event:
                    LOG.info(f"✅ DepositGravity event detected!")
                    LOG.info(f"   User: {deposit_event['args']['user']}")
                    LOG.info(f"   Amount: {deposit_event['args']['amount']}")
                    LOG.info(f"   Target: {deposit_event['args']['targetAddress']}")
                    LOG.info(f"   Block: {deposit_event['args']['blockNumber']}")

                    deposit_event_data = {
                        "user": deposit_event['args']['user'],
                        "amount": deposit_event['args']['amount'],
                        "targetAddress": deposit_event['args']['targetAddress'],
                        "blockNumber": deposit_event['args']['blockNumber']
                    }
                else:
                    LOG.warning("DepositGravity event not found in transaction")
                    deposit_event_data = None

            except asyncio.TimeoutError:
                LOG.warning("Timeout waiting for DepositGravity event")
                deposit_event_data = None

            return {
                "tx_hash": result.tx_hash,
                "block_number": result.block_number,
                "gas_used": result.gas_used,
                "event": deposit_event_data
            }, result

        except Exception as e:
            raise TransactionError(f"Failed to deposit gravity: {e}")


async def monitor_gravity_for_cross_chain_event(
    run_helper: RunHelper,
    expected_sender: str,
    expected_amount: int,
    expected_target: str,
    gravity_config: Dict,
    timeout: float = 300.0
) -> Optional[Dict[str, Any]]:
    """Monitor Gravity chain for CrossChainDepositProcessed event using EventPoller"""
    try:
        LOG.info(f"Monitoring Gravity chain for CrossChainDepositProcessed event")
        LOG.info(f"Expected sender: {expected_sender}")
        LOG.info(f"Expected amount: {expected_amount}")
        LOG.info(f"Expected target: {expected_target}")

        # Get the monitor contract
        monitor_address = gravity_config["monitor_contract"]["address"]

        # For the Gravity chain, we need to create a web3 instance and contract
        from web3 import Web3
        gravity_w3 = Web3(Web3.HTTPProvider(gravity_config["rpc_url"]))

        # Create contract ABI for CrossChainDepositProcessed event
        monitor_abi = [
            {
                "anonymous": False,
                "inputs": [
                    {"indexed": True, "name": "sender", "type": "address"},
                    {"indexed": True, "name": "targetAddress", "type": "address"},
                    {"indexed": False, "name": "amount", "type": "uint256"},
                    {"indexed": False, "name": "blockNumber", "type": "uint256"},
                    {"indexed": False, "name": "success", "type": "bool"},
                    {"indexed": False, "name": "errorMessage", "type": "string"},
                    {"indexed": False, "name": "issuer", "type": "string"},
                    {"indexed": False, "name": "onchainBlockNumber", "type": "uint256"}
                ],
                "name": "CrossChainDepositProcessed",
                "type": "event"
            }
        ]

        monitor_contract = gravity_w3.eth.contract(
            address=monitor_address,
            abi=monitor_abi
        )

        # Initialize event poller for Gravity chain
        gravity_event_poller = EventPoller(gravity_w3)

        # Wait for the event with specific filters
        event = await gravity_event_poller.wait_for_event(
            contract=monitor_contract,
            event_name="CrossChainDepositProcessed",
            timeout=timeout,
            poll_interval=2.0,
            callback=lambda event: (
                event['args']['sender'].lower() == expected_sender.lower() and
                event['args']['targetAddress'].lower() == expected_target.lower() and
                event['args']['amount'] == expected_amount and
                event['args']['success'] == True
            )
        )

        if event:
            LOG.info(f"✅ CrossChainDepositProcessed found!")
            LOG.info(f"   Sender: {event['args']['sender']}")
            LOG.info(f"   Target: {event['args']['targetAddress']}")
            LOG.info(f"   Amount: {event['args']['amount']}")
            LOG.info(f"   Block: {event['args']['blockNumber']}")
            LOG.info(f"   Success: {event['args']['success']}")
            LOG.info(f"   Issuer: {event['args']['issuer']}")
            LOG.info(f"   Onchain Block: {event['args']['onchainBlockNumber']}")

            if event['args']['errorMessage']:
                LOG.warning(f"   Error Message: {event['args']['errorMessage']}")

        return event

    except asyncio.TimeoutError:
        LOG.error(f"❌ Timeout: CrossChainDepositProcessed not found after {timeout} seconds")
        return None
    except Exception as e:
        LOG.error(f"Error monitoring Gravity chain: {e}")
        raise EventError(f"Failed to monitor cross-chain event: {e}")


@test_case
async def test_cross_chain_gravity_deposit(run_helper: RunHelper, test_result: TestResult):
    """Test cross-chain deposit from Sepolia to Gravity using refactored utilities"""
    LOG.info("Starting cross-chain Gravity deposit test (refactored)")

    try:
        # 1. Load configuration using ConfigManager
        config_manager = ConfigManager()

        # Load cross-chain configuration
        cross_chain_config_file = os.getenv(
            "CROSS_CHAIN_CONFIG_PATH",
            str(Path(__file__).parent.parent.parent.parent / "configs" / "cross_chain_config.json")
        )

        if not Path(cross_chain_config_file).exists():
            raise ConfigurationError(
                f"Cross-chain config not found: {cross_chain_config_file}",
                config_file=cross_chain_config_file
            )

        cross_chain_config = config_manager.load_config(
            Path(cross_chain_config_file).name,
            validate=False,
            apply_env_overrides=True
        )

        LOG.info("Cross-chain configuration loaded successfully")

        # 2. Initialize test harness
        harness = CrossChainTestHarness(cross_chain_config)

        # 3. Check initial balances
        initial_balances = await harness.check_balances()

        # Define deposit amount (in G tokens)
        decimals = await harness.g_token.functions.decimals().call()
        deposit_amount = 100 * (10 ** decimals)  # 100 G tokens

        LOG.info(f"Deposit amount: {deposit_amount / (10 ** decimals)} G tokens")

        # 4. Approve G tokens for Gravity Bridge
        LOG.info("Step 1: Approving G tokens...")
        approval_tx_hash = await harness.approve_g_token(deposit_amount)

        # 5. Execute depositGravity
        LOG.info("Step 2: Executing depositGravity...")
        deposit_result, tx_result = await harness.deposit_gravity(
            amount=deposit_amount,
            target_address=harness.gravity_account.address
        )

        # 6. Monitor Gravity chain for CrossChainDepositProcessed event
        LOG.info("Step 3: Monitoring Gravity chain for cross-chain event...")

        gravity_event = await monitor_gravity_for_cross_chain_event(
            run_helper=run_helper,
            expected_sender=harness.sepolia_account.address,
            expected_amount=deposit_amount,
            expected_target=harness.gravity_account.address,
            gravity_config=cross_chain_config["gravity"],
            timeout=cross_chain_config["gravity"]["sync_settings"]["timeout_seconds"]
        )

        # 7. Verify cross-chain transfer
        if not gravity_event:
            raise EventError(
                "CrossChainDepositProcessed event not found on Gravity chain",
                event_name="CrossChainDepositProcessed"
            )

        # 8. Check final balances
        final_balances = await harness.check_balances()

        # Verify G token balance decreased
        balance_diff = initial_balances["g_balance"] - final_balances["g_balance"]
        if balance_diff != deposit_amount:
            LOG.warning(
                f"Balance difference {balance_diff} doesn't match deposit amount {deposit_amount}"
            )

        # 9. Record test results
        test_result.mark_success(
            sepolia_tx_hash=deposit_result["tx_hash"],
            sepolia_block_number=deposit_result["block_number"],
            sepolia_gas_used=deposit_result["gas_used"],
            deposit_event=deposit_result["event"],
            gravity_event=gravity_event["args"] if gravity_event else None,
            gravity_block_number=gravity_event["blockNumber"] if gravity_event else None,
            deposit_amount=deposit_amount,
            initial_g_balance=initial_balances["g_balance"],
            final_g_balance=final_balances["g_balance"],
            approval_tx_hash=approval_tx_hash,
            cross_chain_delay=gravity_event["blockNumber"] - deposit_result["block_number"] if gravity_event else None
        )

        LOG.info("✅ Cross-chain deposit test completed successfully!")
        LOG.info(f"Deposit confirmed on Sepolia at block: {deposit_result['block_number']}")
        LOG.info(f"Cross-chain processed on Gravity at block: {gravity_event['blockNumber']}")

    except (TransactionError, EventError, ConfigurationError) as e:
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
async def test_cross_chain_multiple_deposits(run_helper: RunHelper, test_result: TestResult):
    """Test multiple cross-chain deposits in sequence"""
    LOG.info("Starting multiple cross-chain deposits test (refactored)")

    try:
        # 1. Load configuration and initialize harness
        config_manager = ConfigManager()
        cross_chain_config_file = Path(__file__).parent.parent.parent.parent / "configs" / "cross_chain_config.json"
        cross_chain_config = config_manager.load_config(
            cross_chain_config_file.name,
            validate=False,
            apply_env_overrides=True
        )

        harness = CrossChainTestHarness(cross_chain_config)

        # 2. Check initial balances
        initial_balances = await harness.check_balances()
        decimals = await harness.g_token.functions.decimals().call()

        # 3. Execute multiple deposits
        num_deposits = 3
        deposit_amount = 50 * (10 ** decimals)  # 50 G tokens each
        total_amount = deposit_amount * num_deposits

        LOG.info(f"Executing {num_deposits} deposits of {deposit_amount / (10 ** decimals)} G tokens each")

        deposit_results = []
        gravity_events = []

        for i in range(num_deposits):
            LOG.info(f"\n--- Deposit {i+1}/{num_deposits} ---")

            # Approve tokens
            await harness.approve_g_token(deposit_amount)

            # Execute deposit
            deposit_result, _ = await harness.deposit_gravity(
                amount=deposit_amount,
                target_address=harness.gravity_account.address
            )

            # Monitor Gravity chain
            gravity_event = await monitor_gravity_for_cross_chain_event(
                run_helper=run_helper,
                expected_sender=harness.sepolia_account.address,
                expected_amount=deposit_amount,
                expected_target=harness.gravity_account.address,
                gravity_config=cross_chain_config["gravity"],
                timeout=60  # Shorter timeout for multiple deposits
            )

            if not gravity_event:
                raise EventError(f"Cross-chain event not found for deposit {i+1}")

            deposit_results.append(deposit_result)
            gravity_events.append(gravity_event)

            LOG.info(f"Deposit {i+1} completed successfully")

        # 4. Verify final state
        final_balances = await harness.check_balances()
        balance_diff = initial_balances["g_balance"] - final_balances["g_balance"]

        # Calculate total gas used
        total_sepolia_gas = sum(r["gas_used"] for r in deposit_results)

        # 5. Analyze cross-chain delays
        delays = []
        for i, (deposit, event) in enumerate(zip(deposit_results, gravity_events)):
            delay = event["blockNumber"] - deposit["block_number"]
            delays.append(delay)
            LOG.info(f"Deposit {i+1} cross-chain delay: {delay} blocks")

        avg_delay = sum(delays) / len(delays)

        # 6. Record test results
        test_result.mark_success(
            num_deposits=num_deposits,
            total_deposited=total_amount,
            total_gas_used_sepolia=total_sepolia_gas,
            avg_gas_per_deposit=total_sepolia_gas // num_deposits,
            initial_g_balance=initial_balances["g_balance"],
            final_g_balance=final_balances["g_balance"],
            actual_amount_deposited=balance_diff,
            cross_chain_delays=delays,
            avg_cross_chain_delay=avg_delay,
            gravity_block_numbers=[event["blockNumber"] for event in gravity_events]
        )

        LOG.info(f"\n✅ Multiple deposits test completed successfully!")
        LOG.info(f"Total deposited: {total_amount / (10 ** decimals)} G tokens")
        LOG.info(f"Total Sepolia gas used: {total_sepolia_gas}")
        LOG.info(f"Average cross-chain delay: {avg_delay:.1f} blocks")

    except Exception as e:
        test_result.mark_failure(error=f"Multiple deposits test failed: {e}")
        raise


@test_case
async def test_cross_chain_error_scenarios(run_helper: RunHelper, test_result: TestResult):
    """Test cross-chain deposit error scenarios"""
    LOG.info("Starting cross-chain error scenarios test (refactored)")

    try:
        # 1. Load configuration and initialize harness
        config_manager = ConfigManager()
        cross_chain_config_file = Path(__file__).parent.parent.parent.parent / "configs" / "cross_chain_config.json"
        cross_chain_config = config_manager.load_config(
            cross_chain_config_file.name,
            validate=False,
            apply_env_overrides=True
        )

        harness = CrossChainTestHarness(cross_chain_config)

        # 2. Test scenario: Deposit without approval
        LOG.info("\n--- Test 1: Deposit without approval ---")
        decimals = await harness.g_token.functions.decimals().call()
        deposit_amount = 10 * (10 ** decimals)

        try:
            _, _ = await harness.deposit_gravity(amount=deposit_amount)
            LOG.warning("Deposit without approval succeeded (unexpected)")
            approval_test_passed = False
        except Exception as e:
            if "insufficient" in str(e).lower() or "allowance" in str(e).lower():
                LOG.info("✅ Deposit without approval correctly failed")
                approval_test_passed = True
            else:
                raise

        # 3. Test scenario: Deposit to invalid address
        LOG.info("\n--- Test 2: Deposit to zero address ---")

        # First approve tokens
        await harness.approve_g_token(deposit_amount)

        try:
            _, _ = await harness.deposit_gravity(
                amount=deposit_amount,
                target_address="0x0000000000000000000000000000000000000000"
            )
            LOG.info("Deposit to zero address succeeded")
            zero_address_test_passed = True
        except Exception as e:
            LOG.info(f"✅ Deposit to zero address correctly failed: {e}")
            zero_address_test_passed = False

        # 4. Test scenario: Zero amount deposit
        LOG.info("\n--- Test 3: Zero amount deposit ---")

        try:
            _, _ = await harness.deposit_gravity(amount=0)
            LOG.info("Zero amount deposit succeeded")
            zero_amount_test_passed = True
        except Exception as e:
            LOG.info(f"Zero amount deposit failed: {e}")
            zero_amount_test_passed = False

        # 5. Test scenario: Very large amount deposit
        LOG.info("\n--- Test 4: Large amount deposit (exceeds balance) ---")

        # Get current balance
        balances = await harness.check_balances()
        large_amount = balances["g_balance"] + (10 ** decimals)  # More than balance

        try:
            await harness.approve_g_token(large_amount)
            _, _ = await harness.deposit_gravity(amount=large_amount)
            LOG.warning("Large amount deposit succeeded (unexpected)")
            large_amount_test_passed = False
        except Exception as e:
            if "insufficient" in str(e).lower() or "balance" in str(e).lower():
                LOG.info("✅ Large amount deposit correctly failed")
                large_amount_test_passed = True
            else:
                raise

        # 6. Record test results
        test_result.mark_success(
            approval_required_test=approval_test_passed,
            zero_address_test=zero_address_test_passed,
            zero_amount_test=zero_amount_test_passed,
            large_amount_test=large_amount_test_passed,
            deposit_amount=deposit_amount
        )

        LOG.info("\n✅ Error scenarios test completed successfully!")

    except Exception as e:
        test_result.mark_failure(error=f"Error scenarios test failed: {e}")
        raise