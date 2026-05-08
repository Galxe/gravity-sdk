"""
Economic Model Tests — Deflation-Only Mode

Verifies Gravity's economic model:
  - Block reward = 0 (no inflation)
  - BaseFee is burned (EIP-1559 deflation)
  - Validators earn only tips (maxPriorityFeePerGas)
  - Rewards can be withdrawn via StakePool.withdrawRewards()

Genesis validator staker = Hardhat account #1
  Address:     0x70997970C51812dc3A010C7d01b50e0d17dc79C8
  Private key: 0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d
"""

import pytest
import logging
import asyncio
from web3 import Web3
from eth_account import Account
from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils.transaction_builder import TransactionBuilder, TransactionOptions, run_sync
from gravity_e2e.utils.staking_utils import get_pool_contract

LOG = logging.getLogger(__name__)

# Genesis validator staker — Hardhat account #1 (known private key)
VALIDATOR_STAKER_KEY = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d"
VALIDATOR_STAKER = Account.from_key(VALIDATOR_STAKER_KEY)


def _get_coinbase(w3: Web3) -> str:
    """Get the coinbase (StakePool address) from the latest block."""
    block = w3.eth.get_block("latest")
    coinbase = block.get("miner")
    assert coinbase, "Could not determine miner/coinbase from block"
    return Web3.to_checksum_address(coinbase)


async def _send_tip_txs(w3: Web3, sender, receiver_addr: str, count: int, tip_gwei: int = 2):
    """
    Send `count` EIP-1559 transactions with explicit tip.
    Returns list of (tx_hash, gasUsed, effectiveGasPrice, baseFee, tipPaid).
    """
    tb = TransactionBuilder(w3, sender)
    results = []

    for i in range(count):
        block = await run_sync(w3.eth.get_block, "latest")
        base_fee = block.get("baseFeePerGas", 50_000_000_000)
        priority_fee = tip_gwei * 10**9
        max_fee = base_fee + priority_fee + 1_000_000  # buffer

        result = await tb.build_and_send_tx(
            to=receiver_addr,
            value=i * 1000,  # trivial value variation
            options=TransactionOptions(
                gas_limit=21000,
                max_priority_fee_per_gas=priority_fee,
                max_fee_per_gas=max_fee,
            ),
        )
        assert result.success, f"Tx {i} failed: {result.error}"

        receipt = await run_sync(w3.eth.get_transaction_receipt, result.tx_hash)
        gas_used = receipt["gasUsed"]
        effective_price = receipt["effectiveGasPrice"]

        # Actual tip = min(maxPriorityFeePerGas, effectiveGasPrice - baseFee)
        tip_paid = min(priority_fee, effective_price - base_fee) * gas_used
        base_burned = base_fee * gas_used

        results.append({
            "tx_hash": result.tx_hash,
            "gas_used": gas_used,
            "effective_price": effective_price,
            "base_fee": base_fee,
            "tip_paid": tip_paid,
            "base_burned": base_burned,
        })
        LOG.info(f"Tx {i+1}/{count}: tip={tip_paid} wei, burned={base_burned} wei")

    return results


# ═══════════════════════════════════════════════════════════════════════
# Test 1: Empty blocks produce no reward
# ═══════════════════════════════════════════════════════════════════════

@pytest.mark.asyncio
async def test_empty_block_no_reward(cluster: Cluster):
    """
    Wait for a few empty blocks (no user transactions).
    Verify that the validator's StakePool getRewardBalance() == 0.
    This proves block reward is 0 (no inflation).
    """
    assert await cluster.set_full_live(timeout=60), "Cluster failed to start"
    node = cluster.get_node("node1")
    w3 = node.w3

    coinbase = _get_coinbase(w3)
    LOG.info(f"Coinbase (StakePool): {coinbase}")

    pool = get_pool_contract(w3, coinbase)

    # Snapshot reward balance before waiting
    reward_before = await run_sync(pool.functions.getRewardBalance().call)
    LOG.info(f"Reward balance before: {reward_before} wei")

    # Wait for a few blocks without sending any transactions
    start_block = w3.eth.block_number
    LOG.info(f"Waiting for empty blocks from block {start_block}...")
    await asyncio.sleep(10)

    end_block = w3.eth.block_number
    blocks_passed = end_block - start_block
    LOG.info(f"Blocks passed: {blocks_passed} ({start_block} -> {end_block})")
    assert blocks_passed >= 2, f"Expected >=2 blocks, got {blocks_passed}"

    reward_after = await run_sync(pool.functions.getRewardBalance().call)
    LOG.info(f"Reward balance after: {reward_after} wei")

    # No transactions → no tips → reward should not increase
    reward_increase = reward_after - reward_before
    assert reward_increase == 0, (
        f"Reward increased by {reward_increase} wei during empty blocks — "
        f"block reward should be 0!"
    )
    LOG.info("PASSED: Empty blocks produce no reward (block reward = 0)")


