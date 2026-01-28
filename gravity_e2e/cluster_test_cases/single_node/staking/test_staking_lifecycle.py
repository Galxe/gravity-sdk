"""
Test Staking Lifecycle (Case 1) - Cluster Test Format
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
async def test_staking_lifecycle(cluster: Cluster):
    """
    Case 1: Complete staking lifecycle test.
    """
    LOG.info("=" * 70)
    LOG.info("Test: Staking Lifecycle (Case 1)")
    LOG.info("=" * 70)

    # Ensure cluster is live
    assert await cluster.set_full_live(timeout=60), "Cluster failed to start"
    
    node = cluster.get_node("node1")
    w3 = Web3(Web3.HTTPProvider(node.url))
    faucet_key = cluster.faucet["private_key"]

    try:
        # Step 1: Create test account
        LOG.info("\n[Step 1] Creating test account...")
        staker = Account.create()
        await fund_account(w3, faucet_key, staker.address, 200 * 10**18)
        LOG.info(f"Staker address: {staker.address}")
        
        # Step 2: Get contract instances
        LOG.info("\n[Step 2] Connecting to Staking contract...")
        staking_contract = get_staking_contract(w3)
        
        # Check minimum stake requirement
        try:
            min_stake = await run_sync(staking_contract.functions.getMinimumStake().call)
            LOG.info(f"Minimum stake requirement: {min_stake / 10**18} ETH")
        except Exception as e:
            LOG.warning(f"Could not get minimum stake: {e}")
        
        # Step 3: Create stake pool
        LOG.info("\n[Step 3] Creating stake pool...")
        tx_builder = TransactionBuilder(w3, staker)
        
        initial_stake = 100 * 10**18  # 100 ETH
        lock_duration_micros = 86400 * 1_000_000 # 1 day
        buffer_micros = 3600 * 1_000_000 # 1 hour buffer
        locked_until = get_current_time_micros() + lock_duration_micros + buffer_micros
        
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
        
        # Step 4: Verify pool state
        LOG.info("\n[Step 4] Verifying pool state...")
        pool_contract = get_pool_contract(w3, pool_address)
        
        active_stake = await run_sync(pool_contract.functions.activeStake().call)
        is_locked = await run_sync(pool_contract.functions.isLocked().call)
        claimable = await run_sync(pool_contract.functions.getClaimableAmount().call)
        
        LOG.info(f"Active stake: {active_stake / 10**18} ETH")
        LOG.info(f"Is locked: {is_locked}")
        LOG.info(f"Claimable amount: {claimable / 10**18} ETH")
        
        assert active_stake == initial_stake, f"Active stake mismatch: expected {initial_stake}, got {active_stake}"
        assert is_locked, "Pool should be locked but isLocked() returned false"
        assert claimable == 0, f"Claimable should be 0 during lock, got {claimable}"
        
        # Step 5: Add more stake
        LOG.info("\n[Step 5] Adding more stake...")
        additional_stake = 50 * 10**18  # 50 ETH
        
        add_result = await tx_builder.build_and_send_tx(
            to=pool_address,
            data=pool_contract.encode_abi('addStake', []),
            value=additional_stake,
            options=TransactionOptions(gas_limit=200_000)
        )
        
        assert add_result.success, f"Failed to add stake: {add_result.error}"
        
        # Verify updated stake
        new_active_stake = await run_sync(pool_contract.functions.activeStake().call)
        expected_total = initial_stake + additional_stake
        
        LOG.info(f"New active stake: {new_active_stake / 10**18} ETH")
        
        assert new_active_stake == expected_total, f"Total stake mismatch: expected {expected_total}, got {new_active_stake}"
        
        # Step 6: Attempt early withdrawal (should return 0)
        LOG.info("\n[Step 6] Attempting early withdrawal...")
        balance_before = w3.eth.get_balance(staker.address)
        
        # We expect a success tx but with 0 withdrawal amount OR revert depending on implementation
        # The original test says "early withdrawal correctly returned 0" so it expects success but no balance change.
        withdraw_result = await tx_builder.build_and_send_tx(
            to=pool_address,
            data=pool_contract.encode_abi('withdrawAvailable', [staker.address]),
            options=TransactionOptions(gas_limit=200_000)
        )
        
        balance_after = w3.eth.get_balance(staker.address)
        
        # Balance should only decrease due to gas (no withdrawal)
        assert balance_after < balance_before, "Withdrawal succeeded despite lockup - security issue!"
        
        LOG.info("Early withdrawal correctly returned 0 (lock enforced)")
        
        LOG.info("\n" + "=" * 70)
        LOG.info("Test 'Staking Lifecycle' PASSED!")
        LOG.info("=" * 70)

    except Exception as e:
        LOG.error(f"Test failed: {e}")
        raise
