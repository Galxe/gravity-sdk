"""Contract and governance helpers shared by oracle E2E suites."""

import asyncio
import json
import shutil
import subprocess
import time
from pathlib import Path
from typing import Optional

from eth_abi import encode
from eth_account import Account
from web3 import Web3

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
FAUCET_ADDRESS = Web3.to_checksum_address(
    "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
)

SOURCE_TYPE_PRICE_FEED = 3
TASK_PRICE_FEED = Web3.keccak(text="price_feed")

_PROPOSAL_CREATED_TOPIC = Web3.keccak(
    text="ProposalCreated(uint64,address,address,bytes32,string)"
)
_SEL_OWNER = Web3.keccak(text="owner()")[:4]
_SEL_ADD_EXECUTOR = Web3.keccak(text="addExecutor(address)")[:4]
_SEL_IS_EXECUTOR = Web3.keccak(text="isExecutor(address)")[:4]
_SEL_GET_POOL = Web3.keccak(text="getPool(uint256)")[:4]
_SEL_GET_POOL_VOTER = Web3.keccak(text="getPoolVoter(address)")[:4]
_SEL_GET_POOL_VOTING_POWER = Web3.keccak(text="getPoolVotingPowerNow(address)")[:4]
_SEL_CREATE_PROPOSAL = Web3.keccak(
    text="createProposal(address,address[],bytes[],string)"
)[:4]
_SEL_VOTE = Web3.keccak(text="vote(address,uint64,uint128,bool)")[:4]
_SEL_RESOLVE = Web3.keccak(text="resolve(uint64)")[:4]
_SEL_EXECUTE = Web3.keccak(text="execute(uint64,address[],bytes[])")[:4]
_SEL_GET_PROPOSAL_STATE = Web3.keccak(text="getProposalState(uint64)")[:4]

_MAX_UINT128 = (1 << 128) - 1
_PROPOSAL_STATE_SUCCEEDED = 1
_VOTING_DURATION_SECS = 5


def function_calldata(fn) -> bytes:
    encoded = fn._encode_transaction_data()
    return bytes.fromhex(encoded[2:]) if isinstance(encoded, str) else bytes(encoded)


def _send_tx(
    w3: Web3,
    to: Optional[str],
    data,
    *,
    gas: int = 1_000_000,
) -> dict:
    sender = Account.from_key(FAUCET_KEY)
    tx = {
        "data": data,
        "gas": gas,
        "gasPrice": w3.eth.gas_price,
        "nonce": w3.eth.get_transaction_count(sender.address),
        "chainId": w3.eth.chain_id,
    }
    if to is not None:
        tx["to"] = to
    signed = sender.sign_transaction(tx)
    tx_hash = w3.eth.send_raw_transaction(signed.raw_transaction)
    receipt = w3.eth.wait_for_transaction_receipt(tx_hash, timeout=120)
    assert receipt["status"] == 1, f"transaction failed: {receipt}"
    return receipt


def _artifact_file(out_dir: Path, source_name: str, contract_name: str) -> Path:
    path = out_dir / source_name / f"{contract_name}.json"
    if not path.exists():
        raise FileNotFoundError(f"missing {source_name}/{contract_name}.json under {out_dir}")
    return path


def _contracts_repo(suite_dir: Path) -> Path:
    config = tomllib.loads((suite_dir / "genesis.toml").read_text())
    dependency = config.get("dependencies", {}).get("genesis_contracts", {})
    sdk_root = suite_dir.parents[2]
    if dependency.get("path"):
        generated = sdk_root / "external" / "gravity_chain_core_contracts_local"
        return generated if generated.is_dir() else (suite_dir / dependency["path"]).resolve()
    return sdk_root / "external" / "gravity_chain_core_contracts"


def ensure_contract_artifacts(
    suite_dir: Path,
    required: list[tuple[str, str]],
) -> Path:
    contracts_repo = _contracts_repo(suite_dir)
    if not contracts_repo.is_dir():
        raise FileNotFoundError(f"contracts checkout not found under SDK external dependencies")
    if not shutil.which("forge"):
        raise FileNotFoundError("forge is required to build oracle contract artifacts")

    sources = []
    for source_name, _ in required:
        matches = [
            path
            for root_name in ("src", "test", "script")
            for path in (contracts_repo / root_name).rglob(source_name)
            if (contracts_repo / root_name).is_dir()
        ]
        assert len(matches) == 1, f"expected one {source_name}, found {matches}"
        sources.append(str(matches[0].relative_to(contracts_repo)))

    subprocess.run(["forge", "build", *dict.fromkeys(sources)], cwd=contracts_repo, check=True)
    out_dir = contracts_repo / "out"
    for source_name, contract_name in required:
        _artifact_file(out_dir, source_name, contract_name)
    return out_dir


def load_artifact(out_dir: Path, source_name: str, contract_name: str) -> dict:
    return json.loads(_artifact_file(out_dir, source_name, contract_name).read_text())


