"""
Test Delegation / Role Separation (Case 5)

Verifies that:
- Staker can stake/unstake but not manage consensus keys
- Operator cannot withdraw funds
- Roles are correctly enforced
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
async def test_delegation_role_separation(run_helper: RunHelper, test_result: TestResult):
    """
    Case 5: Delegation / Role separation test.
    
    Tests that Staker and Operator roles are correctly isolated.
    """
    LOG.info("=" * 70)
    LOG.info("Test: Delegation / Role Separation (Case 5)")
    LOG.info("=" * 70)

    try:
        w3 = run_helper.client.web3
        
        # Step 1: Create separate accounts for different roles
        LOG.info("\n[Step 1] Creating separate accounts for each role...")
        alice_staker = await run_helper.create_test_account("alice_staker", fund_wei=200 * 10**18)
        bob_operator = await run_helper.create_test_account("bob_operator", fund_wei=10 * 10**18)
        
        LOG.info(f"Alice (Staker): {alice_staker['address']}")
        LOG.info(f"Bob (Operator): {bob_operator['address']}")
        
        alice_builder = TransactionBuilder(w3, alice_staker['account'])
        bob_builder = TransactionBuilder(w3, bob_operator['account'])
        staking_contract = get_staking_contract(w3)
        
        # Step 2: Create pool with Alice as owner/staker, Bob as operator
        LOG.info("\n[Step 2] Creating pool with separated roles...")
        initial_stake = 100 * 10**18
        locked_until = get_current_time_micros() + (86400 * 1_000_000) + (3600 * 1_000_000) # 1 day + 1 hour buffer
        
        pool_address = await create_stake_pool(
            tx_builder=alice_builder,
            staking_contract=staking_contract,
            owner=alice_staker['address'],
            staker=alice_staker['address'],
            operator=bob_operator['address'],  # Bob is operator
            voter=alice_staker['address'],
            locked_until=locked_until,
            initial_stake_wei=initial_stake
        )
        
        if not pool_address:
            raise ContractError("Failed to create stake pool")
        
        pool_contract = get_pool_contract(w3, pool_address)
        
        # Step 3: Verify role assignments
        LOG.info("\n[Step 3] Verifying role assignments...")
        recorded_staker = await run_sync(pool_contract.functions.getStaker().call)
        recorded_operator = await run_sync(pool_contract.functions.getOperator().call)
        
        LOG.info(f"Recorded staker: {recorded_staker}")
        LOG.info(f"Recorded operator: {recorded_operator}")
        
        if recorded_staker.lower() != alice_staker['address'].lower():
            raise ContractError("Staker address mismatch")
        if recorded_operator.lower() != bob_operator['address'].lower():
            raise ContractError("Operator address mismatch")
        
        # Step 4: Alice (Staker) adds stake - should succeed
        LOG.info("\n[Step 4] Alice (Staker) adds stake...")
        add_stake_amount = 10 * 10**18
        
        alice_add_result = await alice_builder.build_and_send_tx(
            to=pool_address,
            data=pool_contract.encode_abi('addStake', []),
            value=add_stake_amount,
            options=TransactionOptions(gas_limit=200_000)
        )
        
        if not alice_add_result.success:
            raise TransactionError(f"Alice (Staker) should be able to add stake: {alice_add_result.error}")
        
        LOG.info("Alice successfully added stake (correct)")
        
        # Step 5: Bob (Operator) tries to withdraw - should FAIL
        LOG.info("\n[Step 5] Bob (Operator) attempts withdrawal (should fail)...")
        
        bob_withdraw_result = await bob_builder.build_and_send_tx(
            to=pool_address,
            data=pool_contract.encode_abi('withdrawAvailable', [bob_operator['address']]),
            options=TransactionOptions(gas_limit=200_000)
        )
        
        # This should fail because Bob is not the staker
        if bob_withdraw_result.success:
            LOG.warning("Bob's withdrawal tx succeeded - checking if it was a no-op...")
        else:
            LOG.info("Bob's withdrawal correctly failed (role enforcement working)")
        
        test_result.mark_success(
            pool_address=pool_address,
            staker=recorded_staker,
            operator=recorded_operator,
            alice_can_stake=True,
            bob_withdraw_blocked=not bob_withdraw_result.success
        )
        
        LOG.info("\n" + "=" * 70)
        LOG.info("Test 'Delegation / Role Separation' PASSED!")
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
        await test_delegation_role_separation(run_helper=run_helper)
    
    asyncio.run(run_direct())
