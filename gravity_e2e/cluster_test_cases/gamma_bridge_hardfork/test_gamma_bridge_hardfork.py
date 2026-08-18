"""Prove sourceType=0 bridge continuity across the Gamma hardfork."""

import asyncio
import json
import logging
import os
from pathlib import Path
import time

from hexbytes import HexBytes
import pytest
import requests
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils.bridge_utils import (
    BRIDGE_RECEIVER_ABI,
    GBRIDGE_RECEIVER_ADDRESS,
    NATIVE_MINTED_TOPIC0,
    poll_all_native_minted,
)
from gravity_e2e.utils.hardfork import (
    block_timestamp,
    wait_for_activation_block,
)


LOG = logging.getLogger(__name__)
SUITE_DIR = Path(__file__).resolve().parent
METADATA_FILE = "gamma_bridge_hardfork_metadata.json"
SUMMARY_FILE = "gamma_bridge_hardfork_summary.json"
SOURCE_TYPE_BLOCKCHAIN = 0
SOURCE_ID = 31337
CONFIRMATION_BLOCKS = 16
DEFAULT_TIMEOUT_SECONDS = 15 * 60

GAMMA_CONTRACTS = (
    {
        "name": "NativeOracle",
        "address": Web3.to_checksum_address(
            "0x00000000000000000000000000000001625f4000"
        ),
        "preForkCodeHash": "0x30dd3888ce26735c0d6c5a036b48a1de668dd5506efa7588ce450f976da28255",
        "postForkCodeHash": "0x981087ccdaa0b7843960782e99b078ccdd3820b331f86ce337d9750c5565d984",
    },
    {
        "name": "OracleTaskConfig",
        "address": Web3.to_checksum_address(
            "0x00000000000000000000000000000001625f1009"
        ),
        "preForkCodeHash": "0x74127baf705119810746598b2695ff5fa38f94bd778f0edae46799ffd3606bda",
        "postForkCodeHash": "0xa21bf93e6123b0104b9ea851b8154fb342a5b576c22c71f15f851e266faa9f7f",
    },
)

NATIVE_ORACLE_ABI = [
    {
        "type": "function",
        "name": "getLatestNonce",
        "inputs": [
            {"name": "sourceType", "type": "uint32"},
            {"name": "sourceId", "type": "uint256"},
        ],
        "outputs": [{"name": "nonce", "type": "uint128"}],
        "stateMutability": "view",
    },
    {
        "type": "function",
        "name": "getRecord",
        "inputs": [
            {"name": "sourceType", "type": "uint32"},
            {"name": "sourceId", "type": "uint256"},
            {"name": "nonce", "type": "uint128"},
        ],
        "outputs": [
            {
                "name": "record",
                "type": "tuple",
                "components": [
                    {"name": "recordedAt", "type": "uint64"},
                    {"name": "blockNumber", "type": "uint256"},
                    {"name": "data", "type": "bytes"},
                ],
            }
        ],
        "stateMutability": "view",
    },
    {
        "type": "function",
        "name": "getSourceProgress",
        "inputs": [
            {"name": "sourceType", "type": "uint32"},
            {"name": "sourceId", "type": "uint256"},
        ],
        "outputs": [
            {
                "name": "progress",
                "type": "tuple",
                "components": [
                    {"name": "latestNonce", "type": "uint128"},
                    {"name": "latestPosition", "type": "uint128"},
                ],
            }
        ],
        "stateMutability": "view",
    },
]


def _metadata() -> dict:
    return json.loads((SUITE_DIR / "artifacts" / METADATA_FILE).read_text())


def _write_summary(payload: dict) -> None:
    (SUITE_DIR / "artifacts" / SUMMARY_FILE).write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n"
    )


def _set_finalized(rpc_url: str, block_number: int) -> None:
    response = requests.post(
        rpc_url,
        json={
            "jsonrpc": "2.0",
            "method": "mock_setFinalized",
            "params": [block_number],
            "id": 1,
        },
        timeout=5,
    )
    response.raise_for_status()
    result = response.json()
    if "error" in result:
        raise RuntimeError(f"mock_setFinalized failed: {result['error']}")
    assert int(result["result"]) == block_number


async def _wait_for_block(node_id: str, w3: Web3, target: int) -> None:
    timeout = int(
        os.environ.get(
            "GAMMA_BRIDGE_BLOCK_TIMEOUT_SECONDS",
            str(DEFAULT_TIMEOUT_SECONDS),
        )
    )
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            if w3.eth.block_number >= target:
                return
        except Exception:
            pass
        await asyncio.sleep(1)
    raise TimeoutError(f"{node_id} did not reach block {target}")


