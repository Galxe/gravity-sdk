"""
E2E verification that a genesis validator can be configured with three
different EVM addresses for operator / owner / staker, and that role-gated
calls on-chain respect that separation.

Fixture: cluster_test_cases/distinct_roles/{cluster,genesis}.toml uses
anvil[1] as operator, anvil[2] as owner, anvil[3] as staker. The test
signs with the matching anvil private keys.
"""

import logging
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
    STAKE_POOL_ABI,
    get_pool_contract,
)

LOG = logging.getLogger(__name__)

# Derive all 4 anvil accounts from the default foundry/anvil mnemonic so
# the private keys stay in lockstep with the addresses in genesis.toml.
Account.enable_unaudited_hdwallet_features()
_ANVIL_MNEMONIC = "test test test test test test test test test test test junk"
_anvil = [
    Account.from_mnemonic(
        _ANVIL_MNEMONIC, account_path=f"m/44'/60'/0'/0/{i}"
    ).key.hex()
    for i in range(4)
]
ANVIL_FAUCET_KEY   = _anvil[0]  # 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
ANVIL_OPERATOR_KEY = _anvil[1]  # 0x70997970C51812dc3A010C7d01b50e0d17dc79C8
ANVIL_OWNER_KEY    = _anvil[2]  # 0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC
ANVIL_STAKER_KEY   = _anvil[3]  # 0x90F79bf6EB2c4f870365E785982E1f101E93b906

# Minimal extra ABI for Staking.getPool / getPoolCount (not exposed in staking_utils).
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

# Minimal ValidatorManagement ABI for setFeeRecipient.
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


def _staking_index_contract(w3: Web3):
    return w3.eth.contract(address=STAKING_PROXY_ADDRESS, abi=STAKING_INDEX_ABI)


def _validator_manager_contract(w3: Web3):
    return w3.eth.contract(address=VALIDATOR_MANAGER_ADDRESS, abi=VALIDATOR_MANAGER_ABI)


async def _fund(w3: Web3, target: str, amount_wei: int):
    faucet = Account.from_key(ANVIL_FAUCET_KEY)
    if w3.eth.get_balance(faucet.address) < amount_wei + Web3.to_wei(1, "ether"):
        # Faucet account isn't loaded on this chain — skip, target must already be funded.
        return
    tb = TransactionBuilder(w3, faucet)
    result = await tb.send_ether(target, amount_wei)
    assert result.success, f"Funding {target} failed: {result.error}"


@pytest.mark.asyncio
@pytest.mark.validator
async def test_genesis_validator_has_distinct_roles(cluster: Cluster):
    """The genesis StakePool should report three different EVM addresses for
    owner / staker / operator, matching the role overrides in genesis.toml."""
    assert await cluster.set_full_live(timeout=60), "Cluster failed to start"
    node = cluster.get_node("node1")
    w3 = node.w3

    operator = Account.from_key(ANVIL_OPERATOR_KEY)
    owner = Account.from_key(ANVIL_OWNER_KEY)
    staker = Account.from_key(ANVIL_STAKER_KEY)

    staking = _staking_index_contract(w3)
    pool_count = await run_sync(staking.functions.getPoolCount().call)
    assert pool_count >= 1, "No stake pools registered at genesis"

    pool_addr = await run_sync(staking.functions.getPool(0).call)
    LOG.info(f"Genesis StakePool #0 = {pool_addr}")

    pool = get_pool_contract(w3, pool_addr)
    on_chain_operator = await run_sync(pool.functions.getOperator().call)
    on_chain_staker = await run_sync(pool.functions.getStaker().call)

    assert on_chain_operator.lower() == operator.address.lower(), (
        f"operator mismatch: chain={on_chain_operator} expected={operator.address}"
    )
    assert on_chain_staker.lower() == staker.address.lower(), (
        f"staker mismatch: chain={on_chain_staker} expected={staker.address}"
    )
    # Note: there is no getOwner() on IStakePool; Ownable's owner() would
    # require its ABI. The setStaker test below implicitly verifies owner
    # identity by signing with ANVIL_OWNER_KEY and observing success.

    assert on_chain_operator.lower() != on_chain_staker.lower(), (
        "operator and staker resolved to the same address — fixture broken"
    )
    assert on_chain_operator.lower() != owner.address.lower(), (
        "operator and owner resolved to the same address — fixture broken"
    )
    assert on_chain_staker.lower() != owner.address.lower(), (
        "staker and owner resolved to the same address — fixture broken"
    )


