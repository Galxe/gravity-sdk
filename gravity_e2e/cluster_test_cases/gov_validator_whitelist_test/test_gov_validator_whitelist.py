"""
Governance Validator-Whitelist E2E Test

Exercises the address-based validator whitelist introduced in the contracts
branch `feat/whitelist-and-staking-config-setters`:

  ValidatorManagement._allowedPools : mapping(address => bool)
  setValidatorPoolAllowed(address,bool)      -- GOVERNANCE only
  setPermissionlessJoinEnabled(bool)         -- GOVERNANCE only
  isValidatorPoolAllowed(address)            -- view
  PoolNotWhitelisted(address) error          -- revert from register/join

Lifecycle:
  Phase 0: cluster up (4 nodes)
  Phase 1: preconditions — Governance.owner == faucet, the 4 genesis pools
           are auto-whitelisted, permissionless mode is off
  Phase 2: generate fresh ECDSA + BLS keys, fund the ECDSA from faucet,
           createPool — succeeds (createPool itself is not whitelist-gated)
  Phase 3: registerValidator on the new pool — expected to revert with
           PoolNotWhitelisted(new_pool)
  Phase 4: governance proposal targeting
           ValidatorManagement.setValidatorPoolAllowed(new_pool, true),
           full propose -> vote -> resolve -> execute lifecycle. Whitelist
           writes are immediate (no pending / epoch-boundary layer), so
           isValidatorPoolAllowed flips to true the moment execute lands.
  Phase 5: retry registerValidator + joinValidatorSet — both succeed;
           validator status becomes PENDING_ACTIVE
  Phase 6: liveness sanity — chain keeps producing blocks on all 4 nodes

Run:
    ./gravity_e2e/run_test.sh gov_validator_whitelist_test -k test_whitelist_governance_lifecycle
"""

import asyncio
import logging
import os
import subprocess
import tempfile
import time
from pathlib import Path

import pytest
import yaml
from eth_abi import encode
from eth_account import Account
from web3 import Web3
from web3.exceptions import ContractLogicError

from gravity_e2e.cluster.manager import Cluster

LOG = logging.getLogger(__name__)

# ── System addresses ─────────────────────────────────────────────────
GOVERNANCE = Web3.to_checksum_address("0x00000000000000000000000000000001625F3000")
STAKING = Web3.to_checksum_address("0x00000000000000000000000000000001625F2000")
VALIDATOR_MANAGER = Web3.to_checksum_address("0x00000000000000000000000000000001625F2001")
RECONFIGURATION = Web3.to_checksum_address("0x00000000000000000000000000000001625F2003")

FAUCET_KEY = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
FAUCET_ADDR = Web3.to_checksum_address("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")

# Matches genesis.toml [genesis.staking_config].minimum_stake and
# [genesis.validator_config].minimum_bond. 2 ETH gives us margin above both.
MIN_STAKE_WEI = 10**18
NEW_POOL_STAKE_WEI = 2 * 10**18

MAX_UINT128 = (1 << 128) - 1
VOTING_DURATION_SECS = 5      # matches genesis.toml voting_duration_micros (5e6)
EPOCH_INTERVAL_SECS = 30      # matches genesis.toml epoch_interval_micros (3e7)

# ProposalState enum: 0=PENDING 1=SUCCEEDED 2=FAILED 3=EXECUTED 4=CANCELLED
PROPOSAL_STATE_SUCCEEDED = 1


# ── Selectors ────────────────────────────────────────────────────────
def _selector(sig: str) -> bytes:
    return Web3.keccak(text=sig)[:4]


# Governance
SEL_OWNER = _selector("owner()")
SEL_ADD_EXECUTOR = _selector("addExecutor(address)")
SEL_IS_EXECUTOR = _selector("isExecutor(address)")
SEL_CREATE_PROPOSAL = _selector("createProposal(address,address[],bytes[],string)")
SEL_VOTE = _selector("vote(address,uint64,uint128,bool)")
SEL_RESOLVE = _selector("resolve(uint64)")
SEL_EXECUTE = _selector("execute(uint64,address[],bytes[])")
SEL_GET_PROPOSAL_STATE = _selector("getProposalState(uint64)")

# Staking
SEL_GET_POOL = _selector("getPool(uint256)")
SEL_GET_POOL_VOTER = _selector("getPoolVoter(address)")
SEL_GET_POOL_VOTING_POWER_NOW = _selector("getPoolVotingPowerNow(address)")
SEL_CREATE_POOL = _selector("createPool(address,address,address,address,uint64)")

