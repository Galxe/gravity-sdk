"""Hype/HIP-3-style price feed oracle e2e test."""

import asyncio
import json
import logging
import shutil
import subprocess
import time
from pathlib import Path
from typing import Optional

import pytest
from eth_abi import encode
from eth_account import Account
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster

try:
    import tomllib
except ImportError:
    import tomli as tomllib

LOG = logging.getLogger(__name__)

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
GOOGL_FEED_ID = 1002
ROUND_ID = 1
RESOLVED_AT = 2010
DECIMALS = 8
AGG_WEIGHTED_MEAN = 1
SOURCE_COUNT = 3
TOTAL_WEIGHT = 5
EXPECTED_NVDA_PRICE = 19_538_000_000
EXPECTED_GOOGL_PRICE = 35_364_400_000

NVDA_URI = (
    "gravity://3/1001/price_feed?"
    "round=1&resolvedAt=2010&decimals=8&aggregationMode=1&"
    "minSourceCount=3&minTotalWeight=5&maxStaleness=60&"
    "observations=hype-info:2000:19538000000:3,"
    "nasdaq-shadow:2000:19540000000:1,"
    "index-shadow:2000:19536000000:1"
)
GOOGL_URI = (
    "gravity://3/1002/price_feed?"
    "round=1&resolvedAt=2010&decimals=8&aggregationMode=1&"
    "minSourceCount=3&minTotalWeight=5&maxStaleness=60&"
    "observations=hype-info:2000:35364000000:3,"
    "nasdaq-shadow:2000:35370000000:1,"
    "index-shadow:2000:35360000000:1"
)

DATA_RECORDED_TOPIC0 = Web3.keccak(
    text="DataRecorded(uint32,uint256,uint128,uint256)"
).hex()
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


def _topic(value: int) -> str:
    return "0x" + value.to_bytes(32, "big").hex()


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
        ("MultiSourceOracleResolver.sol", "MultiSourceOracleResolver"),
        ("NativeOracle.sol", "NativeOracle"),
        ("OracleTaskConfig.sol", "OracleTaskConfig"),
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

    raise FileNotFoundError("Oracle contract artifacts not found; run forge build in contracts repo")


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

    vote_data = SEL_VOTE + encode(
        ["address", "uint64", "uint128", "bool"],
        [pool_addr, proposal_id, MAX_UINT128, True],
    )
    _send_tx(w3, GOVERNANCE_ADDRESS, vote_data, FAUCET_KEY)
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


async def _poll_data_recorded(w3: Web3, feed_id: int, timeout: int = 180):
    deadline = time.monotonic() + timeout
    filter_params = {
        "fromBlock": 0,
        "toBlock": "latest",
        "address": NATIVE_ORACLE_ADDRESS,
        "topics": [DATA_RECORDED_TOPIC0, _topic(SOURCE_TYPE_PRICE_FEED), _topic(feed_id)],
    }
    while time.monotonic() < deadline:
        logs = await asyncio.to_thread(w3.eth.get_logs, filter_params)
        if logs:
            return logs
        await asyncio.sleep(2)
    return []


async def _wait_for_latest_price(resolver, feed_id: int, timeout: int = 120):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        latest = resolver.functions.latestPrice(feed_id).call()
        if latest[0]:
            return latest
        await asyncio.sleep(2)
    raise TimeoutError(f"latestPrice({feed_id}) was not stored")


def _assert_price_round(latest, expected_price: int):
    assert latest[0] is True
    assert latest[1] == ROUND_ID
    assert latest[2] == RESOLVED_AT
    assert latest[3] == DECIMALS
    assert latest[4] == AGG_WEIGHTED_MEAN
    assert latest[5] == SOURCE_COUNT
    assert latest[6] == TOTAL_WEIGHT
    assert latest[7] == expected_price


@pytest.mark.asyncio
async def test_hype_price_feed_resolves_nvda_and_googl(cluster: Cluster):
    LOG.info("Verifying gravity node is live")
    assert await cluster.set_full_live(timeout=120), "Gravity node failed to become live"
    assert await cluster.check_block_increasing(timeout=60), "Gravity chain is not producing blocks"

    node = cluster.get_node("node1")
    assert node is not None, "node1 not found in cluster"
    w3 = node.w3
    assert w3 is not None and w3.is_connected(), "node1 web3 not connected"

    contracts_out = _ensure_contracts_out()
    resolver_artifact = _load_artifact(
        contracts_out,
        "MultiSourceOracleResolver.sol",
        "MultiSourceOracleResolver",
    )
    native_artifact = _load_artifact(contracts_out, "NativeOracle.sol", "NativeOracle")
    task_artifact = _load_artifact(contracts_out, "OracleTaskConfig.sol", "OracleTaskConfig")

    resolver = _deploy_contract(w3, resolver_artifact, FAUCET_KEY)
    native_oracle = w3.eth.contract(address=NATIVE_ORACLE_ADDRESS, abi=native_artifact["abi"])
    task_config = w3.eth.contract(address=ORACLE_TASK_CONFIG_ADDRESS, abi=task_artifact["abi"])
    pool_addr = _pool0_voted_by_faucet(w3)

    setup_datas = [
        _function_calldata(
            native_oracle.functions.setDefaultCallback(
                SOURCE_TYPE_PRICE_FEED,
                resolver.address,
            )
        ),
        _function_calldata(
            task_config.functions.setTask(
                SOURCE_TYPE_PRICE_FEED,
                NVDA_FEED_ID,
                TASK_PRICE_FEED,
                NVDA_URI.encode(),
            )
        ),
        _function_calldata(
            task_config.functions.setTask(
                SOURCE_TYPE_PRICE_FEED,
                GOOGL_FEED_ID,
                TASK_PRICE_FEED,
                GOOGL_URI.encode(),
            )
        ),
    ]
    await _execute_governance_proposal(
        w3,
        pool_addr,
        [NATIVE_ORACLE_ADDRESS, ORACLE_TASK_CONFIG_ADDRESS, ORACLE_TASK_CONFIG_ADDRESS],
        setup_datas,
        "hype-price-feed-e2e-setup",
    )

    for feed_id, expected_price in [
        (NVDA_FEED_ID, EXPECTED_NVDA_PRICE),
        (GOOGL_FEED_ID, EXPECTED_GOOGL_PRICE),
    ]:
        logs = await _poll_data_recorded(w3, feed_id, timeout=180)
        assert logs, f"No sourceType=3 DataRecorded event observed for feedId={feed_id}"
        assert native_oracle.functions.getLatestNonce(SOURCE_TYPE_PRICE_FEED, feed_id).call() == ROUND_ID

        latest = await _wait_for_latest_price(resolver, feed_id, timeout=60)
        _assert_price_round(latest, expected_price)
        stored = resolver.functions.priceRounds(feed_id, ROUND_ID).call()
        _assert_price_round(stored, expected_price)
        LOG.info(
            "Hype price feed resolved: feedId=%s roundId=%s price=%s",
            feed_id,
            ROUND_ID,
            expected_price,
        )
