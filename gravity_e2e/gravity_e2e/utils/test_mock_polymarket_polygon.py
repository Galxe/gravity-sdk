"""Unit coverage for the deterministic Polygon settlement fixture."""

import json
from urllib.request import ProxyHandler, Request, build_opener

from eth_abi import decode

from gravity_e2e.utils.mock_polymarket_polygon import (
    CONDITION_ID,
    CONDITION_RESOLUTION_TOPIC,
    CTF_ADDRESS,
    LOG_INDEX,
    ORACLE_ADDRESS,
    OUTCOME_SLOT_COUNT,
    PAYOUT_NUMERATORS,
    QUESTION_ID,
    SOURCE_BLOCK,
    derive_condition_id,
    mock_polymarket_polygon_server,
)


def _rpc(url: str, method: str, params: list):
    payload = json.dumps(
        {"jsonrpc": "2.0", "id": 1, "method": method, "params": params}
    ).encode()
    request = Request(url, data=payload, headers={"content-type": "application/json"})
    with build_opener(ProxyHandler({})).open(request, timeout=2) as response:
        body = json.loads(response.read())
    assert "error" not in body, body
    return body["result"]


def test_condition_identity_and_event_encoding_are_canonical():
    assert CONDITION_ID == derive_condition_id()
    assert CONDITION_ID == (
        "0xd874d3b83fa09e192fdda031b6a3b3ec78be60cb82678aa67b23f8fd027c86ae"
    )

    with mock_polymarket_polygon_server(port=0) as server:
        logs = _rpc(
            server.rpc_url,
            "eth_getLogs",
            [
                {
                    "fromBlock": hex(SOURCE_BLOCK),
                    "toBlock": hex(SOURCE_BLOCK),
                    "address": CTF_ADDRESS,
                    "topics": [CONDITION_RESOLUTION_TOPIC, CONDITION_ID],
                }
            ],
        )

    assert len(logs) == 1
    log = logs[0]
    assert log["topics"][1] == CONDITION_ID
    assert log["topics"][2].endswith(ORACLE_ADDRESS.removeprefix("0x").lower())
    assert log["topics"][3] == QUESTION_ID
    assert int(log["logIndex"], 16) == LOG_INDEX
    assert decode(["uint256", "uint256[]"], bytes.fromhex(log["data"][2:])) == (
        OUTCOME_SLOT_COUNT,
        tuple(PAYOUT_NUMERATORS),
    )


def test_finalized_head_is_independent_and_requests_are_recorded():
    with mock_polymarket_polygon_server(port=0) as server:
        finalized = _rpc(
            server.rpc_url,
            "eth_getBlockByNumber",
            ["finalized", False],
        )
        latest = _rpc(server.rpc_url, "eth_getBlockByNumber", ["latest", False])
        assert int(finalized["number"], 16) == SOURCE_BLOCK - 1
        assert int(latest["number"], 16) == SOURCE_BLOCK

        server.set_finalized_block(SOURCE_BLOCK)
        finalized = _rpc(
            server.rpc_url,
            "eth_getBlockByNumber",
            ["finalized", False],
        )
        assert int(finalized["number"], 16) == SOURCE_BLOCK
        assert server.request_count("eth_getBlockByNumber") == 3


def test_log_filter_rejects_wrong_condition_and_tracks_ranges():
    with mock_polymarket_polygon_server(port=0) as server:
        wrong = "0x" + "ff" * 32
        logs = _rpc(
            server.rpc_url,
            "eth_getLogs",
            [
                {
                    "fromBlock": hex(SOURCE_BLOCK - 1),
                    "toBlock": hex(SOURCE_BLOCK),
                    "address": CTF_ADDRESS,
                    "topics": [CONDITION_RESOLUTION_TOPIC, wrong],
                }
            ],
        )

        assert logs == []
        assert server.scan_ranges() == [(SOURCE_BLOCK - 1, SOURCE_BLOCK)]
