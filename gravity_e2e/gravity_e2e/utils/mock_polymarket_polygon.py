"""
MockPolymarketPolygon - deterministic local Polygon JSON-RPC fixture for oracle E2E tests.

The mock only implements the Polygon methods used by the gravity-reth
Polymarket settlement source:
  - eth_chainId / net_version
  - eth_blockNumber
  - eth_getBlockByNumber("finalized" | "latest" | hex)
  - eth_getLogs

It preloads CTF ConditionResolution logs so the e2e cluster can exercise the
relayer + UnsupportedJWK consensus + NativeOracle path without a real Polygon
RPC endpoint or secrets.
"""

import json
import logging
import threading
import time
from http.server import BaseHTTPRequestHandler, HTTPServer
from typing import Any, Dict, List, Optional

LOG = logging.getLogger(__name__)

POLYGON_CHAIN_ID = 137
CTF_ADDRESS = "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045"
UMA_ORACLE = "0xd91E80cF2E7be2e162c6513ceD06f1dD0dA35296"
MATCH_MARKET_ID = 1_897_398
MATCH_CONDITION_ID = "0x2afe86f96be81a0d89ed776bedbd52d1c75bc47b49e6f0f791ddd009f52faf23"
MATCH_QUESTION_ID = "0x49a5e94a4b5a400dcd720ca1875fcd49ba55c303e43bf091bc175df72f74f501"
MATCH_TX_HASH = "0x97828bf9110f78c07f1ad5cff5415875b67b3fe032e19ee6aa2317355861aab2"
MATCH_BLOCK = 89_222_209
MATCH_LOG_INDEX = 2_077

FED_BINARY_MARKET_ID = 7_202_626
FED_BINARY_CONDITION_ID = "0xfed086f96be81a0d89ed776bedbd52d1c75bc47b49e6f0f791ddd009f52faf23"
FED_BINARY_QUESTION_ID = "0xfed5e94a4b5a400dcd720ca1875fcd49ba55c303e43bf091bc175df72f74f501"
FED_BINARY_TX_HASH = "0xfed28bf9110f78c07f1ad5cff5415875b67b3fe032e19ee6aa2317355861aab2"
FED_BINARY_BLOCK = 89_222_209
FED_BINARY_LOG_INDEX = 2_078

# Backward-compatible aliases used by the original match-market test.
DRAW_MARKET_ID = MATCH_MARKET_ID
DRAW_CONDITION_ID = MATCH_CONDITION_ID
DRAW_QUESTION_ID = MATCH_QUESTION_ID
DRAW_TX_HASH = MATCH_TX_HASH
DRAW_BLOCK = MATCH_BLOCK
DRAW_LOG_INDEX = MATCH_LOG_INDEX

CONDITION_RESOLUTION_TOPIC0 = "0xb44d84d3289691f71497564b85d4233648d9dbae8cbdbb4329f301c3a0185894"


def _to_hex(value: int) -> str:
    return hex(value)


def _pad_topic(value: int) -> str:
    return "0x" + value.to_bytes(32, "big").hex()


def _uint256(value: int) -> bytes:
    return value.to_bytes(32, "big")


def _address_topic(address: str) -> str:
    return "0x" + bytes.fromhex(address.replace("0x", "")).rjust(32, b"\x00").hex()


def _fake_hash(seed: int) -> str:
    return "0x" + seed.to_bytes(32, "big").hex()


def generate_condition_resolution_log(
    block_number: int = MATCH_BLOCK,
    log_index: int = MATCH_LOG_INDEX,
    payout_numerators: Optional[List[int]] = None,
    condition_id: str = MATCH_CONDITION_ID,
    question_id: str = MATCH_QUESTION_ID,
    tx_hash: str = MATCH_TX_HASH,
) -> Dict[str, Any]:
    """Generate a CTF ConditionResolution log for a configured mock market."""
    payouts = payout_numerators or [0, 1, 0]
    data = _uint256(len(payouts)) + _uint256(64) + _uint256(len(payouts))
    data += b"".join(_uint256(payout) for payout in payouts)
    return {
        "address": CTF_ADDRESS.lower(),
        "topics": [
            CONDITION_RESOLUTION_TOPIC0,
            condition_id.lower(),
            _address_topic(UMA_ORACLE).lower(),
            question_id.lower(),
        ],
        "data": "0x" + data.hex(),
        "blockNumber": _to_hex(block_number),
        "blockHash": _fake_hash(block_number + 0x100),
        "transactionHash": tx_hash,
        "transactionIndex": "0x0",
        "logIndex": _to_hex(log_index),
        "removed": False,
    }


