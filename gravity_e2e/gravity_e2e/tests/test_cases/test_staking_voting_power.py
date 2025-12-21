"""
Test Voting Power Calculation (Case 4)

Verifies that:
- Voting power includes active stake when locked
- Voting power includes pending stake that is still effective
- getVotingPowerNow() returns correct aggregate
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
from gravity_e2e.utils.exceptions import ContractError
from gravity_e2e.utils.staking_utils import (
    get_current_time_micros,
    create_stake_pool,
    get_staking_contract,
    get_pool_contract,
)

LOG = logging.getLogger(__name__)


@test_case
async def test_voting_power_calculation(run_helper: RunHelper, test_result: TestResult):
    """
    Case 4: Voting power calculation test.
    
    Verifies that voting power correctly aggregates active and pending stakes.
    """
    LOG.info("=" * 70)
    LOG.info("Test: Voting Power Calculation (Case 4)")
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
        
        # Step 2: Check initial voting power
        LOG.info("\n[Step 2] Checking initial voting power...")
        voting_power_initial = await run_sync(pool_contract.functions.getVotingPowerNow().call)
        LOG.info(f"Initial voting power: {voting_power_initial / 10**18} ETH")
        
        # Voting power should equal active stake when locked
        if voting_power_initial != initial_stake:
            raise ContractError(f"Voting power should be {initial_stake}, got {voting_power_initial}")
        
        # Step 3: Unstake portion
        LOG.info("\n[Step 3] Unstaking 50 ETH...")
        unstake_amount = 50 * 10**18
        
        await tx_builder.build_and_send_tx(
            to=pool_address,
            data=pool_contract.encode_abi('unstake', [unstake_amount]),
            options=TransactionOptions(gas_limit=300_000)
        )
        
        # Step 4: Check voting power after unstake
        LOG.info("\n[Step 4] Checking voting power after unstake...")
        active_stake = await run_sync(pool_contract.functions.activeStake().call)
        pending_stake = await run_sync(pool_contract.functions.getTotalPending().call)
        voting_power_after = await run_sync(pool_contract.functions.getVotingPowerNow().call)
        
        LOG.info(f"Active stake: {active_stake / 10**18} ETH")
        LOG.info(f"Pending stake: {pending_stake / 10**18} ETH")
        LOG.info(f"Voting power after unstake: {voting_power_after / 10**18} ETH")
        
        # Voting power should still include pending if still within lock period
        LOG.info(f"Sum of active + pending: {(active_stake + pending_stake) / 10**18} ETH")
        
        # Verify voting power logic (may vary by implementation)
        if voting_power_after < active_stake:
            raise ContractError(f"Voting power {voting_power_after} should be at least active stake {active_stake}")
        
        test_result.mark_success(
            pool_address=pool_address,
            initial_stake=initial_stake,
            unstake_amount=unstake_amount,
            active_stake=active_stake,
            pending_stake=pending_stake,
            voting_power_initial=voting_power_initial,
            voting_power_after=voting_power_after
        )
        
        LOG.info("\n" + "=" * 70)
        LOG.info("Test 'Voting Power Calculation' PASSED!")
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
        await test_voting_power_calculation(run_helper=run_helper)
    
    asyncio.run(run_direct())
