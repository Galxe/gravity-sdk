"""Helpers for locating timestamp-activated hardfork boundaries."""

import asyncio
import time

from web3 import Web3


def block_timestamp(w3: Web3, block_identifier="latest") -> int:
    return int(w3.eth.get_block(block_identifier)["timestamp"])


async def wait_for_activation_block(
    node_id: str,
    w3: Web3,
    activation_time: int,
    timeout: int,
    poll_interval: float = 1,
) -> int:
    """Return the first canonical block with timestamp >= activation_time."""
    deadline = time.monotonic() + timeout
    latest_number = 0
    latest_timestamp = 0
    while time.monotonic() < deadline:
        try:
            latest_number = int(w3.eth.block_number)
            latest_timestamp = block_timestamp(w3, latest_number)
            if latest_timestamp >= activation_time:
                break
        except Exception:
            pass
        await asyncio.sleep(poll_interval)
    else:
        raise TimeoutError(
            f"{node_id} did not reach timestamp {activation_time} within "
            f"{timeout}s; latest block={latest_number} timestamp={latest_timestamp}"
        )

    low = 0
    high = latest_number
    while low < high:
        middle = (low + high) // 2
        if block_timestamp(w3, middle) >= activation_time:
            high = middle
        else:
            low = middle + 1

    activation_block = low
    if activation_block == 0:
        raise AssertionError("timestamp hardfork must not activate in genesis")
    before = block_timestamp(w3, activation_block - 1)
    current = block_timestamp(w3, activation_block)
    if not before < activation_time <= current:
        raise AssertionError(
            "invalid timestamp boundary: "
            f"block={activation_block} parent={before} current={current} "
            f"activation={activation_time}"
        )
    return activation_block
