"""
Test Early Unstake Restrictions (Case 2) - Cluster Test Format
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
async def test_early_unstake_restrictions(cluster: Cluster):
    """
    Case 2: Early unstake restrictions test.
    """
    LOG.info("=" * 70)
    LOG.info("Test: Early Unstake Restrictions (Case 2)")
    LOG.info("=" * 70)

    assert await cluster.set_full_live(timeout=60), "Cluster failed to start"
    node = cluster.get_node("node1")
    w3 = Web3(Web3.HTTPProvider(node.url))
    faucet_key = cluster.faucet["private_key"]

    try:
        # Step 1: Create account and pool
        LOG.info("\n[Step 1] Setting up test account and pool...")
        staker = Account.create()
        await fund_account(w3, faucet_key, staker.address, 200 * 10**18)
        
        tx_builder = TransactionBuilder(w3, staker)
        staking_contract = get_staking_contract(w3)
        
        initial_stake = 100 * 10**18
        # 1 day + 1 hour buffer
        locked_until = get_current_time_micros() + (86400 + 3600) * 1_000_000 
        
        pool_address = await create_stake_pool(
            tx_builder=tx_builder,
            staking_contract=staking_contract,
            owner=staker.address,
            staker=staker.address,
            operator=staker.address,
            voter=staker.address,
            locked_until=locked_until,
            initial_stake_wei=initial_stake
        )
        
        assert pool_address, "Failed to create stake pool"
        
        pool_contract = get_pool_contract(w3, pool_address)
        
        # Step 2: Check initial state
        LOG.info("\n[Step 2] Checking initial state...")
        active_before = await run_sync(pool_contract.functions.activeStake().call)
        pending_before = await run_sync(pool_contract.functions.getTotalPending().call)
        
        LOG.info(f"Active stake before: {active_before / 10**18} ETH")
        LOG.info(f"Pending before: {pending_before / 10**18} ETH")
        
        assert active_before == initial_stake, "Initial active stake mismatch"
        
        # Step 3: Unstake a portion
        LOG.info("\n[Step 3] Unstaking 20 ETH...")
        unstake_amount = 20 * 10**18
        
        unstake_result = await tx_builder.build_and_send_tx(
            to=pool_address,
            data=pool_contract.encode_abi('unstake', [unstake_amount]),
            options=TransactionOptions(gas_limit=300_000)
        )
        
        assert unstake_result.success, f"Unstake failed: {unstake_result.error}"
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
        assert active_after == active_before - unstake_amount, "Active stake did not decrease correctly"
        
        # Verify pending increased
        assert pending_after == pending_before + unstake_amount, "Pending did not increase correctly"
        
        # Verify nothing is claimable yet (still in unbonding period)
        assert claimable == 0, f"Claimable should be 0 during unbonding, got {claimable}"
        
        # Step 5: Attempt withdrawal (should return 0)
        LOG.info("\n[Step 5] Attempting withdrawal of pending funds...")
        balance_before = w3.eth.get_balance(staker.address)
        
        withdraw_result = await tx_builder.build_and_send_tx(
            to=pool_address,
            data=pool_contract.encode_abi('withdrawAvailable', [staker.address]),
            options=TransactionOptions(gas_limit=200_000)
        )
        
        balance_after = w3.eth.get_balance(staker.address)
        
        # Funds should not increase (only gas spent)
        assert balance_after < balance_before, "Withdrawal succeeded despite unbonding period"
        
        LOG.info("\n" + "=" * 70)
        LOG.info("Test 'Early Unstake Restrictions' PASSED!")
        LOG.info("=" * 70)

    except Exception as e:
        LOG.error(f"Test failed: {e}")
        raise
