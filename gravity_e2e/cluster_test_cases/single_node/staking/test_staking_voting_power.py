"""
Test Voting Power Calculation (Case 4) - Cluster Test Format
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
async def test_voting_power_calculation(cluster: Cluster):
    """
    Case 4: Voting power calculation test.
    """
    LOG.info("=" * 70)
    LOG.info("Test: Voting Power Calculation (Case 4)")
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
        locked_until = get_current_time_micros() + (86400 * 1_000_000) + (3600 * 1_000_000)
        
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
        
        # Step 2: Check initial voting power
        LOG.info("\n[Step 2] Checking initial voting power...")
        voting_power_initial = await run_sync(pool_contract.functions.getVotingPowerNow().call)
        LOG.info(f"Initial voting power: {voting_power_initial / 10**18} ETH")
        
        # Voting power should equal active stake when locked
        assert voting_power_initial == initial_stake, f"Voting power mismatch: expected {initial_stake}, got {voting_power_initial}"
        
        # Step 3: Unstake portion
        LOG.info("\n[Step 3] Unstaking 50 ETH...")
        unstake_amount = 50 * 10**18
        
        res = await tx_builder.build_and_send_tx(
            to=pool_address,
            data=pool_contract.encode_abi('unstake', [unstake_amount]),
            options=TransactionOptions(gas_limit=300_000)
        )
        assert res.success, f"Unstake failed: {res.error}"
        
        # Step 4: Check voting power after unstake
        LOG.info("\n[Step 4] Checking voting power after unstake...")
        active_stake = await run_sync(pool_contract.functions.activeStake().call)
        pending_stake = await run_sync(pool_contract.functions.getTotalPending().call)
        voting_power_after = await run_sync(pool_contract.functions.getVotingPowerNow().call)
        
        LOG.info(f"Active stake: {active_stake / 10**18} ETH")
        LOG.info(f"Pending stake: {pending_stake / 10**18} ETH")
        LOG.info(f"Voting power after unstake: {voting_power_after / 10**18} ETH")
        
        # Voting power should still include pending if still within lock period
        # Verify voting power logic (implementation dependent, but typically sum for DKG)
        assert voting_power_after >= active_stake, f"Voting power {voting_power_after} should be at least active stake {active_stake}"
        
        LOG.info("\n" + "=" * 70)
        LOG.info("Test 'Voting Power Calculation' PASSED!")
        LOG.info("=" * 70)

    except Exception as e:
        LOG.error(f"Test failed: {e}")
        raise