def deploy_contract(w3: Web3, artifact: dict):
    bytecode = artifact["bytecode"]
    if isinstance(bytecode, dict):
        bytecode = bytecode["object"]
    if not bytecode.startswith("0x"):
        bytecode = f"0x{bytecode}"

    sender = Account.from_key(FAUCET_KEY)
    factory = w3.eth.contract(abi=artifact["abi"], bytecode=bytecode)
    tx = factory.constructor().build_transaction({
        "from": sender.address,
        "gas": 8_000_000,
        "gasPrice": w3.eth.gas_price,
        "nonce": w3.eth.get_transaction_count(sender.address),
        "chainId": w3.eth.chain_id,
    })
    signed = sender.sign_transaction(tx)
    tx_hash = w3.eth.send_raw_transaction(signed.raw_transaction)
    receipt = w3.eth.wait_for_transaction_receipt(tx_hash, timeout=120)
    assert receipt["status"] == 1, f"contract deployment failed: {receipt}"
    return w3.eth.contract(address=receipt["contractAddress"], abi=artifact["abi"])


def _call(w3: Web3, to: str, data: bytes) -> bytes:
    return w3.eth.call({"to": to, "data": data})


def _decode_address(raw: bytes) -> str:
    return Web3.to_checksum_address("0x" + raw[-20:].hex())


def _ensure_faucet_executor(w3: Web3) -> None:
    owner = _decode_address(_call(w3, GOVERNANCE_ADDRESS, _SEL_OWNER))
    assert owner == FAUCET_ADDRESS, f"Governance.owner expected faucet, got {owner}"
    is_executor = bool(int.from_bytes(
        _call(
            w3,
            GOVERNANCE_ADDRESS,
            _SEL_IS_EXECUTOR + encode(["address"], [FAUCET_ADDRESS]),
        ),
        "big",
    ))
    if not is_executor:
        _send_tx(
            w3,
            GOVERNANCE_ADDRESS,
            _SEL_ADD_EXECUTOR + encode(["address"], [FAUCET_ADDRESS]),
        )


def faucet_voting_pool(w3: Web3) -> str:
    pool = _decode_address(
        _call(w3, STAKING_ADDRESS, _SEL_GET_POOL + encode(["uint256"], [0]))
    )
    voter = _decode_address(
        _call(
            w3,
            STAKING_ADDRESS,
            _SEL_GET_POOL_VOTER + encode(["address"], [pool]),
        )
    )
    assert voter == FAUCET_ADDRESS, f"pool[0].voter expected faucet, got {voter}"
    voting_power = int.from_bytes(
        _call(
            w3,
            STAKING_ADDRESS,
            _SEL_GET_POOL_VOTING_POWER + encode(["address"], [pool]),
        ),
        "big",
    )
    assert voting_power >= 10**18, f"pool[0] voting power too low: {voting_power}"
    return pool


async def execute_governance_proposal(
    w3: Web3,
    pool: str,
    targets: list[str],
    datas: list[bytes],
    description: str,
) -> dict:
    assert len(targets) == len(datas) and targets
    _ensure_faucet_executor(w3)
    create_data = _SEL_CREATE_PROPOSAL + encode(
        ["address", "address[]", "bytes[]", "string"],
        [pool, targets, datas, description],
    )
    w3.eth.call({
        "from": FAUCET_ADDRESS,
        "to": GOVERNANCE_ADDRESS,
        "data": create_data,
        "gas": 5_000_000,
    })
    receipt = _send_tx(w3, GOVERNANCE_ADDRESS, create_data, gas=5_000_000)
    proposal_id = next(
        (
            int.from_bytes(log["topics"][1], "big")
            for log in receipt["logs"]
            if log["topics"] and bytes(log["topics"][0]) == bytes(_PROPOSAL_CREATED_TOPIC)
        ),
        None,
    )
    assert proposal_id is not None, "ProposalCreated event not found"

    vote_data = _SEL_VOTE + encode(
        ["address", "uint64", "uint128", "bool"],
        [pool, proposal_id, _MAX_UINT128, True],
    )
    _send_tx(w3, GOVERNANCE_ADDRESS, vote_data)
    vote_block = w3.eth.block_number
    await asyncio.sleep(_VOTING_DURATION_SECS + 2)
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline and w3.eth.block_number < vote_block + 3:
        await asyncio.sleep(1)
    assert w3.eth.block_number >= vote_block + 3, "chain did not advance past vote block"

    _send_tx(
        w3,
        GOVERNANCE_ADDRESS,
        _SEL_RESOLVE + encode(["uint64"], [proposal_id]),
    )
    state = int.from_bytes(
        _call(
            w3,
            GOVERNANCE_ADDRESS,
            _SEL_GET_PROPOSAL_STATE + encode(["uint64"], [proposal_id]),
        ),
        "big",
    )
    assert state == _PROPOSAL_STATE_SUCCEEDED, f"proposal state is {state}"

    execute_data = _SEL_EXECUTE + encode(
        ["uint64", "address[]", "bytes[]"],
        [proposal_id, targets, datas],
    )
    return _send_tx(w3, GOVERNANCE_ADDRESS, execute_data, gas=5_000_000)


async def wait_for_latest_price(
    native_oracle,
    resolver,
    feed_id: int,
    target_nonce: int,
    *,
    timeout: int = 300,
):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        progress = tuple(
            native_oracle.functions.getSourceProgress(
                SOURCE_TYPE_PRICE_FEED, feed_id
            ).call()
        )
        latest = tuple(resolver.functions.latestPrice(feed_id).call())
        if progress[0] >= target_nonce and latest[0] > 0 and latest[1] == progress[1]:
            return progress, latest
        await asyncio.sleep(2)
    raise TimeoutError(f"latestPrice({feed_id}) did not reach delivery nonce {target_nonce}")
