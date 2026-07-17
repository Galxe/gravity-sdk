"""Exercise the V2 -> Off -> V2 randomness lifecycle on 4 validators + 1 VFN.

The Off -> V2 boundary intentionally has one transition epoch without block
randomness: Reconfiguration chooses the transition path from the current Off
config, then applies the pending V2 config at the end of that transition. DKG
starts at the following boundary and randomness resumes one epoch later.

The VFN is stopped during that warm-up epoch and restarted after randomness
resumes, proving that recovery preserves both zero- and nonzero-randomness
headers across the transition.
"""

from __future__ import annotations

import asyncio
import logging
import time

import pytest
from eth_abi import decode, encode
from eth_account import Account
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.cluster.node import Node, NodeRole
from gravity_e2e.utils.transaction_builder import TransactionBuilder

LOG = logging.getLogger(__name__)

GOVERNANCE = Web3.to_checksum_address("0x00000000000000000000000000000001625F3000")
RANDOMNESS_CONFIG = Web3.to_checksum_address("0x00000000000000000000000000000001625F1003")
STAKING = Web3.to_checksum_address("0x00000000000000000000000000000001625F2000")
DKG = Web3.to_checksum_address("0x00000000000000000000000000000001625F2002")
RECONFIGURATION = Web3.to_checksum_address("0x00000000000000000000000000000001625F2003")

FAUCET_KEY = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
FAUCET_ADDR = Web3.to_checksum_address("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")

CONFIG_ABI = "(uint8,(uint128,uint128,uint128))"
Config = tuple[int, tuple[int, int, int]]
OFF_CONFIG: Config = (0, (0, 0, 0))
V2_CONFIG: Config = (
    1,
    (9223372036854775808, 12297829382473033728, 12297829382473033728),
)
ZERO_RANDOMNESS = "0x" + "00" * 32

VOTING_DURATION_SECS = 5
EPOCH_INTERVAL_SECS = 30
MIN_PROPOSAL_WINDOW_SECS = 16
EPOCH_TIMEOUT_SECS = 3 * EPOCH_INTERVAL_SECS + 30
RANDOMNESS_TIMEOUT_SECS = 3 * EPOCH_INTERVAL_SECS + 60
RECOVERY_TIMEOUT_SECS = 4 * EPOCH_INTERVAL_SECS
SAMPLE_BLOCKS = 3
MAX_UINT128 = (1 << 128) - 1


def _selector(signature: str) -> bytes:
    return Web3.keccak(text=signature)[:4]


SEL_OWNER = _selector("owner()")
SEL_ADD_EXECUTOR = _selector("addExecutor(address)")
SEL_IS_EXECUTOR = _selector("isExecutor(address)")
SEL_CREATE_PROPOSAL = _selector("createProposal(address,address[],bytes[],string)")
SEL_VOTE = _selector("vote(address,uint64,uint128,bool)")
SEL_RESOLVE = _selector("resolve(uint64)")
SEL_EXECUTE = _selector("execute(uint64,address[],bytes[])")
SEL_GET_PROPOSAL_STATE = _selector("getProposalState(uint64)")
SEL_GET_POOL = _selector("getPool(uint256)")
SEL_GET_POOL_VOTER = _selector("getPoolVoter(address)")
SEL_GET_POOL_VOTING_POWER_NOW = _selector("getPoolVotingPowerNow(address)")
SEL_SET_RANDOMNESS = _selector("setForNextEpoch((uint8,(uint128,uint128,uint128)))")
SEL_GET_CURRENT_CONFIG = _selector("getCurrentConfig()")
SEL_GET_PENDING_CONFIG = _selector("getPendingConfig()")
SEL_CURRENT_EPOCH = _selector("currentEpoch()")
SEL_REMAINING_TIME = _selector("getRemainingTimeSeconds()")

PROPOSAL_CREATED_TOPIC = Web3.keccak(
    text="ProposalCreated(uint64,address,address,bytes32,string)"
)
DKG_COMPLETED_TOPIC = Web3.keccak(text="DKGCompleted(uint64,bytes32)")
PROPOSAL_STATE_SUCCEEDED = 1