def _account_snapshot(w3: Web3, address: str, block_number: int) -> dict:
    code = bytes(w3.eth.get_code(address, block_identifier=block_number))
    proof = w3.eth.get_proof(address, [], block_identifier=block_number)
    code_hash = Web3.to_hex(Web3.keccak(code)).lower()
    assert Web3.to_hex(HexBytes(proof["codeHash"])).lower() == code_hash
    return {
        "balance": int(proof["balance"]),
        "nonce": int(proof["nonce"]),
        "codeHash": code_hash,
        "codeLength": len(code),
        "storageHash": Web3.to_hex(HexBytes(proof["storageHash"])).lower(),
    }


async def _capture_phase(nodes: dict, block_number: int) -> dict:
    await asyncio.gather(
        *(
            _wait_for_block(node_id, node.w3, block_number)
            for node_id, node in nodes.items()
        )
    )
    replicas = {
        node_id: {
            "blockHash": Web3.to_hex(
                node.w3.eth.get_block(block_number)["hash"]
            ),
            "accounts": {
                contract["name"]: _account_snapshot(
                    node.w3, contract["address"], block_number
                )
                for contract in GAMMA_CONTRACTS
            },
        }
        for node_id, node in nodes.items()
    }
    assert len({entry["blockHash"] for entry in replicas.values()}) == 1
    assert len(
        {
            json.dumps(entry["accounts"], sort_keys=True)
            for entry in replicas.values()
        }
    ) == 1
    canonical = next(iter(replicas.values()))
    return {
        "block": block_number,
        "blockHash": canonical["blockHash"],
        "accounts": canonical["accounts"],
    }


def _oracle(w3: Web3):
    return w3.eth.contract(
        address=GAMMA_CONTRACTS[0]["address"],
        abi=NATIVE_ORACLE_ABI,
    )


def _bridge_state(w3: Web3, nonce: int, include_progress: bool) -> dict:
    oracle = _oracle(w3)
    record = oracle.functions.getRecord(
        SOURCE_TYPE_BLOCKCHAIN, SOURCE_ID, nonce
    ).call()
    state = {
        "latestNonce": int(
            oracle.functions.getLatestNonce(
                SOURCE_TYPE_BLOCKCHAIN, SOURCE_ID
            ).call()
        ),
        "record": {
            "recordedAt": int(record[0]),
            "sourcePosition": int(record[1]),
            "payloadLength": len(record[2]),
        },
    }
    if include_progress:
        progress = oracle.functions.getSourceProgress(
            SOURCE_TYPE_BLOCKCHAIN, SOURCE_ID
        ).call()
        state["progress"] = {
            "latestNonce": int(progress[0]),
            "latestPosition": int(progress[1]),
        }
    return state


def _mint_counts(w3: Web3) -> dict[int, int]:
    receiver = w3.eth.contract(
        address=Web3.to_checksum_address(GBRIDGE_RECEIVER_ADDRESS),
        abi=BRIDGE_RECEIVER_ABI,
    )
    logs = w3.eth.get_logs(
        {
            "address": receiver.address,
            "fromBlock": 0,
            "toBlock": "latest",
            "topics": [NATIVE_MINTED_TOPIC0],
        }
    )
    counts: dict[int, int] = {}
    for log in logs:
        event = receiver.events.NativeMinted().process_log(log)
        nonce = int(event.args.nonce)
        counts[nonce] = counts.get(nonce, 0) + 1
    return counts


