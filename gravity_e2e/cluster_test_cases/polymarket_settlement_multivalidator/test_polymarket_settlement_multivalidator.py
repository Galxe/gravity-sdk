"""Four-validator finalized Polymarket CTF settlement oracle E2E."""

import asyncio
import json
import logging
from pathlib import Path
import time

import pytest
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils import oracle_test_support as support
from gravity_e2e.utils.mock_polymarket_polygon import (
    CONDITION_ID,
    CTF_ADDRESS,
    LOG_INDEX,
    MIRROR_ID,
    MOCK_POLYGON_PORT,
    ORACLE_ADDRESS,
    OUTCOME_SLOT_COUNT,
    QUESTION_ID,
    SOURCE_BLOCK,
    TX_HASH,
    mock_polymarket_polygon_server,
)

LOG = logging.getLogger(__name__)
SUITE_DIR = Path(__file__).resolve().parent
SOURCE_TYPE_POLYMARKET_SETTLEMENT = 6
TASK_POLYMARKET_SETTLEMENT = Web3.keccak(text="polymarket_settlement")
POLYGON_CHAIN_ID = 137
TARGET_NONCE = 1
WINNING_SLOT = 1
SETTLEMENT_KIND_CTF_CONDITION_RESOLUTION = 1
QUORUM_VALIDATORS = 3


def _settlement_uri() -> str:
    return (
        f"gravity://6/{MIRROR_ID}/polymarket_settlement?"
        f"ctf={CTF_ADDRESS}&condition={CONDITION_ID}&fromBlock={SOURCE_BLOCK - 2}&"
        "chainId=137&maxBlocksPerPoll=20"
    )


def _consensus_log(cluster: Cluster, node_id: str) -> Path:
    return cluster.base_dir / node_id / "consensus_log" / "validator.log"


def _relayer_state(cluster: Cluster, node_id: str) -> Path:
    return cluster.base_dir / node_id / "data" / "reth" / "relayer_state.json"


def _line_matches(content: str, marker: str, issuer: str) -> bool:
    return any(marker in line and issuer in line for line in content.splitlines())


def _read_source_state(cluster: Cluster, node_id: str, uri: str) -> dict | None:
    state_path = _relayer_state(cluster, node_id)
    if not state_path.exists():
        return None
    return json.loads(state_path.read_text())["sources"].get(uri)


async def _wait_for_empty_scan_watermarks(
    cluster: Cluster,
    uri: str,
    *,
    timeout: int = 240,
) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        states = {
            node_id: _read_source_state(cluster, node_id, uri)
            for node_id in cluster.nodes
        }
        if all(
            state is not None
            and int(state["last_nonce"]) == 0
            and int(state["last_nonce_block"]) == 0
            and int(state["cursor_block"]) >= SOURCE_BLOCK - 1
            for state in states.values()
        ):
            return
        await asyncio.sleep(2)
    raise AssertionError(f"validators did not persist empty finalized scans: {states}")


async def _wait_for_terminal_relayer_state(
    cluster: Cluster,
    uri: str,
    *,
    timeout: int = 180,
) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        states = {
            node_id: _read_source_state(cluster, node_id, uri)
            for node_id in cluster.nodes
        }
        if all(
            state is not None
            and int(state["last_nonce"]) == TARGET_NONCE
            and int(state["last_nonce_block"]) == SOURCE_BLOCK
            and int(state["cursor_block"]) >= SOURCE_BLOCK
            for state in states.values()
        ):
            return
        await asyncio.sleep(2)
    raise AssertionError(f"validators did not persist terminal settlement: {states}")


async def _wait_for_consensus_evidence(
    cluster: Cluster,
    *,
    timeout: int = 180,
) -> None:
    issuer = f"gravity://6/{MIRROR_ID}"
    missing_observers = set(cluster.nodes)
    certifiers = set()
    quorum_seen = False
    deadline = time.monotonic() + timeout

    while time.monotonic() < deadline:
        logs = {}
        for node_id in cluster.nodes:
            path = _consensus_log(cluster, node_id)
            logs[node_id] = path.read_text(errors="replace") if path.exists() else ""

        for node_id in list(missing_observers):
            if _line_matches(logs[node_id], "JWKObserver spawned.", issuer):
                missing_observers.remove(node_id)
        for node_id, content in logs.items():
            if _line_matches(content, "Start certifying update.", issuer):
                certifiers.add(node_id)
        quorum_seen = quorum_seen or any(
            any(
                "Peer vote aggregated." in line
                and issuer in line
                and (
                    "threshold_exceeded=true" in line
                    or '"threshold_exceeded":true' in line
                )
                for line in content.splitlines()
            )
            for content in logs.values()
        )

        if not missing_observers and len(certifiers) >= QUORUM_VALIDATORS and quorum_seen:
            return
        await asyncio.sleep(2)

    raise AssertionError(
        "missing validator oracle evidence: "
        f"observers={sorted(missing_observers)}, "
        f"certifiers={sorted(certifiers)}, quorum_seen={quorum_seen}"
    )


