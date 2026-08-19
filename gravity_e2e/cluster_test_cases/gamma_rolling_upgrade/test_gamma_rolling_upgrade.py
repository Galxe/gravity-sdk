"""Roll four validators from pre-Gamma reth to the activation binary."""

import asyncio
import hashlib
import json
import logging
import os
from pathlib import Path
import shutil
import time

from hexbytes import HexBytes
import pytest
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils.hardfork import (
    block_timestamp,
    wait_for_activation_block,
)


LOG = logging.getLogger(__name__)
SUITE_DIR = Path(__file__).resolve().parent
METADATA_FILE = "gamma_rolling_upgrade_metadata.json"
SUMMARY_FILE = "gamma_rolling_upgrade_summary.json"
CONFIRMATION_BLOCKS = 16
DEFAULT_MIN_SECONDS_PER_REMAINING_NODE = 120
DEFAULT_BLOCK_WAIT_TIMEOUT_SECONDS = 15 * 60
RECONFIGURATION_ADDRESS = Web3.to_checksum_address(
    "0x00000000000000000000000000000001625F2003"
)
SEL_CURRENT_EPOCH = Web3.keccak(text="currentEpoch()")[:4]
SEL_REMAINING_TIME = Web3.keccak(text="getRemainingTimeSeconds()")[:4]
MIN_ROLLING_EPOCH_HEADROOM_SECONDS = 10 * 60

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


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _metadata() -> dict:
    return json.loads((SUITE_DIR / "artifacts" / METADATA_FILE).read_text())


def _write_summary(payload: dict) -> None:
    path = SUITE_DIR / "artifacts" / SUMMARY_FILE
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")


def _node_binary(cluster: Cluster, node_id: str) -> Path:
    return cluster.base_dir / node_id / "bin" / "gravity_node"


def _running_binary_hash(cluster: Cluster, node_id: str) -> str:
    node = cluster.get_node(node_id)
    assert node is not None and node.pid_file.exists()
    pid = int(node.pid_file.read_text().strip())
    return _sha256(Path(f"/proc/{pid}/exe"))


def _assert_binary_matrix(
    cluster: Cluster, upgraded: set[str], old_hash: str, new_hash: str
) -> None:
    for node_id in sorted(cluster.nodes):
        expected = new_hash if node_id in upgraded else old_hash
        assert _sha256(_node_binary(cluster, node_id)) == expected
        assert _running_binary_hash(cluster, node_id) == expected


def _replace_binary(source: Path, destination: Path) -> None:
    temporary = destination.with_name(f".{destination.name}.rolling.tmp")
    temporary.unlink(missing_ok=True)
    shutil.copy2(source, temporary)
    temporary.chmod(0o755)
    os.replace(temporary, destination)


