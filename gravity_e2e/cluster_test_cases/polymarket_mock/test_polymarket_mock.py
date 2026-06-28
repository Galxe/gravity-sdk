"""Polymarket match-market oracle mock e2e test."""

import asyncio
import json
import logging
import os
import secrets
import shutil
import subprocess
import time
import urllib.request
from pathlib import Path
from typing import Optional

import pytest
from eth_abi import encode
from eth_account import Account
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils.mock_polymarket_polygon import MATCH_MARKET_ID

try:
    import tomllib
except ImportError:
    import tomli as tomllib

LOG = logging.getLogger(__name__)

NATIVE_ORACLE_ADDRESS = Web3.to_checksum_address(
    "0x00000000000000000000000000000001625F4000"
)
GOVERNANCE_ADDRESS = Web3.to_checksum_address(
    "0x00000000000000000000000000000001625F3000"
)
STAKING_ADDRESS = Web3.to_checksum_address(
    "0x00000000000000000000000000000001625F2000"
)

FAUCET_KEY = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
FAUCET_ADDR = Web3.to_checksum_address("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")

SOURCE_TYPE_POLYMARKET_SETTLEMENT = 6
POLYGON_CHAIN_ID = 137
MATCH_OUTCOME_COUNT = 3
SETTLEMENT_KIND_CTF_CONDITION_RESOLUTION = 1

DATA_RECORDED_TOPIC0 = Web3.keccak(
    text="DataRecorded(uint32,uint256,uint128,uint256)"
).hex()
MARKET_CREATED_TOPIC0 = Web3.keccak(
    text="MarketCreated(uint256,bytes32,uint256)"
)
PROPOSAL_CREATED_TOPIC0 = Web3.keccak(
    text="ProposalCreated(uint64,address,address,bytes32,string)"
)

SEL_OWNER = Web3.keccak(text="owner()")[:4]
SEL_ADD_EXECUTOR = Web3.keccak(text="addExecutor(address)")[:4]
SEL_IS_EXECUTOR = Web3.keccak(text="isExecutor(address)")[:4]
SEL_GET_POOL = Web3.keccak(text="getPool(uint256)")[:4]
SEL_GET_POOL_VOTER = Web3.keccak(text="getPoolVoter(address)")[:4]
SEL_GET_POOL_VOTING_POWER_NOW = Web3.keccak(text="getPoolVotingPowerNow(address)")[:4]
SEL_CREATE_PROPOSAL = Web3.keccak(text="createProposal(address,address[],bytes[],string)")[:4]
SEL_VOTE = Web3.keccak(text="vote(address,uint64,uint128,bool)")[:4]
SEL_RESOLVE = Web3.keccak(text="resolve(uint64)")[:4]
SEL_EXECUTE = Web3.keccak(text="execute(uint64,address[],bytes[])")[:4]
SEL_GET_PROPOSAL_STATE = Web3.keccak(text="getProposalState(uint64)")[:4]

MAX_UINT128 = (1 << 128) - 1
PROPOSAL_STATE_SUCCEEDED = 1
VOTING_DURATION_SECS = 5

STAKE_UNIT = 10**18
USER_STARTING_BALANCE = 1_000 * STAKE_UNIT
BET_AMOUNTS = [100 * STAKE_UNIT, 200 * STAKE_UNIT, 300 * STAKE_UNIT]
TOTAL_POOL = sum(BET_AMOUNTS)


def _topic(value: int) -> str:
    return "0x" + value.to_bytes(32, "big").hex()


def _topic_hex(topic) -> str:
    value = topic.hex()
    return value if value.startswith("0x") else f"0x{value}"


def _decode_address(raw: bytes) -> str:
    return Web3.to_checksum_address("0x" + raw[-20:].hex())


def _bool_from_return(raw: bytes) -> bool:
    return bool(int.from_bytes(raw, "big"))


def _call(w3: Web3, to: str, data: bytes) -> bytes:
    return w3.eth.call({"to": to, "data": data})


def _as_bytes(data) -> bytes:
    if isinstance(data, bytes):
        return data
    if isinstance(data, str):
        return bytes.fromhex(data[2:] if data.startswith("0x") else data)
    return bytes(data)


def _function_calldata(fn) -> bytes:
    return _as_bytes(fn._encode_transaction_data())