@pytest.mark.cross_chain
@pytest.mark.bridge
@pytest.mark.asyncio
async def test_gamma_bridge_hardfork(cluster: Cluster):
    metadata = _metadata()
    bridge = metadata["bridge"]
    activation_time = int(
        metadata["gammaHardfork"]["activationTime"]
    )
    evidence = {}

    try:
        assert len(cluster.nodes) == 4
        assert await cluster.set_full_live(timeout=180)
        assert await cluster.check_block_increasing(timeout=60)

        node1 = cluster.get_node("node1")
        node4 = cluster.get_node("node4")
        assert node1 is not None and node4 is not None
        recipient = Web3.to_checksum_address(bridge["recipient"])
        initial_balance = node1.w3.eth.get_balance(recipient)

        assert activation_time - block_timestamp(node1.w3) > 60, (
            "insufficient headroom for pre-fork bridge delivery"
        )
        _set_finalized(bridge["rpcUrl"], bridge["sourceBlocks"][0])
        first = await poll_all_native_minted(
            gravity_w3=node1.w3,
            max_nonce=1,
            timeout=300,
            poll_interval=2,
        )
        assert first["missing_nonces"] == set()
        assert _mint_counts(node1.w3).get(1) == 1
        assert node1.w3.eth.get_balance(recipient) - initial_balance == bridge["amount"]

        pre_states = {
            node_id: _bridge_state(node.w3, 1, include_progress=False)
            for node_id, node in cluster.nodes.items()
        }
        assert len({json.dumps(value, sort_keys=True) for value in pre_states.values()}) == 1
        pre_state = next(iter(pre_states.values()))
        assert pre_state["latestNonce"] == 1
        assert pre_state["record"]["recordedAt"] > 0
        assert pre_state["record"]["sourcePosition"] == bridge["sourceBlocks"][0]
        assert pre_state["record"]["payloadLength"] > 0
        evidence["preForkBridge"] = pre_state

        assert activation_time - block_timestamp(node1.w3) > 30
        node4_height = node4.w3.eth.block_number
        node4_timestamp = block_timestamp(node4.w3, node4_height)
        assert node4_timestamp < activation_time
        assert await node4.stop(), "failed to stop node4 before hardfork"

        live_nodes = {
            node_id: node
            for node_id, node in cluster.nodes.items()
            if node_id != "node4"
        }
        activation_block = await wait_for_activation_block(
            "node1",
            node1.w3,
            activation_time,
            DEFAULT_TIMEOUT_SECONDS,
        )
        pre_fork = await _capture_phase(live_nodes, activation_block - 1)
        post_fork = await _capture_phase(live_nodes, activation_block)
        await asyncio.gather(
            *(
                _wait_for_block(
                    node_id,
                    node.w3,
                    activation_block + CONFIRMATION_BLOCKS,
                )
                for node_id, node in live_nodes.items()
            )
        )

        for contract in GAMMA_CONTRACTS:
            before = pre_fork["accounts"][contract["name"]]
            after = post_fork["accounts"][contract["name"]]
            assert before["codeHash"] == contract["preForkCodeHash"]
            assert after["codeHash"] == contract["postForkCodeHash"]
            for field in ("balance", "nonce", "storageHash"):
                assert before[field] == after[field]
        evidence["hardfork"] = {"preFork": pre_fork, "postFork": post_fork}

        peer_tip = max(node.w3.eth.block_number for node in live_nodes.values())
        replay_started = time.monotonic()
        assert await node4.start(rpc_timeout=120), "failed to restart node4"
        await _wait_for_block("node4", node4.w3, peer_tip)
        replay_seconds = round(time.monotonic() - replay_started, 3)
        replay_pre = await _capture_phase(cluster.nodes, activation_block - 1)
        replay_post = await _capture_phase(cluster.nodes, activation_block)
        assert replay_pre == pre_fork
        assert replay_post == post_fork

        replay_states = {
            node_id: _bridge_state(node.w3, 1, include_progress=True)
            for node_id, node in cluster.nodes.items()
        }
        assert len({json.dumps(value, sort_keys=True) for value in replay_states.values()}) == 1
        replay_state = next(iter(replay_states.values()))
        assert replay_state["progress"] == {
            "latestNonce": 1,
            "latestPosition": bridge["sourceBlocks"][0],
        }
        evidence["node4Replay"] = {
            "stoppedAtBlock": node4_height,
            "stoppedAtTimestamp": node4_timestamp,
            "peerTip": peer_tip,
            "recoverySeconds": replay_seconds,
            "bridgeState": replay_state,
        }

        _set_finalized(bridge["rpcUrl"], bridge["sourceBlocks"][1])
        second = await poll_all_native_minted(
            gravity_w3=node1.w3,
            max_nonce=2,
            timeout=300,
            poll_interval=2,
        )
        assert second["missing_nonces"] == set()
        assert _mint_counts(node1.w3) == {1: 1, 2: 1}
        assert node1.w3.eth.get_balance(recipient) - initial_balance == 2 * bridge["amount"]

        post_states = {
            node_id: _bridge_state(node.w3, 2, include_progress=True)
            for node_id, node in cluster.nodes.items()
        }
        assert len({json.dumps(value, sort_keys=True) for value in post_states.values()}) == 1
        post_state = next(iter(post_states.values()))
        assert post_state["latestNonce"] == 2
        assert post_state["progress"] == {
            "latestNonce": 2,
            "latestPosition": bridge["sourceBlocks"][1],
        }
        records_preserved = post_state["record"]["recordedAt"] > 0
        if os.environ.get("GAMMA_REQUIRE_SOURCE0_RECORDS", "0") == "1":
            assert records_preserved, (
                "sourceType=0 getRecord compatibility was required but nonce 2 "
                "was not stored"
            )
        evidence["postForkBridge"] = {
            **post_state,
            "mintCounts": _mint_counts(node1.w3),
            "source0RecordPreserved": records_preserved,
        }

        assert await cluster.check_block_increasing(timeout=60)
        _write_summary(
            {
                "status": "passed",
                "activationTime": activation_time,
                "activationBlock": activation_block,
                "evidence": evidence,
            }
        )
    except BaseException as error:
        _write_summary(
            {
                "status": "failed",
                "activationTime": activation_time,
                "activationBlock": locals().get("activation_block"),
                "evidence": evidence,
                "errorType": type(error).__name__,
                "error": str(error),
            }
        )
        raise
