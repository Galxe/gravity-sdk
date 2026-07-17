"""
Unit tests for gravity_e2e.helpers.governance (H4).

The full proposal lifecycle needs a chain; here we cover the pure pieces
(proposal-id extraction, flag decoding) and the idempotent early return —
an already-enabled flag must produce zero transactions.
"""

import pytest

from gravity_e2e.helpers.governance import (
    PROPOSAL_CREATED_TOPIC,
    SEL_IS_PERMISSIONLESS,
    VALIDATOR_MANAGER,
    enable_permissionless_join,
    extract_proposal_id,
    is_permissionless_join_enabled,
)


def make_receipt(topics_rows):
    return {"logs": [{"topics": topics} for topics in topics_rows]}


class TestExtractProposalId:
    def test_finds_id_in_matching_log(self):
        receipt = make_receipt(
            [
                [b"\x00" * 32],  # unrelated event
                [bytes(PROPOSAL_CREATED_TOPIC), (42).to_bytes(32, "big")],
            ]
        )
        assert extract_proposal_id(receipt) == 42

    def test_none_when_event_absent(self):
        assert extract_proposal_id(make_receipt([[b"\x11" * 32]])) is None
        assert extract_proposal_id({"logs": []}) is None


class FlagOnlyEth:
    """w3.eth that can answer the isPermissionlessJoinEnabled call and
    refuses everything else — proving the idempotent path sends nothing."""

    def __init__(self, enabled: bool):
        self._enabled = enabled

    def call(self, params):
        assert params["to"] == VALIDATOR_MANAGER
        assert params["data"] == SEL_IS_PERMISSIONLESS
        return b"\x00" * 31 + (b"\x01" if self._enabled else b"\x00")

    def __getattr__(self, name):
        raise AssertionError(f"unexpected w3.eth.{name} access")


class FakeW3:
    def __init__(self, enabled: bool):
        self.eth = FlagOnlyEth(enabled)


class TestFlagProbe:
    def test_decodes_flag(self):
        assert is_permissionless_join_enabled(FakeW3(True)) is True
        assert is_permissionless_join_enabled(FakeW3(False)) is False

    @pytest.mark.asyncio
    async def test_enable_is_idempotent_noop_when_already_on(self):
        # FlagOnlyEth raises on any tx-shaped access, so reaching the end
        # proves the early return sent nothing.
        await enable_permissionless_join(
            FakeW3(True), faucet_key="0x" + "11" * 32, faucet_address="0x" + "22" * 20
        )