def _send_tx(
    w3: Web3,
    to: Optional[str],
    data,
    sender_key: str,
    gas: int = 1_000_000,
    value: int = 0,
) -> dict:
    sender = Account.from_key(sender_key)
    tx = {
        "data": data,
        "gas": gas,
        "gasPrice": w3.eth.gas_price,
        "nonce": w3.eth.get_transaction_count(sender.address),
        "chainId": w3.eth.chain_id,
        "value": value,
    }
    if to is not None:
        tx["to"] = to
    signed = sender.sign_transaction(tx)
    tx_hash = w3.eth.send_raw_transaction(signed.raw_transaction)
    receipt = w3.eth.wait_for_transaction_receipt(tx_hash, timeout=90)
    assert receipt["status"] == 1, f"transaction failed: {receipt}"
    return receipt


def _send_contract_tx(w3: Web3, contract, fn, sender_key: str, gas: int = 1_000_000) -> dict:
    return _send_tx(w3, contract.address, _function_calldata(fn), sender_key, gas=gas)


def _contracts_repo_from_genesis() -> Path | None:
    suite_dir = Path(__file__).resolve().parent
    genesis_path = suite_dir / "genesis.toml"
    data = tomllib.loads(genesis_path.read_text())
    dep = data.get("dependencies", {}).get("genesis_contracts", {})
    rel = dep.get("path")
    if not rel:
        return None
    return (suite_dir / rel).resolve()


def _artifact_file(out_dir: Path, source_name: str, contract_name: str) -> Path:
    direct = out_dir / source_name / f"{contract_name}.json"
    if direct.exists():
        return direct
    matches = sorted(out_dir.glob(f"*/{contract_name}.json"))
    if matches:
        return matches[0]
    raise FileNotFoundError(f"missing {contract_name}.json under contracts out dir")


def _ensure_contracts_out() -> Path:
    sdk_root = Path(__file__).resolve().parents[3]
    contracts_repo = _contracts_repo_from_genesis()
    candidates = []
    if contracts_repo is not None:
        candidates.append(contracts_repo / "out")
    candidates.extend(
        [
            sdk_root / "external" / "gravity_chain_core_contracts_local" / "out",
            sdk_root / "external" / "gravity_chain_core_contracts" / "out",
        ]
    )

    required = [
        ("PolymarketSettlementResolver.sol", "PolymarketSettlementResolver"),
        ("PolymarketMatchMarket.sol", "PolymarketMatchMarket"),
        ("MockGToken.sol", "MockGToken"),
        ("NativeOracle.sol", "NativeOracle"),
    ]
    for out_dir in candidates:
        if not out_dir.exists():
            continue
        try:
            for source, name in required:
                _artifact_file(out_dir, source, name)
            return out_dir
        except FileNotFoundError:
            pass

    if contracts_repo is not None and shutil.which("forge"):
        subprocess.run(["forge", "build"], cwd=contracts_repo, check=True)
        out_dir = contracts_repo / "out"
        for source, name in required:
            _artifact_file(out_dir, source, name)
        return out_dir

    raise FileNotFoundError("Polymarket contract artifacts not found; run forge build in contracts repo")


def _load_artifact(out_dir: Path, source_name: str, contract_name: str) -> dict:
    return json.loads(_artifact_file(out_dir, source_name, contract_name).read_text())


def _bytecode(artifact: dict) -> str:
    bytecode = artifact["bytecode"]
    if isinstance(bytecode, dict):
        bytecode = bytecode["object"]
    return bytecode if bytecode.startswith("0x") else f"0x{bytecode}"


def _deploy_contract(w3: Web3, artifact: dict, sender_key: str, args=None, gas: int = 8_000_000):
    sender = Account.from_key(sender_key)
    factory = w3.eth.contract(abi=artifact["abi"], bytecode=_bytecode(artifact))
    constructor = factory.constructor(*(args or []))
    tx = constructor.build_transaction(
        {
            "from": sender.address,
            "gas": gas,
            "gasPrice": w3.eth.gas_price,
            "nonce": w3.eth.get_transaction_count(sender.address),
            "chainId": w3.eth.chain_id,
        }
    )
    signed = sender.sign_transaction(tx)
    tx_hash = w3.eth.send_raw_transaction(signed.raw_transaction)
    receipt = w3.eth.wait_for_transaction_receipt(tx_hash, timeout=120)
    assert receipt["status"] == 1, f"deploy failed: {receipt}"
    return w3.eth.contract(address=receipt["contractAddress"], abi=artifact["abi"])


