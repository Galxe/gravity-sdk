"""
MockAnvil — Lightweight JSON-RPC server for bridge stress tests.

Replaces real Anvil when only eth_getBlockByNumber("finalized"),
eth_getLogs, and eth_chainId are needed. Pre-generates MessageSent
events in memory for high-volume (20K+) bridge testing without
EVM execution overhead.
"""

import json
import logging
import struct
import threading
import time
from http.server import BaseHTTPRequestHandler, HTTPServer
from typing import Any, Dict, List, Optional

LOG = logging.getLogger(__name__)

# --------------------------------------------------------------------------
# Constants matching on-chain contracts
# --------------------------------------------------------------------------

# MessageSent(uint128 indexed nonce, uint256 indexed blockNumber, bytes payload)
MESSAGE_SENT_TOPIC0 = (
    "0x5646e682c7d994bf11f5a2c8addb60d03c83cda3b65025a826346589df43406e"
)

# Deterministic GravityPortal address on Anvil (deployer nonce 1)
DEFAULT_PORTAL_ADDRESS = "0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512"

# Default chain ID for Anvil
ANVIL_CHAIN_ID = 31337


def _to_hex(value: int, byte_width: int = 0) -> str:
    """Convert int to 0x-prefixed hex string."""
    if byte_width:
        return "0x" + value.to_bytes(byte_width, "big").hex()
    return hex(value)


def _pad32(value: int) -> bytes:
    """Left-pad integer to 32 bytes (big-endian)."""
    return value.to_bytes(32, "big")


def _fake_hash(seed: int) -> str:
    """Generate a deterministic fake 32-byte hash from a seed."""
    return "0x" + seed.to_bytes(32, "big").hex()


# --------------------------------------------------------------------------
# Payload encoding — must match GravityPortal + PortalMessage + GBridgeSender
# --------------------------------------------------------------------------


def encode_portal_message(sender: str, nonce: int, message: bytes) -> bytes:
    """
    Encode PortalMessage: sender(20B) || nonce(16B) || message.

    This matches PortalMessage.encodeCalldata() in Solidity.
    """
    sender_bytes = bytes.fromhex(sender.replace("0x", ""))
    assert len(sender_bytes) == 20
    nonce_bytes = nonce.to_bytes(16, "big")
    return sender_bytes + nonce_bytes + message


def encode_bridge_message(amount: int, recipient: str) -> bytes:
    """
    Encode bridge message: abi.encode(uint256 amount, address recipient).

    This matches GBridgeSender._bridgeToGravity():
        bytes memory message = abi.encode(amount, recipient);
    """
    amount_bytes = _pad32(amount)
    # abi.encode pads address to 32 bytes (left-padded with zeros)
    recipient_bytes = bytes.fromhex(recipient.replace("0x", ""))
    assert len(recipient_bytes) == 20
    recipient_padded = b"\x00" * 12 + recipient_bytes
    return amount_bytes + recipient_padded


