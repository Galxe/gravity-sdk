"""EIP-2935 (HISTORY_STORAGE) e2e acceptance.

Verifies post-Prague behavior observable via contract execution:
  P-A1  direct eth_call to HISTORY_STORAGE returns parent block hash
  P-A2  multi-block lookups return correct hashes
  P-A3  Solidity contract using staticcall to HISTORY_STORAGE works on-chain
  P-A4  eth_call with n >= block.number reverts
  P-A5  parent-hash chain remains queryable after an epoch crossing
"""

import asyncio
import json
import logging
from pathlib import Path

import pytest
from web3 import Web3
from web3.exceptions import ContractLogicError

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils.transaction_builder import TransactionBuilder

LOG = logging.getLogger(__name__)

HISTORY_STORAGE_ADDRESS = Web3.to_checksum_address(
    "0x0000F90827F1C53a10cb7A02335B175320002935"
)
CONTRACTS_DIR = Path(__file__).parent / "contracts"


def _load_artifact(name: str):
    with open(CONTRACTS_DIR / f"{name}.json") as f:
        a = json.load(f)
    return a["abi"], a["bytecode"]


async def _wait_for_block(node, target: int, *, timeout: float = 60.0) -> int:
    """Block-poll until node reports block_number >= target. Returns the actual height."""
    deadline = asyncio.get_event_loop().time() + timeout
    while asyncio.get_event_loop().time() < deadline:
        height = node.w3.eth.block_number
        if height >= target:
            return height
        await asyncio.sleep(0.5)
    raise AssertionError(f"timed out waiting for block {target} (last={node.w3.eth.block_number})")


def _system_contract_call(node, n: int, *, block_identifier="latest") -> bytes:
    """Raw eth_call to HISTORY_STORAGE with 32-byte big-endian block number."""
    return node.w3.eth.call(
        {"to": HISTORY_STORAGE_ADDRESS, "data": "0x" + n.to_bytes(32, "big").hex()},
        block_identifier=block_identifier,
    )


@pytest.mark.asyncio
async def test_p_a1_parent_hash_via_eth_call(cluster: Cluster):
    """P-A1: eth_call(HISTORY_STORAGE, abi(N-1)) @ latest == block(N-1).hash."""
    assert await cluster.set_full_live(timeout=60), "cluster failed to become live"
    node = cluster.get_node("node1")

    # Need at least block 2 so N-1 is a populated post-Prague block.
    height = await _wait_for_block(node, 2)
    n = height - 1  # query parent of latest

    raw = _system_contract_call(node, n)
    expected = bytes(node.w3.eth.get_block(n)["hash"])
    LOG.info(f"P-A1 height={height} queried n={n} got={raw.hex()[:16]}… expected={expected.hex()[:16]}…")
    assert raw == expected, f"history slot mismatch: {raw.hex()} != {expected.hex()}"


@pytest.mark.asyncio
async def test_p_a2_multi_block_history(cluster: Cluster):
    """P-A2: eight consecutive lookups all return correct hashes."""
    node = cluster.get_node("node1")

    # Need height ≥ 10 so we can query [N-8, N-1] and they're all post-Prague blocks.
    height = await _wait_for_block(node, 10)

    for i in range(height - 8, height):
        raw = _system_contract_call(node, i)
        expected = bytes(node.w3.eth.get_block(i)["hash"])
        assert raw == expected, f"slot for n={i} mismatch: {raw.hex()} != {expected.hex()}"

    LOG.info(f"P-A2 verified 8 consecutive block hashes [{height-8}, {height-1}]")


@pytest.mark.asyncio
async def test_p_a3_solidity_history_reader(cluster: Cluster):
    """P-A3: HistoryReader.sol staticcalls HISTORY_STORAGE and returns the block hash.

    Proves Solidity contracts can use the EIP-2935 system contract as designed —
    the user-facing capability EIP-2935 unlocks for app developers.
    """
    node = cluster.get_node("node1")
    faucet = cluster.faucet
    assert faucet, "faucet required for contract deploy"

    abi, bytecode = _load_artifact("HistoryReader")
    tb = TransactionBuilder(node.w3, faucet)

    deploy_result = await tb.deploy_contract(bytecode=bytecode, abi=abi)
    assert deploy_result.success, f"deploy failed: {deploy_result.error}"
    contract_addr = deploy_result.tx_receipt["contractAddress"]
    LOG.info(f"P-A3 HistoryReader deployed at {contract_addr}")

    contract = node.w3.eth.contract(address=contract_addr, abi=abi)

    # Pick a block at least 5 back from latest so the slot is definitely SSTORE'd.
    height = await _wait_for_block(node, node.w3.eth.block_number + 5)
    n = height - 5

    via_solidity = bytes(contract.functions.getHash(n).call())
    via_rpc = bytes(node.w3.eth.get_block(n)["hash"])
    assert via_solidity == via_rpc, f"Solidity got {via_solidity.hex()}, RPC got {via_rpc.hex()}"


@pytest.mark.asyncio
async def test_p_a4_out_of_window_reverts(cluster: Cluster):
    """P-A4: eth_call with n >= block.number reverts."""
    node = cluster.get_node("node1")
    height = await _wait_for_block(node, 2)

    with pytest.raises(Exception) as excinfo:
        _system_contract_call(node, height + 1000)
    LOG.info(f"P-A4 expected revert raised: {type(excinfo.value).__name__}")


@pytest.mark.asyncio
async def test_p_a5_history_after_epoch_crossing(cluster: Cluster):
    """P-A5: lookups still work after at least one epoch boundary.

    Genesis epoch_interval_micros = 60_000_000 (60s). We wait ~75s for a fresh
    epoch crossing, then sanity-check the parent-hash chain again.
    """
    node = cluster.get_node("node1")
    start_height = node.w3.eth.block_number
    start_epoch = node.w3.eth.get_block(start_height).get("number")
    LOG.info(f"P-A5 waiting >60s for epoch crossing from block {start_height}")

    # Wait at least 75s of wall-clock. The single-node cluster produces blocks
    # steadily; the exact post-wait height isn't important — only that the
    # post-Prague SSTORE keeps working across the epoch boundary.
    await asyncio.sleep(75)

    height = node.w3.eth.block_number
    assert height > start_height, f"chain did not advance during sleep ({height} <= {start_height})"
    LOG.info(f"P-A5 advanced to block {height}")

    n = height - 1
    raw = _system_contract_call(node, n)
    expected = bytes(node.w3.eth.get_block(n)["hash"])
    assert raw == expected, f"post-epoch history mismatch: {raw.hex()} != {expected.hex()}"
