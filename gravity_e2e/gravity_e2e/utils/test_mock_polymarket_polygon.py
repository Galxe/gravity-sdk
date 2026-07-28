from gravity_e2e.utils.mock_polymarket_polygon import (
    CONDITION_RESOLUTION_TOPIC0,
    CTF_ADDRESS,
    DYNAMIC_BINARY_BLOCK,
    DYNAMIC_BINARY_CONDITION_ID,
    DYNAMIC_BINARY_LOG_INDEX,
    DYNAMIC_BINARY_QUESTION_ID,
    FED_BINARY_BLOCK,
    FED_BINARY_CONDITION_ID,
    FED_BINARY_LOG_INDEX,
    FED_BINARY_QUESTION_ID,
    MockPolymarketPolygon,
)


def _condition_filter(condition_id: str, to_block: str = "latest") -> dict:
    return {
        "fromBlock": hex(FED_BINARY_BLOCK - 10),
        "toBlock": to_block,
        "address": CTF_ADDRESS,
        "topics": [CONDITION_RESOLUTION_TOPIC0, condition_id],
    }


def test_latest_and_finalized_heads_are_independent():
    mock = MockPolymarketPolygon(port=0)
    mock.set_heads(FED_BINARY_BLOCK, FED_BINARY_BLOCK - 1)

    latest = mock.handle_request(
        {"jsonrpc": "2.0", "id": 1, "method": "eth_getBlockByNumber", "params": ["latest", False]}
    )
    finalized = mock.handle_request(
        {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "eth_getBlockByNumber",
            "params": ["finalized", False],
        }
    )

    assert latest["result"]["number"] == hex(FED_BINARY_BLOCK)
    assert finalized["result"]["number"] == hex(FED_BINARY_BLOCK - 1)


def test_conditions_in_the_same_block_are_appended_and_filtered():
    mock = MockPolymarketPolygon(port=0)
    mock.add_condition(
        condition_id=FED_BINARY_CONDITION_ID,
        question_id=FED_BINARY_QUESTION_ID,
        block_number=FED_BINARY_BLOCK,
        log_index=FED_BINARY_LOG_INDEX,
        payout_numerators=[1, 0],
    )
    mock.add_condition(
        condition_id=DYNAMIC_BINARY_CONDITION_ID,
        question_id=DYNAMIC_BINARY_QUESTION_ID,
        block_number=FED_BINARY_BLOCK,
        log_index=DYNAMIC_BINARY_LOG_INDEX,
        payout_numerators=[0, 1],
    )
    mock.set_heads(FED_BINARY_BLOCK)

    first = mock.handle_request(
        {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getLogs",
            "params": [_condition_filter(FED_BINARY_CONDITION_ID)],
        }
    )
    second = mock.handle_request(
        {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "eth_getLogs",
            "params": [_condition_filter(DYNAMIC_BINARY_CONDITION_ID)],
        }
    )

    assert [log["topics"][1] for log in first["result"]] == [
        FED_BINARY_CONDITION_ID
    ]
    assert [log["topics"][1] for log in second["result"]] == [
        DYNAMIC_BINARY_CONDITION_ID
    ]


def test_generic_control_rpc_adds_condition_and_records_scan_range():
    mock = MockPolymarketPolygon(port=0)
    added = mock.handle_request(
        {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "mock_addCondition",
            "params": [
                {
                    "conditionId": DYNAMIC_BINARY_CONDITION_ID,
                    "questionId": DYNAMIC_BINARY_QUESTION_ID,
                    "blockNumber": hex(DYNAMIC_BINARY_BLOCK),
                    "logIndex": hex(DYNAMIC_BINARY_LOG_INDEX),
                    "payoutNumerators": [0, 1],
                }
            ],
        }
    )
    mock.handle_request(
        {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "mock_setHeads",
            "params": [hex(DYNAMIC_BINARY_BLOCK), hex(DYNAMIC_BINARY_BLOCK)],
        }
    )
    filter_params = _condition_filter(
        DYNAMIC_BINARY_CONDITION_ID, hex(DYNAMIC_BINARY_BLOCK)
    )
    logs = mock.handle_request(
        {
            "jsonrpc": "2.0",
            "id": 3,
            "method": "eth_getLogs",
            "params": [filter_params],
        }
    )

    assert added["result"]["condition_id"] == DYNAMIC_BINARY_CONDITION_ID
    assert len(logs["result"]) == 1
    assert any(
        request["method"] == "eth_getLogs" and request["params"] == [filter_params]
        for request in mock.requests
    )


def test_releasing_existing_binary_fixture_remains_backward_compatible():
    mock = MockPolymarketPolygon(port=0)
    release = mock.release_binary_resolution(1)

    assert mock.latest_block == FED_BINARY_BLOCK
    assert mock.finalized_block == FED_BINARY_BLOCK
    assert release["payout_numerators"] == [0, 1]
    assert len(mock._logs[FED_BINARY_BLOCK]) == 1
