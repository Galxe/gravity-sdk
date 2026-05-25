"""
Bridge Multi-Round Oracle E2E Test

Goal: drive several oracle update rounds (default 5) inside a single epoch,
to validate the JWK-consensus / oracle-relayer pipeline under repeated
in-epoch updates.

Mechanism:
  - hooks.py preloads all events into MockAnvil but holds finalized_block=0.
  - This test waits past the initial genesis-driven epoch reconfig, then
    calls mock_setFinalized(boundary) once per round and polls until every
    nonce up to that boundary is minted on the gravity chain.
  - After every round completes, the validator log is scanned to verify the
    epoch held steady across all `jwk txn epoch N, block_number X, data_len Y`
    entries.
"""

import asyncio
import logging
import time
from pathlib import Path

import pytest
import requests

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils.bridge_utils import poll_all_native_minted

LOG = logging.getLogger(__name__)


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


def _scan_jwk_epochs(validator_log: Path):
    """Return list of (epoch, block_number, data_len) from `jwk txn epoch ...` lines."""
    out = []
    if not validator_log.exists():
        return out
    needle = "jwk txn epoch"
    for line in validator_log.read_text(errors="replace").splitlines():
        idx = line.find(needle)
        if idx < 0:
            continue
        try:
            tail = line[idx:]
            # Format: "jwk txn epoch <e>, block number <b>, data len <d>, ..."
            parts = tail.replace(",", " ").split()
            epoch = int(parts[3])
            block_number = int(parts[6])
            data_len = int(parts[9])
            out.append((epoch, block_number, data_len))
        except (ValueError, IndexError):
            continue
    return out


@pytest.mark.cross_chain
@pytest.mark.bridge
@pytest.mark.asyncio
async def test_bridge_multi_round_same_epoch(
    cluster: Cluster,
    preloaded_bridge: dict,
    bridge_verify_timeout: int,
):
    info = preloaded_bridge
    bridge_count = info["bridge_count"]
    amount = info["amount"]
    recipient = info["recipient"]
    rounds = info["rounds"]
    boundaries = info["round_boundaries"]
    mock_rpc_url = info["rpc_url"]

    assert len(boundaries) == rounds, f"boundaries={boundaries} doesn't match rounds={rounds}"
    assert boundaries[-1] == bridge_count, (
        f"last boundary must equal bridge_count: {boundaries[-1]} vs {bridge_count}"
    )

    LOG.info("Verifying gravity nodes are live...")
    assert await cluster.set_full_live(timeout=120), "Gravity nodes failed to become live"
    assert await cluster.check_block_increasing(timeout=60), "Gravity chain not producing blocks"

    node = cluster.get_node("node1")
    assert node is not None
    gravity_w3 = node.w3

    # Let the initial genesis-driven reconfig (epoch 1 -> 2) fully settle so
    # the first round and every subsequent round all land in epoch 2.
    LOG.info("Waiting 10s for the initial epoch reconfig to settle...")
    await asyncio.sleep(10)

    balance_before = gravity_w3.eth.get_balance(recipient)
    LOG.info(f"Balance before: {balance_before} wei")

    round_results = []
    t_start = time.time()
    for r, boundary in enumerate(boundaries, start=1):
        LOG.info(f"--- round {r}/{rounds} : releasing events up to nonce {boundary} ---")
        new_fin = _mock_set_finalized(mock_rpc_url, boundary)
        LOG.info(f"  mock_setFinalized -> {new_fin}")

        t_round = time.time()
        result = await poll_all_native_minted(
            gravity_w3=gravity_w3,
            max_nonce=boundary,
            timeout=bridge_verify_timeout,
            poll_interval=3.0,
        )
        elapsed = time.time() - t_round
        missing = result["missing_nonces"]
        found = result["found_nonces"]
        LOG.info(
            f"  round {r} settled in {elapsed:.1f}s — found {len(found)}/{boundary}, "
            f"missing={sorted(missing)[:10]}"
        )
        assert len(missing) == 0, (
            f"Round {r}: {len(missing)} missing nonces (boundary={boundary})"
        )
        round_results.append({"round": r, "boundary": boundary, "elapsed_s": elapsed})

    total_elapsed = time.time() - t_start
    LOG.info(f"\n{'=' * 60}")
    LOG.info(f"  Multi-round oracle report — {rounds} round(s), {bridge_count} events total")
    LOG.info(f"{'=' * 60}")
    for rr in round_results:
        LOG.info(f"  round {rr['round']:>2}: nonce -> {rr['boundary']:>4}, took {rr['elapsed_s']:>6.1f}s")
    LOG.info(f"  total: {total_elapsed:.1f}s")
    LOG.info(f"{'=' * 60}")

    # Balance check: full cumulative mint accounted for
    balance_after = gravity_w3.eth.get_balance(recipient)
    expected_total = bridge_count * amount
    LOG.info(
        f"Balance: before={balance_before}, after={balance_after}, "
        f"delta={balance_after - balance_before}, expected_total={expected_total}"
    )
    assert balance_after >= expected_total, (
        f"Balance too low: expected at least {expected_total}, got {balance_after}"
    )

    # Same-epoch invariant: every jwk_txn entry observed during the test must
    # share the same epoch. Tolerate the early epoch 1 entry that fires before
    # we hold off the first round (we sleep 10s before releasing anything).
    base_dir = node._infra_path
    validator_log = base_dir / "consensus_log" / "validator.log"
    jwk_entries = _scan_jwk_epochs(validator_log)
    LOG.info(f"Found {len(jwk_entries)} `jwk txn epoch` entries in validator.log")
    for e in jwk_entries:
        LOG.info(f"  jwk txn -> epoch={e[0]}, block={e[1]}, data_len={e[2]}")

    # Group entries that fall within the test window (after t_start_ts). We
    # don't have direct correlation, so heuristic: pick the last `rounds`
    # entries — those correspond to our rounds.
    test_window = jwk_entries[-rounds:]
    epochs_in_window = {e[0] for e in test_window}
    LOG.info(f"Epochs covering the {rounds} test rounds = {sorted(epochs_in_window)}")
    assert len(test_window) == rounds, (
        f"Expected {rounds} jwk txn entries during the test, observed "
        f"{len(test_window)}: {test_window}"
    )
    assert len(epochs_in_window) == 1, (
        f"Multi-round oracle updates spanned multiple epochs {sorted(epochs_in_window)}; "
        f"expected all in one. Detailed: {test_window}"
    )
    total_data_len = sum(e[2] for e in test_window)
    assert total_data_len == bridge_count, (
        f"Sum of data_len across rounds {total_data_len} != bridge_count {bridge_count}"
    )

    LOG.info(
        f"✓ {rounds} oracle update rounds all settled in epoch "
        f"{next(iter(epochs_in_window))} — bridge_count={bridge_count} verified"
    )