def encode_event_data(payload: bytes) -> bytes:
    """
    ABI-encode `bytes payload` as Solidity event data.

    Solidity encodes dynamic `bytes` in event data as:
        offset (32B, = 0x20) || length (32B) || data (padded to 32B boundary)

    This matches what the relayer's decode_abi_bytes() expects.
    """
    offset = _pad32(0x20)  # offset to data = 32
    length = _pad32(len(payload))
    # Pad data to 32-byte boundary
    padded_len = ((len(payload) + 31) // 32) * 32
    data_padded = payload + b"\x00" * (padded_len - len(payload))
    return offset + length + data_padded


# --------------------------------------------------------------------------
# Log generation
# --------------------------------------------------------------------------


def generate_message_sent_log(
    nonce: int,
    block_number: int,
    amount: int,
    recipient: str,
    sender_address: str,
    portal_address: str = DEFAULT_PORTAL_ADDRESS,
    log_index: int = 0,
    tx_index: int = 0,
) -> Dict[str, Any]:
    """
    Generate a single MessageSent event log matching the on-chain format.

    The log structure must exactly match what alloy's provider returns:
    - topics[0] = event signature hash
    - topics[1] = nonce (uint128 → bytes32, right-aligned / left-padded)
    - topics[2] = blockNumber (uint256 → bytes32)
    - data = ABI-encoded `bytes payload`
    """
    # Build payload: PortalMessage(sender, nonce, bridgeMessage)
    bridge_message = encode_bridge_message(amount, recipient)
    portal_message = encode_portal_message(sender_address, nonce, bridge_message)
    event_data = encode_event_data(portal_message)

    # Topics
    topic_nonce = "0x" + nonce.to_bytes(32, "big").hex()
    topic_block = "0x" + block_number.to_bytes(32, "big").hex()

    return {
        "address": portal_address.lower(),
        "topics": [
            MESSAGE_SENT_TOPIC0,
            topic_nonce,
            topic_block,
        ],
        "data": "0x" + event_data.hex(),
        "blockNumber": _to_hex(block_number),
        "blockHash": _fake_hash(block_number + 0x100),
        "transactionHash": _fake_hash(nonce + 0x200),
        "transactionIndex": _to_hex(tx_index),
        "logIndex": _to_hex(log_index),
        "removed": False,
    }


# --------------------------------------------------------------------------
# MockAnvil Server
# --------------------------------------------------------------------------


class MockAnvil:
    """
    Lightweight JSON-RPC server that simulates Anvil for bridge stress tests.

    Only implements:
      - eth_getBlockByNumber("finalized" | "latest" | hex_number)
      - eth_getLogs (filter by address + topics + block range)
      - eth_chainId
      - net_version
    """

    def __init__(
        self,
        port: int = 8546,
        portal_address: str = DEFAULT_PORTAL_ADDRESS,
        chain_id: int = ANVIL_CHAIN_ID,
    ):
        self.port = port
        self.portal_address = portal_address.lower()
        self.chain_id = chain_id
        self.current_block: int = 0
        # logs indexed by block_number
        self._logs: Dict[int, List[Dict]] = {}
        self._server: Optional[HTTPServer] = None
        self._thread: Optional[threading.Thread] = None

    @property
    def rpc_url(self) -> str:
        return f"http://localhost:{self.port}"

    @property
    def is_running(self) -> bool:
        return self._thread is not None and self._thread.is_alive()

    # ------------------------------------------------------------------
    # Event pre-generation
    # ------------------------------------------------------------------

    def preload_events(
        self,
        count: int,
        amount: int,
        recipient: str,
        sender_address: str,
        events_per_block: int = 1,
    ) -> List[int]:
        """
        Pre-generate `count` MessageSent events.

        Events are distributed across blocks, `events_per_block` per block,
        starting from block 1. After preloading, finalized_block is set to
        cover all generated blocks (zero lag).

        Args:
            count: Number of bridge events to generate.
            amount: Bridge amount per event (in wei).
            recipient: Recipient address on gravity chain.
            sender_address: Sender address (bridge sender contract).
            events_per_block: Number of events per block (default 1).

        Returns:
            List of nonces [1, 2, ..., count].
        """
        LOG.info(
            f"MockAnvil: preloading {count} MessageSent events "
            f"({events_per_block} per block)..."
        )
        t0 = time.time()

        nonce = 1
        block_number = 1
        log_index_in_block = 0

        while nonce <= count:
            if block_number not in self._logs:
                self._logs[block_number] = []

            log = generate_message_sent_log(
                nonce=nonce,
                block_number=block_number,
                amount=amount,
                recipient=recipient,
                sender_address=sender_address,
                portal_address=self.portal_address,
                log_index=log_index_in_block,
            )
            self._logs[block_number].append(log)

            log_index_in_block += 1
            if log_index_in_block >= events_per_block:
                block_number += 1
                log_index_in_block = 0
            nonce += 1

        # If the last block wasn't fully filled, still count it
        max_block = max(self._logs.keys()) if self._logs else 0
        self.current_block = max_block

        total_events = sum(len(logs) for logs in self._logs.values())
        elapsed = time.time() - t0
        LOG.info(
            f"MockAnvil: preloaded {total_events} events across "
            f"{len(self._logs)} blocks in {elapsed:.2f}s. "
            f"finalized_block={self.current_block}"
        )

        return list(range(1, count + 1))

    # ------------------------------------------------------------------
    # JSON-RPC handlers
    # ------------------------------------------------------------------

    def handle_request(self, body: dict) -> dict:
        """Route a JSON-RPC request to the appropriate handler."""
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
            else:
                LOG.debug(f"MockAnvil: unsupported method '{method}', returning null")
                result = None

            return {"jsonrpc": "2.0", "id": req_id, "result": result}

        except Exception as e:
            LOG.error(f"MockAnvil: error handling {method}: {e}")
            return {
                "jsonrpc": "2.0",
                "id": req_id,
                "error": {"code": -32603, "message": str(e)},
            }

    def _handle_get_block_by_number(self, params: list) -> Optional[dict]:
        """
        Handle eth_getBlockByNumber.

        Supports "finalized", "latest", "earliest", and hex block numbers.
        Zero finalization lag: finalized == latest == current_block.
        """
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

        # Return minimal block structure — relayer only reads header.number
        return {
            "number": _to_hex(block_num),
            "hash": _fake_hash(block_num + 0x100),
            "parentHash": _fake_hash(block_num + 0x0FF),
            "timestamp": _to_hex(1700000000 + block_num),
            "gasLimit": _to_hex(100_000_000),
            "gasUsed": _to_hex(0),
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
        """
        Handle eth_getLogs.

        Supports filtering by:
          - address (exact match)
          - topics (prefix match)
          - fromBlock / toBlock (inclusive range)
        """
        if not params:
            return []

        filter_obj = params[0]

        # Parse block range
        from_block = self._parse_block_tag(filter_obj.get("fromBlock", "0x0"))
        to_block = self._parse_block_tag(
            filter_obj.get("toBlock", _to_hex(self.current_block))
        )

        # Filter address
        filter_address = filter_obj.get("address", "").lower()

        # Filter topics (array or None)
        filter_topics = filter_obj.get("topics", [])

        results = []
        for block_num in range(from_block, to_block + 1):
            block_logs = self._logs.get(block_num, [])
            for log in block_logs:
                if filter_address and log["address"] != filter_address:
                    continue
                if not self._topics_match(log["topics"], filter_topics):
                    continue
                results.append(log)

        return results

    def _parse_block_tag(self, tag) -> int:
        """Parse a block tag to an integer."""
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
        """
        Check if log topics match the filter.

        filter_topics is a positional array: [topic0, topic1, ...].
        A None/null entry matches any value at that position.
        """
        for i, ft in enumerate(filter_topics):
            if ft is None:
                continue
            if i >= len(log_topics):
                return False
            # Support single value or array of values
            if isinstance(ft, list):
                if log_topics[i] not in ft:
                    return False
            elif log_topics[i] != ft:
                return False
        return True

    # ------------------------------------------------------------------
    # HTTP Server lifecycle
    # ------------------------------------------------------------------

    def start(self) -> None:
        """Start the MockAnvil HTTP server in a background thread."""
        if self.is_running:
            LOG.warning("MockAnvil already running, stopping first...")
            self.stop()

        mock = self  # capture for inner class

        class Handler(BaseHTTPRequestHandler):
            def do_POST(self):
                content_len = int(self.headers.get("Content-Length", 0))
                body_bytes = self.rfile.read(content_len)

                try:
                    body = json.loads(body_bytes)
                except json.JSONDecodeError:
                    self.send_error(400, "Invalid JSON")
                    return

                # Handle batch requests
                if isinstance(body, list):
                    responses = [mock.handle_request(req) for req in body]
                    response_body = json.dumps(responses)
                else:
                    response = mock.handle_request(body)
                    response_body = json.dumps(response)

                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(response_body.encode())

            def log_message(self, format, *args):
                # Suppress default access logging to avoid noise
                pass

        self._server = HTTPServer(("127.0.0.1", self.port), Handler)
        self._thread = threading.Thread(target=self._server.serve_forever, daemon=True)
        self._thread.start()

        # Wait for server to be ready
        import socket

        deadline = time.time() + 5
        while time.time() < deadline:
            try:
                with socket.create_connection(("127.0.0.1", self.port), timeout=1):
                    break
            except (ConnectionRefusedError, OSError):
                time.sleep(0.1)

        LOG.info(f"MockAnvil running at {self.rpc_url} (pid={threading.get_ident()})")

    def stop(self) -> None:
        """Stop the MockAnvil server."""
        if self._server is not None:
            LOG.info("MockAnvil: shutting down...")
            self._server.shutdown()
            self._server = None
            self._thread = None
            LOG.info("MockAnvil: stopped")