def _send_tx(w3: Web3, to: str, data: bytes, gas: int = 1_000_000) -> dict:
    sender = Account.from_key(FAUCET_KEY)
    tx = {
        "to": to,
        "data": data,
        "gas": gas,
        "gasPrice": w3.eth.gas_price,
        "nonce": w3.eth.get_transaction_count(sender.address, "pending"),
        "chainId": w3.eth.chain_id,
    }
    signed = sender.sign_transaction(tx)
    tx_hash = w3.eth.send_raw_transaction(signed.raw_transaction)
    return dict(w3.eth.wait_for_transaction_receipt(tx_hash, timeout=60))


def _call(w3: Web3, to: str, data: bytes) -> bytes:
    return bytes(w3.eth.call({"to": to, "data": data}))


def _decode_address(raw: bytes) -> str:
    return Web3.to_checksum_address("0x" + raw[-20:].hex())


def _current_epoch(w3: Web3) -> int:
    return int.from_bytes(_call(w3, RECONFIGURATION, SEL_CURRENT_EPOCH), "big")


def _remaining_epoch_seconds(w3: Web3) -> int:
    return int.from_bytes(_call(w3, RECONFIGURATION, SEL_REMAINING_TIME), "big")


def _current_config(w3: Web3) -> Config:
    variant, thresholds = decode(
        [CONFIG_ABI], _call(w3, RANDOMNESS_CONFIG, SEL_GET_CURRENT_CONFIG)
    )[0]
    return int(variant), tuple(int(value) for value in thresholds)


def _pending_config(w3: Web3) -> tuple[bool, Config]:
    has_pending, config = decode(
        ["bool", CONFIG_ABI],
        _call(w3, RANDOMNESS_CONFIG, SEL_GET_PENDING_CONFIG),
    )
    variant, thresholds = config
    return bool(has_pending), (
        int(variant),
        tuple(int(value) for value in thresholds),
    )


def _block_randomness(block) -> str:
    value = block.get("mixHash", block.get("mix_hash"))
    assert value is not None, f"block {block['number']} has no mixHash"
    return Web3.to_hex(value).lower()


async def _wait_for_epoch_advance(
    cluster: Cluster, start_epoch: int, nodes: list[Node] | None = None
) -> tuple[int, int]:
    node1 = cluster.get_node("node1")
    observed_nodes = nodes if nodes is not None else list(cluster.nodes.values())
    deadline = time.monotonic() + EPOCH_TIMEOUT_SECS
    while time.monotonic() < deadline:
        epoch = _current_epoch(node1.w3)
        if epoch > start_epoch:
            for node in observed_nodes:
                while _current_epoch(node.w3) < epoch:
                    if time.monotonic() >= deadline:
                        raise TimeoutError(f"{node.id} did not reach epoch {epoch}")
                    await asyncio.sleep(1)
            return epoch, node1.w3.eth.block_number
        await asyncio.sleep(1)
    raise TimeoutError(f"chain did not advance past epoch {start_epoch}")


async def _wait_for_proposal_window(cluster: Cluster) -> int:
    node1 = cluster.get_node("node1")
    deadline = time.monotonic() + EPOCH_TIMEOUT_SECS
    while time.monotonic() < deadline:
        remaining = _remaining_epoch_seconds(node1.w3)
        if remaining >= MIN_PROPOSAL_WINDOW_SECS:
            return _current_epoch(node1.w3)
        current = _current_epoch(node1.w3)
        LOG.info("Only %ss remain in epoch %s; waiting for a clean window", remaining, current)
        await _wait_for_epoch_advance(cluster, current)
    raise TimeoutError("no epoch window was long enough for governance proposal")


async def _wait_for_config(
    cluster: Cluster,
    expected: Config,
    pending: bool = False,
    nodes: list[Node] | None = None,
) -> None:
    observed_nodes = nodes if nodes is not None else list(cluster.nodes.values())
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        matches = True
        for node in observed_nodes:
            if pending:
                has_pending, config = _pending_config(node.w3)
                matches &= has_pending and config == expected
            else:
                matches &= _current_config(node.w3) == expected
            if not matches:
                break
        if matches:
            return
        await asyncio.sleep(1)
    state = {
        node.id: (_pending_config(node.w3) if pending else _current_config(node.w3))
        for node in observed_nodes
    }
    raise AssertionError(f"config did not converge to {expected}: {state}")


