"""Rolling binary upgrade followed by live sourceType 3/6 activation."""

import asyncio
from decimal import Decimal
import hashlib
import json
import logging
import os
from pathlib import Path
import shutil
import time
from urllib.parse import urlencode
from urllib.request import Request, urlopen

import pytest
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils import oracle_test_support as support
from gravity_e2e.utils.bridge_utils import poll_all_native_minted


LOG = logging.getLogger(__name__)
SUITE_DIR = Path(__file__).resolve().parent

SOURCE_TYPE_BRIDGE = 0
SOURCE_TYPE_POLYMARKET = 6
BRIDGE_SOURCE_ID = 31337
POLYGON_CHAIN_ID = 137
CTF_ADDRESS = Web3.to_checksum_address(
    "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045"
)
POLYMARKET_TASK = Web3.keccak(text="polymarket_settlement")
RECONFIGURATION_ADDRESS = Web3.to_checksum_address(
    "0x00000000000000000000000000000001625F2003"
)
SEL_CURRENT_EPOCH = Web3.keccak(text="currentEpoch()")[:4]
SEL_REMAINING_TIME = Web3.keccak(text="getRemainingTimeSeconds()")[:4]

QUORUM_VALIDATORS = 3
MIN_PROPOSAL_WINDOW_SECONDS = 35
EPOCH_TIMEOUT_SECONDS = 180
ORACLE_TIMEOUT_SECONDS = 360


def _metadata() -> dict:
    return json.loads(
        (
            SUITE_DIR
            / "artifacts"
            / "oracle_rolling_activation_metadata.json"
        ).read_text()
    )


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _node_binary(cluster: Cluster, node_id: str) -> Path:
    return cluster.base_dir / node_id / "bin" / "gravity_node"


def _assert_binary_matrix(
    cluster: Cluster, upgraded: set[str], old_hash: str, new_hash: str
) -> None:
    for node_id in cluster.nodes:
        expected = new_hash if node_id in upgraded else old_hash
        actual = _sha256(_node_binary(cluster, node_id))
        assert actual == expected, (
            f"{node_id} binary mismatch: expected={expected[:12]} "
            f"actual={actual[:12]}"
        )


def _replace_binary(source: Path, destination: Path) -> None:
    destination.unlink(missing_ok=True)
    try:
        os.link(source, destination)
    except OSError:
        shutil.copy2(source, destination)
    destination.chmod(0o755)


async def _upgrade_node(cluster: Cluster, node_id: str, new_binary: Path):
    node = cluster.get_node(node_id)
    assert node is not None
    before = max(
        peer.w3.eth.block_number
        for peer_id, peer in cluster.nodes.items()
        if peer_id != node_id
    )
    assert await node.stop(), f"failed to stop {node_id}"
    _replace_binary(new_binary, _node_binary(cluster, node_id))
    assert await node.start(), f"failed to restart {node_id} with new binary"

    deadline = time.monotonic() + 90
    while time.monotonic() < deadline:
        try:
            if node.w3.eth.block_number >= before:
                break
        except Exception:
            pass
        await asyncio.sleep(1)
    else:
        raise TimeoutError(f"{node_id} did not catch up after binary upgrade")

    assert await cluster.check_block_increasing(timeout=45)


def _json_rpc(url: str, method: str, params: list):
    payload = json.dumps(
        {"jsonrpc": "2.0", "id": 1, "method": method, "params": params}
    ).encode()
    request = Request(
        url, data=payload, headers={"Content-Type": "application/json"}
    )
    with urlopen(request, timeout=10) as response:
        body = json.loads(response.read())
    if "error" in body:
        raise RuntimeError(f"{method} failed: {body['error']}")
    return body["result"]


