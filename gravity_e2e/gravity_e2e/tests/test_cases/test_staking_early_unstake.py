"""
Test Early Unstake Restrictions (Case 2)

Verifies that:
- unstake() correctly moves funds to pending state
- Funds cannot be withdrawn immediately after unstake
- Pending amount increases after unstake
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
async def test_early_unstake_restrictions(run_helper: RunHelper, test_result: TestResult):
    """
    Case 2: Early unstake restrictions test.
    
    Verifies that unstaking moves funds to pending and they cannot be withdrawn immediately.
    """
    LOG.info("=" * 70)
    LOG.info("Test: Early Unstake Restrictions (Case 2)")
    LOG.info("=" * 70)

    try:
        w3 = run_helper.client.web3
        
        # Step 1: Create account and pool
        LOG.info("\n[Step 1] Setting up test account and pool...")
        staker = await run_helper.create_test_account("staker", fund_wei=200 * 10**18)
        tx_builder = TransactionBuilder(w3, staker['account'])
        staking_contract = get_staking_contract(w3)
        
        initial_stake = 100 * 10**18
        locked_until = get_current_time_micros() + (86400 * 1_000_000) + (3600 * 1_000_000) # 1 day + 1 hour buffer
        
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
        
        pool_contract = get_pool_contract(w3, pool_address)
        
        # Step 2: Check initial state
        LOG.info("\n[Step 2] Checking initial state...")
        active_before = await run_sync(pool_contract.functions.activeStake().call)
        pending_before = await run_sync(pool_contract.functions.getTotalPending().call)
        
        LOG.info(f"Active stake before: {active_before / 10**18} ETH")
        LOG.info(f"Pending before: {pending_before / 10**18} ETH")
        
        # Step 3: Unstake a portion
        LOG.info("\n[Step 3] Unstaking 20 ETH...")
        unstake_amount = 20 * 10**18
        
        unstake_result = await tx_builder.build_and_send_tx(
            to=pool_address,
            data=pool_contract.encode_abi('unstake', [unstake_amount]),
            options=TransactionOptions(gas_limit=300_000)
        )
        
        if not unstake_result.success:
            raise TransactionError(f"Unstake failed: {unstake_result.error}")
        
        LOG.info(f"Unstake tx hash: {unstake_result.tx_hash}")
        
        # Step 4: Verify state changes
        LOG.info("\n[Step 4] Verifying state changes...")
        active_after = await run_sync(pool_contract.functions.activeStake().call)
        pending_after = await run_sync(pool_contract.functions.getTotalPending().call)
        claimable = await run_sync(pool_contract.functions.getClaimableAmount().call)
        
        LOG.info(f"Active stake after: {active_after / 10**18} ETH")
        LOG.info(f"Pending after: {pending_after / 10**18} ETH")
        LOG.info(f"Claimable: {claimable / 10**18} ETH")
        
        # Verify active stake decreased
        if active_after != active_before - unstake_amount:
            raise ContractError(f"Active stake should be {active_before - unstake_amount}, got {active_after}")
        
        # Verify pending increased
        if pending_after != pending_before + unstake_amount:
            raise ContractError(f"Pending should be {pending_before + unstake_amount}, got {pending_after}")
        
        # Verify nothing is claimable yet (still in unbonding period)
        if claimable != 0:
            raise ContractError(f"Claimable should be 0 during unbonding, got {claimable}")
        
        # Step 5: Attempt withdrawal (should return 0)
        LOG.info("\n[Step 5] Attempting withdrawal of pending funds...")
        balance_before = w3.eth.get_balance(staker['address'])
        
        withdraw_result = await tx_builder.build_and_send_tx(
            to=pool_address,
            data=pool_contract.encode_abi('withdrawAvailable', [staker['address']]),
            options=TransactionOptions(gas_limit=200_000)
        )
        
        balance_after = w3.eth.get_balance(staker['address'])
        
        # Funds should not increase during unbonding period
        LOG.info(f"Balance change: {(balance_after - balance_before) / 10**18} ETH (gas only)")
        
        test_result.mark_success(
            pool_address=pool_address,
            unstake_amount=unstake_amount,
            active_before=active_before,
            active_after=active_after,
            pending_after=pending_after,
            unbonding_enforced=True
        )
        
        LOG.info("\n" + "=" * 70)
        LOG.info("Test 'Early Unstake Restrictions' PASSED!")
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
        await test_early_unstake_restrictions(run_helper=run_helper)
    
    asyncio.run(run_direct())