def _mock_set_winning_slot(rpc_url: str, winning_slot: int) -> dict:
    payload = json.dumps(
        {
            "jsonrpc": "2.0",
            "method": "mock_setWinningSlot",
            "params": [winning_slot],
            "id": 1,
        }
    ).encode()
    req = urllib.request.Request(
        rpc_url,
        data=payload,
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=5) as resp:
        body = json.loads(resp.read())
    if "error" in body:
        raise RuntimeError(f"mock_setWinningSlot failed: {body['error']}")
    return body["result"]


async def _poll_data_recorded(w3: Web3, timeout: int = 120):
    deadline = time.time() + timeout
    filter_params = {
        "fromBlock": 0,
        "toBlock": "latest",
        "address": NATIVE_ORACLE_ADDRESS,
        "topics": [DATA_RECORDED_TOPIC0, _topic(SOURCE_TYPE_POLYMARKET_SETTLEMENT), _topic(MATCH_MARKET_ID)],
    }
    while time.time() < deadline:
        logs = await asyncio.to_thread(w3.eth.get_logs, filter_params)
        if logs:
            return logs
        await asyncio.sleep(2)
    return []


async def _wait_for_chain_time(w3: Web3, target_ts: int, timeout: Optional[int] = None):
    latest = w3.eth.get_block("latest")
    if timeout is None:
        timeout = max(30, target_ts - latest["timestamp"] + 30)
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        latest = w3.eth.get_block("latest")
        if latest["timestamp"] >= target_ts:
            return latest
        await asyncio.sleep(1)
    raise TimeoutError(f"chain timestamp did not reach {target_ts}")


async def _wait_for_resolver_settlement(resolver, mirror_id: int, condition_id: bytes, timeout: int = 120):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        settlement = resolver.functions.getSettlement(mirror_id, condition_id).call()
        if settlement[0]:
            return settlement
        await asyncio.sleep(2)
    raise TimeoutError("resolver settlement was not stored")


def _ensure_faucet_executor(w3: Web3):
    owner = _decode_address(_call(w3, GOVERNANCE_ADDRESS, SEL_OWNER))
    assert owner == FAUCET_ADDR, f"Governance.owner expected faucet, got {owner}"
    is_exec = _bool_from_return(
        _call(w3, GOVERNANCE_ADDRESS, SEL_IS_EXECUTOR + encode(["address"], [FAUCET_ADDR]))
    )
    if not is_exec:
        _send_tx(
            w3,
            GOVERNANCE_ADDRESS,
            SEL_ADD_EXECUTOR + encode(["address"], [FAUCET_ADDR]),
            FAUCET_KEY,
        )


async def _execute_governance_proposal(
    w3: Web3,
    pool_addr: str,
    targets: list[str],
    datas: list[bytes],
    description: str,
    gas: int = 5_000_000,
) -> dict:
    _ensure_faucet_executor(w3)
    create_data = SEL_CREATE_PROPOSAL + encode(
        ["address", "address[]", "bytes[]", "string"],
        [pool_addr, targets, datas, description],
    )
    try:
        w3.eth.call({"from": FAUCET_ADDR, "to": GOVERNANCE_ADDRESS, "data": create_data, "gas": gas})
    except Exception as exc:
        pytest.fail(f"createProposal would revert: {exc!r}")
    receipt = _send_tx(w3, GOVERNANCE_ADDRESS, create_data, FAUCET_KEY, gas=gas)

    proposal_id = None
    for log in receipt["logs"]:
        if log["topics"] and bytes(log["topics"][0]) == bytes(PROPOSAL_CREATED_TOPIC0):
            proposal_id = int.from_bytes(log["topics"][1], "big")
            break
    assert proposal_id is not None, "ProposalCreated event not found"

    vote_data = SEL_VOTE + encode(["address", "uint64", "uint128", "bool"], [pool_addr, proposal_id, MAX_UINT128, True])
    receipt = _send_tx(w3, GOVERNANCE_ADDRESS, vote_data, FAUCET_KEY)
    vote_block = w3.eth.block_number

    await asyncio.sleep(VOTING_DURATION_SECS + 2)
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline and w3.eth.block_number < vote_block + 3:
        await asyncio.sleep(1)
    assert w3.eth.block_number >= vote_block + 3, "chain did not advance past vote block"

    resolve_data = SEL_RESOLVE + encode(["uint64"], [proposal_id])
    _send_tx(w3, GOVERNANCE_ADDRESS, resolve_data, FAUCET_KEY)
    state = int.from_bytes(
        _call(w3, GOVERNANCE_ADDRESS, SEL_GET_PROPOSAL_STATE + encode(["uint64"], [proposal_id])),
        "big",
    )
    assert state == PROPOSAL_STATE_SUCCEEDED, f"proposal state is not SUCCEEDED: {state}"

    exec_data = SEL_EXECUTE + encode(["uint64", "address[]", "bytes[]"], [proposal_id, targets, datas])
    return _send_tx(w3, GOVERNANCE_ADDRESS, exec_data, FAUCET_KEY, gas=gas)