async def _release_bridge_round(
    cluster: Cluster, bridge: dict, target_nonce: int
) -> None:
    assert (
        _json_rpc(
            bridge["rpcUrl"], "mock_setFinalized", [target_nonce]
        )
        == target_nonce
    )
    node1 = cluster.get_node("node1")
    result = await poll_all_native_minted(
        gravity_w3=node1.w3,
        max_nonce=target_nonce,
        timeout=ORACLE_TIMEOUT_SECONDS,
        poll_interval=2,
    )
    assert not result["missing_nonces"], result["missing_nonces"]


def _current_epoch(w3: Web3) -> int:
    return int.from_bytes(
        w3.eth.call(
            {"to": RECONFIGURATION_ADDRESS, "data": SEL_CURRENT_EPOCH}
        ),
        "big",
    )


def _remaining_epoch_seconds(w3: Web3) -> int:
    return int.from_bytes(
        w3.eth.call(
            {"to": RECONFIGURATION_ADDRESS, "data": SEL_REMAINING_TIME}
        ),
        "big",
    )


async def _wait_for_epoch_advance(
    cluster: Cluster, start_epoch: int
) -> int:
    deadline = time.monotonic() + EPOCH_TIMEOUT_SECONDS
    node1 = cluster.get_node("node1")
    while time.monotonic() < deadline:
        epoch = _current_epoch(node1.w3)
        if epoch > start_epoch:
            for node_id, node in cluster.nodes.items():
                while _current_epoch(node.w3) < epoch:
                    if time.monotonic() >= deadline:
                        raise TimeoutError(
                            f"{node_id} did not reach epoch {epoch}"
                        )
                    await asyncio.sleep(1)
            return epoch
        await asyncio.sleep(1)
    raise TimeoutError(f"chain did not advance past epoch {start_epoch}")


async def _wait_for_proposal_window(cluster: Cluster) -> int:
    node1 = cluster.get_node("node1")
    deadline = time.monotonic() + EPOCH_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        remaining = _remaining_epoch_seconds(node1.w3)
        if remaining >= MIN_PROPOSAL_WINDOW_SECONDS:
            return _current_epoch(node1.w3)
        current = _current_epoch(node1.w3)
        LOG.info(
            "Only %ss remain in epoch %s; waiting for the next proposal window",
            remaining,
            current,
        )
        await _wait_for_epoch_advance(cluster, current)
    raise TimeoutError("no epoch had enough time for governance activation")


def _fetch_binance_price(
    base_url: str, pair: str, bucket_start_ms: int
) -> int:
    bucket_end_ms = bucket_start_ms + support.INTERVAL_MS - 1
    query = urlencode(
        {
            "pair": pair,
            "interval": "1m",
            "startTime": bucket_start_ms,
            "endTime": bucket_end_ms,
            "limit": 1,
        }
    )
    with urlopen(
        f"{base_url}/fapi/v1/indexPriceKlines?{query}", timeout=20
    ) as response:
        rows = json.loads(response.read())
    assert len(rows) == 1, f"Binance returned {len(rows)} rows for {pair}"
    assert rows[0][0] == bucket_start_ms
    assert rows[0][6] == bucket_end_ms
    scaled = Decimal(rows[0][4]) * (10**support.DECIMALS)
    assert scaled == scaled.to_integral_value()
    return int(scaled)


def _validator_log(cluster: Cluster, node_id: str) -> Path:
    return cluster.base_dir / node_id / "consensus_log" / "validator.log"


def _log_contains(cluster: Cluster, node_id: str, marker: str, issuer: str):
    path = _validator_log(cluster, node_id)
    content = path.read_text(errors="replace") if path.exists() else ""
    return any(
        marker in line and issuer in line for line in content.splitlines()
    )