def _assert_no_pending_config(
    cluster: Cluster, nodes: list[Node] | None = None
) -> None:
    observed_nodes = nodes if nodes is not None else list(cluster.nodes.values())
    for node in observed_nodes:
        has_pending, config = _pending_config(node.w3)
        assert not has_pending, f"{node.id} still has pending config {config}"


async def _wait_for_height(node, target: int, deadline: float) -> None:
    while node.w3.eth.block_number < target:
        if time.monotonic() >= deadline:
            raise TimeoutError(f"{node.id} did not sync to block {target}")
        await asyncio.sleep(1)


async def _assert_nodes_agree(
    cluster: Cluster,
    heights: list[int],
    nodes: list[Node] | None = None,
    timeout: float = 60,
) -> None:
    observed_nodes = nodes if nodes is not None else list(cluster.nodes.values())
    deadline = time.monotonic() + timeout
    expected = {
        height: cluster.get_node("node1").w3.eth.get_block(height) for height in heights
    }
    for node in observed_nodes:
        await _wait_for_height(node, heights[-1], deadline)
        for height in heights:
            actual = node.w3.eth.get_block(height)
            assert actual["hash"] == expected[height]["hash"], (
                f"{node.id} canonical hash mismatch at block {height}"
            )
            assert _block_randomness(actual) == _block_randomness(expected[height]), (
                f"{node.id} randomness mismatch at block {height}"
            )


async def _wait_for_randomness_mode(
    cluster: Cluster,
    enabled: bool,
    min_height: int = 1,
    nodes: list[Node] | None = None,
) -> list[int]:
    node1 = cluster.get_node("node1")
    deadline = time.monotonic() + RANDOMNESS_TIMEOUT_SECS
    latest = node1.w3.eth.block_number
    next_height = max(min_height, latest - SAMPLE_BLOCKS + 1)
    consecutive: list[int] = []

    while time.monotonic() < deadline:
        latest = node1.w3.eth.block_number
        while next_height <= latest:
            randomness = _block_randomness(node1.w3.eth.get_block(next_height))
            matches = (randomness != ZERO_RANDOMNESS) == enabled
            consecutive = (consecutive + [next_height])[-SAMPLE_BLOCKS:] if matches else []
            next_height += 1
            if len(consecutive) == SAMPLE_BLOCKS:
                await _assert_nodes_agree(cluster, consecutive, nodes=nodes)
                LOG.info(
                    "Randomness %s on all nodes for blocks %s",
                    "enabled" if enabled else "disabled",
                    consecutive,
                )
                return consecutive
        await asyncio.sleep(1)
    raise AssertionError(
        f"did not observe {SAMPLE_BLOCKS} consecutive blocks with randomness "
        f"{'enabled' if enabled else 'disabled'}"
    )


def _dkg_completed_epochs(w3: Web3, from_block: int, to_block: int) -> list[int]:
    logs = w3.eth.get_logs(
        {
            "address": DKG,
            "fromBlock": from_block,
            "toBlock": to_block,
            "topics": [DKG_COMPLETED_TOPIC],
        }
    )
    return [int.from_bytes(log["topics"][1], "big") for log in logs]


async def _assert_vfn_user_tx(
    cluster: Cluster, randomness_enabled: bool
) -> int:
    vfn = cluster.get_node("vfn1")
    receiver = Account.create()
    amount = Web3.to_wei(0.01, "ether")
    result = await TransactionBuilder(vfn.w3, cluster.faucet).send_ether(
        receiver.address, amount
    )
    assert result.success, f"VFN user transaction failed: {result.error}"
    assert result.block_number is not None

    await _assert_nodes_agree(cluster, [result.block_number])
    block = cluster.get_node("node1").w3.eth.get_block(result.block_number)
    randomness = _block_randomness(block)
    assert (randomness != ZERO_RANDOMNESS) == randomness_enabled, (
        f"block {result.block_number} randomness mode mismatch: {randomness}"
    )
    for node in cluster.nodes.values():
        assert node.w3.eth.get_balance(receiver.address) == amount, (
            f"{node.id} did not apply VFN transaction from block {result.block_number}"
        )
    return result.block_number


