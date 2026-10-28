"""A committed bad-proposer Oracle QC is dropped without losing good work."""

import asyncio
import json
import os
from pathlib import Path
import re
import time
from typing import Optional

import pytest
from eth_account import Account
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils import oracle_test_support as support
from gravity_e2e.utils.mock_binance_index import (
    BUCKET_START_MS,
    DECIMALS,
    MOCK_BINANCE_PORT,
    mock_binance_index_kline_server,
)


# The normal suite can run with a production binary. CI explicitly opts into
# this test and builds gravity_node with `--features byzantine-test`.
pytestmark = pytest.mark.skipif(
    os.environ.get("GRAVITY_ORACLE_BAD_QC_E2E") != "1",
    reason="set GRAVITY_ORACLE_BAD_QC_E2E=1 and build gravity_node with --features byzantine-test",
)

SUITE_DIR = Path(__file__).resolve().parent
GOOD_SOURCE_ID = 1004
BAD_SOURCE_ID = 2001
BAD_ISSUER = "gravity://3/2001/price_feed"
BAD_NONCE = 1
SYSTEM_CALLER = Web3.to_checksum_address("0x00000000000000000000000000000001625f0000")
CONTROL_NAME = "invalid_oracle_qc_control.json"
EVIDENCE_NAME = "invalid_oracle_qc_evidence.json"
COMMITTED_BLOCK_RE = re.compile(r"HashValue\(([0-9a-f]{64})\), (\d+)\)")
BAD_QC_WARNING = "discarding Oracle update with invalid quorum certificate in block"
ORACLE_DELIVERED_TOPIC = Web3.keccak(
    text="OracleDelivered(uint32,uint256,uint128,uint128,bytes32)"
)


def _good_uri() -> str:
    return (
        f"gravity://3/{GOOD_SOURCE_ID}/price_feed?"
        "provider=binance_index_kline_v1&pair=NVDAUSDT&interval=1m&"
        f"bucketStartMs={BUCKET_START_MS}&decimals={DECIMALS}&graceMs=0"
    )


def _validator_author(cluster: Cluster, node_id: str) -> str:
    identity_path = cluster.base_dir / node_id / "config" / "identity.public.yaml"
    identity = identity_path.read_text()
    match = re.search(r"^account_address:\s*(?:0x)?([0-9a-fA-F]{64})\s*$", identity, re.M)
    assert match is not None, f"{node_id} has no consensus author in {identity_path}"
    return "0x" + match.group(1).lower()


def _write_control(path: Path, target_author: str, *, active: bool) -> None:
    document = {
        "schemaVersion": 1,
        "active": active,
        "faultId": "oracle-bad-qc-2001",
        "behavior": "invalid_oracle_qc",
        "nodeId": "node1",
        "targetAuthor": target_author,
    }
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(document))
    temporary.replace(path)


def _read_evidence(path: Path) -> list[dict]:
    if not path.exists():
        return []
    document = json.loads(path.read_text())
    assert document["schemaVersion"] == 1
    assert document["faultId"] == "oracle-bad-qc-2001"
    assert document["behavior"] == "invalid_oracle_qc"
    assert document["nodeId"] == "node1"
    assert document["issuer"] == BAD_ISSUER
    assert document["sourceType"] == support.SOURCE_TYPE_PRICE_FEED
    assert document["sourceId"] == BAD_SOURCE_ID
    assert document["nonce"] == BAD_NONCE
    return document["proposals"]


def _committed_block_numbers(log_path: Path) -> dict[str, int]:
    if not log_path.exists():
        return {}
    commits = {}
    for line in log_path.read_text(errors="replace").splitlines():
        if "commit blocks:" not in line:
            continue
        for block_id, number in COMMITTED_BLOCK_RE.findall(line):
            commits[block_id] = int(number)
    return commits


