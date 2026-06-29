import logging

import pytest
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils.transaction_builder import run_sync

LOG = logging.getLogger(__name__)

ZERO_WORD = "0x" + "00" * 32
RANDOMNESS_BY_HEIGHT_PRECOMPILE = "0x00000000000000000000000000000001625f5002"
RANDOMNESS_BY_HEIGHT_RECENT_GAS = 4_000
HELPER_RUNTIME = (
    # calldata[0:32] = height
    "60206000600037"
    # staticcall(gas(), RANDOMNESS_BY_HEIGHT_PRECOMPILE, 0, 32, 64, 64)
    "60406040602060007300000000000000000000000000000001625f50025afa"
    # storage[0] = found
    "604051600055"
    # storage[1] = randomness
    "606051600155"
    # storage[2] = found == 1 && randomness == calldata[32:64]
    "604051600114606051602035141660025500"
)
HELPER_INITCODE = (
    f"60{len(bytes.fromhex(HELPER_RUNTIME)):02x}600c600039"
    f"60{len(bytes.fromhex(HELPER_RUNTIME)):02x}6000f3"
    f"{HELPER_RUNTIME}"
)
CURRENT_HELPER_RUNTIME = (
    # mem[0:32] = block.number
    "43600052"
    # staticcall(gas(), RANDOMNESS_BY_HEIGHT_PRECOMPILE, 0, 32, 64, 64)
    "60406040602060007300000000000000000000000000000001625f50025afa"
    # storage[0] = found
    "604051600055"
    # storage[1] = randomness
    "606051600155"
    # storage[2] = block.number
    "43600255"
)
CURRENT_HELPER_INITCODE = (
    f"60{len(bytes.fromhex(CURRENT_HELPER_RUNTIME)):02x}600c600039"
    f"60{len(bytes.fromhex(CURRENT_HELPER_RUNTIME)):02x}6000f3"
    f"{CURRENT_HELPER_RUNTIME}"
)


def _encode_height(height: int) -> str:
    return "0x" + height.to_bytes(32, byteorder="big").hex()


async def _send_raw_tx(w3: Web3, account, tx):
    tx.setdefault("chainId", await run_sync(lambda: w3.eth.chain_id))
    tx.setdefault("nonce", await run_sync(w3.eth.get_transaction_count, account.address, "pending"))
    tx.setdefault("gasPrice", await run_sync(lambda: w3.eth.gas_price))

    signed = account.sign_transaction(tx)
    raw_tx = getattr(signed, "raw_transaction", None) or getattr(signed, "rawTransaction")
    tx_hash = await run_sync(w3.eth.send_raw_transaction, raw_tx)
    return await run_sync(w3.eth.wait_for_transaction_receipt, tx_hash, 120)


async def _deploy_helper(w3: Web3, account) -> str:
    receipt = await _send_raw_tx(
        w3,
        account,
        {
            "data": "0x" + HELPER_INITCODE,
            "value": 0,
            "gas": 200_000,
        },
    )
    assert receipt["status"] == 1, "Randomness helper deployment failed"
    helper = receipt["contractAddress"]
    assert helper, "Randomness helper deployment returned no contractAddress"
    return Web3.to_checksum_address(helper)


async def _deploy_current_helper(w3: Web3, account) -> str:
    receipt = await _send_raw_tx(
        w3,
        account,
        {
            "data": "0x" + CURRENT_HELPER_INITCODE,
            "value": 0,
            "gas": 200_000,
        },
    )
    assert receipt["status"] == 1, "Current randomness helper deployment failed"
    helper = receipt["contractAddress"]
    assert helper, "Current randomness helper deployment returned no contractAddress"
    return Web3.to_checksum_address(helper)


async def _query_randomness_by_height_tx(
    w3: Web3,
    account,
    helper: str,
    height: int,
    expected_randomness: str,
) -> tuple[int, str, int, dict]:
    receipt = await _send_raw_tx(
        w3,
        account,
        {
            "to": helper,
            "data": _encode_height(height) + expected_randomness.removeprefix("0x"),
            "value": 0,
            "gas": 120_000,
        },
    )
    assert receipt["status"] == 1, f"Randomness helper tx failed for block {height}"

    found_raw = await run_sync(w3.eth.get_storage_at, helper, 0)
    randomness_raw = await run_sync(w3.eth.get_storage_at, helper, 1)
    ok_raw = await run_sync(w3.eth.get_storage_at, helper, 2)
    return (
        int.from_bytes(found_raw, byteorder="big"),
        _normalize_hash(randomness_raw),
        int.from_bytes(ok_raw, byteorder="big"),
        dict(receipt),
    )