@pytest.mark.asyncio
@pytest.mark.validator
async def test_only_staker_can_add_stake(cluster: Cluster):
    assert await cluster.set_full_live(timeout=60), "Cluster failed to start"
    node = cluster.get_node("node1")
    w3 = node.w3

    operator = Account.from_key(ANVIL_OPERATOR_KEY)
    owner = Account.from_key(ANVIL_OWNER_KEY)
    staker = Account.from_key(ANVIL_STAKER_KEY)

    # Make sure all three have enough gas + the staker has enough to addStake.
    for eoa, amount in (
        (operator.address, Web3.to_wei(2, "ether")),
        (owner.address, Web3.to_wei(2, "ether")),
        (staker.address, Web3.to_wei(10, "ether")),
    ):
        pre = w3.eth.get_balance(eoa)
        await _fund(w3, eoa, amount)
        post = w3.eth.get_balance(eoa)
        LOG.info(f"  fund {eoa}: {pre} -> {post} (delta {post - pre})")

    staking = _staking_index_contract(w3)
    pool_addr = await run_sync(staking.functions.getPool(0).call)
    pool = get_pool_contract(w3, pool_addr)
    on_chain_staker = await run_sync(pool.functions.getStaker().call)
    LOG.info(f"  pool={pool_addr} on-chain staker={on_chain_staker} expected={staker.address}")
    assert on_chain_staker.lower() == staker.address.lower(), (
        f"on-chain staker is {on_chain_staker}, expected {staker.address} — "
        "another test may have rotated it (test ordering bug)"
    )
    add_stake_data = pool.encode_abi("addStake", [])
    stake_value = Web3.to_wei(1, "ether")

    # Non-staker callers must revert.
    for label, key in (("operator", ANVIL_OPERATOR_KEY), ("owner", ANVIL_OWNER_KEY)):
        acct = Account.from_key(key)
        tb = TransactionBuilder(w3, acct)
        result = await tb.build_and_send_tx(
            to=pool_addr,
            data=add_stake_data,
            value=stake_value,
            options=TransactionOptions(gas_limit=200_000),
        )
        assert not result.success, (
            f"{label} unexpectedly allowed to addStake — role separation broken"
        )
        LOG.info(f"{label}.addStake correctly rejected: {result.error}")

    # Staker succeeds.
    tb = TransactionBuilder(w3, staker)
    before = await run_sync(pool.functions.activeStake().call)
    result = await tb.build_and_send_tx(
        to=pool_addr,
        data=add_stake_data,
        value=stake_value,
        options=TransactionOptions(gas_limit=250_000),
    )
    assert result.success, f"staker.addStake failed: {result.error}"
    after = await run_sync(pool.functions.activeStake().call)
    assert after >= before + stake_value, (
        f"activeStake did not grow: before={before} after={after}"
    )


@pytest.mark.asyncio
@pytest.mark.validator
async def test_only_operator_can_set_fee_recipient(cluster: Cluster):
    assert await cluster.set_full_live(timeout=60), "Cluster failed to start"
    node = cluster.get_node("node1")
    w3 = node.w3

    operator = Account.from_key(ANVIL_OPERATOR_KEY)
    owner = Account.from_key(ANVIL_OWNER_KEY)
    staker = Account.from_key(ANVIL_STAKER_KEY)
    new_fee_recipient = Account.create().address

    for eoa in (operator.address, owner.address, staker.address):
        await _fund(w3, eoa, Web3.to_wei(1, "ether"))

    staking = _staking_index_contract(w3)
    pool_addr = await run_sync(staking.functions.getPool(0).call)

    validator_manager = _validator_manager_contract(w3)
    data = validator_manager.encode_abi(
        "setFeeRecipient", [pool_addr, new_fee_recipient]
    )

    # Non-operator callers must revert.
    for label, key in (("owner", ANVIL_OWNER_KEY), ("staker", ANVIL_STAKER_KEY)):
        tb = TransactionBuilder(w3, Account.from_key(key))
        result = await tb.build_and_send_tx(
            to=VALIDATOR_MANAGER_ADDRESS,
            data=data,
            options=TransactionOptions(gas_limit=200_000),
        )
        assert not result.success, (
            f"{label} unexpectedly allowed to setFeeRecipient — role separation broken"
        )
        LOG.info(f"{label}.setFeeRecipient correctly rejected: {result.error}")

    # Operator succeeds.
    tb = TransactionBuilder(w3, operator)
    result = await tb.build_and_send_tx(
        to=VALIDATOR_MANAGER_ADDRESS,
        data=data,
        options=TransactionOptions(gas_limit=200_000),
    )
    assert result.success, f"operator.setFeeRecipient failed: {result.error}"


@pytest.mark.asyncio
@pytest.mark.validator
async def test_only_owner_can_rotate_staker(cluster: Cluster):
    """setStaker is gated by Ownable — only the pool owner can rotate it.
    This is the closest proxy we have for asserting the owner identity
    without pulling in the Ownable ABI."""
    assert await cluster.set_full_live(timeout=60), "Cluster failed to start"
    node = cluster.get_node("node1")
    w3 = node.w3

    operator = Account.from_key(ANVIL_OPERATOR_KEY)
    owner = Account.from_key(ANVIL_OWNER_KEY)
    staker = Account.from_key(ANVIL_STAKER_KEY)
    new_staker = Account.create().address

    for eoa in (operator.address, owner.address, staker.address):
        await _fund(w3, eoa, Web3.to_wei(1, "ether"))

    staking = _staking_index_contract(w3)
    pool_addr = await run_sync(staking.functions.getPool(0).call)
    pool = get_pool_contract(w3, pool_addr)
    data = pool.encode_abi("setStaker", [new_staker])

    # Non-owner callers must revert.
    for label, key in (("operator", ANVIL_OPERATOR_KEY), ("staker", ANVIL_STAKER_KEY)):
        tb = TransactionBuilder(w3, Account.from_key(key))
        result = await tb.build_and_send_tx(
            to=pool_addr,
            data=data,
            options=TransactionOptions(gas_limit=200_000),
        )
        assert not result.success, (
            f"{label} unexpectedly allowed to setStaker — owner identity wrong"
        )
        LOG.info(f"{label}.setStaker correctly rejected: {result.error}")

    # Owner succeeds; confirm the rotation took.
    tb = TransactionBuilder(w3, owner)
    result = await tb.build_and_send_tx(
        to=pool_addr,
        data=data,
        options=TransactionOptions(gas_limit=200_000),
    )
    assert result.success, f"owner.setStaker failed: {result.error}"

    rotated = await run_sync(pool.functions.getStaker().call)
    assert rotated.lower() == new_staker.lower(), (
        f"setStaker did not apply: on-chain staker={rotated}, expected={new_staker}"
    )
