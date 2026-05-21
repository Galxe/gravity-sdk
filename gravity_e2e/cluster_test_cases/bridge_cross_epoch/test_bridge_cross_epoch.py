"""
Bridge Cross-Epoch E2E Test

Goal: drive bridge events in epoch N, force an epoch transition, then drive
more bridge events in epoch N+1. Both batches must process successfully and
the validator log must show the corresponding `jwk txn epoch` entries fall
in different epochs.

Mechanism:
  - genesis.toml uses a short epoch_interval (60s) so the chain rolls into a
    new epoch within the test window.
  - hooks.py preloads all events, finalized_block=0.
  - This test:
      1. Waits past the genesis epoch reconfig.
      2. Reads current epoch from validator.log; releases first batch.
      3. Polls until first batch is minted.
      4. Waits for `EpochManager starting new epoch` to advance.
      5. Releases second batch; polls until everything is minted.
      6. Asserts the two `jwk txn epoch` rows fall in different epochs.
"""

import asyncio
import logging
import re
import time
from pathlib import Path

import pytest
import requests

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils.bridge_utils import poll_all_native_minted

LOG = logging.getLogger(__name__)

_EPOCH_LINE_RE = re.compile(r'EpochManager starting new epoch\. \{"epoch":(\d+)\}')
_JWK_TXN_RE = re.compile(r'jwk txn epoch (\d+), block number (\d+), data len (\d+)')


def _mock_set_finalized(rpc_url: str, block: int) -> int:
    resp = requests.post(
        rpc_url,
        json={"jsonrpc": "2.0", "method": "mock_setFinalized", "params": [block], "id": 1},
        timeout=5,
    )
    resp.raise_for_status()
    body = resp.json()
    if "error" in body:
        raise RuntimeError(f"mock_setFinalized failed: {body['error']}")
    return int(body["result"])


def _latest_epoch_in_log(validator_log: Path) -> int:
    """Highest `EpochManager starting new epoch. {"epoch":N}` value seen so far."""
    if not validator_log.exists():
        return 0
    text = validator_log.read_text(errors="replace")
    epochs = [int(m.group(1)) for m in _EPOCH_LINE_RE.finditer(text)]
    return max(epochs) if epochs else 0


def _scan_jwk_txns(validator_log: Path):
    """Return list of (epoch, block_number, data_len)."""
    if not validator_log.exists():
        return []
    return [
        (int(m.group(1)), int(m.group(2)), int(m.group(3)))
        for m in _JWK_TXN_RE.finditer(validator_log.read_text(errors="replace"))
    ]


async def _wait_for_epoch_advance(validator_log: Path, current: int, timeout_s: int) -> int:
    """Block until the validator log records an epoch strictly greater than `current`."""
    deadline = time.time() + timeout_s
    while time.time() < deadline:
        latest = _latest_epoch_in_log(validator_log)
        if latest > current:
            return latest
        await asyncio.sleep(2)
    raise TimeoutError(
        f"Timed out after {timeout_s}s waiting for epoch > {current}; "
        f"latest seen = {_latest_epoch_in_log(validator_log)}"
    )