def _pool0_voted_by_faucet(w3: Web3) -> str:
    pool_raw = _call(w3, STAKING_ADDRESS, SEL_GET_POOL + encode(["uint256"], [0]))
    pool_addr = _decode_address(pool_raw)
    voter_raw = _call(w3, STAKING_ADDRESS, SEL_GET_POOL_VOTER + encode(["address"], [pool_addr]))
    voter_addr = _decode_address(voter_raw)
    assert voter_addr == FAUCET_ADDR, f"pool[0].voter expected faucet, got {voter_addr}"
    voting_power = int.from_bytes(
        _call(w3, STAKING_ADDRESS, SEL_GET_POOL_VOTING_POWER_NOW + encode(["address"], [pool_addr])),
        "big",
    )
    assert voting_power >= STAKE_UNIT, f"pool[0] voting power too low: {voting_power}"
    return pool_addr


def _market_id_from_receipt(receipt: dict) -> int:
    for log in receipt["logs"]:
        if log["topics"] and bytes(log["topics"][0]) == bytes(MARKET_CREATED_TOPIC0):
            return int.from_bytes(log["topics"][1], "big")
    raise AssertionError("MarketCreated event not found")


@pytest.mark.asyncio
async def test_polymarket_match_market_mock_resolves_random_score(cluster: Cluster):
    metadata_path = Path(__file__).with_name("mock_polymarket_metadata.json")
    metadata = json.loads(metadata_path.read_text())

    LOG.info("Verifying gravity node is live")
    assert await cluster.set_full_live(timeout=120), "Gravity node failed to become live"
    assert await cluster.check_block_increasing(timeout=60), "Gravity chain is not producing blocks"

    node = cluster.get_node("node1")
    assert node is not None, "node1 not found in cluster"
    w3 = node.w3
    assert w3 is not None and w3.is_connected(), "node1 web3 not connected"

    contracts_out = _ensure_contracts_out()
    resolver_artifact = _load_artifact(contracts_out, "PolymarketSettlementResolver.sol", "PolymarketSettlementResolver")
    market_artifact = _load_artifact(contracts_out, "PolymarketMatchMarket.sol", "PolymarketMatchMarket")
    token_artifact = _load_artifact(contracts_out, "MockGToken.sol", "MockGToken")
    native_artifact = _load_artifact(contracts_out, "NativeOracle.sol", "NativeOracle")

    resolver = _deploy_contract(w3, resolver_artifact, FAUCET_KEY)
    market = _deploy_contract(w3, market_artifact, FAUCET_KEY)
    token = _deploy_contract(w3, token_artifact, FAUCET_KEY)
    native_oracle = w3.eth.contract(address=NATIVE_ORACLE_ADDRESS, abi=native_artifact["abi"])

    condition_id = bytes.fromhex(metadata["condition_id"][2:])
    ctf = Web3.to_checksum_address(metadata["ctf"])
    pool_addr = _pool0_voted_by_faucet(w3)

    now_ts = w3.eth.get_block("latest")["timestamp"]
    closes_at = now_ts + 120
    oracle_deadline = now_ts + 300
    settlement_ref = (
        SOURCE_TYPE_POLYMARKET_SETTLEMENT,
        MATCH_MARKET_ID,
        condition_id,
        resolver.address,
        ctf,
        POLYGON_CHAIN_ID,
        MATCH_OUTCOME_COUNT,
        [0, 1, 2],
        0,
    )
    create_params = (
        Web3.keccak(text="Portugal vs Colombia random e2e match market"),
        now_ts,
        closes_at,
        oracle_deadline,
        token.address,
        settlement_ref,
    )

    setup_datas = [
        _function_calldata(
            native_oracle.functions.setCallback(
                SOURCE_TYPE_POLYMARKET_SETTLEMENT,
                MATCH_MARKET_ID,
                resolver.address,
            )
        ),
        _function_calldata(
            resolver.functions.registerMirror(
                MATCH_MARKET_ID,
                POLYGON_CHAIN_ID,
                ctf,
                condition_id,
                MATCH_OUTCOME_COUNT,
            )
        ),
        _function_calldata(market.functions.createMarket(create_params)),
    ]
    receipt = await _execute_governance_proposal(
        w3,
        pool_addr,
        [NATIVE_ORACLE_ADDRESS, resolver.address, market.address],
        setup_datas,
        "polymarket-match-market-e2e-setup",
    )
    market_id = _market_id_from_receipt(receipt)

    bettors = [Account.create(f"polymarket-bettor-{i}") for i in range(MATCH_OUTCOME_COUNT)]
    for account in bettors:
        _send_tx(w3, account.address, b"", FAUCET_KEY, gas=21_000, value=STAKE_UNIT)
        _send_contract_tx(w3, token, token.functions.mint(account.address, USER_STARTING_BALANCE), FAUCET_KEY)
        _send_contract_tx(w3, token, token.functions.approve(market.address, USER_STARTING_BALANCE), account.key)

    for outcome, (account, amount) in enumerate(zip(bettors, BET_AMOUNTS)):
        _send_contract_tx(w3, market, market.functions.placeBet(market_id, outcome, amount), account.key, gas=1_500_000)

    stored = market.functions.getMarket(market_id).call()
    assert stored[8] == TOTAL_POOL, f"market totalPool mismatch: {stored[8]}"

    await _wait_for_chain_time(w3, closes_at)
    _send_contract_tx(w3, market, market.functions.lockMarket(market_id), FAUCET_KEY)

    forced_slot = os.environ.get("POLYMARKET_MOCK_WINNING_SLOT")
    winning_slot = int(forced_slot) if forced_slot is not None else secrets.randbelow(MATCH_OUTCOME_COUNT)
    assert 0 <= winning_slot < MATCH_OUTCOME_COUNT, f"invalid winning slot: {winning_slot}"
    release = _mock_set_winning_slot(metadata["rpc_url"], winning_slot)
    metadata.update(release)
    metadata_path.write_text(json.dumps(metadata, indent=2))
    LOG.info("Released mock Polymarket settlement: winning_slot=%s payout=%s", winning_slot, release["payout_numerators"])

    logs = await _poll_data_recorded(w3, timeout=180)
    assert logs, "No Polymarket sourceType=6 DataRecorded event observed"
    first = logs[0]
    assert _topic_hex(first["topics"][1]) == _topic(SOURCE_TYPE_POLYMARKET_SETTLEMENT)
    assert _topic_hex(first["topics"][2]) == _topic(MATCH_MARKET_ID)

    settlement = await _wait_for_resolver_settlement(resolver, MATCH_MARKET_ID, condition_id, timeout=60)
    assert settlement[0] is True
    assert settlement[2] == POLYGON_CHAIN_ID
    assert Web3.to_checksum_address(settlement[3]) == ctf
    assert settlement[6] == MATCH_OUTCOME_COUNT
    assert settlement[9] == SETTLEMENT_KIND_CTF_CONDITION_RESOLUTION
    assert resolver.functions.getPayoutNumerators(MATCH_MARKET_ID, condition_id).call() == release["payout_numerators"]

    _send_contract_tx(w3, market, market.functions.settleMarket(market_id), FAUCET_KEY, gas=2_000_000)
    final_market = market.functions.getMarket(market_id).call()
    assert final_market[5] == 2, f"market did not settle: status={final_market[5]}"
    assert final_market[7] == winning_slot, f"winning outcome mismatch: {final_market[7]} vs {winning_slot}"

    winner = bettors[winning_slot]
    assert market.functions.claimable(market_id, winner.address).call() == TOTAL_POOL
    balance_before = token.functions.balanceOf(winner.address).call()
    _send_contract_tx(w3, market, market.functions.claim(market_id), winner.key, gas=1_500_000)
    balance_after = token.functions.balanceOf(winner.address).call()
    assert balance_after - balance_before == TOTAL_POOL
    assert market.functions.claimable(market_id, winner.address).call() == 0

    for outcome, account in enumerate(bettors):
        if outcome == winning_slot:
            continue
        assert market.functions.claimable(market_id, account.address).call() == 0

    LOG.info(
        "Polymarket match market resolved and claimed: marketId=%s winningSlot=%s totalPool=%s",
        market_id,
        winning_slot,
        TOTAL_POOL,
    )