async def _wait_for_settlement(native_oracle, resolver, *, timeout: int = 300):
    condition = bytes.fromhex(CONDITION_ID.removeprefix("0x"))
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        progress = tuple(
            native_oracle.functions.getSourceProgress(
                SOURCE_TYPE_POLYMARKET_SETTLEMENT,
                MIRROR_ID,
            ).call()
        )
        settlement = tuple(resolver.functions.getSettlement(MIRROR_ID, condition).call())
        if progress == (TARGET_NONCE, SOURCE_BLOCK) and settlement[0]:
            return progress, settlement
        await asyncio.sleep(2)
    raise TimeoutError("Polymarket settlement did not reach NativeOracle and resolver")


async def _wait_for_block(w3, block_number: int, timeout: int = 60) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if w3.eth.block_number >= block_number:
            return
        await asyncio.sleep(1)
    raise TimeoutError(f"RPC did not reach Gravity block {block_number}")


def _assert_settlement(settlement: tuple) -> None:
    assert settlement[0] is True
    assert settlement[1] == TARGET_NONCE
    assert settlement[2] == POLYGON_CHAIN_ID
    assert Web3.to_checksum_address(settlement[3]) == Web3.to_checksum_address(CTF_ADDRESS)
    assert Web3.to_checksum_address(settlement[4]) == Web3.to_checksum_address(ORACLE_ADDRESS)
    assert bytes(settlement[5]) == bytes.fromhex(QUESTION_ID.removeprefix("0x"))
    assert settlement[6] == OUTCOME_SLOT_COUNT
    assert bytes(settlement[7]) == bytes.fromhex(TX_HASH.removeprefix("0x"))
    assert settlement[8] == LOG_INDEX
    assert settlement[9] == SETTLEMENT_KIND_CTF_CONDITION_RESOLUTION