@pytest.mark.cross_chain
@pytest.mark.bridge
@pytest.mark.asyncio
async def test_bridge_cross_epoch(
    cluster: Cluster,
    preloaded_bridge: dict,
    bridge_verify_timeout: int,
    request,
):
    info = preloaded_bridge
    bridge_count = info["bridge_count"]
    amount = info["amount"]
    recipient = info["recipient"]
    first_batch_count = info["first_batch_count"]
    second_batch_block = info["second_batch_block"]
    mock_rpc_url = info["rpc_url"]
    cross_epoch_wait = int(request.config.getoption("--cross-epoch-wait-timeout"))

    LOG.info("Verifying gravity nodes are live...")
    assert await cluster.set_full_live(timeout=120), "Gravity nodes failed to become live"
    assert await cluster.check_block_increasing(timeout=60), "Gravity chain not producing blocks"

    node = cluster.get_node("node1")
    assert node is not None
    gravity_w3 = node.w3

    validator_log = node._infra_path / "consensus_log" / "validator.log"
    LOG.info(f"Validator log: {validator_log}")

    # Let the genesis reconfig (epoch 1 -> 2) settle.
    LOG.info("Waiting 10s for initial epoch reconfig to settle...")
    await asyncio.sleep(10)
    base_epoch = _latest_epoch_in_log(validator_log)
    LOG.info(f"Base epoch before first batch = {base_epoch}")

    balance_before = gravity_w3.eth.get_balance(recipient)
    LOG.info(f"Balance before: {balance_before} wei")

    # Phase 1: release first batch inside `base_epoch`.
    LOG.info(
        f"Phase 1: releasing first batch ({first_batch_count} events) "
        f"-> mock_setFinalized({first_batch_count})"
    )
    _mock_set_finalized(mock_rpc_url, first_batch_count)

    t0 = time.time()
    first_result = await poll_all_native_minted(
        gravity_w3=gravity_w3,
        max_nonce=first_batch_count,
        timeout=bridge_verify_timeout,
        poll_interval=3.0,
    )
    first_elapsed = time.time() - t0
    first_missing = first_result["missing_nonces"]
    LOG.info(
        f"  first batch settled in {first_elapsed:.1f}s — found "
        f"{len(first_result['found_nonces'])}/{first_batch_count}, missing={sorted(first_missing)[:10]}"
    )
    assert len(first_missing) == 0, f"First batch missing {len(first_missing)} nonces"
    epoch_after_first = _latest_epoch_in_log(validator_log)
    LOG.info(f"Epoch after first batch = {epoch_after_first}")

    # Phase 2: wait for epoch to advance before releasing the second batch.
    LOG.info(
        f"Phase 2: waiting up to {cross_epoch_wait}s for epoch to advance past "
        f"{epoch_after_first}..."
    )
    next_epoch = await _wait_for_epoch_advance(
        validator_log, epoch_after_first, cross_epoch_wait
    )
    LOG.info(f"  epoch advanced -> {next_epoch}")

    # Phase 3: release second batch inside the new epoch.
    second_batch_size = bridge_count - first_batch_count
    LOG.info(
        f"Phase 3: releasing second batch ({second_batch_size} events) "
        f"-> mock_setFinalized({second_batch_block})"
    )
    _mock_set_finalized(mock_rpc_url, second_batch_block)

    t1 = time.time()
    final_result = await poll_all_native_minted(
        gravity_w3=gravity_w3,
        max_nonce=bridge_count,
        timeout=bridge_verify_timeout,
        poll_interval=3.0,
    )
    second_elapsed = time.time() - t1
    final_missing = final_result["missing_nonces"]
    LOG.info(
        f"  second batch settled in {second_elapsed:.1f}s — total found "
        f"{len(final_result['found_nonces'])}/{bridge_count}, missing={sorted(final_missing)[:10]}"
    )
    assert len(final_missing) == 0, f"Second batch missing {len(final_missing)} nonces"

    # Balance check
    balance_after = gravity_w3.eth.get_balance(recipient)
    expected_total = bridge_count * amount
    LOG.info(
        f"Balance: before={balance_before}, after={balance_after}, "
        f"delta={balance_after - balance_before}, expected_total={expected_total}"
    )
    assert balance_after >= expected_total, (
        f"Balance too low: expected at least {expected_total}, got {balance_after}"
    )

    # Validate that the two jwk_txn rows from this test fall in DIFFERENT epochs.
    txns = _scan_jwk_txns(validator_log)
    LOG.info(f"Found {len(txns)} `jwk txn epoch` rows in validator.log:")
    for t in txns:
        LOG.info(f"  jwk txn -> epoch={t[0]}, block={t[1]}, data_len={t[2]}")

    test_window = txns[-2:]  # the two we just drove
    assert len(test_window) >= 2, f"Expected at least 2 jwk txn rows, got {len(test_window)}"
    e1, e2 = test_window[0][0], test_window[1][0]
    assert e2 > e1, (
        f"Cross-epoch invariant failed: both jwk txns landed in same epoch "
        f"(first={test_window[0]}, second={test_window[1]})"
    )

    sum_data_len = sum(t[2] for t in test_window)
    assert sum_data_len == bridge_count, (
        f"jwk txn data_len sum mismatch: {sum_data_len} != {bridge_count} "
        f"(entries: {test_window})"
    )

    LOG.info(
        f"✓ Cross-epoch bridge verified: first batch jwk_txn in epoch {e1}, "
        f"second batch in epoch {e2}. total {bridge_count} events minted."
    )