# ValidatorManagement
SEL_SET_VALIDATOR_POOL_ALLOWED = _selector("setValidatorPoolAllowed(address,bool)")
SEL_IS_VALIDATOR_POOL_ALLOWED = _selector("isValidatorPoolAllowed(address)")
SEL_IS_PERMISSIONLESS_JOIN_ENABLED = _selector("isPermissionlessJoinEnabled()")
SEL_REGISTER_VALIDATOR = _selector(
    "registerValidator(address,string,bytes,bytes,bytes,bytes)"
)
SEL_JOIN_VALIDATOR_SET = _selector("joinValidatorSet(address)")
SEL_GET_VALIDATOR_STATUS = _selector("getValidatorStatus(address)")
SEL_GET_ACTIVE_VALIDATOR_COUNT = _selector("getActiveValidatorCount()")
SEL_IS_TRANSITION_IN_PROGRESS = _selector("isTransitionInProgress()")
SEL_CURRENT_EPOCH = _selector("currentEpoch()")

# Error selector: PoolNotWhitelisted(address) - used to recognize the revert
ERR_POOL_NOT_WHITELISTED = _selector("PoolNotWhitelisted(address)")

# ValidatorStatus enum: 0=INACTIVE 1=PENDING_ACTIVE 2=ACTIVE 3=PENDING_INACTIVE
VALIDATOR_STATUS_PENDING_ACTIVE = 1
VALIDATOR_STATUS_ACTIVE = 2

# EPOCH wait budget: 3 × epoch interval + slack. Must be generous because the
# chain can skip an epoch boundary if the previous reconfiguration is still
# finalising when the next tick arrives.
EPOCH_ADVANCE_TIMEOUT_S = 3 * EPOCH_INTERVAL_SECS + 30


# ── Transaction helpers ──────────────────────────────────────────────
def _send_tx(
    w3: Web3,
    to: str,
    data: bytes,
    sender_key: str,
    gas: int = 500_000,
    value: int = 0,
) -> dict:
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
        "value": value,
    }
    signed = sender.sign_transaction(tx)
    tx_hash = w3.eth.send_raw_transaction(signed.raw_transaction)
    return w3.eth.wait_for_transaction_receipt(tx_hash, timeout=60)


def _call(w3: Web3, to: str, data: bytes, sender: str | None = None) -> bytes:
    params = {"to": to, "data": data}
    if sender is not None:
        params["from"] = sender
    return w3.eth.call(params)


def _decode_address(raw: bytes) -> str:
    return Web3.to_checksum_address("0x" + raw[-20:].hex())


def _bool_from_return(raw: bytes) -> bool:
    return int.from_bytes(raw[-32:], "big") != 0


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


async def _wait_stable(w3: Web3, timeout_s: int = 60) -> None:
    """Wait until Reconfiguration.isTransitionInProgress() is false.

    Many ValidatorManagement entrypoints (createPool, registerValidator,
    joinValidatorSet) revert with ReconfigurationInProgress during epoch
    transitions. Call this before each sensitive write.
    """
    deadline = time.monotonic() + timeout_s
    while time.monotonic() < deadline:
        try:
            if not _bool_from_return(
                _call(w3, RECONFIGURATION, SEL_IS_TRANSITION_IN_PROGRESS)
            ):
                return
        except Exception:
            pass
        await asyncio.sleep(1)
    raise TimeoutError(f"reconfiguration window did not close within {timeout_s}s")


# ── gravity_cli integration ──────────────────────────────────────────
def _find_gravity_cli() -> Path:
    """Locate the gravity_cli binary, matching node_manager._find_gravity_cli."""
    workspace = Path(__file__).resolve().parents[3]
    for sub in ("debug", "release", "quick-release"):
        p = workspace / "target" / sub / "gravity_cli"
        if p.is_file():
            return p
    raise FileNotFoundError(
        f"gravity_cli not found under {workspace}/target/{{debug,release,quick-release}}"
    )


