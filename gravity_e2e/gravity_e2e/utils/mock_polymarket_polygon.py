"""Deterministic localhost Polygon JSON-RPC fixture for oracle tests."""

from collections import Counter
from contextlib import contextmanager
import json
import logging
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

from eth_abi.packed import encode_packed
from eth_utils import keccak

LOG = logging.getLogger(__name__)

POLYGON_CHAIN_ID = 137
MOCK_POLYGON_PORT = 18_548
CTF_ADDRESS = "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045"
ORACLE_ADDRESS = "0xd91E80cF2E7be2e162c6513ceD06f1dD0dA35296"
MIRROR_ID = 9_001
QUESTION_ID = "0x" + "22" * 32
OUTCOME_SLOT_COUNT = 2
PAYOUT_NUMERATORS = [0, 1]
SOURCE_BLOCK = 50_000_000
LOG_INDEX = 17
TX_HASH = "0x" + "aa" * 32

CONDITION_RESOLUTION_TOPIC = "0x" + keccak(
    text="ConditionResolution(bytes32,address,bytes32,uint256,uint256[])"
).hex()


def derive_condition_id(
    oracle: str = ORACLE_ADDRESS,
    question_id: str = QUESTION_ID,
    outcome_slot_count: int = OUTCOME_SLOT_COUNT,
) -> str:
    packed = encode_packed(
        ["address", "bytes32", "uint256"],
        [oracle, bytes.fromhex(question_id.removeprefix("0x")), outcome_slot_count],
    )
    return "0x" + keccak(packed).hex()


CONDITION_ID = derive_condition_id()


def _quantity(value: int) -> str:
    return hex(value)


def _hash(seed: int) -> str:
    return "0x" + seed.to_bytes(32, "big").hex()


def _word(value: int) -> bytes:
    return value.to_bytes(32, "big")


def _address_topic(address: str) -> str:
    return "0x" + bytes.fromhex(address.removeprefix("0x")).rjust(32, b"\x00").hex()


def condition_resolution_log() -> dict:
    data = _word(OUTCOME_SLOT_COUNT) + _word(64) + _word(len(PAYOUT_NUMERATORS))
    data += b"".join(_word(value) for value in PAYOUT_NUMERATORS)
    return {
        "address": CTF_ADDRESS.lower(),
        "topics": [
            CONDITION_RESOLUTION_TOPIC,
            CONDITION_ID,
            _address_topic(ORACLE_ADDRESS),
            QUESTION_ID,
        ],
        "data": "0x" + data.hex(),
        "blockNumber": _quantity(SOURCE_BLOCK),
        "blockHash": _hash(SOURCE_BLOCK + 0x100),
        "transactionHash": TX_HASH,
        "transactionIndex": "0x0",
        "logIndex": _quantity(LOG_INDEX),
        "removed": False,
    }


class MockPolymarketPolygon(ThreadingHTTPServer):
    """Thread-safe JSON-RPC server with one CTF log and a controlled finalized head."""

    allow_reuse_address = True

    def __init__(self, address):
        super().__init__(address, _MockPolygonHandler)
        self.latest_block = SOURCE_BLOCK
        self.finalized_block = SOURCE_BLOCK - 1
        self._log = condition_resolution_log()
        self._request_counts = Counter()
        self._scan_ranges = []
        self._lock = threading.Lock()

    @property
    def rpc_url(self) -> str:
        host, port = self.server_address
        return f"http://{host}:{port}"

    def set_finalized_block(self, block_number: int) -> None:
        if block_number > self.latest_block:
            raise ValueError("finalized block cannot exceed latest block")
        with self._lock:
            self.finalized_block = block_number

    def request_count(self, method: str) -> int:
        with self._lock:
            return self._request_counts[method]

    def scan_ranges(self) -> list[tuple[int, int]]:
        with self._lock:
            return list(self._scan_ranges)

    def handle_rpc(self, request: dict) -> dict:
        method = request.get("method")
        params = request.get("params", [])
        request_id = request.get("id")
        with self._lock:
            self._request_counts[method] += 1

        try:
            if method == "eth_chainId":
                result = _quantity(POLYGON_CHAIN_ID)
            elif method == "net_version":
                result = str(POLYGON_CHAIN_ID)
            elif method == "eth_blockNumber":
                result = _quantity(self.latest_block)
            elif method == "eth_getBlockByNumber":
                result = self._block_by_number(params)
            elif method == "eth_getLogs":
                result = self._logs(params)
            else:
                return {
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "error": {"code": -32601, "message": f"unsupported method: {method}"},
                }
            return {"jsonrpc": "2.0", "id": request_id, "result": result}
        except (IndexError, KeyError, TypeError, ValueError) as error:
            return {
                "jsonrpc": "2.0",
                "id": request_id,
                "error": {"code": -32602, "message": str(error)},
            }

    def _parse_block(self, value) -> int:
        if value == "finalized":
            return self.finalized_block
        if value in ("latest", "safe", "pending"):
            return self.latest_block
        if value == "earliest":
            return 0
        return int(value, 16) if isinstance(value, str) and value.startswith("0x") else int(value)

    def _block_by_number(self, params: list) -> dict | None:
        block_number = self._parse_block(params[0])
        if block_number > self.latest_block:
            return None
        return {
            "number": _quantity(block_number),
            "hash": _hash(block_number + 0x100),
            "parentHash": _hash(block_number + 0xFF),
            "timestamp": _quantity(1_700_000_000 + block_number),
            "gasLimit": _quantity(30_000_000),
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

    def _logs(self, params: list) -> list[dict]:
        query = params[0]
        from_block = self._parse_block(query.get("fromBlock", "earliest"))
        to_block = self._parse_block(query.get("toBlock", "latest"))
        with self._lock:
            self._scan_ranges.append((from_block, to_block))

        if not (from_block <= SOURCE_BLOCK <= to_block):
            return []
        if query.get("address", "").lower() not in ("", CTF_ADDRESS.lower()):
            return []

        filters = query.get("topics", [])
        topics = self._log["topics"]
        for index, expected in enumerate(filters):
            if expected is None:
                continue
            if index >= len(topics):
                return []
            allowed = expected if isinstance(expected, list) else [expected]
            if topics[index].lower() not in {value.lower() for value in allowed}:
                return []
        return [self._log]


class _MockPolygonHandler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def do_POST(self):
        try:
            content_length = int(self.headers.get("content-length", "0"))
            request = json.loads(self.rfile.read(content_length))
            if isinstance(request, list):
                response = [self.server.handle_rpc(item) for item in request]
            else:
                response = self.server.handle_rpc(request)
            payload = json.dumps(response).encode()
            self.send_response(200)
            self.send_header("content-type", "application/json")
            self.send_header("content-length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)
        except (json.JSONDecodeError, ValueError) as error:
            self.send_error(400, str(error))

    def log_message(self, fmt: str, *args):
        LOG.debug("mock Polygon: " + fmt, *args)


@contextmanager
def mock_polymarket_polygon_server(port: int = MOCK_POLYGON_PORT):
    server = MockPolymarketPolygon(("127.0.0.1", port))
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield server
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)
