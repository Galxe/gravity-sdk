"""
Unit tests for gravity_e2e.helpers.tx_sender.

The send loop runs against an in-memory fake cluster (no network): every
send is instantly confirmable, so the tests can observe the pause gate and
the counters without timing flakiness beyond generous sleeps.
"""

import asyncio

import pytest
from eth_account import Account

from gravity_e2e.helpers.tx_sender import TxSender, percentile


class FakeEth:
    """The slice of w3.eth the sender touches."""

    def __init__(self):
        self.chain_id = 1337
        self.sent = 0
        # sign_transaction lives on w3.eth.account in web3 v6/v7
        self.account = self

    def get_transaction_count(self, _address, _state):
        return self.sent

    def sign_transaction(self, tx, _key):
        class Signed:
            raw_transaction = b"\x01" + tx["nonce"].to_bytes(4, "big")

        return Signed()

    def send_raw_transaction(self, raw):
        self.sent += 1
        return b"\xaa" * 32

    def get_transaction_receipt(self, _tx_hash):
        return {"status": 1}


class FakeNode:
    def __init__(self):
        self.w3 = type("W3", (), {})()
        self.w3.eth = FakeEth()


class FakeCluster:
    def __init__(self, node_ids):
        self._nodes = {node_id: FakeNode() for node_id in node_ids}

    def get_node(self, node_id):
        return self._nodes[node_id]


class TestPercentile:
    def test_interpolates(self):
        values = [1.0, 2.0, 3.0, 4.0]
        assert percentile(values, 0) == 1.0
        assert percentile(values, 100) == 4.0
        assert percentile(values, 50) == 2.5

    def test_single_value(self):
        assert percentile([7.0], 99) == 7.0

    def test_empty_raises(self):
        with pytest.raises(ValueError):
            percentile([], 50)


class TestTargeting:
    def test_fallback_requires_fallback_id(self):
        sender = TxSender(FakeCluster(["a"]), Account.create(), "a")
        with pytest.raises(ValueError, match="no fallback_node_id"):
            sender.set_fallback(True)

    def test_fallback_switches_target(self):
        sender = TxSender(
            FakeCluster(["a", "b"]), Account.create(), "a", fallback_node_id="b"
        )
        assert sender._current_node_id == "a"
        sender.set_fallback(True)
        assert sender._current_node_id == "b"
        sender.set_fallback(False)
        assert sender._current_node_id == "a"

    def test_success_rate_zero_when_nothing_sent(self):
        sender = TxSender(FakeCluster(["a"]), Account.create(), "a")
        assert sender.success_rate == 0.0


class TestPauseGate:
    @pytest.mark.asyncio
    async def test_pause_stops_sends_and_resume_restarts(self):
        cluster = FakeCluster(["a"])
        sender = TxSender(
            cluster, Account.create(), "a", tx_interval=0.01, receipt_timeout=1.0
        )
        sender.start()
        try:
            deadline = asyncio.get_event_loop().time() + 2.0
            while sender.total_confirmed < 2:
                assert asyncio.get_event_loop().time() < deadline, (
                    "sender never confirmed against the fake"
                )
                await asyncio.sleep(0.01)

            sender.pause()
            await asyncio.sleep(0.05)  # drain the in-flight send
            paused_at = sender.total_sent
            await asyncio.sleep(0.2)
            assert sender.total_sent == paused_at, "sends continued while paused"
            assert sender.total_failed == 0, "pause must not count failures"

            sender.resume()
            deadline = asyncio.get_event_loop().time() + 2.0
            while sender.total_sent <= paused_at:
                assert asyncio.get_event_loop().time() < deadline, (
                    "sender did not resume"
                )
                await asyncio.sleep(0.01)
        finally:
            await sender.stop()

        assert sender.total_confirmed >= 2
        assert sender.success_rate > 0
