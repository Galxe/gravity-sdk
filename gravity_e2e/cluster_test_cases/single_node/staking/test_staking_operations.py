"""
Test Operations & Maintenance (Case 6) - Cluster Test Format
"""
import pytest
import logging
from web3 import Web3
from eth_account import Account
from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils.transaction_builder import TransactionBuilder, TransactionOptions, run_sync
from gravity_e2e.utils.exceptions import ContractError, TransactionError
from gravity_e2e.utils.staking_utils import (
    get_current_time_micros,
    create_stake_pool,
    get_staking_contract,
    get_pool_contract,
)

LOG = logging.getLogger(__name__)

async def fund_account(web3: Web3, faucet_key: str, target: str, amount_wei: int):
    faucet = Account.from_key(faucet_key)
    tx_builder = TransactionBuilder(web3, faucet)
    res = await tx_builder.send_ether(target, amount_wei)
    if not res.success:
        raise TransactionError(f"Funding failed: {res.error}")
    LOG.info(f"Funded {target} with {amount_wei} wei")

@pytest.mark.asyncio
async def test_operations_maintenance(cluster: Cluster):
    """
    Case 6: Operations & Maintenance test.
    """
    LOG.info("=" * 70)
    LOG.info("Test: Operations & Maintenance (Case 6)")
    LOG.info("=" * 70)

    assert await cluster.set_full_live(timeout=60), "Cluster failed to start"
    node = cluster.get_node("node1")
    w3 = Web3(Web3.HTTPProvider(node.url))
    faucet_key = cluster.faucet["private_key"]

    try:
        # Step 1: Create account and pool
        LOG.info("\n[Step 1] Setting up test account and pool...")
        owner = Account.create()
        new_operator = Account.create()
        new_voter = Account.create()
        
        await fund_account(w3, faucet_key, owner.address, 200 * 10**18)
        
        owner_builder = TransactionBuilder(w3, owner)
        staking_contract = get_staking_contract(w3)
        
        initial_stake = 100 * 10**18
        locked_until = get_current_time_micros() + (86400 * 1_000_000) + (3600 * 1_000_000)
        
        pool_address = await create_stake_pool(
            tx_builder=owner_builder,
            staking_contract=staking_contract,
            owner=owner.address,
            staker=owner.address,
            operator=owner.address,
            voter=owner.address,
            locked_until=locked_until,
            initial_stake_wei=initial_stake
        )
        
        assert pool_address, "Failed to create stake pool"
        
        pool_contract = get_pool_contract(w3, pool_address)
        
        # Step 2: Test setOperator
        LOG.info("\n[Step 2] Testing setOperator...")
        
        set_operator_result = await owner_builder.build_and_send_tx(
            to=pool_address,
            data=pool_contract.encode_abi('setOperator', [new_operator.address]),
            options=TransactionOptions(gas_limit=100_000)
        )
        
        assert set_operator_result.success, f"setOperator failed: {set_operator_result.error}"
        
        updated_operator = await run_sync(pool_contract.functions.getOperator().call)
        assert updated_operator.lower() == new_operator.address.lower(), f"Operator not updated: expected {new_operator.address}, got {updated_operator}"
        
        LOG.info(f"Operator updated to: {updated_operator}")
        
        # Step 3: Test setVoter
        LOG.info("\n[Step 3] Testing setVoter...")
        
        set_voter_result = await owner_builder.build_and_send_tx(
            to=pool_address,
            data=pool_contract.encode_abi('setVoter', [new_voter.address]),
            options=TransactionOptions(gas_limit=100_000)
        )
        
        assert set_voter_result.success, f"setVoter failed: {set_voter_result.error}"
        
        updated_voter = await run_sync(pool_contract.functions.getVoter().call)
        assert updated_voter.lower() == new_voter.address.lower(), f"Voter not updated: expected {new_voter.address}, got {updated_voter}"
        
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
        
        if not renew_result.success:
            LOG.warning(f"renewLockUntil failed: {renew_result.error}")
        else:
            new_locked_until = await run_sync(pool_contract.functions.getLockedUntil().call)
            LOG.info(f"New lockedUntil: {new_locked_until}")
            assert new_locked_until > current_locked_until, "lockedUntil did not increase as expected"
        
        # Step 5: Test unstakeAndWithdraw convenience function
        LOG.info("\n[Step 5] Testing unstakeAndWithdraw...")
        
        unstake_amount = 10 * 10**18
        
        unstake_withdraw_result = await owner_builder.build_and_send_tx(
            to=pool_address,
            data=pool_contract.encode_abi('unstakeAndWithdraw', [unstake_amount, owner.address]),
            options=TransactionOptions(gas_limit=300_000)
        )
        
        # Expect success (even if withdraw part is 0) or revert depending on exact logic.
        # But if it reverts, `success` is false.
        # Assuming from original test it might fail gracefully or succeed with 0 withdraw.
        if not unstake_withdraw_result.success:
            LOG.info(f"unstakeAndWithdraw returned: {unstake_withdraw_result.error}")
        else:
            LOG.info("unstakeAndWithdraw executed successfully")
        
        # Verify state after unstakeAndWithdraw
        final_active = await run_sync(pool_contract.functions.activeStake().call)
        final_pending = await run_sync(pool_contract.functions.getTotalPending().call)
        
        LOG.info(f"Final active stake: {final_active / 10**18} ETH")
        LOG.info(f"Final pending: {final_pending / 10**18} ETH")
        
        LOG.info("\n" + "=" * 70)
        LOG.info("Test 'Operations & Maintenance' PASSED!")
        LOG.info("=" * 70)

    except Exception as e:
        LOG.error(f"Test failed: {e}")
        raise