def _normalize_block_id(value: str) -> str:
    block_id = value.removeprefix("0x").lower()
    assert len(block_id) == 64 and all(ch in "0123456789abcdef" for ch in block_id)
    return block_id


def _send_self_transfer(w3: Web3, nonce: int) -> str:
    sender = Account.from_key(support.FAUCET_KEY)
    signed = sender.sign_transaction(
        {
            "to": sender.address,
            "value": 1,
            "gas": 30_000,
            "gasPrice": w3.eth.gas_price,
            "nonce": nonce,
            "chainId": w3.eth.chain_id,
        }
    )
    return Web3.to_hex(w3.eth.send_raw_transaction(signed.raw_transaction)).lower()


async def _wait_for_committed_injection(
    cluster: Cluster, w3: Web3, native, evidence_path: Path
):
    log_path = cluster.base_dir / "node1" / "consensus_log" / "validator.log"
    submitted = set()
    checked = {}
    next_nonce = w3.eth.get_transaction_count(support.FAUCET_ADDRESS, "pending")
    next_send_at = 0.0
    last_confirmed_nonce = w3.eth.get_transaction_count(support.FAUCET_ADDRESS, "latest")
    last_progress_at = time.monotonic()
    deadline = time.monotonic() + 180

    while time.monotonic() < deadline:
        now = time.monotonic()
        if now >= next_send_at:
            confirmed_nonce = w3.eth.get_transaction_count(
                support.FAUCET_ADDRESS, "latest"
            )
            pending_nonce = w3.eth.get_transaction_count(
                support.FAUCET_ADDRESS, "pending"
            )
            if confirmed_nonce > last_confirmed_nonce:
                last_confirmed_nonce = confirmed_nonce
                last_progress_at = now
            if now - last_progress_at > 3 and pending_nonce < next_nonce:
                next_nonce = pending_nonce
                last_progress_at = now
            if next_nonce < confirmed_nonce + 3:
                submitted.add(_send_self_transfer(w3, next_nonce))
                next_nonce += 1
            next_send_at = now + 0.2

        commits = _committed_block_numbers(log_path)
        for proposal in _read_evidence(evidence_path):
            block_id = _normalize_block_id(proposal["blockId"])
            if block_id not in commits or block_id in checked:
                continue
            height = commits[block_id]
            if w3.eth.block_number <= height:
                continue
            assert proposal["userTxnCount"] > 0
            assert proposal["validOracleIssuer"].startswith(
                f"gravity://3/{GOOD_SOURCE_ID}/price_feed?"
            )
            block = w3.eth.get_block(height, full_transactions=True)
            successor = w3.eth.get_block(height + 1)
            assert bytes(successor["parentBeaconBlockRoot"]).hex() == block_id
            good_transactions = [
                tx for tx in block["transactions"]
                if _record_source_id(native, tx) == GOOD_SOURCE_ID
            ]
            user_transactions = [
                tx for tx in block["transactions"]
                if Web3.to_hex(tx["hash"]).lower() in submitted
            ]
            good_succeeded = bool(good_transactions) and all(
                w3.eth.get_transaction_receipt(tx["hash"])["status"] == 1
                for tx in good_transactions
            )
            user_succeeded = bool(user_transactions) and all(
                w3.eth.get_transaction_receipt(tx["hash"])["status"] == 1
                for tx in user_transactions
            )
            delivered = bool(
                _delivered_logs(w3, native, height, GOOD_SOURCE_ID)
            )
            if good_succeeded and user_succeeded and delivered:
                return proposal, height, submitted
            checked[block_id] = {
                "height": height,
                "good_succeeded": good_succeeded,
                "user_succeeded": user_succeeded,
                "delivered": delivered,
            }
        await asyncio.sleep(0.1)

    raise AssertionError(
        "No committed node1 proposal containing the forged Oracle QC had both "
        "a successfully delivered genuine Oracle update and a successful user "
        "transaction within 180s. "
        "Check the mock feed, node1 consensus log, and build gravity_node "
        "with --features byzantine-test. "
        f"injection_proposals={_read_evidence(evidence_path)!r}, "
        f"checked={checked!r}, "
        f"sent={len(submitted)}, latest_nonce="
        f"{w3.eth.get_transaction_count(support.FAUCET_ADDRESS, 'latest')}, "
        f"next_nonce={next_nonce}"
    )