# ═══════════════════════════════════════════════════════════════════════
# Test 2: Tips go to the validator's StakePool
# ═══════════════════════════════════════════════════════════════════════

@pytest.mark.asyncio
async def test_tip_goes_to_validator(cluster: Cluster):
    """
    Send 5 EIP-1559 transactions with explicit tip.
    Verify the validator's StakePool reward balance increases by at least
    the sum of tips paid.
    """
    assert await cluster.set_full_live(timeout=60), "Cluster failed to start"
    node = cluster.get_node("node1")
    w3 = node.w3
    faucet = cluster.faucet

    # Create sender
    sender = Account.create()
    tb_faucet = TransactionBuilder(w3, faucet)
    res = await tb_faucet.send_ether(sender.address, Web3.to_wei(10, "ether"))
    assert res.success

    coinbase = _get_coinbase(w3)
    pool = get_pool_contract(w3, coinbase)

    reward_before = await run_sync(pool.functions.getRewardBalance().call)
    LOG.info(f"Reward before: {reward_before} wei")

    # Send 5 transactions with tip
    tx_results = await _send_tip_txs(w3, sender, sender.address, count=5, tip_gwei=2)
    total_tip = sum(r["tip_paid"] for r in tx_results)
    LOG.info(f"Total tip paid: {total_tip} wei ({total_tip / 10**18:.6f} ETH)")

    reward_after = await run_sync(pool.functions.getRewardBalance().call)
    reward_increase = reward_after - reward_before
    LOG.info(f"Reward after: {reward_after} wei (increase: {reward_increase} wei)")

    assert reward_increase >= total_tip, (
        f"StakePool reward increase ({reward_increase}) < total tip ({total_tip})"
    )
    LOG.info("PASSED: Tips correctly routed to validator StakePool")


# ═══════════════════════════════════════════════════════════════════════
# Test 3: BaseFee is burned (deflation), only tips go to validator
# ═══════════════════════════════════════════════════════════════════════

@pytest.mark.asyncio
async def test_basefee_burned_deflation(cluster: Cluster):
    """
    Send 5 transactions. Verify that:
    - StakePool reward increase ≈ total tips (not total gas fees)
    - BaseFee portion does NOT go to the validator
    This proves the deflation mechanism (baseFee burned).
    """
    assert await cluster.set_full_live(timeout=60), "Cluster failed to start"
    node = cluster.get_node("node1")
    w3 = node.w3
    faucet = cluster.faucet

    sender = Account.create()
    tb_faucet = TransactionBuilder(w3, faucet)
    res = await tb_faucet.send_ether(sender.address, Web3.to_wei(10, "ether"))
    assert res.success

    coinbase = _get_coinbase(w3)
    pool = get_pool_contract(w3, coinbase)

    pool_balance_before = await run_sync(w3.eth.get_balance, coinbase)
    reward_before = await run_sync(pool.functions.getRewardBalance().call)

    tx_results = await _send_tip_txs(w3, sender, sender.address, count=5, tip_gwei=3)

    total_tip = sum(r["tip_paid"] for r in tx_results)
    total_burned = sum(r["base_burned"] for r in tx_results)
    total_gas_fee = sum(r["gas_used"] * r["effective_price"] for r in tx_results)

    LOG.info(f"Total gas fee: {total_gas_fee} wei")
    LOG.info(f"  ├─ Tip (to validator): {total_tip} wei")
    LOG.info(f"  └─ BaseFee (burned):   {total_burned} wei")

    pool_balance_after = await run_sync(w3.eth.get_balance, coinbase)
    reward_after = await run_sync(pool.functions.getRewardBalance().call)

    balance_increase = pool_balance_after - pool_balance_before
    reward_increase = reward_after - reward_before

    LOG.info(f"StakePool balance increase: {balance_increase} wei")
    LOG.info(f"StakePool reward increase:  {reward_increase} wei")

    # The balance increase should be approximately the tip, NOT the full gas fee
    # Allow 5% tolerance for rounding / concurrent block activity
    assert reward_increase < total_gas_fee, (
        f"Reward increase ({reward_increase}) >= total gas fee ({total_gas_fee}), "
        f"baseFee was NOT burned!"
    )
    assert reward_increase >= total_tip, (
        f"Reward increase ({reward_increase}) < tip ({total_tip}), tip not delivered!"
    )

    LOG.info(f"PASSED: BaseFee burned ({total_burned} wei), only tips ({total_tip} wei) go to validator")