@pytest.mark.asyncio
async def test_four_validators_finalize_polymarket_settlement(cluster: Cluster):
    assert len(cluster.nodes) == 4
    assert await cluster.set_full_live(timeout=180)
    assert await cluster.check_block_increasing(timeout=60)
    validator_set = await cluster.validator_list()
    assert {validator.id for validator in validator_set.active} == set(cluster.nodes)

    node1 = cluster.get_node("node1")
    assert node1 is not None and node1.w3.is_connected()
    w3 = node1.w3

    required = [
        ("PolymarketSettlementResolver.sol", "PolymarketSettlementResolver"),
        ("NativeOracle.sol", "NativeOracle"),
        ("OracleTaskConfig.sol", "OracleTaskConfig"),
    ]
    contracts_out = support.ensure_contract_artifacts(SUITE_DIR, required)
    resolver_artifact = support.load_artifact(
        contracts_out,
        "PolymarketSettlementResolver.sol",
        "PolymarketSettlementResolver",
    )
    native_artifact = support.load_artifact(
        contracts_out,
        "NativeOracle.sol",
        "NativeOracle",
    )
    task_artifact = support.load_artifact(
        contracts_out,
        "OracleTaskConfig.sol",
        "OracleTaskConfig",
    )

    resolver = support.deploy_contract(w3, resolver_artifact)
    native_oracle = w3.eth.contract(
        address=support.NATIVE_ORACLE_ADDRESS,
        abi=native_artifact["abi"],
    )
    task_config = w3.eth.contract(
        address=support.ORACLE_TASK_CONFIG_ADDRESS,
        abi=task_artifact["abi"],
    )
    uri = _settlement_uri()
    relayer_config = json.loads((SUITE_DIR / "relayer_config.json").read_text())

    with mock_polymarket_polygon_server(MOCK_POLYGON_PORT) as mock:
        assert relayer_config["uri_mappings"] == {uri: mock.rpc_url}
        await support.execute_governance_proposal(
            w3,
            support.faucet_voting_pool(w3),
            [
                support.NATIVE_ORACLE_ADDRESS,
                resolver.address,
                support.ORACLE_TASK_CONFIG_ADDRESS,
            ],
            [
                support.function_calldata(
                    native_oracle.functions.setDefaultCallback(
                        SOURCE_TYPE_POLYMARKET_SETTLEMENT,
                        resolver.address,
                    )
                ),
                support.function_calldata(
                    resolver.functions.registerMirror(
                        MIRROR_ID,
                        POLYGON_CHAIN_ID,
                        Web3.to_checksum_address(CTF_ADDRESS),
                        bytes.fromhex(CONDITION_ID.removeprefix("0x")),
                        OUTCOME_SLOT_COUNT,
                    )
                ),
                support.function_calldata(
                    task_config.functions.setTask(
                        SOURCE_TYPE_POLYMARKET_SETTLEMENT,
                        MIRROR_ID,
                        TASK_POLYMARKET_SETTLEMENT,
                        uri.encode(),
                    )
                ),
            ],
            "deterministic-polymarket-settlement-multivalidator-e2e",
        )

        await _wait_for_empty_scan_watermarks(cluster, uri)
        assert tuple(
            native_oracle.functions.getSourceProgress(
                SOURCE_TYPE_POLYMARKET_SETTLEMENT,
                MIRROR_ID,
            ).call()
        ) == (0, 0)
        assert resolver.functions.isSettlementObserved(
            MIRROR_ID,
            bytes.fromhex(CONDITION_ID.removeprefix("0x")),
        ).call() is False
        assert (SOURCE_BLOCK - 1, SOURCE_BLOCK - 1) in mock.scan_ranges()
        assert mock.request_count("eth_getLogs") >= len(cluster.nodes)

        mock.set_finalized_block(SOURCE_BLOCK)
        progress, settlement = await _wait_for_settlement(native_oracle, resolver)
        await _wait_for_consensus_evidence(cluster)
        await _wait_for_terminal_relayer_state(cluster, uri)

        assert progress == (TARGET_NONCE, SOURCE_BLOCK)
        _assert_settlement(settlement)
        observation = tuple(
            resolver.functions.getSettlementObservation(
                MIRROR_ID,
                bytes.fromhex(CONDITION_ID.removeprefix("0x")),
            ).call()
        )
        assert observation[0] == 1
        assert observation[1] == WINNING_SLOT
        assert observation[2] == TARGET_NONCE
        assert observation[3] > 0
        assert bytes(observation[4]) == bytes.fromhex(TX_HASH.removeprefix("0x"))
        assert observation[5] == LOG_INDEX

        assert (SOURCE_BLOCK, SOURCE_BLOCK) in mock.scan_ranges()
        assert mock.request_count("eth_chainId") >= len(cluster.nodes)
        assert mock.request_count("eth_getLogs") >= 2 * len(cluster.nodes)

        snapshot_block = w3.eth.block_number
        for node_id, node in cluster.nodes.items():
            await _wait_for_block(node.w3, snapshot_block)
            replica_native = node.w3.eth.contract(
                address=support.NATIVE_ORACLE_ADDRESS,
                abi=native_artifact["abi"],
            )
            replica_resolver = node.w3.eth.contract(
                address=resolver.address,
                abi=resolver_artifact["abi"],
            )
            replica_progress = tuple(
                replica_native.functions.getSourceProgress(
                    SOURCE_TYPE_POLYMARKET_SETTLEMENT,
                    MIRROR_ID,
                ).call(block_identifier=snapshot_block)
            )
            replica_settlement = tuple(
                replica_resolver.functions.getSettlement(
                    MIRROR_ID,
                    bytes.fromhex(CONDITION_ID.removeprefix("0x")),
                ).call(block_identifier=snapshot_block)
            )
            assert replica_progress == progress, node_id
            assert replica_settlement == settlement, node_id

        LOG.info(
            "Four-validator Polymarket QC stored mirror=%s block=%s winningSlot=%s",
            MIRROR_ID,
            SOURCE_BLOCK,
            WINNING_SLOT,
        )