class MockPolymarketPolygon:
    def __init__(self, port: int = 8546, chain_id: int = POLYGON_CHAIN_ID):
        self.port = port
        self.chain_id = chain_id
        self.current_block = MATCH_BLOCK - 1
        self._logs: Dict[int, List[Dict[str, Any]]] = {}
        self._server: Optional[HTTPServer] = None
        self._thread: Optional[threading.Thread] = None

    @property
    def rpc_url(self) -> str:
        return f"http://localhost:{self.port}"

    @property
    def is_running(self) -> bool:
        return self._thread is not None and self._thread.is_alive()

    def preload_draw_resolution(self) -> Dict[str, Any]:
        log = self.preload_match_resolution(1, visible=True)
        LOG.info(
            "MockPolymarketPolygon: preloaded Draw settlement at block=%s logIndex=%s",
            MATCH_BLOCK,
            MATCH_LOG_INDEX,
        )
        return log

    def preload_match_resolution(self, winning_slot: int, visible: bool = False) -> Dict[str, Any]:
        if winning_slot < 0 or winning_slot > 2:
            raise ValueError(f"winning_slot must be 0, 1, or 2; got {winning_slot}")
        payouts = [0, 0, 0]
        payouts[winning_slot] = 1
        log = generate_condition_resolution_log(payout_numerators=payouts)
        self._logs[MATCH_BLOCK] = [log]
        self.current_block = MATCH_BLOCK if visible else MATCH_BLOCK - 1
        LOG.info(
            "MockPolymarketPolygon: prepared winning_slot=%s payout=%s visible=%s",
            winning_slot,
            payouts,
            visible,
        )
        return log

    def release_match_resolution(self, winning_slot: int) -> Dict[str, Any]:
        if winning_slot < 0 or winning_slot > 2:
            raise ValueError(f"winning_slot must be 0, 1, or 2; got {winning_slot}")
        payouts = [0, 0, 0]
        payouts[winning_slot] = 1
        log = self.preload_match_resolution(winning_slot, visible=True)
        return {
            "market_id": MATCH_MARKET_ID,
            "condition_id": MATCH_CONDITION_ID,
            "question_id": MATCH_QUESTION_ID,
            "ctf": CTF_ADDRESS,
            "oracle": UMA_ORACLE,
            "tx_hash": MATCH_TX_HASH,
            "block": MATCH_BLOCK,
            "log_index": MATCH_LOG_INDEX,
            "winning_slot": winning_slot,
            "payout_numerators": payouts,
            "source_log": log,
        }

    def preload_binary_resolution(self, winning_slot: int, visible: bool = False) -> Dict[str, Any]:
        if winning_slot < 0 or winning_slot > 1:
            raise ValueError(f"winning_slot must be 0 or 1; got {winning_slot}")
        payouts = [0, 0]
        payouts[winning_slot] = 1
        log = generate_condition_resolution_log(
            block_number=FED_BINARY_BLOCK,
            log_index=FED_BINARY_LOG_INDEX,
            payout_numerators=payouts,
            condition_id=FED_BINARY_CONDITION_ID,
            question_id=FED_BINARY_QUESTION_ID,
            tx_hash=FED_BINARY_TX_HASH,
        )
        self._logs[FED_BINARY_BLOCK] = [log]
        self.current_block = FED_BINARY_BLOCK if visible else FED_BINARY_BLOCK - 1
        LOG.info(
            "MockPolymarketPolygon: prepared binary winning_slot=%s payout=%s visible=%s",
            winning_slot,
            payouts,
            visible,
        )
        return log

    def release_binary_resolution(self, winning_slot: int) -> Dict[str, Any]:
        if winning_slot < 0 or winning_slot > 1:
            raise ValueError(f"winning_slot must be 0 or 1; got {winning_slot}")
        payouts = [0, 0]
        payouts[winning_slot] = 1
        log = self.preload_binary_resolution(winning_slot, visible=True)
        return {
            "market_id": FED_BINARY_MARKET_ID,
            "condition_id": FED_BINARY_CONDITION_ID,
            "question_id": FED_BINARY_QUESTION_ID,
            "ctf": CTF_ADDRESS,
            "oracle": UMA_ORACLE,
            "tx_hash": FED_BINARY_TX_HASH,
            "block": FED_BINARY_BLOCK,
            "log_index": FED_BINARY_LOG_INDEX,
            "winning_slot": winning_slot,
            "payout_numerators": payouts,
            "source_log": log,
        }

    def handle_request(self, body: dict) -> dict:
        method = body.get("method", "")
        params = body.get("params", [])
        req_id = body.get("id", 1)

        try:
            if method == "eth_getBlockByNumber":
                result = self._handle_get_block_by_number(params)
            elif method == "eth_getLogs":
                result = self._handle_get_logs(params)
            elif method == "eth_chainId":
                result = _to_hex(self.chain_id)
            elif method == "net_version":
                result = str(self.chain_id)
            elif method == "eth_blockNumber":
                result = _to_hex(self.current_block)
            elif method == "mock_setWinningSlot":
                winning_slot = params[0] if params else 1
                if isinstance(winning_slot, str):
                    winning_slot = int(winning_slot, 16) if winning_slot.startswith("0x") else int(winning_slot)
                result = self.release_match_resolution(int(winning_slot))
            elif method == "mock_setBinaryWinningSlot":
                winning_slot = params[0] if params else 0
                if isinstance(winning_slot, str):
                    winning_slot = int(winning_slot, 16) if winning_slot.startswith("0x") else int(winning_slot)
                result = self.release_binary_resolution(int(winning_slot))
            else:
                LOG.debug("MockPolymarketPolygon: unsupported method '%s'", method)
                result = None
            return {"jsonrpc": "2.0", "id": req_id, "result": result}
        except Exception as exc:
            LOG.error("MockPolymarketPolygon: error handling %s: %s", method, exc)
            return {
                "jsonrpc": "2.0",
                "id": req_id,
                "error": {"code": -32603, "message": str(exc)},
            }

    def _handle_get_block_by_number(self, params: list) -> Optional[dict]:
        if not params:
            return None

        block_tag = params[0]
        if block_tag in ("finalized", "latest", "safe", "pending"):
            block_num = self.current_block
        elif block_tag == "earliest":
            block_num = 0
        elif isinstance(block_tag, str) and block_tag.startswith("0x"):
            block_num = int(block_tag, 16)
        else:
            block_num = int(block_tag)

        if block_num > self.current_block:
            return None

        return {
            "number": _to_hex(block_num),
            "hash": _fake_hash(block_num + 0x100),
            "parentHash": _fake_hash(block_num + 0x0FF),
            "timestamp": _to_hex(1_700_000_000 + block_num),
            "gasLimit": _to_hex(100_000_000),
            "gasUsed": "0x0",
            "miner": "0x" + "00" * 20,
            "difficulty": "0x0",
            "totalDifficulty": "0x0",
            "size": "0x100",
            "nonce": "0x0000000000000000",
            "extraData": "0x",
            "logsBloom": "0x" + "00" * 256,
            "transactionsRoot": "0x" + "00" * 32,
            "stateRoot": "0x" + "00" * 32,
            "receiptsRoot": "0x" + "00" * 32,
            "sha3Uncles": "0x" + "00" * 32,
            "uncles": [],
            "transactions": [],
            "baseFeePerGas": "0x0",
            "mixHash": "0x" + "00" * 32,
        }

    def _handle_get_logs(self, params: list) -> list:
        if not params:
            return []
        filter_obj = params[0]
        from_block = self._parse_block_tag(filter_obj.get("fromBlock", "0x0"))
        to_block = self._parse_block_tag(filter_obj.get("toBlock", _to_hex(self.current_block)))
        filter_address = filter_obj.get("address", "").lower()
        filter_topics = filter_obj.get("topics", [])

        results = []
        for block_num in range(from_block, to_block + 1):
            for log in self._logs.get(block_num, []):
                if filter_address and log["address"] != filter_address:
                    continue
                if not self._topics_match(log["topics"], filter_topics):
                    continue
                results.append(log)
        return results

    def _parse_block_tag(self, tag) -> int:
        if isinstance(tag, int):
            return tag
        if tag in ("latest", "finalized", "safe", "pending"):
            return self.current_block
        if tag == "earliest":
            return 0
        if isinstance(tag, str) and tag.startswith("0x"):
            return int(tag, 16)
        return int(tag)

    @staticmethod
    def _topics_match(log_topics: list, filter_topics: list) -> bool:
        for idx, filter_topic in enumerate(filter_topics):
            if filter_topic is None:
                continue
            if idx >= len(log_topics):
                return False
            if isinstance(filter_topic, list):
                allowed = [topic.lower() for topic in filter_topic]
                if log_topics[idx].lower() not in allowed:
                    return False
            elif log_topics[idx].lower() != filter_topic.lower():
                return False
        return True

    def start(self) -> None:
        if self.is_running:
            self.stop()

        mock = self

        class Handler(BaseHTTPRequestHandler):
            def do_POST(self):
                content_len = int(self.headers.get("Content-Length", 0))
                body_bytes = self.rfile.read(content_len)
                try:
                    body = json.loads(body_bytes)
                except json.JSONDecodeError:
                    self.send_error(400, "Invalid JSON")
                    return

                if isinstance(body, list):
                    response_body = json.dumps([mock.handle_request(req) for req in body])
                else:
                    response_body = json.dumps(mock.handle_request(body))

                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(response_body.encode())

            def log_message(self, fmt, *args):
                pass

        self._server = HTTPServer(("127.0.0.1", self.port), Handler)
        self.port = self._server.server_port
        self._thread = threading.Thread(target=self._server.serve_forever, daemon=True)
        self._thread.start()

        import socket

        deadline = time.time() + 5
        while time.time() < deadline:
            try:
                with socket.create_connection(("127.0.0.1", self.port), timeout=1):
                    break
            except (ConnectionRefusedError, OSError):
                time.sleep(0.1)

        LOG.info("MockPolymarketPolygon running at %s", self.rpc_url)

    def stop(self) -> None:
        server = self._server
        thread = self._thread
        if server is not None:
            LOG.info("MockPolymarketPolygon: shutting down")
            self._server = None
            self._thread = None
            server.shutdown()
            server.server_close()
            if thread is not None:
                thread.join(timeout=1)
