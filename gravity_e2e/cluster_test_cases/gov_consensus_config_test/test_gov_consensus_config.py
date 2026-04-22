"""
Governance Consensus-Config E2E Test

Exercises the full on-chain governance lifecycle against a 4-validator
cluster, using a proposal that rewrites the active ConsensusConfig via
ConsensusConfig.setForNextEpoch(bytes).

Lifecycle:
  Phase 0: Cluster up (all 4 nodes live)
  Phase 1: Preconditions — Governance.owner == faucet, baseline config,
           pool[0].voter == faucet (node1.address=faucet trick)
  Phase 2: addExecutor(faucet) — faucet is both owner and executor
  Phase 3: createProposal — target ConsensusConfig.setForNextEpoch(NEW_BYTES)
  Phase 4: vote YES with full voting power
  Phase 5: resolve after voting window
  Phase 6: execute; verify getPendingConfig() == NEW_BYTES
  Phase 7: wait past the next epoch boundary (applyPendingConfig)
  Phase 8: verify getCurrentConfig() == NEW_BYTES and all nodes keep producing blocks

Pins contracts ref to `fix/governance-initialize-from-genesis`, which adds
Governance.initialize(address) invoked from Genesis.initialize. Bump once the
contracts PR lands on main.

Run:
    ./gravity_e2e/run_test.sh gov_consensus_config_test -k test_gov_consensus_config_lifecycle
"""

import asyncio
import logging
import time

import pytest
from eth_abi import encode
from eth_account import Account
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster

LOG = logging.getLogger(__name__)

# ── Addresses ────────────────────────────────────────────────────────
GOVERNANCE = Web3.to_checksum_address("0x00000000000000000000000000000001625F3000")
CONSENSUS_CONFIG = Web3.to_checksum_address("0x00000000000000000000000000000001625F1007")
STAKING = Web3.to_checksum_address("0x00000000000000000000000000000001625F2000")

FAUCET_KEY = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
FAUCET_ADDR = Web3.to_checksum_address("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")

# ── Consensus-config payloads ────────────────────────────────────────
# OLD_BYTES matches aggregate_genesis default + this suite's genesis.toml.
OLD_BYTES = bytes.fromhex(
    "0301010a00000000000000280000000000000001010000000a"
    "000000000000000100010200000000000000000020000000000000"
)
# NEW_BYTES is the target config this test flips the chain to.
NEW_BYTES = bytes.fromhex(
    "0301010a00000000000000280000000000000002010100000000000000"
    "010000000000000001000000000000000a0000000a0000000000000001"
    "0000000000000001050000000a000000000000000100010200000000000000000020000000000000"
)

# ── Selectors ────────────────────────────────────────────────────────
def _selector(sig: str) -> bytes:
    return Web3.keccak(text=sig)[:4]


SEL_OWNER = _selector("owner()")
SEL_ADD_EXECUTOR = _selector("addExecutor(address)")
SEL_IS_EXECUTOR = _selector("isExecutor(address)")
SEL_GET_POOL = _selector("getPool(uint256)")
SEL_GET_POOL_VOTER = _selector("getPoolVoter(address)")
SEL_GET_POOL_VOTING_POWER_NOW = _selector("getPoolVotingPowerNow(address)")
SEL_CREATE_PROPOSAL = _selector("createProposal(address,address[],bytes[],string)")
SEL_VOTE = _selector("vote(address,uint64,uint128,bool)")
SEL_RESOLVE = _selector("resolve(uint64)")
SEL_EXECUTE = _selector("execute(uint64,address[],bytes[])")
SEL_GET_PROPOSAL_STATE = _selector("getProposalState(uint64)")
SEL_SET_FOR_NEXT_EPOCH = _selector("setForNextEpoch(bytes)")
SEL_GET_CURRENT_CONFIG = _selector("getCurrentConfig()")
SEL_GET_PENDING_CONFIG = _selector("getPendingConfig()")
SEL_CURRENT_EPOCH = _selector("currentEpoch()")
RECONFIGURATION = Web3.to_checksum_address("0x00000000000000000000000000000001625F2003")

# ProposalState: 0=PENDING, 1=SUCCEEDED, 2=FAILED, 3=EXECUTED, 4=CANCELLED
PROPOSAL_STATE_SUCCEEDED = 1