async def _wait_for_block(
    node_id: str, w3: Web3, target: int, timeout: int | None = None
) -> None:
    if timeout is None:
        timeout = int(
            os.environ.get(
                "GAMMA_ROLLING_BLOCK_TIMEOUT_SECONDS",
                str(DEFAULT_BLOCK_WAIT_TIMEOUT_SECONDS),
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
    last_height = None
    try:
        last_height = w3.eth.block_number
    except Exception:
        pass
    raise TimeoutError(
        f"{node_id} did not reach block {target} within {timeout}s; "
        f"last height was {last_height}"
    )


async def _canonical_checkpoint(cluster: Cluster) -> dict:
    heights = {
        node_id: node.w3.eth.block_number
        for node_id, node in cluster.nodes.items()
    }
    target = max(1, min(heights.values()) - 2)
    hashes = {
        node_id: Web3.to_hex(node.w3.eth.get_block(target)["hash"])
        for node_id, node in cluster.nodes.items()
    }
    assert len(set(hashes.values())) == 1, hashes
    return {"block": target, "blockHash": next(iter(hashes.values()))}


def _uint_call(w3: Web3, selector: bytes) -> int:
    return int.from_bytes(
        w3.eth.call({"to": RECONFIGURATION_ADDRESS, "data": selector}),
        "big",
    )


def _epoch_snapshot(w3: Web3) -> dict:
    return {
        "epoch": _uint_call(w3, SEL_CURRENT_EPOCH),
        "remainingSeconds": _uint_call(w3, SEL_REMAINING_TIME),
    }


async def _upgrade_node(
    cluster: Cluster,
    node_id: str,
    new_binary: Path,
    rollout_epoch: int,
) -> dict:
    node = cluster.get_node(node_id)
    assert node is not None
    peer_tip = max(
        peer.w3.eth.block_number
        for peer_id, peer in cluster.nodes.items()
        if peer_id != node_id
    )
    epoch_before = _epoch_snapshot(node.w3)
    assert epoch_before["epoch"] == rollout_epoch
    assert (
        epoch_before["remainingSeconds"]
        > MIN_ROLLING_EPOCH_HEADROOM_SECONDS
    ), f"unsafe epoch window before replacing {node_id}: {epoch_before}"
    started = time.monotonic()
    assert await node.stop(), f"failed to stop {node_id}"
    _replace_binary(new_binary, _node_binary(cluster, node_id))
    assert await node.start(rpc_timeout=120), f"failed to start {node_id}"
    await _wait_for_block(node_id, node.w3, peer_tip)
    assert await cluster.check_block_increasing(timeout=60)
    checkpoint = await _canonical_checkpoint(cluster)
    epoch_after = _epoch_snapshot(node.w3)
    assert epoch_after["epoch"] == rollout_epoch, (
        f"epoch changed while replacing {node_id}: "
        f"before={epoch_before} after={epoch_after}"
    )
    return {
        "node": node_id,
        "peerTipBeforeStop": peer_tip,
        "caughtUpHeight": node.w3.eth.block_number,
        "durationSeconds": round(time.monotonic() - started, 3),
        "canonicalCheckpoint": checkpoint,
        "epochBefore": epoch_before,
        "epochAfter": epoch_after,
    }


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


async def _capture_phase(
    cluster: Cluster, block_number: int
) -> dict:
    await asyncio.gather(
        *(
            _wait_for_block(node_id, node.w3, block_number)
            for node_id, node in cluster.nodes.items()
        )
    )
    replicas = {}
    for node_id, node in cluster.nodes.items():
        replicas[node_id] = {
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
    assert len({value["blockHash"] for value in replicas.values()}) == 1
    assert len(
        {
            json.dumps(value["accounts"], sort_keys=True)
            for value in replicas.values()
        }
    ) == 1
    canonical = next(iter(replicas.values()))
    return {
        "block": block_number,
        "blockHash": canonical["blockHash"],
        "accounts": canonical["accounts"],
    }


async def _verify_hardfork(cluster: Cluster, activation_time: int) -> dict:
    node1 = cluster.get_node("node1")
    assert node1 is not None
    activation_block = await wait_for_activation_block(
        "node1",
        node1.w3,
        activation_time,
        DEFAULT_BLOCK_WAIT_TIMEOUT_SECONDS,
    )
    pre_fork = await _capture_phase(cluster, activation_block - 1)
    post_fork = await _capture_phase(cluster, activation_block)
    await asyncio.gather(
        *(
            _wait_for_block(
                node_id,
                node.w3,
                activation_block + CONFIRMATION_BLOCKS,
            )
            for node_id, node in cluster.nodes.items()
        )
    )
    for snapshot in (pre_fork, post_fork):
        hashes = {
            Web3.to_hex(node.w3.eth.get_block(snapshot["block"])["hash"])
            for node in cluster.nodes.values()
        }
        assert hashes == {snapshot["blockHash"]}

    for contract in GAMMA_CONTRACTS:
        before = pre_fork["accounts"][contract["name"]]
        after = post_fork["accounts"][contract["name"]]
        assert before["codeHash"] == contract["preForkCodeHash"]
        assert after["codeHash"] == contract["postForkCodeHash"]
        assert before["codeLength"] > 0 and after["codeLength"] > 0
        for field in ("balance", "nonce", "storageHash"):
            assert before[field] == after[field]
    return {
        "activationTime": activation_time,
        "activationBlock": activation_block,
        "preFork": pre_fork,
        "postFork": post_fork,
    }


@pytest.mark.asyncio
async def test_gamma_rolling_binary_upgrade(cluster: Cluster):
    metadata = _metadata()
    old_hash = metadata["oldBinarySha256"]
    new_hash = metadata["newBinarySha256"]
    activation_time = int(
        metadata["gammaHardfork"]["activationTime"]
    )
    new_binary = Path(os.environ["GRAVITY_NEW_BINARY"]).resolve()
    min_seconds = int(
        os.environ.get(
            "GAMMA_ROLLING_MIN_SECONDS_PER_NODE",
            str(DEFAULT_MIN_SECONDS_PER_REMAINING_NODE),
        )
    )
    evidence = []
    upgraded: set[str] = set()

    try:
        assert len(cluster.nodes) == 4
        assert await cluster.set_full_live(timeout=180)
        assert await cluster.check_block_increasing(timeout=60)
        _assert_binary_matrix(cluster, upgraded, old_hash, new_hash)
        node1 = cluster.get_node("node1")
        assert node1 is not None
        rollout_epoch = _epoch_snapshot(node1.w3)["epoch"]

        for node_id in sorted(cluster.nodes):
            current_timestamp = max(
                block_timestamp(node.w3) for node in cluster.nodes.values()
            )
            remaining = len(cluster.nodes) - len(upgraded)
            required_headroom = min_seconds * remaining + 30
            assert activation_time - current_timestamp > required_headroom, (
                f"insufficient pre-activation headroom before {node_id}: "
                f"timestamp={current_timestamp} activation={activation_time} "
                f"required={required_headroom}"
            )
            evidence.append(
                await _upgrade_node(
                    cluster, node_id, new_binary, rollout_epoch
                )
            )
            upgraded.add(node_id)
            _assert_binary_matrix(cluster, upgraded, old_hash, new_hash)

        assert upgraded == set(cluster.nodes)
        final_upgrade_tip = max(
            node.w3.eth.block_number for node in cluster.nodes.values()
        )
        final_upgrade_timestamp = max(
            block_timestamp(node.w3) for node in cluster.nodes.values()
        )
        assert final_upgrade_timestamp < activation_time
        hardfork = await _verify_hardfork(cluster, activation_time)
        assert _epoch_snapshot(cluster.get_node("node1").w3)["epoch"] == (
            rollout_epoch
        ), "epoch changed before Gamma activation completed"

        restart_node = cluster.get_node("node4")
        assert restart_node is not None
        restart_target = max(
            node.w3.eth.block_number for node in cluster.nodes.values()
        )
        restart_started = time.monotonic()
        restart_epoch_before = _epoch_snapshot(restart_node.w3)
        assert restart_epoch_before["epoch"] == rollout_epoch
        assert (
            restart_epoch_before["remainingSeconds"]
            > MIN_ROLLING_EPOCH_HEADROOM_SECONDS
        )
        assert await restart_node.restart(rpc_timeout=120)
        await _wait_for_block("node4", restart_node.w3, restart_target)
        assert await cluster.check_block_increasing(timeout=60)
        _assert_binary_matrix(cluster, upgraded, old_hash, new_hash)
        restart_evidence = {
            "node": "node4",
            "targetBlock": restart_target,
            "recoverySeconds": round(
                time.monotonic() - restart_started, 3
            ),
            "canonicalCheckpoint": await _canonical_checkpoint(cluster),
            "epochBefore": restart_epoch_before,
            "epochAfter": _epoch_snapshot(restart_node.w3),
        }
        assert restart_evidence["epochAfter"]["epoch"] == rollout_epoch

        summary = {
            "status": "passed",
            "activationTime": activation_time,
            "rolloutEpoch": rollout_epoch,
            "allValidatorsUpgradedAtBlock": final_upgrade_tip,
            "allValidatorsUpgradedAtTimestamp": final_upgrade_timestamp,
            "oldBinarySha256": old_hash,
            "newBinarySha256": new_hash,
            "upgrades": evidence,
            "hardfork": hardfork,
            "postForkRestart": restart_evidence,
        }
        _write_summary(summary)
        LOG.info("Gamma rolling upgrade summary: %s", summary)
    except BaseException as error:
        _write_summary(
            {
                "status": "failed",
                "activationTime": activation_time,
                "oldBinarySha256": old_hash,
                "newBinarySha256": new_hash,
                "upgrades": evidence,
                "upgradedNodes": sorted(upgraded),
                "errorType": type(error).__name__,
                "error": str(error),
            }
        )
        raise
