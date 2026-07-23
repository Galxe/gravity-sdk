"""Shared contract, governance, and price-feed helpers for oracle E2E suites."""

import asyncio
from contextlib import nullcontext
import json
import os
import shutil
import subprocess
import time
from pathlib import Path
from typing import Optional

import pytest
from eth_abi import encode
from eth_account import Account
from web3 import Web3

from gravity_e2e.utils.mock_binance_index import (
    BUCKET_START_MS,
    DECIMALS,
    INTERVAL_MS,
    MOCK_BINANCE_PORT,
    format_price,
    mock_binance_index_kline_server,
)

try:
    import tomllib
except ImportError:
    import tomli as tomllib


NATIVE_ORACLE_ADDRESS = Web3.to_checksum_address(
    "0x00000000000000000000000000000001625F4000"
)
ORACLE_TASK_CONFIG_ADDRESS = Web3.to_checksum_address(
    "0x00000000000000000000000000000001625F1009"
)
GOVERNANCE_ADDRESS = Web3.to_checksum_address(
    "0x00000000000000000000000000000001625F3000"
)
STAKING_ADDRESS = Web3.to_checksum_address(
    "0x00000000000000000000000000000001625F2000"
)

FAUCET_KEY = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
FAUCET_ADDR = Web3.to_checksum_address("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")

SOURCE_TYPE_PRICE_FEED = 3
TASK_PRICE_FEED = Web3.keccak(text="price_feed")
NVDA_FEED_ID = 1001
TSLA_FEED_ID = 1002
TARGET_DELIVERY_NONCE = 3
EXPECTED_NVDA_PRICE = 19_612_645_000
EXPECTED_TSLA_PRICE = 40_117_545_000
DEFAULT_LIVE_BINANCE_BASE_URL = "https://fapi.binance.com"
DEFAULT_BINANCE_GRACE_MS = 120_000
MODE_MOCK = "mock"
MODE_LIVE = "live"

DATA_RECORDED_TOPIC0 = Web3.keccak(
    text="DataRecorded(uint32,uint256,uint128,uint256)"
).hex()
MARKET_CREATED_TOPIC0 = Web3.keccak(text="MarketCreated(uint256,bytes32,uint256)")
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


def topic(value: int) -> str:
    return "0x" + value.to_bytes(32, "big").hex()


def topic_hex(value) -> str:
    result = value.hex()
    return result if result.startswith("0x") else f"0x{result}"


def as_bytes(data) -> bytes:
    if isinstance(data, bytes):
        return data
    if isinstance(data, str):
        return bytes.fromhex(data[2:] if data.startswith("0x") else data)
    return bytes(data)


def function_calldata(fn) -> bytes:
    return as_bytes(fn._encode_transaction_data())


