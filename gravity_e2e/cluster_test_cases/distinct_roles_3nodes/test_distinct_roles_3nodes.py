"""
E2E: 3-validator cluster where every validator has a different
operator / owner / staker EVM address. Exercises role separation on
multiple pools and proves the chain keeps producing blocks across
epoch boundaries.

Fixture: cluster_test_cases/distinct_roles_3nodes/{cluster,genesis}.toml
uses anvil[1..9] split 3-per-validator:
    validator0 → operator=anvil[1], owner=anvil[2], staker=anvil[3]
    validator1 → operator=anvil[4], owner=anvil[5], staker=anvil[6]
    validator2 → operator=anvil[7], owner=anvil[8], staker=anvil[9]

Epoch interval is set to 30 seconds in genesis.toml so the cross-epoch
liveness check only needs ~35 s of wall time.
"""

import asyncio
import logging
import time

import pytest
from eth_account import Account
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils.transaction_builder import (
    TransactionBuilder,
    TransactionOptions,
    run_sync,
)
from gravity_e2e.utils.staking_utils import (
    STAKING_PROXY_ADDRESS,
    VALIDATOR_MANAGER_ADDRESS,
    get_pool_contract,
)

LOG = logging.getLogger(__name__)

# Derive all 10 anvil accounts from the default foundry/anvil mnemonic so
# the private keys here stay in lockstep with the addresses baked into
# genesis.toml. Hardcoding the raw hex private keys is typo-prone — a single
# wrong byte derives to a totally different address and breaks the test.
Account.enable_unaudited_hdwallet_features()
_ANVIL_MNEMONIC = "test test test test test test test test test test test junk"
ANVIL_KEYS = [
    Account.from_mnemonic(
        _ANVIL_MNEMONIC, account_path=f"m/44'/60'/0'/0/{i}"
    ).key.hex()
    for i in range(10)
]

FAUCET_KEY = ANVIL_KEYS[0]

# (operator_key, owner_key, staker_key) per validator, in the same order as
# genesis_validators in genesis.toml.
VALIDATOR_ROLES = [
    (ANVIL_KEYS[1], ANVIL_KEYS[2], ANVIL_KEYS[3]),
    (ANVIL_KEYS[4], ANVIL_KEYS[5], ANVIL_KEYS[6]),
    (ANVIL_KEYS[7], ANVIL_KEYS[8], ANVIL_KEYS[9]),
]

EPOCH_SECONDS = 30  # mirror genesis.toml epoch_interval_micros

# Minimal extra ABI for Staking factory pool indexing (not in staking_utils).
STAKING_INDEX_ABI = [
    {
        "inputs": [{"internalType": "uint256", "name": "index", "type": "uint256"}],
        "name": "getPool",
        "outputs": [{"internalType": "address", "name": "", "type": "address"}],
        "stateMutability": "view",
        "type": "function",
    },
    {
        "inputs": [],
        "name": "getPoolCount",
        "outputs": [{"internalType": "uint256", "name": "", "type": "uint256"}],
        "stateMutability": "view",
        "type": "function",
    },
]

VALIDATOR_MANAGER_ABI = [
    {
        "inputs": [
            {"internalType": "address", "name": "stakePool", "type": "address"},
            {"internalType": "address", "name": "newRecipient", "type": "address"},
        ],
        "name": "setFeeRecipient",
        "outputs": [],
        "stateMutability": "nonpayable",
        "type": "function",
    },
]


def _staking_index(w3: Web3):
    return w3.eth.contract(address=STAKING_PROXY_ADDRESS, abi=STAKING_INDEX_ABI)


def _validator_manager(w3: Web3):
    return w3.eth.contract(address=VALIDATOR_MANAGER_ADDRESS, abi=VALIDATOR_MANAGER_ABI)


async def _fund(w3: Web3, target: str, amount_wei: int):
    faucet = Account.from_key(FAUCET_KEY)
    tb = TransactionBuilder(w3, faucet)
    result = await tb.send_ether(target, amount_wei)
    assert result.success, f"Funding {target} failed: {result.error}"


def _pool_addresses(w3: Web3) -> list[str]:
    staking = _staking_index(w3)
    count = staking.functions.getPoolCount().call()
    assert count >= 3, f"Expected >=3 genesis pools, got {count}"
    return [staking.functions.getPool(i).call() for i in range(3)]


@pytest.mark.asyncio
@pytest.mark.validator
async def test_all_three_pools_have_distinct_roles(cluster: Cluster):
    """Every genesis pool must report the three distinct EVM addresses
    wired in the fixture, and no two pools may share an operator."""
    assert await cluster.set_full_live(timeout=90), "Cluster failed to start"
    node = cluster.get_node("node1")
    w3 = node.w3

    pools = _pool_addresses(w3)
    LOG.info(f"genesis pools: {pools}")

    seen_operators = set()
    seen_stakers = set()
    for i, pool_addr in enumerate(pools):
        op_key, owner_key, staker_key = VALIDATOR_ROLES[i]
        expected_op = Account.from_key(op_key).address
        expected_staker = Account.from_key(staker_key).address
        expected_owner = Account.from_key(owner_key).address

        pool = get_pool_contract(w3, pool_addr)
        on_chain_op = await run_sync(pool.functions.getOperator().call)
        on_chain_staker = await run_sync(pool.functions.getStaker().call)

        assert on_chain_op.lower() == expected_op.lower(), (
            f"validator{i} operator mismatch: chain={on_chain_op}, expected={expected_op}"
        )
        assert on_chain_staker.lower() == expected_staker.lower(), (
            f"validator{i} staker mismatch: chain={on_chain_staker}, expected={expected_staker}"
        )
        assert expected_op.lower() != expected_owner.lower()
        assert expected_op.lower() != expected_staker.lower()
        assert expected_owner.lower() != expected_staker.lower()

        seen_operators.add(expected_op.lower())
        seen_stakers.add(expected_staker.lower())

    assert len(seen_operators) == 3, f"operators not unique across pools: {seen_operators}"
    assert len(seen_stakers) == 3, f"stakers not unique across pools: {seen_stakers}"


