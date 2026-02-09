"""
Unit tests for MockAnvil.

Validates:
1. Event payload encoding matches the on-chain format
2. JSON-RPC responses have correct structure
3. eth_getLogs filtering works correctly
4. Payload can be decoded by the same logic as blockchain_source.rs
"""

import json
import pytest
import requests
import time

from gravity_e2e.utils.mock_anvil import (
    MockAnvil,
    encode_bridge_message,
    encode_event_data,
    encode_portal_message,
    generate_message_sent_log,
    DEFAULT_PORTAL_ADDRESS,
    MESSAGE_SENT_TOPIC0,
)


# ============================================================================
# Payload Encoding Tests
# ============================================================================


class TestPayloadEncoding:
    """Test that payload encoding matches the Solidity contracts."""

    SENDER = "0x9fE46736679d2D9a65F0992F2272dE9f3c7fa6e0"
    RECIPIENT = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
    AMOUNT = 1000 * 10**18
    NONCE = 1

    def test_bridge_message_encoding(self):
        """abi.encode(uint256 amount, address recipient) = 64 bytes."""
        msg = encode_bridge_message(self.AMOUNT, self.RECIPIENT)
        assert len(msg) == 64
        # First 32 bytes = amount
        amount_decoded = int.from_bytes(msg[:32], "big")
        assert amount_decoded == self.AMOUNT
        # Last 20 bytes = recipient address
        assert msg[44:64] == bytes.fromhex(self.RECIPIENT[2:].lower())

    def test_portal_message_encoding(self):
        """PortalMessage: sender(20B) || nonce(16B) || message."""
        bridge_msg = encode_bridge_message(self.AMOUNT, self.RECIPIENT)
        portal_msg = encode_portal_message(self.SENDER, self.NONCE, bridge_msg)
        # 20 + 16 + 64 = 100 bytes
        assert len(portal_msg) == 100
        # First 20 bytes = sender
        assert portal_msg[:20] == bytes.fromhex(self.SENDER[2:].lower())
        # Bytes 20-36 = nonce (uint128, big-endian)
        nonce_decoded = int.from_bytes(portal_msg[20:36], "big")
        assert nonce_decoded == self.NONCE
        # Bytes 36+ = bridge message
        assert portal_msg[36:] == bridge_msg

    def test_event_data_encoding(self):
        """ABI-encoded bytes: offset(32B) || length(32B) || data(padded)."""
        payload = b"\x01\x02\x03"
        encoded = encode_event_data(payload)
        # offset = 0x20 (32)
        offset = int.from_bytes(encoded[:32], "big")
        assert offset == 32
        # length at offset
        length = int.from_bytes(encoded[32:64], "big")
        assert length == 3
        # data starts at byte 64
        assert encoded[64:67] == payload
        # Padded to 32 bytes
        assert len(encoded) == 96  # 32 + 32 + 32 (padded)

    def test_event_data_round_trip(self):
        """
        Verify that the ABI-encoded event data can be decoded by the
        same logic used in blockchain_source.rs::decode_abi_bytes().
        """
        bridge_msg = encode_bridge_message(self.AMOUNT, self.RECIPIENT)
        portal_msg = encode_portal_message(self.SENDER, self.NONCE, bridge_msg)
        event_data = encode_event_data(portal_msg)

        # Simulate decode_abi_bytes from blockchain_source.rs
        assert len(event_data) >= 64
        offset = int.from_bytes(event_data[24:32], "big")
        length = int.from_bytes(event_data[offset + 24 : offset + 32], "big")
        data_start = offset + 32
        raw_payload = event_data[data_start : data_start + length]

        assert raw_payload == portal_msg
        assert len(raw_payload) == 100

        # Verify we can extract sender and nonce from raw_payload
        sender = "0x" + raw_payload[:20].hex()
        assert sender.lower() == self.SENDER.lower()
        nonce = int.from_bytes(raw_payload[20:36], "big")
        assert nonce == self.NONCE

    def test_generate_log_structure(self):
        """Verify generated log has all required fields."""
        log = generate_message_sent_log(
            nonce=1,
            block_number=42,
            amount=self.AMOUNT,
            recipient=self.RECIPIENT,
            sender_address=self.SENDER,
        )

        assert log["address"] == DEFAULT_PORTAL_ADDRESS.lower()
        assert len(log["topics"]) == 3
        assert log["topics"][0] == MESSAGE_SENT_TOPIC0
        assert log["blockNumber"] == hex(42)
        assert log["data"].startswith("0x")
        assert log["removed"] is False
        assert "transactionHash" in log
        assert "logIndex" in log

    def test_nonce_topic_encoding(self):
        """Topics must be uint128 → bytes32 (left-padded with zeros)."""
        log = generate_message_sent_log(
            nonce=42,
            block_number=1,
            amount=self.AMOUNT,
            recipient=self.RECIPIENT,
            sender_address=self.SENDER,
        )
        nonce_topic = log["topics"][1]
        # bytes32 = 64 hex chars + "0x" prefix
        assert len(nonce_topic) == 66
        # Left-padded: first 30 bytes should be zero, last 2 bytes = 42
        nonce_val = int(nonce_topic, 16)
        assert nonce_val == 42

    def test_block_number_topic_encoding(self):
        """topics[2] = blockNumber as uint256 → bytes32."""
        log = generate_message_sent_log(
            nonce=1,
            block_number=12345,
            amount=self.AMOUNT,
            recipient=self.RECIPIENT,
            sender_address=self.SENDER,
        )
        block_topic = log["topics"][2]
        block_val = int(block_topic, 16)
        assert block_val == 12345