async def _wait_for_consensus_evidence(
    cluster: Cluster, issuers: list[str]
) -> None:
    missing_observers = {
        (node_id, issuer)
        for node_id in cluster.nodes
        for issuer in issuers
    }
    certifiers = {issuer: set() for issuer in issuers}
    missing_quorums = set(issuers)
    deadline = time.monotonic() + ORACLE_TIMEOUT_SECONDS

    while time.monotonic() < deadline:
        for node_id, issuer in list(missing_observers):
            if _log_contains(
                cluster, node_id, "JWKObserver spawned.", issuer
            ):
                missing_observers.remove((node_id, issuer))

        for issuer in issuers:
            for node_id in cluster.nodes:
                if _log_contains(
                    cluster, node_id, "Start certifying update.", issuer
                ):
                    certifiers[issuer].add(node_id)
                if _log_contains(
                    cluster, node_id, "Peer vote aggregated.", issuer
                ):
                    content = _validator_log(
                        cluster, node_id
                    ).read_text(errors="replace")
                    if any(
                        "Peer vote aggregated." in line
                        and issuer in line
                        and (
                            "threshold_exceeded=true" in line
                            or '"threshold_exceeded":true' in line
                        )
                        for line in content.splitlines()
                    ):
                        missing_quorums.discard(issuer)

        if (
            not missing_observers
            and not missing_quorums
            and all(
                len(certifiers[issuer]) >= QUORUM_VALIDATORS
                for issuer in issuers
            )
        ):
            return
        await asyncio.sleep(2)

    raise AssertionError(
        "missing consensus evidence: "
        f"observers={sorted(missing_observers)} "
        f"certifiers={ {key: sorted(value) for key, value in certifiers.items()} } "
        f"quorums={sorted(missing_quorums)}"
    )


async def _wait_for_polymarket_settlement(
    resolver, mirror_id: int, condition_id: bytes
) -> tuple:
    deadline = time.monotonic() + ORACLE_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        settlement = tuple(
            resolver.functions.getSettlement(
                mirror_id, condition_id
            ).call()
        )
        if settlement[0]:
            return settlement
        await asyncio.sleep(2)
    raise TimeoutError(f"Polymarket mirror {mirror_id} was not settled")


def _relayer_sources(cluster: Cluster, node_id: str) -> dict:
    path = (
        cluster.base_dir
        / node_id
        / "data"
        / "reth"
        / "relayer_state.json"
    )
    assert path.exists(), f"{node_id} has no relayer state"
    return json.loads(path.read_text())["sources"]


def _assert_quorum_relayer_sources(
    cluster: Cluster, expected_uris: list[str]
) -> dict[str, set[str]]:
    persisted_by_uri = {uri: set() for uri in expected_uris}
    for node_id in cluster.nodes:
        sources = _relayer_sources(cluster, node_id)
        for uri in expected_uris:
            if uri in sources:
                persisted_by_uri[uri].add(node_id)
    for uri, nodes in persisted_by_uri.items():
        assert len(nodes) >= QUORUM_VALIDATORS, (
            f"only {sorted(nodes)} persisted {uri}; "
            f"need {QUORUM_VALIDATORS} independent fetchers"
        )
    return persisted_by_uri


async def _wait_for_relayer_source(
    cluster: Cluster,
    node_id: str,
    uri: str,
    minimum_nonce: int,
) -> dict:
    deadline = time.monotonic() + 90
    while time.monotonic() < deadline:
        try:
            source = _relayer_sources(cluster, node_id).get(uri)
        except (AssertionError, json.JSONDecodeError):
            source = None
        if source is not None and int(source["last_nonce"]) >= minimum_nonce:
            return source
        await asyncio.sleep(1)
    raise TimeoutError(
        f"{node_id} did not persist {uri} at nonce {minimum_nonce}"
    )