def send_tx(
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


def send_contract_tx(w3: Web3, contract, fn, sender_key: str, gas: int = 1_000_000) -> dict:
    return send_tx(w3, contract.address, function_calldata(fn), sender_key, gas=gas)


def artifact_file(out_dir: Path, source_name: str, contract_name: str) -> Path:
    direct = out_dir / source_name / f"{contract_name}.json"
    if direct.exists():
        return direct
    raise FileNotFoundError(
        f"missing {source_name}/{contract_name}.json under contracts out dir"
    )


def contracts_repo_from_genesis(suite_dir: Path) -> Path | None:
    data = tomllib.loads((suite_dir / "genesis.toml").read_text())
    rel = data.get("dependencies", {}).get("genesis_contracts", {}).get("path")
    return (suite_dir / rel).resolve() if rel else None


def required_contract_sources(
    contracts_repo: Path, required: list[tuple[str, str]]
) -> list[Path]:
    source_names = dict.fromkeys(source for source, _ in required)
    source_roots = [contracts_repo / name for name in ("src", "test", "script")]
    paths = []

    for source_name in source_names:
        matches = [
            path
            for root in source_roots
            if root.is_dir()
            for path in root.rglob(source_name)
        ]
        if len(matches) != 1:
            relative_matches = [str(path.relative_to(contracts_repo)) for path in matches]
            raise FileNotFoundError(
                f"expected exactly one source file named {source_name}; "
                f"found {relative_matches}"
            )
        paths.append(matches[0].relative_to(contracts_repo))

    return paths


def ensure_contracts_out(
    suite_dir: Path,
    required: list[tuple[str, str]],
    error_label: str = "Oracle",
) -> Path:
    sdk_root = suite_dir.parents[2]
    contracts_repo = contracts_repo_from_genesis(suite_dir)
    if contracts_repo is not None and shutil.which("forge"):
        sources = required_contract_sources(contracts_repo, required)
        subprocess.run(
            ["forge", "build", *(str(source) for source in sources)],
            cwd=contracts_repo,
            check=True,
        )
        out_dir = contracts_repo / "out"
        for source, name in required:
            artifact_file(out_dir, source, name)
        return out_dir

    candidates = []
    candidates.extend(
        [
            sdk_root / "external" / "gravity_chain_core_contracts_local" / "out",
            sdk_root / "external" / "gravity_chain_core_contracts" / "out",
        ]
    )

    for out_dir in candidates:
        if not out_dir.exists():
            continue
        try:
            for source, name in required:
                artifact_file(out_dir, source, name)
            return out_dir
        except FileNotFoundError:
            pass

    raise FileNotFoundError(
        f"{error_label} contract artifacts not found; run forge build in contracts repo"
    )


def load_artifact(out_dir: Path, source_name: str, contract_name: str) -> dict:
    return json.loads(artifact_file(out_dir, source_name, contract_name).read_text())


def deploy_contract(
    w3: Web3,
    artifact: dict,
    sender_key: str,
    args=None,
    gas: int = 8_000_000,
):
    sender = Account.from_key(sender_key)
    factory = w3.eth.contract(abi=artifact["abi"], bytecode=_bytecode(artifact))
    tx = factory.constructor(*(args or [])).build_transaction(
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


def _bytecode(artifact: dict) -> str:
    bytecode = artifact["bytecode"]
    if isinstance(bytecode, dict):
        bytecode = bytecode["object"]
    return bytecode if bytecode.startswith("0x") else f"0x{bytecode}"


def _call(w3: Web3, to: str, data: bytes) -> bytes:
    return w3.eth.call({"to": to, "data": data})


def _decode_address(raw: bytes) -> str:
    return Web3.to_checksum_address("0x" + raw[-20:].hex())


def _ensure_faucet_executor(w3: Web3):
    owner = _decode_address(_call(w3, GOVERNANCE_ADDRESS, SEL_OWNER))
    assert owner == FAUCET_ADDR, f"Governance.owner expected faucet, got {owner}"
    is_executor = bool(
        int.from_bytes(
            _call(w3, GOVERNANCE_ADDRESS, SEL_IS_EXECUTOR + encode(["address"], [FAUCET_ADDR])),
            "big",
        )
    )
    if not is_executor:
        send_tx(
            w3,
            GOVERNANCE_ADDRESS,
            SEL_ADD_EXECUTOR + encode(["address"], [FAUCET_ADDR]),
            FAUCET_KEY,
        )


async def execute_governance_proposal(
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
    receipt = send_tx(w3, GOVERNANCE_ADDRESS, create_data, FAUCET_KEY, gas=gas)

    proposal_id = next(
        (
            int.from_bytes(log["topics"][1], "big")
            for log in receipt["logs"]
            if log["topics"] and bytes(log["topics"][0]) == bytes(PROPOSAL_CREATED_TOPIC0)
        ),
        None,
    )
    assert proposal_id is not None, "ProposalCreated event not found"

    vote_data = SEL_VOTE + encode(
        ["address", "uint64", "uint128", "bool"],
        [pool_addr, proposal_id, MAX_UINT128, True],
    )
    send_tx(w3, GOVERNANCE_ADDRESS, vote_data, FAUCET_KEY)
    vote_block = w3.eth.block_number

    await asyncio.sleep(VOTING_DURATION_SECS + 2)
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline and w3.eth.block_number < vote_block + 3:
        await asyncio.sleep(1)
    assert w3.eth.block_number >= vote_block + 3, "chain did not advance past vote block"

    resolve_data = SEL_RESOLVE + encode(["uint64"], [proposal_id])
    send_tx(w3, GOVERNANCE_ADDRESS, resolve_data, FAUCET_KEY)
    state = int.from_bytes(
        _call(w3, GOVERNANCE_ADDRESS, SEL_GET_PROPOSAL_STATE + encode(["uint64"], [proposal_id])),
        "big",
    )
    assert state == PROPOSAL_STATE_SUCCEEDED, f"proposal state is not SUCCEEDED: {state}"

    execute_data = SEL_EXECUTE + encode(
        ["uint64", "address[]", "bytes[]"], [proposal_id, targets, datas]
    )
    return send_tx(w3, GOVERNANCE_ADDRESS, execute_data, FAUCET_KEY, gas=gas)


def pool0_voted_by_faucet(w3: Web3) -> str:
    pool_addr = _decode_address(_call(w3, STAKING_ADDRESS, SEL_GET_POOL + encode(["uint256"], [0])))
    voter_addr = _decode_address(
        _call(w3, STAKING_ADDRESS, SEL_GET_POOL_VOTER + encode(["address"], [pool_addr]))
    )
    assert voter_addr == FAUCET_ADDR, f"pool[0].voter expected faucet, got {voter_addr}"
    voting_power = int.from_bytes(
        _call(
            w3,
            STAKING_ADDRESS,
            SEL_GET_POOL_VOTING_POWER_NOW + encode(["address"], [pool_addr]),
        ),
        "big",
    )
    assert voting_power >= STAKE_UNIT, f"pool[0] voting power too low: {voting_power}"
    return pool_addr


def market_id_from_receipt(receipt: dict) -> int:
    for log in receipt["logs"]:
        if log["topics"] and bytes(log["topics"][0]) == bytes(MARKET_CREATED_TOPIC0):
            return int.from_bytes(log["topics"][1], "big")
    raise AssertionError("MarketCreated event not found")


async def wait_for_chain_time(w3: Web3, target_ts: int, timeout: Optional[int] = None):
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


async def poll_price_recorded(w3: Web3, feed_id: int, timeout: int = 180):
    deadline = time.monotonic() + timeout
    filter_params = {
        "fromBlock": 0,
        "toBlock": "latest",
        "address": NATIVE_ORACLE_ADDRESS,
        "topics": [DATA_RECORDED_TOPIC0, topic(SOURCE_TYPE_PRICE_FEED), topic(feed_id)],
    }
    while time.monotonic() < deadline:
        logs = await asyncio.to_thread(w3.eth.get_logs, filter_params)
        if logs:
            return logs
        await asyncio.sleep(2)
    return []


async def wait_for_latest_nonce(native_oracle, feed_id: int, target_nonce: int, timeout: int = 240):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        nonce = native_oracle.functions.getLatestNonce(SOURCE_TYPE_PRICE_FEED, feed_id).call()
        if nonce >= target_nonce:
            return nonce
        await asyncio.sleep(2)
    raise TimeoutError(f"latest nonce for feedId={feed_id} did not reach {target_nonce}")


async def wait_for_price_round(resolver, feed_id: int, round_id: int, timeout: int = 120):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        stored = resolver.functions.priceRounds(feed_id, round_id).call()
        if stored[0]:
            return stored
        await asyncio.sleep(2)
    raise TimeoutError(f"priceRounds({feed_id}, {round_id}) was not stored")


def assert_price_round(latest, expected_price: int, round_id: int, resolved_at: int):
    assert latest[0] is True
    assert latest[1] == round_id
    assert latest[2] == resolved_at
    assert latest[3] == DECIMALS
    assert latest[4] == expected_price


def price_feed_mode() -> str:
    return os.environ.get("BINANCE_PRICE_FEED_MODE", MODE_MOCK).strip().lower()


def is_live_mode() -> bool:
    return price_feed_mode() == MODE_LIVE


def binance_base_url() -> str:
    if is_live_mode():
        return os.environ.get("BINANCE_PRICE_FEED_BASE_URL", DEFAULT_LIVE_BINANCE_BASE_URL)
    return f"http://127.0.0.1:{MOCK_BINANCE_PORT}"


def binance_grace_ms() -> int:
    return int(os.environ.get("BINANCE_PRICE_FEED_GRACE_MS", str(DEFAULT_BINANCE_GRACE_MS)))


def round_start_ms(first_bucket_start_ms: int, delivery_nonce: int) -> int:
    return first_bucket_start_ms + (delivery_nonce - 1) * INTERVAL_MS


def round_id(first_bucket_start_ms: int, delivery_nonce: int) -> int:
    return round_start_ms(first_bucket_start_ms, delivery_nonce) // INTERVAL_MS


def round_end_ms(first_bucket_start_ms: int, delivery_nonce: int) -> int:
    return round_start_ms(first_bucket_start_ms, delivery_nonce) + INTERVAL_MS - 1


def price_feed_uri(feed_id: int, pair: str, bucket_start_ms: int) -> str:
    return (
        f"gravity://3/{feed_id}/price_feed?"
        f"provider=binance_index_kline_v1&pair={pair}&interval=1m&"
        f"bucketStartMs={bucket_start_ms}&decimals=8&graceMs={binance_grace_ms()}"
    )


def server_context():
    if is_live_mode():
        return nullcontext(binance_base_url())
    if os.environ.get("BINANCE_PRICE_FEED_EXTERNAL_MOCK") == "1":
        return nullcontext(f"http://127.0.0.1:{MOCK_BINANCE_PORT}")
    return mock_binance_index_kline_server(MOCK_BINANCE_PORT)