async def _wait_for_height(nodes: dict, height: int, timeout: int = 45) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if all(node.w3.eth.block_number >= height for node in nodes.values()):
            return
        await asyncio.sleep(0.5)
    raise AssertionError(f"Nodes {list(nodes)} did not reach block {height}")


async def _wait_for_guard_warnings(cluster: Cluster, nodes: dict, block_id: str) -> None:
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        missing = []
        for node_id in nodes:
            path = cluster.base_dir / node_id / "consensus_log" / "validator.log"
            content = path.read_text(errors="replace") if path.exists() else ""
            if not any(
                BAD_QC_WARNING in line and block_id[:8] in line
                for line in content.splitlines()
            ):
                missing.append(node_id)
        if not missing:
            return
        await asyncio.sleep(0.5)
    raise AssertionError(f"SDK bad-QC rejection missing at {block_id}: {missing}")


def _record_source_id(native, transaction) -> Optional[int]:
    if transaction["to"] is None or transaction["to"].lower() != native.address.lower():
        return None
    try:
        fn, arguments = native.decode_function_input(transaction["input"])
    except ValueError:
        return None
    return int(arguments["sourceId"]) if fn.fn_name in ("record", "recordBatch") else None


def _delivered_logs(w3: Web3, native, height: int, source_id: int):
    topic = lambda value: Web3.to_hex(value.to_bytes(32, "big"))
    return w3.eth.get_logs(
        {
            "fromBlock": height,
            "toBlock": height,
            "address": native.address,
            "topics": [
                Web3.to_hex(ORACLE_DELIVERED_TOPIC),
                topic(support.SOURCE_TYPE_PRICE_FEED),
                topic(source_id),
            ],
        }
    )


