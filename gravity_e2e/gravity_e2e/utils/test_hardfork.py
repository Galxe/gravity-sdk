from dataclasses import dataclass

import pytest

from gravity_e2e.utils.hardfork import wait_for_activation_block


@dataclass
class _Eth:
    timestamps: list[int]

    @property
    def block_number(self) -> int:
        return len(self.timestamps) - 1

    def get_block(self, block_identifier):
        number = self.block_number if block_identifier == "latest" else block_identifier
        return {"timestamp": self.timestamps[number]}


@dataclass
class _Web3:
    eth: _Eth


@pytest.mark.asyncio
async def test_wait_for_activation_block_finds_first_timestamp_at_or_after_boundary():
    w3 = _Web3(_Eth([10, 11, 14, 20, 21]))

    assert await wait_for_activation_block("node1", w3, 15, timeout=1) == 3


@pytest.mark.asyncio
async def test_wait_for_activation_block_rejects_genesis_activation():
    w3 = _Web3(_Eth([15, 16]))

    with pytest.raises(AssertionError, match="must not activate in genesis"):
        await wait_for_activation_block("node1", w3, 15, timeout=1)