@pytest.mark.asyncio
@pytest.mark.validator
async def test_cross_epoch_liveness(cluster: Cluster):
    """Wait out more than one epoch interval and confirm the 3-node cluster
    keeps producing blocks. Also confirms each node's view agrees."""
    assert await cluster.set_full_live(timeout=90), "Cluster failed to start"

    nodes = [cluster.get_node(f"node{i + 1}") for i in range(3)]
    start_heights = [n.get_block_number() for n in nodes]
    start_wall = time.time()
    LOG.info(f"start heights: {start_heights}")

    # Wait for ~1.5 epochs to make sure at least one epoch boundary passes.
    wait_seconds = int(EPOCH_SECONDS * 1.5) + 5
    LOG.info(f"Waiting {wait_seconds}s for cross-epoch liveness check...")
    await asyncio.sleep(wait_seconds)

    end_heights = [n.get_block_number() for n in nodes]
    elapsed = time.time() - start_wall
    LOG.info(f"end heights (after {elapsed:.1f}s): {end_heights}")

    for i, (before, after) in enumerate(zip(start_heights, end_heights)):
        assert after > before, (
            f"node{i + 1} stopped producing blocks across epoch boundary: "
            f"{before} -> {after}"
        )

    # All nodes should be within a small window of each other (consensus).
    spread = max(end_heights) - min(end_heights)
    assert spread <= 5, f"Node heights diverged too much after epoch: {end_heights}"


@pytest.mark.asyncio
@pytest.mark.validator
async def test_post_epoch_role_gating_holds(cluster: Cluster):
    """After at least one epoch boundary, each validator's operator can
    still call setFeeRecipient and each staker can still call addStake.
    Non-role callers must still be rejected."""
    assert await cluster.set_full_live(timeout=90), "Cluster failed to start"
    node = cluster.get_node("node1")
    w3 = node.w3

    pools = _pool_addresses(w3)

    # Pre-fund every role EOA (9 accounts) with enough to pay gas + stake.
    for op_key, owner_key, staker_key in VALIDATOR_ROLES:
        for key, amount in (
            (op_key, Web3.to_wei(2, "ether")),
            (owner_key, Web3.to_wei(2, "ether")),
            (staker_key, Web3.to_wei(5, "ether")),
        ):
            await _fund(w3, Account.from_key(key).address, amount)

    # Wait past one epoch boundary.
    wait_seconds = EPOCH_SECONDS + 5
    LOG.info(f"Sleeping {wait_seconds}s to cross an epoch boundary...")
    await asyncio.sleep(wait_seconds)

    validator_manager = _validator_manager(w3)

    for i, pool_addr in enumerate(pools):
        op_key, owner_key, staker_key = VALIDATOR_ROLES[i]
        operator = Account.from_key(op_key)
        owner = Account.from_key(owner_key)
        staker = Account.from_key(staker_key)
        pool = get_pool_contract(w3, pool_addr)

        # Post-epoch setFeeRecipient: operator ok, owner+staker rejected.
        new_recipient = Account.create().address
        set_fee_data = validator_manager.encode_abi(
            "setFeeRecipient", [pool_addr, new_recipient]
        )

        for label, key in (("owner", owner_key), ("staker", staker_key)):
            tb = TransactionBuilder(w3, Account.from_key(key))
            result = await tb.build_and_send_tx(
                to=VALIDATOR_MANAGER_ADDRESS,
                data=set_fee_data,
                options=TransactionOptions(gas_limit=200_000),
            )
            assert not result.success, (
                f"validator{i} {label}.setFeeRecipient unexpectedly accepted"
            )

        tb = TransactionBuilder(w3, operator)
        result = await tb.build_and_send_tx(
            to=VALIDATOR_MANAGER_ADDRESS,
            data=set_fee_data,
            options=TransactionOptions(gas_limit=200_000),
        )
        assert result.success, (
            f"validator{i} operator.setFeeRecipient failed post-epoch: {result.error}"
        )

        # Post-epoch addStake: staker ok, operator+owner rejected.
        add_stake_data = pool.encode_abi("addStake", [])
        stake_value = Web3.to_wei(1, "ether")

        for label, key in (("operator", op_key), ("owner", owner_key)):
            tb = TransactionBuilder(w3, Account.from_key(key))
            result = await tb.build_and_send_tx(
                to=pool_addr,
                data=add_stake_data,
                value=stake_value,
                options=TransactionOptions(gas_limit=250_000),
            )
            assert not result.success, (
                f"validator{i} {label}.addStake unexpectedly accepted"
            )

        tb = TransactionBuilder(w3, staker)
        before = await run_sync(pool.functions.activeStake().call)
        result = await tb.build_and_send_tx(
            to=pool_addr,
            data=add_stake_data,
            value=stake_value,
            options=TransactionOptions(gas_limit=300_000),
        )
        assert result.success, (
            f"validator{i} staker.addStake failed post-epoch: {result.error}"
        )
        after = await run_sync(pool.functions.activeStake().call)
        assert after >= before + stake_value, (
            f"validator{i} activeStake did not grow: before={before} after={after}"
        )
        LOG.info(
            f"validator{i} post-epoch addStake OK: activeStake {before} -> {after}"
        )
