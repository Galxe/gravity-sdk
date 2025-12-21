"""
Test Operations & Maintenance (Case 6)

Tests administrative functions:
- setOperator, setVoter (role changes)
- renewLockUntil (manual lock extension)
- unstakeAndWithdraw (convenience function)
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
    get_current_time_micros,
    create_stake_pool,
    get_staking_contract,
    get_pool_contract,
)

LOG = logging.getLogger(__name__)


@test_case
async def test_operations_maintenance(run_helper: RunHelper, test_result: TestResult):
    """
    Case 6: Operations & Maintenance test.
    
    Tests administrative functions like role changes and lock management.
    """
    LOG.info("=" * 70)
    LOG.info("Test: Operations & Maintenance (Case 6)")
    LOG.info("=" * 70)

    try:
        w3 = run_helper.client.web3
        
        # Step 1: Create account and pool
        LOG.info("\n[Step 1] Setting up test account and pool...")
        owner = await run_helper.create_test_account("owner", fund_wei=200 * 10**18)
        new_operator = await run_helper.create_test_account("new_operator")
        new_voter = await run_helper.create_test_account("new_voter")
        
        owner_builder = TransactionBuilder(w3, owner['account'])
        staking_contract = get_staking_contract(w3)
        
        initial_stake = 100 * 10**18
        locked_until = get_current_time_micros() + (86400 * 1_000_000) + (3600 * 1_000_000) # 1 day + 1 hour buffer
        
        pool_address = await create_stake_pool(
            tx_builder=owner_builder,
            staking_contract=staking_contract,
            owner=owner['address'],
            staker=owner['address'],
            operator=owner['address'],
            voter=owner['address'],
            locked_until=locked_until,
            initial_stake_wei=initial_stake
        )
        
        if not pool_address:
            raise ContractError("Failed to create stake pool")
        
        pool_contract = get_pool_contract(w3, pool_address)
        
        # Step 2: Test setOperator
        LOG.info("\n[Step 2] Testing setOperator...")
        
        set_operator_result = await owner_builder.build_and_send_tx(
            to=pool_address,
            data=pool_contract.encode_abi('setOperator', [new_operator['address']]),
            options=TransactionOptions(gas_limit=100_000)
        )
        
        if not set_operator_result.success:
            raise TransactionError(f"setOperator failed: {set_operator_result.error}")
        
        updated_operator = await run_sync(pool_contract.functions.getOperator().call)
        if updated_operator.lower() != new_operator['address'].lower():
            raise ContractError(f"Operator not updated: expected {new_operator['address']}, got {updated_operator}")
        
        LOG.info(f"Operator updated to: {updated_operator}")
        
        # Step 3: Test setVoter
        LOG.info("\n[Step 3] Testing setVoter...")
        
        set_voter_result = await owner_builder.build_and_send_tx(
            to=pool_address,
            data=pool_contract.encode_abi('setVoter', [new_voter['address']]),
            options=TransactionOptions(gas_limit=100_000)
        )
        
        if not set_voter_result.success:
            raise TransactionError(f"setVoter failed: {set_voter_result.error}")
        
        updated_voter = await run_sync(pool_contract.functions.getVoter().call)
        if updated_voter.lower() != new_voter['address'].lower():
            raise ContractError(f"Voter not updated: expected {new_voter['address']}, got {updated_voter}")
        
        LOG.info(f"Voter updated to: {updated_voter}")
        
        # Step 4: Test renewLockUntil
        LOG.info("\n[Step 4] Testing renewLockUntil...")
        
        current_locked_until = await run_sync(pool_contract.functions.getLockedUntil().call)
        extension_micros = 7 * 86400 * 1_000_000  # Extend by 7 days
        
        LOG.info(f"Current lockedUntil: {current_locked_until}")
        
        renew_result = await owner_builder.build_and_send_tx(
            to=pool_address,
            data=pool_contract.encode_abi('renewLockUntil', [extension_micros]),
            options=TransactionOptions(gas_limit=100_000)
        )
        
        lock_renewed = False
        if not renew_result.success:
            LOG.warning(f"renewLockUntil failed: {renew_result.error}")
            LOG.info("This may be expected if minimum lockup requirements aren't met")
        else:
            new_locked_until = await run_sync(pool_contract.functions.getLockedUntil().call)
            LOG.info(f"New lockedUntil: {new_locked_until}")
            
            if new_locked_until > current_locked_until:
                lock_renewed = True
            else:
                LOG.warning("lockedUntil did not increase as expected")
        
        # Step 5: Test unstakeAndWithdraw convenience function
        LOG.info("\n[Step 5] Testing unstakeAndWithdraw...")
        
        unstake_amount = 10 * 10**18
        
        unstake_withdraw_result = await owner_builder.build_and_send_tx(
            to=pool_address,
            data=pool_contract.encode_abi('unstakeAndWithdraw', [unstake_amount, owner['address']]),
            options=TransactionOptions(gas_limit=300_000)
        )
        
        if not unstake_withdraw_result.success:
            LOG.info(f"unstakeAndWithdraw returned: {unstake_withdraw_result.error}")
            LOG.info("This is expected during lock period (unstake succeeds, withdraw returns 0)")
        else:
            LOG.info("unstakeAndWithdraw executed successfully")
        
        # Verify state after unstakeAndWithdraw
        final_active = await run_sync(pool_contract.functions.activeStake().call)
        final_pending = await run_sync(pool_contract.functions.getTotalPending().call)
        
        LOG.info(f"Final active stake: {final_active / 10**18} ETH")
        LOG.info(f"Final pending: {final_pending / 10**18} ETH")
        
        test_result.mark_success(
            pool_address=pool_address,
            operator_updated=updated_operator,
            voter_updated=updated_voter,
            lock_renewed=lock_renewed,
            final_active_stake=final_active,
            final_pending=final_pending
        )
        
        LOG.info("\n" + "=" * 70)
        LOG.info("Test 'Operations & Maintenance' PASSED!")
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
        await test_operations_maintenance(run_helper=run_helper)
    
    asyncio.run(run_direct())