async def _query_current_randomness_tx(w3: Web3, account, helper: str) -> tuple[int, str, int, dict]:
    receipt = await _send_raw_tx(
        w3,
        account,
        {
            "to": helper,
            "data": "0x",
            "value": 0,
            "gas": 120_000,
        },
    )
    assert receipt["status"] == 1, "Current randomness helper tx failed"

    found_raw = await run_sync(w3.eth.get_storage_at, helper, 0)
    randomness_raw = await run_sync(w3.eth.get_storage_at, helper, 1)
    block_number_raw = await run_sync(w3.eth.get_storage_at, helper, 2)
    return (
        int.from_bytes(found_raw, byteorder="big"),
        _normalize_hash(randomness_raw),
        int.from_bytes(block_number_raw, byteorder="big"),
        dict(receipt),
    )


def _normalize_hash(value) -> str:
    if isinstance(value, str):
        hex_value = value
    else:
        hex_value = Web3.to_hex(value)

    return "0x" + hex_value.removeprefix("0x").zfill(64).lower()


def _block_randomness(block) -> str:
    value = (
        block.get("mixHash")
        or block.get("mix_hash")
        or block.get("prevRandao")
        or block.get("prev_randao")
    )
    assert value is not None, f"Block {block.get('number')} has no mixHash/prevRandao"
    return _normalize_hash(value)


def _decode_randomness_precompile_output(output) -> tuple[int, str]:
    output_hex = Web3.to_hex(output).removeprefix("0x")
    assert len(output_hex) == 128, f"unexpected precompile output length: 0x{output_hex}"
    return int(output_hex[:64], 16), "0x" + output_hex[64:].lower()


def _iter_call_frames(frame):
    yield frame
    for child in frame.get("calls", []) or []:
        yield from _iter_call_frames(child)


def _parse_quantity(value) -> int:
    if isinstance(value, int):
        return value
    if isinstance(value, str):
        return int(value, 16)
    raise AssertionError(f"unexpected quantity value: {value!r}")


async def _assert_debug_trace_block_has_randomness_call(
    w3: Web3,
    block_number: int,
    tx_hash,
    expected_precompile_gas: int,
):
    response = await run_sync(
        w3.provider.make_request,
        "debug_traceBlockByNumber",
        [
            hex(block_number),
            {
                "tracer": "callTracer",
                "tracerConfig": {"onlyTopLevel": False},
            },
        ],
    )
    assert "error" not in response, f"debug_traceBlockByNumber failed: {response.get('error')}"

    tx_hash_hex = Web3.to_hex(tx_hash).lower()
    traces = response.get("result")
    assert isinstance(traces, list), f"unexpected debug_traceBlockByNumber result: {response}"

    target_trace = None
    for trace in traces:
        result = trace.get("result", trace)
        if (trace.get("txHash") or result.get("txHash") or "").lower() == tx_hash_hex:
            target_trace = result
            break

    # Some reth versions return callTracer block traces in transaction order without txHash.
    if target_trace is None:
        block = await run_sync(w3.eth.get_block, block_number, False)
        tx_hashes = [Web3.to_hex(tx).lower() for tx in block["transactions"]]
        assert tx_hash_hex in tx_hashes, f"tx {tx_hash_hex} not found in block {block_number}"
        trace = traces[tx_hashes.index(tx_hash_hex)]
        target_trace = trace.get("result", trace)

    matched = [
        frame
        for frame in _iter_call_frames(target_trace)
        if (frame.get("to") or "").lower() == RANDOMNESS_BY_HEIGHT_PRECOMPILE.lower()
    ]
    assert matched, (
        "debug_traceBlockByNumber did not replay a call to "
        f"{RANDOMNESS_BY_HEIGHT_PRECOMPILE} for tx {tx_hash_hex} in block {block_number}"
    )
    for frame in matched:
        assert "error" not in frame, (
            f"debug_traceBlockByNumber replayed randomness precompile with error: {frame}"
        )
        assert "gasUsed" in frame, f"randomness precompile trace frame has no gasUsed: {frame}"
        gas_used = _parse_quantity(frame["gasUsed"])
        assert gas_used == expected_precompile_gas, (
            f"debug_traceBlockByNumber replayed randomness precompile with gasUsed={gas_used}, "
            f"expected {expected_precompile_gas}: {frame}"
        )
    LOG.info(
        "debug_traceBlockByNumber block %s tx %s found %s randomness precompile call(s), gas=%s",
        block_number,
        tx_hash_hex,
        len(matched),
        expected_precompile_gas,
    )