def _prepare_governance(w3: Web3) -> str:
    assert _decode_address(_call(w3, GOVERNANCE, SEL_OWNER)) == FAUCET_ADDR
    pool = _decode_address(_call(w3, STAKING, SEL_GET_POOL + encode(["uint256"], [0])))
    voter = _decode_address(
        _call(w3, STAKING, SEL_GET_POOL_VOTER + encode(["address"], [pool]))
    )
    assert voter == FAUCET_ADDR, f"pool[0] voter is {voter}, expected faucet"
    voting_power = int.from_bytes(
        _call(
            w3,
            STAKING,
            SEL_GET_POOL_VOTING_POWER_NOW + encode(["address"], [pool]),
        ),
        "big",
    )
    assert voting_power >= 10**18

    is_executor = _call(
        w3, GOVERNANCE, SEL_IS_EXECUTOR + encode(["address"], [FAUCET_ADDR])
    )[-1] == 1
    if not is_executor:
        receipt = _send_tx(
            w3,
            GOVERNANCE,
            SEL_ADD_EXECUTOR + encode(["address"], [FAUCET_ADDR]),
        )
        assert receipt["status"] == 1, f"addExecutor failed: {receipt}"
    return pool


async def _execute_proposal(
    w3: Web3, pool: str, target: str, call_data: bytes, description: str
) -> int:
    create_data = SEL_CREATE_PROPOSAL + encode(
        ["address", "address[]", "bytes[]", "string"],
        [pool, [target], [call_data], description],
    )
    w3.eth.call(
        {"from": FAUCET_ADDR, "to": GOVERNANCE, "data": create_data, "gas": 1_500_000}
    )
    receipt = _send_tx(w3, GOVERNANCE, create_data, gas=1_500_000)
    assert receipt["status"] == 1, f"createProposal failed: {receipt}"

    proposal_id = None
    for event in receipt["logs"]:
        if event["topics"] and bytes(event["topics"][0]) == bytes(PROPOSAL_CREATED_TOPIC):
            proposal_id = int.from_bytes(event["topics"][1], "big")
            break
    assert proposal_id is not None, "ProposalCreated event not found"

    vote_data = SEL_VOTE + encode(
        ["address", "uint64", "uint128", "bool"],
        [pool, proposal_id, MAX_UINT128, True],
    )
    receipt = _send_tx(w3, GOVERNANCE, vote_data)
    assert receipt["status"] == 1, f"vote failed: {receipt}"
    vote_block = w3.eth.block_number

    await asyncio.sleep(VOTING_DURATION_SECS + 2)
    deadline = time.monotonic() + 30
    while w3.eth.block_number < vote_block + 3 and time.monotonic() < deadline:
        await asyncio.sleep(1)
    assert w3.eth.block_number >= vote_block + 3, "chain stopped after governance vote"

    receipt = _send_tx(w3, GOVERNANCE, SEL_RESOLVE + encode(["uint64"], [proposal_id]))
    assert receipt["status"] == 1, f"resolve failed: {receipt}"
    state = int.from_bytes(
        _call(
            w3,
            GOVERNANCE,
            SEL_GET_PROPOSAL_STATE + encode(["uint64"], [proposal_id]),
        ),
        "big",
    )
    assert state == PROPOSAL_STATE_SUCCEEDED, f"proposal state is {state}"

    execute_data = SEL_EXECUTE + encode(
        ["uint64", "address[]", "bytes[]"],
        [proposal_id, [target], [call_data]],
    )
    receipt = _send_tx(w3, GOVERNANCE, execute_data, gas=1_500_000)
    assert receipt["status"] == 1, f"execute failed: {receipt}"
    return int(receipt["blockNumber"])


async def _queue_randomness_config(
    cluster: Cluster, pool: str, config: Config, description: str
) -> tuple[int, int]:
    node1 = cluster.get_node("node1")
    epoch = await _wait_for_proposal_window(cluster)
    call_data = SEL_SET_RANDOMNESS + encode([CONFIG_ABI], [config])
    block_number = await _execute_proposal(
        node1.w3, pool, RANDOMNESS_CONFIG, call_data, description
    )
    assert _current_epoch(node1.w3) == epoch, (
        "governance proposal crossed an epoch boundary; transition semantics are ambiguous"
    )
    await _wait_for_config(cluster, config, pending=True)
    return epoch, block_number