# ============================================================================
# MockAnvil Server Tests
# ============================================================================


class TestMockAnvilServer:
    """Test MockAnvil as a running JSON-RPC server."""

    @pytest.fixture(autouse=True)
    def server(self):
        """Start/stop MockAnvil for each test."""
        mock = MockAnvil(port=18546)  # Use non-standard port for tests
        mock.start()
        yield mock
        mock.stop()

    def _rpc(self, mock, method, params=None):
        """Make a JSON-RPC call to the mock server."""
        resp = requests.post(
            mock.rpc_url,
            json={
                "jsonrpc": "2.0",
                "method": method,
                "params": params or [],
                "id": 1,
            },
            timeout=5,
        )
        resp.raise_for_status()
        return resp.json()

    def test_chain_id(self, server):
        """eth_chainId should return 31337 (0x7a69)."""
        result = self._rpc(server, "eth_chainId")
        assert int(result["result"], 16) == 31337

    def test_block_number_initially_zero(self, server):
        """Before preloading, current block should be 0."""
        result = self._rpc(server, "eth_blockNumber")
        assert int(result["result"], 16) == 0

    def test_get_block_finalized_zero_lag(self, server):
        """finalized == latest (zero lag)."""
        server.preload_events(
            count=10,
            amount=1000 * 10**18,
            recipient="0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            sender_address="0x9fE46736679d2D9a65F0992F2272dE9f3c7fa6e0",
            events_per_block=1,
        )
        latest = self._rpc(server, "eth_getBlockByNumber", ["latest", False])
        finalized = self._rpc(server, "eth_getBlockByNumber", ["finalized", False])
        assert latest["result"]["number"] == finalized["result"]["number"]
        assert int(latest["result"]["number"], 16) == 10

    def test_preload_one_per_block(self, server):
        """1 event per block → N blocks for N events."""
        sender = "0x9fE46736679d2D9a65F0992F2272dE9f3c7fa6e0"
        recipient = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"

        server.preload_events(
            count=5,
            amount=1000 * 10**18,
            recipient=recipient,
            sender_address=sender,
            events_per_block=1,
        )

        # Should have 5 blocks (1-5), each with 1 event
        assert server.current_block == 5
        for block_num in range(1, 6):
            assert block_num in server._logs
            assert len(server._logs[block_num]) == 1

    def test_get_logs_full_range(self, server):
        """eth_getLogs should return all events in range."""
        sender = "0x9fE46736679d2D9a65F0992F2272dE9f3c7fa6e0"
        recipient = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"

        server.preload_events(
            count=10,
            amount=1000 * 10**18,
            recipient=recipient,
            sender_address=sender,
            events_per_block=1,
        )

        result = self._rpc(
            server,
            "eth_getLogs",
            [
                {
                    "address": DEFAULT_PORTAL_ADDRESS,
                    "topics": [MESSAGE_SENT_TOPIC0],
                    "fromBlock": "0x1",
                    "toBlock": "0xa",
                }
            ],
        )

        logs = result["result"]
        assert len(logs) == 10
        # Verify nonces are sequential
        for i, log in enumerate(logs):
            nonce = int(log["topics"][1], 16)
            assert nonce == i + 1

    def test_get_logs_partial_range(self, server):
        """Filtering by block range should return subset."""
        sender = "0x9fE46736679d2D9a65F0992F2272dE9f3c7fa6e0"
        recipient = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"

        server.preload_events(
            count=20,
            amount=1000 * 10**18,
            recipient=recipient,
            sender_address=sender,
            events_per_block=1,
        )

        # Get only blocks 5-10 (6 events)
        result = self._rpc(
            server,
            "eth_getLogs",
            [
                {
                    "address": DEFAULT_PORTAL_ADDRESS,
                    "topics": [MESSAGE_SENT_TOPIC0],
                    "fromBlock": "0x5",
                    "toBlock": "0xa",
                }
            ],
        )

        logs = result["result"]
        assert len(logs) == 6
        nonces = [int(log["topics"][1], 16) for log in logs]
        assert nonces == [5, 6, 7, 8, 9, 10]

    def test_get_logs_address_filter(self, server):
        """Logs should only match the portal_address."""
        sender = "0x9fE46736679d2D9a65F0992F2272dE9f3c7fa6e0"
        recipient = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"

        server.preload_events(
            count=5,
            amount=1000 * 10**18,
            recipient=recipient,
            sender_address=sender,
            events_per_block=1,
        )

        # Query with wrong address → 0 results
        result = self._rpc(
            server,
            "eth_getLogs",
            [
                {
                    "address": "0x0000000000000000000000000000000000000000",
                    "topics": [MESSAGE_SENT_TOPIC0],
                    "fromBlock": "0x1",
                    "toBlock": "0x5",
                }
            ],
        )
        assert len(result["result"]) == 0

    def test_get_logs_empty_range(self, server):
        """No events before preload → empty result."""
        result = self._rpc(
            server,
            "eth_getLogs",
            [
                {
                    "address": DEFAULT_PORTAL_ADDRESS,
                    "topics": [MESSAGE_SENT_TOPIC0],
                    "fromBlock": "0x1",
                    "toBlock": "0xa",
                }
            ],
        )
        assert len(result["result"]) == 0

    def test_get_block_returns_none_for_future(self, server):
        """Block beyond current_block should return null."""
        result = self._rpc(
            server, "eth_getBlockByNumber", ["0xffffff", False]
        )
        assert result["result"] is None

    def test_batch_request(self, server):
        """Batch JSON-RPC requests should work."""
        server.preload_events(
            count=5,
            amount=1000 * 10**18,
            recipient="0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            sender_address="0x9fE46736679d2D9a65F0992F2272dE9f3c7fa6e0",
            events_per_block=1,
        )

        resp = requests.post(
            server.rpc_url,
            json=[
                {"jsonrpc": "2.0", "method": "eth_chainId", "params": [], "id": 1},
                {"jsonrpc": "2.0", "method": "eth_blockNumber", "params": [], "id": 2},
            ],
            timeout=5,
        )
        results = resp.json()
        assert len(results) == 2
        assert int(results[0]["result"], 16) == 31337
        assert int(results[1]["result"], 16) == 5

    def test_large_preload_performance(self, server):
        """Verify 20K events can be preloaded quickly."""
        t0 = time.time()
        server.preload_events(
            count=20000,
            amount=1000 * 10**18,
            recipient="0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            sender_address="0x9fE46736679d2D9a65F0992F2272dE9f3c7fa6e0",
            events_per_block=1,
        )
        elapsed = time.time() - t0

        assert server.current_block == 20000
        total_logs = sum(len(v) for v in server._logs.values())
        assert total_logs == 20000
        # Should be done in < 30 seconds (usually < 5s)
        assert elapsed < 30, f"Preloading 20K events took {elapsed:.1f}s (too slow)"

    def test_large_get_logs_performance(self, server):
        """Verify eth_getLogs with 20K events in 100-block chunks is fast."""
        server.preload_events(
            count=20000,
            amount=1000 * 10**18,
            recipient="0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            sender_address="0x9fE46736679d2D9a65F0992F2272dE9f3c7fa6e0",
            events_per_block=1,
        )

        # Simulate relayer polling: 100 blocks at a time
        total_found = 0
        t0 = time.time()
        cursor = 0
        while cursor < 20000:
            from_block = cursor + 1
            to_block = min(cursor + 100, 20000)
            result = self._rpc(
                server,
                "eth_getLogs",
                [
                    {
                        "address": DEFAULT_PORTAL_ADDRESS,
                        "topics": [MESSAGE_SENT_TOPIC0],
                        "fromBlock": hex(from_block),
                        "toBlock": hex(to_block),
                    }
                ],
            )
            total_found += len(result["result"])
            cursor = to_block
        elapsed = time.time() - t0

        assert total_found == 20000
        # 200 HTTP requests should complete in < 30 seconds
        assert elapsed < 30, f"200 polls took {elapsed:.1f}s (too slow)"