@pytest.mark.asyncio
async def test_mixed_binary_bridge_then_governance_activates_live_oracles(
    cluster: Cluster,
):
    metadata = _metadata()
    old_hash = metadata["oldBinarySha256"]
    new_hash = metadata["newBinarySha256"]
    bridge = metadata["bridge"]
    binance = metadata["binance"]
    polymarket = metadata["polymarket"]
    new_binary = Path(os.environ["GRAVITY_NEW_BINARY"]).resolve()

    assert binance["mode"] in {"official-archive", "production-rest"}
    if binance["mode"] == "official-archive":
        assert len(binance["provenance"]) == len(binance["feeds"])
        assert {item["pair"] for item in binance["provenance"]} == {
            feed["pair"] for feed in binance["feeds"]
        }
        assert all(
            len(item["sha256"]) == 64 for item in binance["provenance"]
        )

    assert len(cluster.nodes) == 4
    assert await cluster.set_full_live(timeout=180)
    assert await cluster.check_block_increasing(timeout=60)
    active = await cluster.validator_list()
    assert {node.id for node in active.active} == set(cluster.nodes)
    _assert_binary_matrix(cluster, set(), old_hash, new_hash)

    node1 = cluster.get_node("node1")
    w3 = node1.w3
    required = [
        ("PriceFeedResolver.sol", "PriceFeedResolver"),
        ("PolymarketSettlementResolver.sol", "PolymarketSettlementResolver"),
        ("NativeOracle.sol", "NativeOracle"),
        ("OracleTaskConfig.sol", "OracleTaskConfig"),
    ]
    contracts_out = support.ensure_contracts_out(
        SUITE_DIR, required, "rolling Oracle activation"
    )
    price_artifact = support.load_artifact(
        contracts_out, "PriceFeedResolver.sol", "PriceFeedResolver"
    )
    polymarket_artifact = support.load_artifact(
        contracts_out,
        "PolymarketSettlementResolver.sol",
        "PolymarketSettlementResolver",
    )
    native_artifact = support.load_artifact(
        contracts_out, "NativeOracle.sol", "NativeOracle"
    )
    task_artifact = support.load_artifact(
        contracts_out, "OracleTaskConfig.sol", "OracleTaskConfig"
    )
    native_oracle = w3.eth.contract(
        address=support.NATIVE_ORACLE_ADDRESS,
        abi=native_artifact["abi"],
    )
    task_config = w3.eth.contract(
        address=support.ORACLE_TASK_CONFIG_ADDRESS,
        abi=task_artifact["abi"],
    )

    assert task_config.functions.getSourceTypes().call() == [SOURCE_TYPE_BRIDGE]
    assert (
        native_oracle.functions.getLatestNonce(
            support.SOURCE_TYPE_PRICE_FEED, 1001
        ).call()
        == 0
    )
    assert (
        native_oracle.functions.getLatestNonce(
            SOURCE_TYPE_POLYMARKET, int(polymarket["mirrorId"])
        ).call()
        == 0
    )

    # Old-only and every mixed-binary composition must continue processing the
    # historical sourceType=0 bridge stream.
    await _release_bridge_round(cluster, bridge, 1)
    upgraded = set()
    for target_nonce, node_id in enumerate(cluster.nodes, start=2):
        await _upgrade_node(cluster, node_id, new_binary)
        upgraded.add(node_id)
        _assert_binary_matrix(cluster, upgraded, old_hash, new_hash)
        await _release_bridge_round(cluster, bridge, target_nonce)

    assert upgraded == set(cluster.nodes)
    assert task_config.functions.getSourceTypes().call() == [SOURCE_TYPE_BRIDGE]

    price_resolver = support.deploy_contract(
        w3, price_artifact, support.FAUCET_KEY
    )
    polymarket_resolver = support.deploy_contract(
        w3, polymarket_artifact, support.FAUCET_KEY
    )
    pool_addr = support.pool0_voted_by_faucet(w3)
    mirror_id = int(polymarket["mirrorId"])
    condition_id = bytes.fromhex(polymarket["conditionId"][2:])
    price_uris = {
        int(feed["feedId"]): feed["taskUri"]
        for feed in binance["feeds"]
    }

    setup_epoch = await _wait_for_proposal_window(cluster)
    targets = [
        support.NATIVE_ORACLE_ADDRESS,
        polymarket_resolver.address,
        support.NATIVE_ORACLE_ADDRESS,
    ]
    datas = [
        support.function_calldata(
            native_oracle.functions.setDefaultCallback(
                support.SOURCE_TYPE_PRICE_FEED,
                price_resolver.address,
            )
        ),
        support.function_calldata(
            polymarket_resolver.functions.registerMirror(
                mirror_id,
                POLYGON_CHAIN_ID,
                CTF_ADDRESS,
                condition_id,
                2,
            )
        ),
        support.function_calldata(
            native_oracle.functions.setCallback(
                SOURCE_TYPE_POLYMARKET,
                mirror_id,
                polymarket_resolver.address,
            )
        ),
    ]
    for feed_id, uri in price_uris.items():
        targets.append(support.ORACLE_TASK_CONFIG_ADDRESS)
        datas.append(
            support.function_calldata(
                task_config.functions.setTask(
                    support.SOURCE_TYPE_PRICE_FEED,
                    feed_id,
                    support.TASK_PRICE_FEED,
                    uri.encode(),
                )
            )
        )
    targets.append(support.ORACLE_TASK_CONFIG_ADDRESS)
    datas.append(
        support.function_calldata(
            task_config.functions.setTask(
                SOURCE_TYPE_POLYMARKET,
                mirror_id,
                POLYMARKET_TASK,
                polymarket["taskUri"].encode(),
            )
        )
    )
    receipt = await support.execute_governance_proposal(
        w3,
        pool_addr,
        targets,
        datas,
        "activate-live-source-types-3-and-6-after-rolling-upgrade",
        gas=8_000_000,
    )

    assert _current_epoch(w3) == setup_epoch
    assert set(task_config.functions.getSourceTypes().call()) == {
        SOURCE_TYPE_BRIDGE,
        support.SOURCE_TYPE_PRICE_FEED,
        SOURCE_TYPE_POLYMARKET,
    }
    for feed_id in price_uris:
        assert (
            native_oracle.functions.getLatestNonce(
                support.SOURCE_TYPE_PRICE_FEED, feed_id
            ).call()
            == 0
        )
    assert (
        native_oracle.functions.getLatestNonce(
            SOURCE_TYPE_POLYMARKET, mirror_id
        ).call()
        == 0
    )
    issuers = [
        *(f"gravity://3/{feed_id}" for feed_id in price_uris),
        f"gravity://6/{mirror_id}",
    ]
    for node_id in cluster.nodes:
        for issuer in issuers:
            assert not _log_contains(
                cluster, node_id, "JWKObserver spawned.", issuer
            ), f"{node_id} started {issuer} before the epoch boundary"

    activation_epoch = await _wait_for_epoch_advance(cluster, setup_epoch)
    assert activation_epoch == setup_epoch + 1

    expected_prices = {
        int(feed["feedId"]): await asyncio.to_thread(
            _fetch_binance_price,
            binance["baseUrl"],
            feed["pair"],
            int(binance["bucketStartMs"]),
        )
        for feed in binance["feeds"]
    }
    target_round = int(binance["bucketStartMs"]) // support.INTERVAL_MS
    target_resolved_at = (
        int(binance["bucketStartMs"]) + support.INTERVAL_MS - 1
    )
    price_rounds = {}
    price_progress = {}
    for feed_id, expected_price in expected_prices.items():
        progress, stored = await support.wait_for_latest_price(
            native_oracle,
            price_resolver,
            feed_id,
            1,
            timeout=ORACLE_TIMEOUT_SECONDS,
        )
        progress = tuple(progress)
        stored = tuple(stored)
        assert progress[0] >= 1
        assert progress[1] == target_resolved_at
        assert stored[0]
        assert stored[1] == target_round
        assert stored[2] == target_resolved_at
        assert stored[3] == support.DECIMALS
        assert stored[4] == expected_price
        price_progress[feed_id] = progress
        price_rounds[feed_id] = stored

    settlement = await _wait_for_polymarket_settlement(
        polymarket_resolver, mirror_id, condition_id
    )
    polymarket_progress = tuple(
        await support.wait_for_source_progress(
            native_oracle,
            SOURCE_TYPE_POLYMARKET,
            mirror_id,
            1,
            timeout=ORACLE_TIMEOUT_SECONDS,
        )
    )
    assert polymarket_progress == (1, int(polymarket["blockNumber"]))
    assert settlement[1] == 1
    assert settlement[2] == POLYGON_CHAIN_ID
    assert Web3.to_checksum_address(settlement[3]) == CTF_ADDRESS
    assert Web3.to_hex(settlement[7]).lower() == polymarket[
        "transactionHash"
    ].lower()
    assert settlement[8] == int(polymarket["logIndex"])
    assert polymarket_resolver.functions.getPayoutNumerators(
        mirror_id, condition_id
    ).call() == polymarket["payoutNumerators"]

    await _wait_for_consensus_evidence(cluster, issuers)
    expected_uris = [*price_uris.values(), polymarket["taskUri"]]
    persisted_by_uri = _assert_quorum_relayer_sources(
        cluster, expected_uris
    )

    # sourceType=0 remains live after 3/6 activate.
    await _release_bridge_round(cluster, bridge, int(bridge["eventCount"]))

    for node_id, node in cluster.nodes.items():
        price_replica = node.w3.eth.contract(
            address=price_resolver.address, abi=price_artifact["abi"]
        )
        native_replica = node.w3.eth.contract(
            address=support.NATIVE_ORACLE_ADDRESS,
            abi=native_artifact["abi"],
        )
        for feed_id, expected in price_rounds.items():
            assert tuple(
                price_replica.functions.latestPrice(feed_id).call()
            ) == expected, f"{node_id} price state diverged"
            assert tuple(
                native_replica.functions.getSourceProgress(
                    support.SOURCE_TYPE_PRICE_FEED, feed_id
                ).call()
            ) == price_progress[feed_id], (
                f"{node_id} price progress diverged"
            )

        polymarket_replica = node.w3.eth.contract(
            address=polymarket_resolver.address,
            abi=polymarket_artifact["abi"],
        )
        assert tuple(
            polymarket_replica.functions.getSettlement(
                mirror_id, condition_id
            ).call()
        ) == settlement, f"{node_id} Polymarket state diverged"
        assert tuple(
            native_replica.functions.getSourceProgress(
                SOURCE_TYPE_POLYMARKET, mirror_id
            ).call()
        ) == polymarket_progress, f"{node_id} Polymarket progress diverged"

    lagging_polymarket_nodes = (
        set(cluster.nodes) - persisted_by_uri[polymarket["taskUri"]]
    )
    restart_node_id = (
        sorted(lagging_polymarket_nodes)[0]
        if lagging_polymarket_nodes
        else "node4"
    )
    assert await cluster.get_node(restart_node_id).restart()
    assert await cluster.check_block_increasing(timeout=60)
    _assert_binary_matrix(cluster, set(cluster.nodes), old_hash, new_hash)
    for uri in expected_uris:
        await _wait_for_relayer_source(
            cluster, restart_node_id, uri, minimum_nonce=1
        )

    restarted_price = cluster.get_node(restart_node_id).w3.eth.contract(
        address=price_resolver.address, abi=price_artifact["abi"]
    )
    for feed_id, expected in price_rounds.items():
        assert tuple(
            restarted_price.functions.latestPrice(feed_id).call()
        ) == expected

    LOG.info(
        "Rolling Oracle activation passed: old=%s new=%s epoch=%s "
        "BinanceMode=%s Binance=%s PolymarketMirror=%s governanceBlock=%s",
        old_hash[:12],
        new_hash[:12],
        activation_epoch,
        binance["mode"],
        expected_prices,
        mirror_id,
        receipt["blockNumber"],
    )