# ═══════════════════════════════════════════════════════════════════════
# Test 4: Validator can withdraw rewards to own account
# ═══════════════════════════════════════════════════════════════════════

@pytest.mark.asyncio
async def test_withdraw_rewards_to_account(cluster: Cluster):
    """
    1. Generate tips by sending transactions
    2. Verify getRewardBalance() > 0
    3. Call withdrawRewards(recipient) as staker
    4. Verify recipient received the rewards
    5. Verify getRewardBalance() == 0 after withdrawal
    """
    assert await cluster.set_full_live(timeout=60), "Cluster failed to start"
    node = cluster.get_node("node1")
    w3 = node.w3
    faucet = cluster.faucet

    # Fund the sender for tip generation
    sender = Account.create()
    tb_faucet = TransactionBuilder(w3, faucet)
    res = await tb_faucet.send_ether(sender.address, Web3.to_wei(10, "ether"))
    assert res.success

    coinbase = _get_coinbase(w3)
    pool = get_pool_contract(w3, coinbase)
    LOG.info(f"StakePool address: {coinbase}")

    # Verify the staker matches our known account
    staker_addr = await run_sync(pool.functions.getStaker().call)
    LOG.info(f"StakePool staker: {staker_addr}")
    assert staker_addr.lower() == VALIDATOR_STAKER.address.lower(), (
        f"Staker mismatch: expected {VALIDATOR_STAKER.address}, got {staker_addr}"
    )

    # Step 1: Generate tips
    LOG.info("Generating tips...")
    await _send_tip_txs(w3, sender, sender.address, count=5, tip_gwei=5)

    reward_balance = await run_sync(pool.functions.getRewardBalance().call)
    LOG.info(f"Reward balance after tips: {reward_balance} wei ({reward_balance / 10**18:.6f} ETH)")
    assert reward_balance > 0, "No reward accumulated after sending tip transactions"

    # Step 2: Fund staker for gas (withdrawRewards tx itself costs gas)
    res = await tb_faucet.send_ether(VALIDATOR_STAKER.address, Web3.to_wei(1, "ether"))
    assert res.success

    # Step 3: Create a recipient and withdraw
    recipient = Account.create()
    recipient_balance_before = await run_sync(w3.eth.get_balance, recipient.address)
    assert recipient_balance_before == 0

    LOG.info(f"Withdrawing rewards to {recipient.address}...")
    staker_tb = TransactionBuilder(w3, VALIDATOR_STAKER)
    withdraw_result = await staker_tb.build_and_send_tx(
        to=coinbase,
        data=pool.encode_abi("withdrawRewards", [recipient.address]),
        options=TransactionOptions(gas_limit=200_000, max_priority_fee_per_gas=0, max_fee_per_gas=10**11),
    )
    assert withdraw_result.success, f"withdrawRewards failed: {withdraw_result.error}"

    # Parse RewardsWithdrawn event
    receipt = await run_sync(w3.eth.get_transaction_receipt, withdraw_result.tx_hash)
    events = pool.events.RewardsWithdrawn().process_receipt(receipt)
    if events:
        withdrawn_amount = events[0]["args"]["amount"]
        LOG.info(f"RewardsWithdrawn event: amount={withdrawn_amount} wei")
    else:
        LOG.warning("RewardsWithdrawn event not found in receipt")
        withdrawn_amount = reward_balance  # fallback

    # Step 4: Verify recipient received the rewards
    recipient_balance_after = await run_sync(w3.eth.get_balance, recipient.address)
    LOG.info(f"Recipient balance: {recipient_balance_before} -> {recipient_balance_after} wei")
    assert recipient_balance_after >= reward_balance, (
        f"Recipient balance ({recipient_balance_after}) < expected ({reward_balance})"
    )

    # Step 5: Verify reward balance is 0 after withdrawal
    # withdrawRewards tx has tip=0 so no new rewards enter the pool
    remaining = await run_sync(pool.functions.getRewardBalance().call)
    LOG.info(f"Remaining reward balance: {remaining} wei")
    assert remaining == 0, f"Reward balance should be 0 after withdrawal, got {remaining}"

    LOG.info(f"PASSED: Withdrew {withdrawn_amount} wei rewards to {recipient.address}")

