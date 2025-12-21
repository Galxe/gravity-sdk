"""
Test Staking Lifecycle (Case 1)

Tests the complete staking flow from pool creation to stake addition to lockup verification.
Verifies that:
- Pool can be created with initial stake
- Additional stake can be added
- Funds are correctly locked
- Claimable amount is 0 during lock period
"""

import sys
from pathlib import Path

# Add package to path for absolute imports
_current_dir = Path(__file__).resolve().parent
_package_root = _current_dir.parent.parent.parent
if str(_package_root) not in sys.path:
    sys.path.insert(0, str(_package_root))

import asyncio
import logging

from gravity_e2e.helpers.test_helpers import RunHelper, TestResult, test_case
from gravity_e2e.utils.transaction_builder import TransactionBuilder, TransactionOptions, run_sync
from gravity_e2e.utils.exceptions import ContractError, TransactionError
from gravity_e2e.utils.staking_utils import (
    STAKING_PROXY_ADDRESS,
    STAKING_ABI,
    STAKE_POOL_ABI,
    get_current_time_micros,
    create_stake_pool,
    get_staking_contract,
    get_pool_contract,
)

LOG = logging.getLogger(__name__)


@test_case
async def test_staking_lifecycle(run_helper: RunHelper, test_result: TestResult):
    """
    Case 1: Complete staking lifecycle test.
    
    Tests the full flow from pool creation to stake addition to lockup verification.
    """
    LOG.info("=" * 70)
    LOG.info("Test: Staking Lifecycle (Case 1)")
    LOG.info("=" * 70)

    try:
        w3 = run_helper.client.web3
        
        # Step 1: Create test account
        LOG.info("\n[Step 1] Creating test account...")
        staker = await run_helper.create_test_account("staker", fund_wei=200 * 10**18)
        LOG.info(f"Staker address: {staker['address']}")
        
        # Step 2: Get contract instances
        LOG.info("\n[Step 2] Connecting to Staking contract...")
        staking_contract = get_staking_contract(w3)
        
        # Check minimum stake requirement
        try:
            min_stake = await run_sync(staking_contract.functions.getMinimumStake().call)
            LOG.info(f"Minimum stake requirement: {min_stake / 10**18} ETH")
        except Exception as e:
            LOG.warning(f"Could not get minimum stake: {e}")
            min_stake = 1 * 10**18  # Fallback to 1 ETH
        
        # Step 3: Create stake pool
        LOG.info("\n[Step 3] Creating stake pool...")
        tx_builder = TransactionBuilder(w3, staker['account'])
        
        initial_stake = 100 * 10**18  # 100 ETH
        # Lock for 1 day (in microseconds)
        lock_duration_micros = 86400 * 1_000_000
        # Add 1 hour buffer
        buffer_micros = 3600 * 1_000_000
        locked_until = get_current_time_micros() + lock_duration_micros + buffer_micros
        
        pool_address = await create_stake_pool(
            tx_builder=tx_builder,
            staking_contract=staking_contract,
            owner=staker['address'],
            staker=staker['address'],
            operator=staker['address'],
            voter=staker['address'],
            locked_until=locked_until,
            initial_stake_wei=initial_stake
        )
        
        if not pool_address:
            raise ContractError("Failed to create stake pool")
        
        # Step 4: Verify pool state
        LOG.info("\n[Step 4] Verifying pool state...")
        pool_contract = get_pool_contract(w3, pool_address)
        
        active_stake = await run_sync(pool_contract.functions.activeStake().call)
        is_locked = await run_sync(pool_contract.functions.isLocked().call)
        claimable = await run_sync(pool_contract.functions.getClaimableAmount().call)
        
        LOG.info(f"Active stake: {active_stake / 10**18} ETH")
        LOG.info(f"Is locked: {is_locked}")
        LOG.info(f"Claimable amount: {claimable / 10**18} ETH")
        
        # Verify initial stake
        if active_stake != initial_stake:
            raise ContractError(f"Active stake mismatch: expected {initial_stake}, got {active_stake}")
        
        # Verify lock is active
        if not is_locked:
            raise ContractError("Pool should be locked but isLocked() returned false")
        
        # Verify nothing is claimable (still locked)
        if claimable != 0:
            raise ContractError(f"Claimable should be 0 during lock, got {claimable}")
        
        # Step 5: Add more stake
        LOG.info("\n[Step 5] Adding more stake...")
        additional_stake = 50 * 10**18  # 50 ETH
        
        add_result = await tx_builder.build_and_send_tx(
            to=pool_address,
            data=pool_contract.encode_abi('addStake', []),
            value=additional_stake,
            options=TransactionOptions(gas_limit=200_000)
        )
        
        if not add_result.success:
            raise TransactionError(f"Failed to add stake: {add_result.error}")
        
        # Verify updated stake
        new_active_stake = await run_sync(pool_contract.functions.activeStake().call)
        expected_total = initial_stake + additional_stake
        
        LOG.info(f"New active stake: {new_active_stake / 10**18} ETH")
        
        if new_active_stake != expected_total:
            raise ContractError(f"Total stake mismatch: expected {expected_total}, got {new_active_stake}")
        
        # Step 6: Attempt early withdrawal (should return 0)
        LOG.info("\n[Step 6] Attempting early withdrawal...")
        balance_before = w3.eth.get_balance(staker['address'])
        
        withdraw_result = await tx_builder.build_and_send_tx(
            to=pool_address,
            data=pool_contract.encode_abi('withdrawAvailable', [staker['address']]),
            options=TransactionOptions(gas_limit=200_000)
        )
        
        balance_after = w3.eth.get_balance(staker['address'])
        
        # Balance should only decrease due to gas (no withdrawal)
        if balance_after > balance_before:
            raise ContractError("Withdrawal succeeded despite lockup - security issue!")
        
        LOG.info("Early withdrawal correctly returned 0 (lock enforced)")
        
        # Record success
        test_result.mark_success(
            pool_address=pool_address,
            initial_stake=initial_stake,
            additional_stake=additional_stake,
            total_stake=new_active_stake,
            is_locked=is_locked,
            lock_enforced=True
        )
        
        LOG.info("\n" + "=" * 70)
        LOG.info("Test 'Staking Lifecycle' PASSED!")
        LOG.info("=" * 70)

    except Exception as e:
        LOG.error(f"Test failed: {e}")
        test_result.mark_failure(error=str(e))
        raise


# Allow direct execution
if __name__ == "__main__":
    logging.basicConfig(
        level=logging.INFO,
        format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
    )
    
    from gravity_e2e.core.client.gravity_client import GravityClient
    
    async def run_direct():
        client = GravityClient("http://127.0.0.1:8545", "test_node")
        run_helper = RunHelper(
            client=client,
            working_dir=str(Path(__file__).parent),
            faucet_account=None
        )
        await test_staking_lifecycle(run_helper=run_helper)
    
    asyncio.run(run_direct())