MAX_UINT128 = (1 << 128) - 1
VOTING_DURATION_SECS = 5     # matches genesis.toml voting_duration_micros
EPOCH_INTERVAL_SECS = 30     # matches genesis.toml epoch_interval_micros


# ── Transaction helpers ──────────────────────────────────────────────
def _send_tx(w3: Web3, to: str, data: bytes, sender_key: str, gas: int = 500_000) -> dict:
    """Send a signed tx from `sender_key` and return the mined receipt."""
    sender = Account.from_key(sender_key)
    nonce = w3.eth.get_transaction_count(sender.address)
    tx = {
        "to": to,
        "data": data,
        "gas": gas,
        "gasPrice": w3.eth.gas_price,
        "nonce": nonce,
        "chainId": w3.eth.chain_id,
    }
    signed = sender.sign_transaction(tx)
    tx_hash = w3.eth.send_raw_transaction(signed.raw_transaction)
    return w3.eth.wait_for_transaction_receipt(tx_hash, timeout=60)


def _call(w3: Web3, to: str, data: bytes) -> bytes:
    return w3.eth.call({"to": to, "data": data})


def _decode_address(raw: bytes) -> str:
    return Web3.to_checksum_address("0x" + raw[-20:].hex())


def _decode_bytes(raw: bytes) -> bytes:
    """Decode an ABI-encoded dynamic bytes return value."""
    # layout: [offset (32)][length (32)][data (padded)]
    length = int.from_bytes(raw[32:64], "big")
    return bytes(raw[64:64 + length])


def _decode_pending_config(raw: bytes) -> tuple[bool, bytes]:
    """Decode (bool hasPending, bytes config) — tuple with one dynamic tail."""
    has_pending = raw[31] == 1
    # The bytes offset is stored at slot 1 (32:64) relative to tuple start.
    bytes_offset = int.from_bytes(raw[32:64], "big")
    length = int.from_bytes(raw[bytes_offset:bytes_offset + 32], "big")
    data = bytes(raw[bytes_offset + 32:bytes_offset + 32 + length])
    return has_pending, data


def _current_epoch(w3: Web3) -> int:
    raw = _call(w3, RECONFIGURATION, SEL_CURRENT_EPOCH)
    return int.from_bytes(raw[-8:], "big")


async def _wait_for_epoch_advance(w3: Web3, start_epoch: int, timeout_s: int) -> int:
    """Poll Reconfiguration.currentEpoch() until it advances past start_epoch."""
    deadline = time.monotonic() + timeout_s
    while time.monotonic() < deadline:
        try:
            cur = _current_epoch(w3)
            if cur > start_epoch:
                return cur
        except Exception as e:
            LOG.debug(f"currentEpoch transient error: {e}")
        await asyncio.sleep(3)
    raise TimeoutError(
        f"chain did not advance past epoch {start_epoch} within {timeout_s}s"
    )


# ════════════════════════════════════════════════════════════════════════
# Test
# ════════════════════════════════════════════════════════════════════════