@pytest.mark.asyncio
async def test_committed_invalid_oracle_qc_drops_only_bad_update(cluster: Cluster):
    generated_genesis = json.loads((SUITE_DIR / "artifacts" / "genesis.json").read_text())
    assert "oracleSignatureVerificationTime" not in generated_genesis["config"]
    assert len(cluster.nodes) == 4
    assert await cluster.set_full_live(timeout=180)
    assert await cluster.check_block_increasing(timeout=60)
    node1 = cluster.get_node("node1")
    assert node1 is not None and node1.w3.is_connected()
    w3 = node1.w3

    contracts_out = support.ensure_contract_artifacts(
        SUITE_DIR,
        [
            ("PriceFeedResolver.sol", "PriceFeedResolver"),
            ("NativeOracle.sol", "NativeOracle"),
            ("OracleTaskConfig.sol", "OracleTaskConfig"),
        ],
    )
    resolver_artifact = support.load_artifact(
        contracts_out, "PriceFeedResolver.sol", "PriceFeedResolver"
    )
    native_artifact = support.load_artifact(contracts_out, "NativeOracle.sol", "NativeOracle")
    task_artifact = support.load_artifact(
        contracts_out, "OracleTaskConfig.sol", "OracleTaskConfig"
    )
    resolver = support.deploy_contract(w3, resolver_artifact)
    native = w3.eth.contract(address=support.NATIVE_ORACLE_ADDRESS, abi=native_artifact["abi"])
    task_config = w3.eth.contract(
        address=support.ORACLE_TASK_CONFIG_ADDRESS, abi=task_artifact["abi"]
    )

    with mock_binance_index_kline_server(MOCK_BINANCE_PORT):
        targets = [
            support.NATIVE_ORACLE_ADDRESS,
            support.NATIVE_ORACLE_ADDRESS,
            support.ORACLE_TASK_CONFIG_ADDRESS,
            support.ORACLE_TASK_CONFIG_ADDRESS,
            support.ORACLE_TASK_CONFIG_ADDRESS,
        ]
        calls = [
            support.function_calldata(
                native.functions.setCallback(
                    support.SOURCE_TYPE_PRICE_FEED, BAD_SOURCE_ID, resolver.address
                )
            ),
            support.function_calldata(
                native.functions.setCallback(
                    support.SOURCE_TYPE_PRICE_FEED, GOOD_SOURCE_ID, resolver.address
                )
            ),
            support.function_calldata(
                task_config.functions.setTask(
                    support.SOURCE_TYPE_PRICE_FEED,
                    GOOD_SOURCE_ID,
                    support.TASK_PRICE_FEED,
                    _good_uri().encode(),
                )
            ),
            support.function_calldata(
                task_config.functions.removeTask(
                    support.SOURCE_TYPE_PRICE_FEED, 1001, support.TASK_PRICE_FEED
                )
            ),
            support.function_calldata(
                task_config.functions.removeTask(
                    support.SOURCE_TYPE_PRICE_FEED, 1002, support.TASK_PRICE_FEED
                )
            ),
        ]
        await support.execute_governance_proposal(
            w3, support.faucet_voting_pool(w3), targets, calls, "invalid-oracle-qc-e2e"
        )
        assert tuple(
            native.functions.getSourceProgress(
                support.SOURCE_TYPE_PRICE_FEED, BAD_SOURCE_ID
            ).call()
        ) == (0, 0)

        control_path = cluster.base_dir / CONTROL_NAME
        evidence_path = cluster.base_dir / EVIDENCE_NAME
        evidence_path.unlink(missing_ok=True)
        target_author = _validator_author(cluster, "node1")
        node4 = cluster.get_node("node4")
        assert node4 is not None
        live_nodes = {node_id: node for node_id, node in cluster.nodes.items() if node_id != "node4"}
        node4_restart_required = False
        control_active = False
        try:
            # A 3-of-4 voting quorum can certify and commit while node4 is
            # offline. Restarting it later exercises block sync/replay over H.
            node4_restart_required = True
            assert await node4.stop(), "failed to stop node4 before bad QC proposal"
            _write_control(control_path, target_author, active=True)
            control_active = True
            proposal, height, submitted = await _wait_for_committed_injection(
                cluster, w3, native, evidence_path
            )
            _write_control(control_path, target_author, active=False)
            control_active = False

            block_id = _normalize_block_id(proposal["blockId"])
            await _wait_for_height(live_nodes, height + 4)
            await _wait_for_guard_warnings(cluster, live_nodes, proposal["blockId"])
            peer_tip = max(node.w3.eth.block_number for node in live_nodes.values())
            assert await node4.start(rpc_timeout=120), "failed to restart node4"
            node4_restart_required = False
            await _wait_for_height({"node4": node4}, peer_tip, timeout=120)
            recovery_log = (cluster.base_dir / "node4" / "consensus_log" / "validator.log").read_text(
                errors="replace"
            )
            assert any(
                f"recover block [id: {block_id[:8]}," in line
                and f"block_number: Some({height})" in line
                for line in recovery_log.splitlines()
            ), f"node4 did not replay bad-QC block {block_id} through BlockStore recovery"

            snapshots = {}
            followup_snapshots = {}
            for node_id, node in cluster.nodes.items():
                block = node.w3.eth.get_block(height, full_transactions=True)
                successor = node.w3.eth.get_block(height + 1)
                assert bytes(successor["parentBeaconBlockRoot"]).hex() == block_id, (
                    node_id,
                    height,
                    block_id,
                )
                snapshots[node_id] = (
                    bytes(block["hash"]),
                    bytes(block["stateRoot"]),
                    bytes(block["receiptsRoot"]),
                    bytes(block["transactionsRoot"]),
                )
                followup = node.w3.eth.get_block(height + 4)
                followup_snapshots[node_id] = (
                    bytes(followup["hash"]),
                    bytes(followup["stateRoot"]),
                )

                bad_record_transactions = [
                    tx for tx in block["transactions"]
                    if _record_source_id(native, tx) == BAD_SOURCE_ID
                ]
                replica_native = node.w3.eth.contract(
                    address=support.NATIVE_ORACLE_ADDRESS, abi=native_artifact["abi"]
                )
                assert not bad_record_transactions, (
                    f"{node_id} executed a NativeOracle system transaction for bad QC "
                    f"at committed height {height}"
                )
                assert not _delivered_logs(node.w3, replica_native, height, BAD_SOURCE_ID)
                assert tuple(
                    replica_native.functions.getSourceProgress(
                        support.SOURCE_TYPE_PRICE_FEED, BAD_SOURCE_ID
                    ).call(block_identifier=height)
                ) == (0, 0)
                replica_resolver = node.w3.eth.contract(
                    address=resolver.address, abi=resolver_artifact["abi"]
                )
                assert tuple(
                    replica_resolver.functions.latestPrice(BAD_SOURCE_ID).call(
                        block_identifier=height
                    )
                ) == (0, 0, 0)

                good_record_transactions = [
                    tx for tx in block["transactions"]
                    if _record_source_id(native, tx) == GOOD_SOURCE_ID
                ]
                assert good_record_transactions, (
                    f"{node_id} lost the same-block valid Oracle update at {height}"
                )
                assert all(
                    node.w3.eth.get_transaction_receipt(tx["hash"])["status"] == 1
                    for tx in good_record_transactions
                )
                assert _delivered_logs(node.w3, replica_native, height, GOOD_SOURCE_ID)
                prior_good = tuple(
                    replica_native.functions.getSourceProgress(
                        support.SOURCE_TYPE_PRICE_FEED, GOOD_SOURCE_ID
                    ).call(block_identifier=height - 1)
                )
                current_good = tuple(
                    replica_native.functions.getSourceProgress(
                        support.SOURCE_TYPE_PRICE_FEED, GOOD_SOURCE_ID
                    ).call(block_identifier=height)
                )
                assert current_good[0] > prior_good[0], node_id

                # Metadata consumes one SYSTEM_CALLER nonce on every block;
                # each delivered Oracle record consumes another. Compare the
                # actual H-1/H account nonce with all system transactions in H.
                system_transactions = [
                    tx for tx in block["transactions"]
                    if tx["from"].lower() == SYSTEM_CALLER.lower()
                ]
                assert all(
                    tx["from"].lower() == SYSTEM_CALLER.lower()
                    for tx in good_record_transactions
                )
                nonce_before = node.w3.eth.get_transaction_count(SYSTEM_CALLER, height - 1)
                nonce_after = node.w3.eth.get_transaction_count(SYSTEM_CALLER, height)
                assert nonce_after - nonce_before == len(system_transactions), (
                    node_id,
                    height,
                    nonce_before,
                    nonce_after,
                    len(system_transactions),
                )
                assert sorted(tx["nonce"] for tx in system_transactions) == list(
                    range(nonce_before, nonce_after)
                ), node_id

                user_transactions = [
                    tx for tx in block["transactions"]
                    if Web3.to_hex(tx["hash"]).lower() in submitted
                ]
                assert user_transactions, f"{node_id} lost the same-block user transaction"
                assert all(
                    node.w3.eth.get_transaction_receipt(tx["hash"])["status"] == 1
                    for tx in user_transactions
                )

            assert len(set(snapshots.values())) == 1, snapshots
            assert len(set(followup_snapshots.values())) == 1, followup_snapshots
        finally:
            try:
                if control_active:
                    _write_control(control_path, target_author, active=False)
            finally:
                if node4_restart_required:
                    assert await node4.start(rpc_timeout=120), "failed to restore node4 after test"