def _generate_bls_identity() -> dict:
    """Shell out to `gravity_cli genesis generate-key` and parse the YAML output.

    Returns dict with consensus_public_key, consensus_pop, network_public_key
    (all hex strings without 0x), plus account_address and private keys.
    """
    cli = _find_gravity_cli()
    with tempfile.NamedTemporaryFile(suffix=".yaml", delete=False) as f:
        out_path = f.name
    try:
        res = subprocess.run(
            [str(cli), "genesis", "generate-key", "--output-file", out_path],
            capture_output=True,
            text=True,
            timeout=30,
        )
        if res.returncode != 0:
            raise RuntimeError(
                f"gravity_cli generate-key failed: rc={res.returncode}\n"
                f"stdout: {res.stdout}\nstderr: {res.stderr}"
            )
        with open(out_path) as fh:
            identity = yaml.safe_load(fh)
    finally:
        os.unlink(out_path)

    for required in ("consensus_public_key", "consensus_pop", "network_public_key"):
        if required not in identity:
            raise RuntimeError(f"generate-key output missing {required}: {identity}")
    return identity


# ════════════════════════════════════════════════════════════════════════
# Test
# ════════════════════════════════════════════════════════════════════════

@pytest.mark.asyncio
async def test_whitelist_governance_lifecycle(cluster: Cluster):
    # ── Phase 0: Cluster up ──────────────────────────────────────────
    LOG.info("=" * 60)
    LOG.info("  GOVERNANCE VALIDATOR-WHITELIST E2E TEST")
    LOG.info("=" * 60)

    assert await cluster.set_full_live(timeout=180), "Cluster failed to become fully live"
    assert len(cluster.nodes) == 4, f"Expected 4 nodes, got {len(cluster.nodes)}"

    node1 = cluster.get_node("node1")
    w3 = node1.w3
    assert w3 is not None and w3.is_connected(), "node1 web3 not connected"
    LOG.info(f"Cluster up: {len(cluster.nodes)} nodes. node1 block={w3.eth.block_number}")

    # ── Phase 1: Preconditions ───────────────────────────────────────
    LOG.info("\n📌 Phase 1: Preconditions")

    owner_addr = _decode_address(_call(w3, GOVERNANCE, SEL_OWNER))
    assert owner_addr == FAUCET_ADDR, (
        f"Governance.owner() expected {FAUCET_ADDR}, got {owner_addr}. "
        f"Check contracts branch and genesis.toml governance_owner field."
    )
    LOG.info(f"  Governance.owner() = {owner_addr}")

    perm_raw = _call(w3, VALIDATOR_MANAGER, SEL_IS_PERMISSIONLESS_JOIN_ENABLED)
    assert not _bool_from_return(perm_raw), (
        "permissionless join must start disabled for the whitelist to gate joins"
    )
    LOG.info("  isPermissionlessJoinEnabled() = False")

    genesis_pools = []
    for i in range(4):
        pool_i = _decode_address(_call(w3, STAKING, SEL_GET_POOL + encode(["uint256"], [i])))
        allowed = _bool_from_return(
            _call(w3, VALIDATOR_MANAGER,
                  SEL_IS_VALIDATOR_POOL_ALLOWED + encode(["address"], [pool_i]))
        )
        assert allowed, f"genesis pool[{i}] ({pool_i}) should be auto-whitelisted at genesis"
        genesis_pools.append(pool_i)
    LOG.info(f"  all 4 genesis pools whitelisted: {genesis_pools}")

    pool0 = genesis_pools[0]
    voter = _decode_address(
        _call(w3, STAKING, SEL_GET_POOL_VOTER + encode(["address"], [pool0]))
    )
    assert voter == FAUCET_ADDR, (
        f"pool[0].voter expected {FAUCET_ADDR}, got {voter}. "
        f"Ensure node1.address == faucet in genesis.toml."
    )
    vp = int.from_bytes(
        _call(w3, STAKING, SEL_GET_POOL_VOTING_POWER_NOW + encode(["address"], [pool0])),
        "big",
    )
    assert vp >= 10**18, f"pool[0] voting power too low: {vp}"
    LOG.info(f"  pool[0]={pool0} voter=faucet voting_power={vp}")

    # Wait past the initial epoch transition: Staking.createPool and
    # ValidatorManagement join paths all revert with ReconfigurationInProgress
    # mid-transition. _wait_stable also guards later phases where governance
    # execute can land during a fresh transition.
    await _wait_stable(w3)
    while w3.eth.block_number < 2:
        await asyncio.sleep(1)
    LOG.info(f"  chain stable at block {w3.eth.block_number}")

    # ── Phase 2: fresh key, fund, createPool ─────────────────────────
    LOG.info("\n📌 Phase 2: fund a fresh key and createPool")

    new_account = Account.create()
    NEW_KEY = new_account.key.hex()
    NEW_ADDR = Web3.to_checksum_address(new_account.address)
    LOG.info(f"  fresh ECDSA address: {NEW_ADDR}")

    # Fund from faucet: enough for the stake plus several txs of gas.
    fund_amount = NEW_POOL_STAKE_WEI + 5 * 10**17  # stake + 0.5 ETH for gas
    sender = Account.from_key(FAUCET_KEY)
    nonce = w3.eth.get_transaction_count(sender.address)
    fund_tx = {
        "to": NEW_ADDR, "value": fund_amount,
        "gas": 21000, "gasPrice": w3.eth.gas_price,
        "nonce": nonce, "chainId": w3.eth.chain_id,
    }
    signed = sender.sign_transaction(fund_tx)
    tx_hash = w3.eth.send_raw_transaction(signed.raw_transaction)
    rcpt = w3.eth.wait_for_transaction_receipt(tx_hash, timeout=60)
    assert rcpt["status"] == 1, f"fund transfer failed: {rcpt}"
    bal = w3.eth.get_balance(NEW_ADDR)
    assert bal == fund_amount, f"funded balance {bal} != expected {fund_amount}"
    LOG.info(f"  funded {NEW_ADDR} with {fund_amount / 10**18} ETH")

    # createPool with the fresh key as owner/staker/operator/voter.
    # StakePool's constructor checks _lockedUntil >= nowMicroseconds() + lockupDuration,
    # using the chain's ITimestamp (not wall clock). Hard-code a far-future
    # timestamp like gov_consensus_config_test does — same sentinel as
    # genesis.toml initial_locked_until_micros (≈ year 2027).
    locked_until = 1798848000000000

    create_pool_data = SEL_CREATE_POOL + encode(
        ["address", "address", "address", "address", "uint64"],
        [NEW_ADDR, NEW_ADDR, NEW_ADDR, NEW_ADDR, locked_until],
    )
    # eth_call simulate first so we get a clean revert reason if something's off.
    try:
        w3.eth.call(
            {"from": NEW_ADDR, "to": STAKING, "data": create_pool_data,
             "gas": 5_000_000, "value": NEW_POOL_STAKE_WEI}
        )
    except Exception as e:
        pytest.fail(f"createPool would revert (eth_call): {e!r}")
    pool_receipt = _send_tx(
        w3, STAKING, create_pool_data, NEW_KEY,
        gas=5_000_000, value=NEW_POOL_STAKE_WEI,
    )
    assert pool_receipt["status"] == 1, f"createPool failed: {pool_receipt}"

    # PoolCreated(creator[topic1], pool[topic2], owner[topic3], staker, poolIndex)
    pool_created_topic = Web3.keccak(
        text="PoolCreated(address,address,address,address,uint256)"
    )
    new_pool = None
    for log in pool_receipt["logs"]:
        if log["topics"] and bytes(log["topics"][0]) == bytes(pool_created_topic):
            new_pool = Web3.to_checksum_address("0x" + bytes(log["topics"][2])[-20:].hex())
            break
    assert new_pool is not None, "PoolCreated event not found"
    LOG.info(f"  new pool deployed at {new_pool}")

    # Sanity: the new pool is NOT on the whitelist.
    allowed = _bool_from_return(
        _call(w3, VALIDATOR_MANAGER,
              SEL_IS_VALIDATOR_POOL_ALLOWED + encode(["address"], [new_pool]))
    )
    assert not allowed, f"new pool {new_pool} should not be whitelisted yet"
    LOG.info(f"  isValidatorPoolAllowed({new_pool}) = False (as expected)")

    # Generate real BLS consensus keys for the post-whitelist register/join.
    identity = _generate_bls_identity()
    consensus_pubkey = bytes.fromhex(identity["consensus_public_key"])
    consensus_pop = bytes.fromhex(identity["consensus_pop"])
    # Network addresses must parse at the consensus layer when the validator
    # set is swapped at the next epoch boundary — the chain tries BCS String
    # THEN BCS Vec<NetworkAddress> and panics if neither works. An empty
    # Vec<NetworkAddress> encodes as a single ULEB128 zero byte, which parses
    # cleanly as "no addresses". That's exactly what we want: the 5th
    # validator has no running gravity_node, so no peer should try to dial it.
    network_addr = b"\x00"
    fullnode_addr = b"\x00"
    moniker = "whitelist-e2e"
    assert len(consensus_pubkey) == 48, f"consensus pubkey should be 48 bytes, got {len(consensus_pubkey)}"
    assert len(consensus_pop) == 96, f"consensus pop should be 96 bytes, got {len(consensus_pop)}"
    LOG.info(f"  generated BLS identity (pubkey {len(consensus_pubkey)}B, pop {len(consensus_pop)}B)")

    register_data = SEL_REGISTER_VALIDATOR + encode(
        ["address", "string", "bytes", "bytes", "bytes", "bytes"],
        [new_pool, moniker, consensus_pubkey, consensus_pop, network_addr, fullnode_addr],
    )

    # ── Phase 3: register on new pool reverts with PoolNotWhitelisted ─
    LOG.info("\n📌 Phase 3: registerValidator should revert with PoolNotWhitelisted")

    revert_seen = False
    try:
        _call(w3, VALIDATOR_MANAGER, register_data, sender=NEW_ADDR)
    except ContractLogicError as e:
        # web3.py exposes the revert data on the exception in recent versions.
        # Fall back to matching the error string if structured data is absent.
        data_attr = getattr(e, "data", None) or str(e)
        data_hex = data_attr.hex() if isinstance(data_attr, (bytes, bytearray)) else str(data_attr).lower()
        err_sel = ERR_POOL_NOT_WHITELISTED.hex()
        assert err_sel in data_hex, (
            f"registerValidator should revert with PoolNotWhitelisted(0x{err_sel}); "
            f"got: {data_hex}"
        )
        revert_seen = True
    assert revert_seen, "registerValidator unexpectedly succeeded in eth_call simulation"
    LOG.info("  eth_call surfaced PoolNotWhitelisted revert ✓")

    # ── Phase 4: governance adds the new pool to the whitelist ───────
    LOG.info("\n📌 Phase 4: governance proposal -> setValidatorPoolAllowed(new_pool, true)")

    # addExecutor(faucet) — idempotent: isExecutor is always set afterward.
    _send_tx(w3, GOVERNANCE, SEL_ADD_EXECUTOR + encode(["address"], [FAUCET_ADDR]), FAUCET_KEY)
    assert _bool_from_return(
        _call(w3, GOVERNANCE, SEL_IS_EXECUTOR + encode(["address"], [FAUCET_ADDR]))
    ), "isExecutor(faucet) returned false after addExecutor"
    LOG.info("  faucet is an executor")

    allow_call = SEL_SET_VALIDATOR_POOL_ALLOWED + encode(
        ["address", "bool"], [new_pool, True]
    )
    create_data = SEL_CREATE_PROPOSAL + encode(
        ["address", "address[]", "bytes[]", "string"],
        [pool0, [VALIDATOR_MANAGER], [allow_call], "gov-validator-whitelist-e2e"],
    )
    # Surface revert reason early if the proposal would fail to create.
    try:
        w3.eth.call(
            {"from": FAUCET_ADDR, "to": GOVERNANCE, "data": create_data, "gas": 1_000_000}
        )
    except Exception as e:
        pytest.fail(f"createProposal would revert (eth_call): {e!r}")
    receipt = _send_tx(w3, GOVERNANCE, create_data, FAUCET_KEY, gas=1_000_000)
    assert receipt["status"] == 1, f"createProposal failed: {receipt}"

    proposal_created_topic = Web3.keccak(
        text="ProposalCreated(uint64,address,address,bytes32,string)"
    )
    proposal_id = None
    for log in receipt["logs"]:
        if log["topics"] and bytes(log["topics"][0]) == bytes(proposal_created_topic):
            proposal_id = int.from_bytes(log["topics"][1], "big")
            break
    assert proposal_id is not None, "ProposalCreated event not found"
    LOG.info(f"  proposal_id = {proposal_id}")

    # Vote YES with full voting power from pool[0].
    vote_data = SEL_VOTE + encode(
        ["address", "uint64", "uint128", "bool"],
        [pool0, proposal_id, MAX_UINT128, True],
    )
    receipt = _send_tx(w3, GOVERNANCE, vote_data, FAUCET_KEY)
    assert receipt["status"] == 1, f"vote failed: {receipt}"
    vote_block = w3.eth.block_number
    LOG.info(f"  voted YES at block {vote_block}")

    # Wait past the voting window (wall clock + block clock both).
    await asyncio.sleep(VOTING_DURATION_SECS + 2)
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline and w3.eth.block_number < vote_block + 3:
        await asyncio.sleep(1)
    assert w3.eth.block_number >= vote_block + 3, "chain not advancing past vote block"

    # Resolve → SUCCEEDED.
    receipt = _send_tx(
        w3, GOVERNANCE, SEL_RESOLVE + encode(["uint64"], [proposal_id]), FAUCET_KEY
    )
    assert receipt["status"] == 1, f"resolve failed: {receipt}"
    state = int.from_bytes(
        _call(w3, GOVERNANCE, SEL_GET_PROPOSAL_STATE + encode(["uint64"], [proposal_id]))[-1:],
        "big",
    )
    assert state == PROPOSAL_STATE_SUCCEEDED, f"proposal not SUCCEEDED: state={state}"
    LOG.info("  proposal SUCCEEDED")

    # Execute → fires setValidatorPoolAllowed(new_pool, true).
    exec_data = SEL_EXECUTE + encode(
        ["uint64", "address[]", "bytes[]"],
        [proposal_id, [VALIDATOR_MANAGER], [allow_call]],
    )
    receipt = _send_tx(w3, GOVERNANCE, exec_data, FAUCET_KEY, gas=1_000_000)
    assert receipt["status"] == 1, f"execute failed: {receipt}"
    LOG.info("  execute landed")

    # Verify ValidatorPoolAllowed(new_pool, true) was emitted.
    allowed_topic = Web3.keccak(text="ValidatorPoolAllowed(address,bool)")
    event_seen = False
    for log in receipt["logs"]:
        if (log["topics"] and bytes(log["topics"][0]) == bytes(allowed_topic)
                and len(log["topics"]) >= 2):
            pool_from_evt = Web3.to_checksum_address(
                "0x" + bytes(log["topics"][1])[-20:].hex()
            )
            if pool_from_evt == new_pool and int.from_bytes(bytes(log["data"]), "big") == 1:
                event_seen = True
                break
    assert event_seen, "ValidatorPoolAllowed(new_pool, true) event not found in execute receipt"

    # Whitelist flips immediately — no epoch wait needed.
    allowed_now = _bool_from_return(
        _call(w3, VALIDATOR_MANAGER,
              SEL_IS_VALIDATOR_POOL_ALLOWED + encode(["address"], [new_pool]))
    )
    assert allowed_now, "isValidatorPoolAllowed(new_pool) should be true after execute"
    LOG.info(f"  isValidatorPoolAllowed({new_pool}) = True ✓")

    # ── Phase 5: retry registerValidator + joinValidatorSet ──────────
    LOG.info("\n📌 Phase 5: registerValidator + joinValidatorSet succeed after whitelisting")

    await _wait_stable(w3)
    receipt = _send_tx(
        w3, VALIDATOR_MANAGER, register_data, NEW_KEY, gas=3_000_000
    )
    assert receipt["status"] == 1, f"registerValidator failed after whitelist: {receipt}"
    LOG.info("  registerValidator succeeded")

    await _wait_stable(w3)
    join_data = SEL_JOIN_VALIDATOR_SET + encode(["address"], [new_pool])
    # eth_call first to get a clean revert reason if something else is off.
    try:
        w3.eth.call({"from": NEW_ADDR, "to": VALIDATOR_MANAGER,
                     "data": join_data, "gas": 2_000_000})
    except Exception as e:
        pytest.fail(f"joinValidatorSet would revert (eth_call): {e!r}")
    receipt = _send_tx(w3, VALIDATOR_MANAGER, join_data, NEW_KEY, gas=2_000_000)
    assert receipt["status"] == 1, f"joinValidatorSet failed: {receipt}"

    join_requested_topic = Web3.keccak(text="ValidatorJoinRequested(address)")
    join_evt_seen = any(
        log["topics"] and bytes(log["topics"][0]) == bytes(join_requested_topic)
        and Web3.to_checksum_address("0x" + bytes(log["topics"][1])[-20:].hex()) == new_pool
        for log in receipt["logs"]
    )
    assert join_evt_seen, "ValidatorJoinRequested(new_pool) event not found"
    LOG.info("  joinValidatorSet succeeded, ValidatorJoinRequested emitted")

    status = int.from_bytes(
        _call(w3, VALIDATOR_MANAGER,
              SEL_GET_VALIDATOR_STATUS + encode(["address"], [new_pool]))[-1:],
        "big",
    )
    assert status == VALIDATOR_STATUS_PENDING_ACTIVE, (
        f"expected PENDING_ACTIVE ({VALIDATOR_STATUS_PENDING_ACTIVE}), got {status}"
    )
    LOG.info(f"  getValidatorStatus(new_pool) = PENDING_ACTIVE ✓")

    # Pre-transition validator set snapshot.
    pre_count = int.from_bytes(
        _call(w3, VALIDATOR_MANAGER, SEL_GET_ACTIVE_VALIDATOR_COUNT), "big"
    )
    LOG.info(f"  getActiveValidatorCount() pre-epoch = {pre_count} (genesis set)")
    assert pre_count == 4, f"expected 4 active validators pre-epoch, got {pre_count}"

    # ── Phase 6: wait for epoch advance, new validator becomes ACTIVE ─
    LOG.info("\n📌 Phase 6: wait for epoch boundary, PENDING_ACTIVE -> ACTIVE")

    ep0 = _current_epoch(w3)
    LOG.info(f"  epoch before wait: {ep0}")
    ep1 = await _wait_for_epoch_advance(w3, ep0, timeout_s=EPOCH_ADVANCE_TIMEOUT_S)
    LOG.info(f"  epoch after wait:  {ep1}")

    # Whale-node exception: new pool's 2e18 is 25% of the pre-epoch 8e18
    # staying-total — above the 20% votingPowerIncreaseLimitPct — but the
    # `addedPower > 0` guard in _computeNextEpochValidatorSet() carves out at
    # least one activation per epoch, so the lone joiner still lands ACTIVE.
    new_status = int.from_bytes(
        _call(w3, VALIDATOR_MANAGER,
              SEL_GET_VALIDATOR_STATUS + encode(["address"], [new_pool]))[-1:],
        "big",
    )
    assert new_status == VALIDATOR_STATUS_ACTIVE, (
        f"after epoch advance, expected new validator ACTIVE ({VALIDATOR_STATUS_ACTIVE}), "
        f"got status={new_status}. Possible causes: voting-power-increase-limit blocked "
        f"activation, or chain didn't actually process the epoch reconfiguration."
    )

    active_count = int.from_bytes(
        _call(w3, VALIDATOR_MANAGER, SEL_GET_ACTIVE_VALIDATOR_COUNT), "big"
    )
    assert active_count == 5, (
        f"expected 5 active validators after join, got {active_count}"
    )

    # Print the full 5-validator set for visibility.
    LOG.info(f"  getActiveValidatorCount() = {active_count}")
    for i, pool in enumerate(genesis_pools):
        s = int.from_bytes(
            _call(w3, VALIDATOR_MANAGER,
                  SEL_GET_VALIDATOR_STATUS + encode(["address"], [pool]))[-1:],
            "big",
        )
        LOG.info(f"    [{i}] {pool}  status={s} (genesis)")
    LOG.info(f"    [4] {new_pool}  status={new_status} (newly joined) ✓")

    # ── Phase 7: liveness sanity with a dead 5th validator ───────────
    # All 4 genesis nodes run real gravity_node processes; the 5th is a paper
    # validator only (no process). Online voting power = 4 × 2e18 = 8e18 of
    # the 10e18 total, i.e. 80% > 2/3 BFT quorum. The chain should keep
    # producing blocks despite the silent 5th signer. auto_evict_enabled=false
    # in genesis.toml so the dead validator won't be pruned mid-test.
    LOG.info("\n📌 Phase 7: chain still producing blocks with a dead 5th validator")

    h = w3.eth.block_number
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline and w3.eth.block_number < h + 5:
        await asyncio.sleep(2)
    assert w3.eth.block_number >= h + 5, (
        f"node1 did not advance 5 blocks (started at {h}, at {w3.eth.block_number})"
    )
    for node_id, node in cluster.nodes.items():
        bn = node.get_block_number()
        assert bn >= h + 3, f"{node_id} lagging after join: block {bn} vs start {h}"

    LOG.info("\n✅ Validator whitelist lifecycle complete — active set now 5 nodes")