@pytest.mark.asyncio
@pytest.mark.randomness
async def test_randomness_by_height_precompile(cluster: Cluster):
    """
    Verify the read-only randomness precompile can query historical block
    randomness by height and returns not-found for a future height.
    """
    assert await cluster.set_full_live(timeout=30), "Cluster failed to become live"

    node = cluster.get_node("node1")
    assert node, "node1 not found in cluster config"
    assert node.w3.is_connected(), "node1 web3 not connected"

    signer = cluster.faucet
    assert signer, "Faucet account is required to deploy randomness helper"
    helper = await _deploy_helper(node.w3, signer)
    LOG.info("Deployed randomness helper at %s", helper)
    current_helper = await _deploy_current_helper(node.w3, signer)
    LOG.info("Deployed current randomness helper at %s", current_helper)

    assert await node.wait_for_block_increase(timeout=30, delta=20), (
        "Node did not produce enough blocks for randomness lookup"
    )

    latest_height = node.get_block_number()
    found, randomness, current_height, receipt = await _query_current_randomness_tx(
        node.w3,
        signer,
        current_helper,
    )
    current_block = node.w3.eth.get_block(current_height, full_transactions=False)
    expected_current_randomness = _block_randomness(current_block)
    LOG.info(
        "Current height %s randomness lookup: found=%s randomness=%s expected=%s",
        current_height,
        found,
        randomness,
        expected_current_randomness,
    )
    assert current_height == receipt["blockNumber"], (
        f"Current helper stored block {current_height}, receipt block {receipt['blockNumber']}"
    )
    assert found == 1, f"Precompile did not find current randomness for block {current_height}"
    assert randomness == expected_current_randomness, (
        f"Current randomness mismatch for block {current_height}: "
        f"precompile={randomness}, header={expected_current_randomness}"
    )
    await _assert_debug_trace_block_has_randomness_call(
        node.w3,
        receipt["blockNumber"],
        receipt["transactionHash"],
        RANDOMNESS_BY_HEIGHT_RECENT_GAS,
    )

    heights = range(max(1, latest_height - 12), max(2, latest_height - 9))
    LOG.info("Testing randomness precompile for heights: %s", list(heights))

    for height in heights:
        block = node.w3.eth.get_block(height, full_transactions=False)
        expected_randomness = _block_randomness(block)

        raw_output = await run_sync(
            node.w3.eth.call,
            {
                "to": RANDOMNESS_BY_HEIGHT_PRECOMPILE,
                "data": _encode_height(height),
            },
        )
        call_found, call_randomness = _decode_randomness_precompile_output(raw_output)
        LOG.info(
            "eth_call height %s randomness lookup: found=%s randomness=%s",
            height,
            call_found,
            call_randomness,
        )
        assert call_found == 1, f"eth_call did not find randomness for block {height}"
        assert call_randomness == expected_randomness, (
            f"eth_call randomness mismatch for block {height}: "
            f"precompile={call_randomness}, header={expected_randomness}"
        )

        found, randomness, ok, receipt = await _query_randomness_by_height_tx(
            node.w3,
            signer,
            helper,
            height,
            expected_randomness,
        )
        LOG.info(
            "Height %s randomness lookup: found=%s randomness=%s expected=%s ok=%s",
            height,
            found,
            randomness,
            expected_randomness,
            ok,
        )

        assert found == 1, f"Precompile did not find randomness for block {height}"
        assert randomness == expected_randomness, (
            f"Randomness mismatch for block {height}: "
            f"precompile={randomness}, header={expected_randomness}"
        )
        assert ok == 1, f"Helper verification failed for block {height}"
        if height == heights.start:
            await _assert_debug_trace_block_has_randomness_call(
                node.w3,
                receipt["blockNumber"],
                receipt["transactionHash"],
                RANDOMNESS_BY_HEIGHT_RECENT_GAS,
            )

    future_height = latest_height + 1_000
    found, randomness, _ok, receipt = await _query_randomness_by_height_tx(
        node.w3,
        signer,
        helper,
        future_height,
        ZERO_WORD,
    )
    LOG.info(
        "Future height %s randomness lookup: found=%s randomness=%s",
        future_height,
        found,
        randomness,
    )

    assert found == 0, f"Future block {future_height} should not be found"
    assert randomness == ZERO_WORD, (
        f"Future block {future_height} should return zero randomness"
    )
    await _assert_debug_trace_block_has_randomness_call(
        node.w3,
        receipt["blockNumber"],
        receipt["transactionHash"],
        RANDOMNESS_BY_HEIGHT_RECENT_GAS,
    )

    raw_future_output = await run_sync(
        node.w3.eth.call,
        {
            "to": RANDOMNESS_BY_HEIGHT_PRECOMPILE,
            "data": _encode_height(future_height),
        },
    )
    call_found, call_randomness = _decode_randomness_precompile_output(raw_future_output)
    LOG.info(
        "eth_call future height %s randomness lookup: found=%s randomness=%s",
        future_height,
        call_found,
        call_randomness,
    )
    assert call_found == 0, f"eth_call future block {future_height} should not be found"
    assert call_randomness == ZERO_WORD, (
        f"eth_call future block {future_height} should return zero randomness"
    )