@pytest.mark.asyncio
async def test_gov_consensus_config_lifecycle(cluster: Cluster):
    # ── Phase 0: Cluster up ──────────────────────────────────────────
    LOG.info("=" * 60)
    LOG.info("  GOVERNANCE CONSENSUS-CONFIG E2E TEST")
    LOG.info("=" * 60)

    assert await cluster.set_full_live(timeout=180), "Cluster failed to become fully live"
    assert len(cluster.nodes) == 4, f"Expected 4 nodes, got {len(cluster.nodes)}"

    node1 = cluster.get_node("node1")
    w3 = node1.w3
    assert w3 is not None and w3.is_connected(), "node1 web3 not connected"

    LOG.info(f"Cluster up: {len(cluster.nodes)} nodes. node1 block={w3.eth.block_number}")

    # ── Phase 1: Preconditions ───────────────────────────────────────
    LOG.info("\n📌 Phase 1: Preconditions")

    owner_raw = _call(w3, GOVERNANCE, SEL_OWNER)
    owner_addr = _decode_address(owner_raw)
    LOG.info(f"  Governance.owner() = {owner_addr}")
    assert owner_addr == FAUCET_ADDR, (
        f"Governance owner wiring regressed: expected {FAUCET_ADDR}, got {owner_addr}. "
        f"Check contracts branch and genesis.toml governance_owner field."
    )

    current_raw = _call(w3, CONSENSUS_CONFIG, SEL_GET_CURRENT_CONFIG)
    current_bytes = _decode_bytes(current_raw)
    assert current_bytes == OLD_BYTES, (
        f"Baseline consensus config mismatch.\n"
        f"  expected: {OLD_BYTES.hex()}\n"
        f"  actual:   {current_bytes.hex()}"
    )
    LOG.info(f"  ConsensusConfig.getCurrentConfig() matches baseline ({len(current_bytes)}B)")

    pool_raw = _call(w3, STAKING, SEL_GET_POOL + encode(["uint256"], [0]))
    pool_addr = _decode_address(pool_raw)
    LOG.info(f"  Staking.getPool(0) = {pool_addr}")

    voter_raw = _call(
        w3, STAKING, SEL_GET_POOL_VOTER + encode(["address"], [pool_addr])
    )
    voter_addr = _decode_address(voter_raw)
    assert voter_addr == FAUCET_ADDR, (
        f"pool[0].voter expected {FAUCET_ADDR}, got {voter_addr}. "
        f"Ensure node1.address == faucet in genesis.toml."
    )

    vp_raw = _call(
        w3, STAKING, SEL_GET_POOL_VOTING_POWER_NOW + encode(["address"], [pool_addr])
    )
    voting_power = int.from_bytes(vp_raw, "big")
    LOG.info(f"  pool voting power = {voting_power}")
    assert voting_power >= 10**18, f"pool voting power too low: {voting_power}"

    # ── Phase 2: addExecutor ─────────────────────────────────────────
    LOG.info("\n📌 Phase 2: addExecutor(faucet)")

    data = SEL_ADD_EXECUTOR + encode(["address"], [FAUCET_ADDR])
    receipt = _send_tx(w3, GOVERNANCE, data, FAUCET_KEY)
    assert receipt["status"] == 1, f"addExecutor failed: {receipt}"

    is_exec_raw = _call(w3, GOVERNANCE, SEL_IS_EXECUTOR + encode(["address"], [FAUCET_ADDR]))
    assert is_exec_raw[-1] == 1, "isExecutor(faucet) returned false after addExecutor"
    LOG.info("  faucet is now an executor")

    # ── Phase 3: createProposal ──────────────────────────────────────
    LOG.info("\n📌 Phase 3: createProposal")

    set_data = SEL_SET_FOR_NEXT_EPOCH + encode(["bytes"], [NEW_BYTES])
    create_data = SEL_CREATE_PROPOSAL + encode(
        ["address", "address[]", "bytes[]", "string"],
        [pool_addr, [CONSENSUS_CONFIG], [set_data], "gov-consensus-config-e2e"],
    )
    # Surface revert reason via eth_call simulation before sending.
    try:
        w3.eth.call({"from": FAUCET_ADDR, "to": GOVERNANCE, "data": create_data, "gas": 1_000_000})
    except Exception as e:
        pytest.fail(f"createProposal would revert (eth_call): {e!r}")
    receipt = _send_tx(w3, GOVERNANCE, create_data, FAUCET_KEY, gas=1_000_000)
    assert receipt["status"] == 1, f"createProposal failed: {receipt}"

    # Extract proposal_id from ProposalCreated event (topic1 = indexed uint64)
    evt_topic = Web3.keccak(
        text="ProposalCreated(uint64,address,address,bytes32,string)"
    )
    proposal_id = None
    for log in receipt["logs"]:
        if log["topics"] and bytes(log["topics"][0]) == bytes(evt_topic):
            proposal_id = int.from_bytes(log["topics"][1], "big")
            break
    assert proposal_id is not None, "ProposalCreated event not found in receipt"
    LOG.info(f"  proposal_id = {proposal_id}")

    # ── Phase 4: vote ────────────────────────────────────────────────
    LOG.info("\n📌 Phase 4: vote YES")

    vote_data = SEL_VOTE + encode(
        ["address", "uint64", "uint128", "bool"],
        [pool_addr, proposal_id, MAX_UINT128, True],
    )
    receipt = _send_tx(w3, GOVERNANCE, vote_data, FAUCET_KEY)
    assert receipt["status"] == 1, f"vote failed: {receipt}"
    vote_block = w3.eth.block_number
    LOG.info(f"  voted at block {vote_block}")

    # ── Phase 5: resolve ─────────────────────────────────────────────
    LOG.info("\n📌 Phase 5: resolve after voting window")

    # Wall-clock and on-chain timestamp must both cross the voting deadline.
    await asyncio.sleep(VOTING_DURATION_SECS + 2)
    # Also make sure a few blocks pass so ITimestamp advances.
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline and w3.eth.block_number < vote_block + 3:
        await asyncio.sleep(1)
    assert w3.eth.block_number >= vote_block + 3, "chain not advancing past vote block"

    resolve_data = SEL_RESOLVE + encode(["uint64"], [proposal_id])
    receipt = _send_tx(w3, GOVERNANCE, resolve_data, FAUCET_KEY)
    assert receipt["status"] == 1, f"resolve failed: {receipt}"

    state_raw = _call(
        w3, GOVERNANCE, SEL_GET_PROPOSAL_STATE + encode(["uint64"], [proposal_id])
    )
    state = int.from_bytes(state_raw[-1:], "big")
    assert state == PROPOSAL_STATE_SUCCEEDED, (
        f"proposal not SUCCEEDED after resolve: state={state}"
    )
    LOG.info("  proposal SUCCEEDED")

    # ── Phase 6: execute ─────────────────────────────────────────────
    LOG.info("\n📌 Phase 6: execute")

    exec_data = SEL_EXECUTE + encode(
        ["uint64", "address[]", "bytes[]"],
        [proposal_id, [CONSENSUS_CONFIG], [set_data]],
    )
    receipt = _send_tx(w3, GOVERNANCE, exec_data, FAUCET_KEY, gas=1_000_000)
    assert receipt["status"] == 1, f"execute failed: {receipt}"

    pending_raw = _call(w3, CONSENSUS_CONFIG, SEL_GET_PENDING_CONFIG)
    has_pending, pending_bytes = _decode_pending_config(pending_raw)
    assert has_pending, "hasPendingConfig should be true after execute"
    assert pending_bytes == NEW_BYTES, (
        f"pending config mismatch:\n  expected: {NEW_BYTES.hex()}\n  actual:   {pending_bytes.hex()}"
    )
    LOG.info("  pending consensus config == NEW_BYTES")

    # ── Phase 7: wait past next epoch boundary ───────────────────────
    LOG.info("\n📌 Phase 7: wait past next epoch boundary")

    ep0 = _current_epoch(w3)
    LOG.info(f"  epoch before wait: {ep0}")

    ep_after = await _wait_for_epoch_advance(
        w3, ep0, timeout_s=3 * EPOCH_INTERVAL_SECS + 30
    )
    LOG.info(f"  epoch after wait:  {ep_after}")

    # ── Phase 8: verify swap + chain still producing ─────────────────
    LOG.info("\n📌 Phase 8: verify swap + continued block production")

    current_raw = _call(w3, CONSENSUS_CONFIG, SEL_GET_CURRENT_CONFIG)
    current_bytes = _decode_bytes(current_raw)
    assert current_bytes == NEW_BYTES, (
        f"config did not swap:\n  expected: {NEW_BYTES.hex()}\n  actual:   {current_bytes.hex()}"
    )
    LOG.info("  getCurrentConfig() == NEW_BYTES")

    pending_raw = _call(w3, CONSENSUS_CONFIG, SEL_GET_PENDING_CONFIG)
    has_pending2, _ = _decode_pending_config(pending_raw)
    assert not has_pending2, "hasPendingConfig should be false after apply"

    h = w3.eth.block_number
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline and w3.eth.block_number < h + 5:
        await asyncio.sleep(2)
    assert w3.eth.block_number >= h + 5, (
        f"node1 did not advance 5 blocks after swap (started at {h}, at {w3.eth.block_number})"
    )

    for node_id, node in cluster.nodes.items():
        bn = node.get_block_number()
        LOG.info(f"  {node_id} block={bn}")
        assert bn >= h + 3, f"{node_id} lagging after swap: {bn} < {h + 3}"

    LOG.info("\n✅ gov_consensus_config lifecycle complete")