@pytest.mark.asyncio
@pytest.mark.randomness
@pytest.mark.epoch
async def test_randomness_on_off_on_with_vfn_recovery(cluster: Cluster):
    assert await cluster.set_full_live(timeout=180), "cluster failed to become fully live"
    validators = [
        node for node in cluster.nodes.values() if node.role == NodeRole.GENESIS
    ]
    vfns = [node for node in cluster.nodes.values() if node.role == NodeRole.VFN]
    assert len(validators) == 4, f"expected 4 validators, got {len(validators)}"
    assert len(vfns) == 1, f"expected 1 VFN, got {len(vfns)}"

    node1 = cluster.get_node("node1")
    pool = _prepare_governance(node1.w3)
    await _wait_for_config(cluster, V2_CONFIG)

    # Genesis starts in V2. Wait for the first completed DKG to make block
    # randomness mandatory before beginning the transition sequence.
    await _wait_for_randomness_mode(cluster, enabled=True)

    on_epoch, off_queued_block = await _queue_randomness_config(
        cluster, pool, OFF_CONFIG, "randomness-e2e-v2-to-off"
    )
    off_epoch, off_transition_block = await _wait_for_epoch_advance(cluster, on_epoch)
    await _wait_for_config(cluster, OFF_CONFIG)
    _assert_no_pending_config(cluster)
    assert on_epoch in _dkg_completed_epochs(
        node1.w3, off_queued_block + 1, off_transition_block
    ), "V2 -> Off boundary should finish the DKG selected from the old V2 config"
    await _wait_for_randomness_mode(
        cluster, enabled=False, min_height=off_transition_block + 1
    )

    off_epoch, on_queued_block = await _queue_randomness_config(
        cluster, pool, V2_CONFIG, "randomness-e2e-off-to-v2"
    )
    activation_epoch, activation_block = await _wait_for_epoch_advance(cluster, off_epoch)
    await _wait_for_config(cluster, V2_CONFIG)
    _assert_no_pending_config(cluster)
    assert not _dkg_completed_epochs(node1.w3, on_queued_block + 1, activation_block), (
        "Off -> V2 activation boundary must be immediate and must not run DKG"
    )

    # V2 is active now, but no session targets activation_epoch yet. A user
    # transaction through the VFN must land in a zero-randomness block.
    await _assert_vfn_user_tx(cluster, randomness_enabled=False)

    # Keep the VFN offline while validators finish the warm-up epoch and enter
    # the first randomized epoch. On restart it must retrieve and preserve both
    # kinds of headers from its validator peer.
    vfn = vfns[0]
    assert await vfn.stop(), "VFN failed to stop before randomness recovery"
    try:
        offline_start_height = node1.w3.eth.block_number + 1
        warmup_blocks = await _wait_for_randomness_mode(
            cluster,
            enabled=False,
            min_height=offline_start_height,
            nodes=validators,
        )

        resumed_epoch, resumed_block = await _wait_for_epoch_advance(
            cluster, activation_epoch, nodes=validators
        )
        assert resumed_epoch == activation_epoch + 1
        completed_epochs = _dkg_completed_epochs(
            node1.w3, activation_block + 1, resumed_block
        )
        assert activation_epoch in completed_epochs, (
            f"expected DKG completion for dealer epoch {activation_epoch}, "
            f"got {completed_epochs}"
        )
        await _wait_for_config(cluster, V2_CONFIG, nodes=validators)
        resumed_blocks = await _wait_for_randomness_mode(
            cluster,
            enabled=True,
            min_height=resumed_block + 1,
            nodes=validators,
        )
    finally:
        assert await vfn.start(), "VFN failed to restart after randomness recovery"

    recovery_blocks = warmup_blocks + resumed_blocks
    await _assert_nodes_agree(
        cluster,
        recovery_blocks,
        timeout=RECOVERY_TIMEOUT_SECS,
    )
    await _wait_for_config(cluster, V2_CONFIG)
    _assert_no_pending_config(cluster)
    await _assert_vfn_user_tx(cluster, randomness_enabled=True)

    LOG.info(
        "Randomness transition passed: V2(epoch=%s) -> Off(epoch=%s) -> "
        "V2 warm-up(epoch=%s) -> V2 randomized(epoch=%s), "
        "VFN recovered blocks=%s",
        on_epoch,
        off_epoch,
        activation_epoch,
        resumed_epoch,
        recovery_blocks,
    )
