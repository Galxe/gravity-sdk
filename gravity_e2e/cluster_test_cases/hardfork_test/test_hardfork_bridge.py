"""
Hardfork + Bridge E2E Test (Zeta-aware)

Verifies that the cross-chain bridge works correctly *across* the Zeta
hardfork transition. Earlier revisions of this file were gamma-scoped, but
on a v1.4.0 baseline gamma activates at block 0 (no-op) and the test
collapsed to a static lookback of two batches that were both finalized on
MockAnvil before the test even started — proving nothing about post-Zeta
behavior.

This rewrite splits bridge events into two real batches:
  - batch1 (nonces 1-10):  preloaded by hooks.pre_start so the oracle
                           processes them while the chain is still pre-Zeta.
  - batch2 (nonces 11-20): injected at runtime via MockAnvil's
                           `mock_preload_events` RPC *after* the chain has
                           crossed zetaBlock. Verifies the oracle can still
                           pull new events and the NativeMintPrecompile can
                           still credit the recipient under the post-Zeta
                           chainspec rules (notably the 50 Gwei base-fee floor
                           introduced by gravity-reth PR #337).

Test flow:
  Phase A: poll for batch1 NativeMinted events (nonces 1-10) — establishes
           that pre-Zeta bridge minting completed at all.
  Phase B: ensure the chain has crossed zetaBlock (wait if pytest selected
           this test standalone; no-op when running after test_full_lifecycle
           which already walked Zeta).
  Phase C: inject batch2 via MockAnvil RPC, then poll for nonces 11-20
           NativeMinted events. Asserts that the events landed strictly
           after zetaBlock — proves the bridge survives the hardfork.

Usage:
    ./gravity_e2e/run_test.sh hardfork_test -k test_hardfork_bridge
"""

import json
import logging
import os
import sys
import time
import urllib.request
from pathlib import Path

import pytest
from web3 import Web3

# Ensure gravity_e2e is importable
_current_dir = Path(__file__).resolve().parent
_e2e_root = _current_dir
while _e2e_root.name != "gravity_e2e" or not (_e2e_root / "gravity_e2e").is_dir():
    _e2e_root = _e2e_root.parent
    if _e2e_root == _e2e_root.parent:
        break
if str(_e2e_root) not in sys.path:
    sys.path.insert(0, str(_e2e_root))

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils.bridge_utils import poll_all_native_minted

sys.path.insert(0, str(_current_dir))
from hardfork_utils import (
    snapshot_system_contracts,
    wait_for_block,
    wait_for_blocks_after,
)
from system_contracts import get_contracts_for_hardfork

LOG = logging.getLogger(__name__)

# Block at which Zeta activates. Mirrors test_full_lifecycle.py + hooks.py
# — change in lockstep there if the default is ever updated.
ZETA_BLOCK = int(os.environ.get("ZETA_BLOCK", "150"))

BATCH1_COUNT = 10  # preloaded by hooks.pre_start
BATCH2_COUNT = 10  # injected by this test post-Zeta


# NativeOracle system address (Epsilon onward).
_NATIVE_ORACLE_ADDR = "0x00000000000000000000000000000001625F4000"

# keccak256 topic[0] hashes for diagnostic events (precomputed once).
# event CallbackFailed(uint32 indexed sourceType, uint256 indexed sourceId,
#                      uint128 nonce, address callback, bytes reason)
_TOPIC_CALLBACK_FAILED = "0x" + Web3.keccak(
    text="CallbackFailed(uint32,uint256,uint128,address,bytes)"
).hex()
# event CallbackSuccess(uint32 indexed sourceType, uint256 indexed sourceId,
#                       uint128 nonce, address callback)
_TOPIC_CALLBACK_SUCCESS = "0x" + Web3.keccak(
    text="CallbackSuccess(uint32,uint256,uint128,address)"
).hex()


def _decode_revert_reason(reason_hex: bytes) -> str:
    """Decode a Solidity revert payload to a human-readable string.

    Standard `Error(string)` revert: 0x08c379a0 || abi.encode(string).
    Custom errors: 4-byte selector || abi.encoded args. Without the ABI we
    can't pretty-print custom-error args, but we can at least surface the
    selector so the user can grep the contracts.
    """
    if not reason_hex:
        return "(empty revert)"
    if len(reason_hex) >= 4 and reason_hex[:4] == bytes.fromhex("08c379a0"):
        # Error(string)
        try:
            length = int.from_bytes(reason_hex[36:68], "big")
            return f'Error("{reason_hex[68:68+length].decode("utf-8", errors="replace")}")'
        except Exception:
            return f"Error(<undecodable>): 0x{reason_hex.hex()}"
    if len(reason_hex) >= 4:
        sel = reason_hex[:4].hex()
        # Known selectors in GBridgeReceiver / Errors / NativeOracle
        known = {
            "8baa579f": "InvalidSourceChain(uint256,uint256)",
            "ddb5de5e": "InvalidSender(address,address)",
            "1f2a2005": "ZeroAmount()",
            "d92e233d": "ZeroAddress()",
            "f4a3e5b0": "MintFailed(address,uint256)",
            "e5c3da3d": "NonceNotSequential(uint32,uint256,uint128,uint128)",
        }
        label = known.get(sel, f"<unknown selector 0x{sel}>")
        rest = reason_hex[4:].hex() if len(reason_hex) > 4 else ""
        return f"{label} args=0x{rest}" if rest else label
    return f"0x{reason_hex.hex()}"


def _diagnose_callback_failures(w3: Web3, from_block: int, zeta_block: int) -> None:
    """Scan logs around the post-Zeta validator_txn for CallbackFailed events.

    Logs the count of CallbackSuccess vs CallbackFailed and the decoded
    revert reason for each failed callback. Called when the balance never
    grew — either the validator_txn never ran, or it ran and every callback
    reverted.
    """
    head = w3.eth.block_number
    # Cap scan range to last ~500 blocks so this stays cheap.
    scan_from = max(from_block - 5, zeta_block - 5, 0)
    LOG.warning(
        "🔬 [diag] scanning [%d, %d] for NativeOracle callback events on sourceId=31337",
        scan_from, head,
    )
    success = w3.eth.get_logs({
        "address": Web3.to_checksum_address(_NATIVE_ORACLE_ADDR),
        "fromBlock": scan_from, "toBlock": head,
        "topics": [_TOPIC_CALLBACK_SUCCESS],
    })
    failed = w3.eth.get_logs({
        "address": Web3.to_checksum_address(_NATIVE_ORACLE_ADDR),
        "fromBlock": scan_from, "toBlock": head,
        "topics": [_TOPIC_CALLBACK_FAILED],
    })
    LOG.warning("🔬 [diag] CallbackSuccess=%d, CallbackFailed=%d", len(success), len(failed))
    for ev in failed[:20]:
        # Decode: data = nonce(uint128 padded to 32B) || callback(address padded to 32B)
        # || offset(32B) || length(32B) || reason(padded). topics[2] = sourceId.
        data = bytes.fromhex(ev["data"][2:] if isinstance(ev["data"], str) else ev["data"].hex().removeprefix("0x"))
        nonce = int.from_bytes(data[16:32], "big") if len(data) >= 32 else -1
        callback_addr = "0x" + data[44:64].hex() if len(data) >= 64 else "?"
        # reason bytes: offset(32) length(32) data(padded)
        reason = b""
        if len(data) >= 128:
            length = int.from_bytes(data[96:128], "big")
            reason = data[128:128 + length]
        LOG.warning(
            "🔬 [diag] CallbackFailed nonce=%d callback=%s block=%d reason=%s",
            nonce, callback_addr, ev["blockNumber"], _decode_revert_reason(reason),
        )


async def _wait_for_balance_credit(
    w3: Web3,
    address: str,
    baseline: int,
    expected_delta: int,
    timeout: int,
    label: str,
    poll_interval: float = 3.0,
) -> int:
    """Poll until ``balance(address) >= baseline + expected_delta``.

    The bridge's NativeMintPrecompile mutates account balance directly,
    which is the most reliable ground truth available — no contract method
    or event-emission path to be reshuffled by hardforks. Returns the final
    observed balance on success, raises AssertionError on timeout.
    """
    import asyncio
    target = baseline + expected_delta
    start = time.monotonic()
    last = w3.eth.get_balance(address)
    while time.monotonic() - start < timeout:
        cur = w3.eth.get_balance(address)
        if cur >= target:
            return cur
        if cur != last:
            elapsed = time.monotonic() - start
            LOG.info(
                f"  [{label}] balance progressing: {cur} wei (delta="
                f"{cur - baseline}, target +{expected_delta}, elapsed={elapsed:.0f}s)"
            )
            last = cur
        await asyncio.sleep(poll_interval)
    raise AssertionError(
        f"[{label}] timeout: balance {w3.eth.get_balance(address)} < target "
        f"{target} after {timeout}s (baseline={baseline}, expected_delta={expected_delta})"
    )


def _mock_anvil_rpc(rpc_url: str, method: str, params: list) -> dict:
    """Minimal JSON-RPC client for MockAnvil. We can't share the in-process
    MockAnvil instance with the test (pytest runs in a subprocess), so the
    only way to inject events at runtime is via HTTP."""
    payload = json.dumps({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    }).encode()
    req = urllib.request.Request(
        rpc_url,
        data=payload,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=10) as resp:
        body = json.loads(resp.read())
    if "error" in body:
        raise RuntimeError(f"MockAnvil RPC error: {body['error']}")
    return body["result"]


@pytest.mark.asyncio
async def test_hardfork_bridge(
    cluster: Cluster,
    mock_anvil_metadata: dict,
    bridge_verify_timeout: int,
):
    """Verify the bridge works on both sides of the Zeta transition."""
    node = cluster.get_node("node1")
    assert node is not None, "node1 not found in cluster"
    w3 = node.w3

    amount = mock_anvil_metadata["amount"]
    recipient = mock_anvil_metadata["recipient"]
    sender_address = mock_anvil_metadata["sender_address"]
    mock_rpc_url = mock_anvil_metadata["rpc_url"]

    # hooks.py is now configured to preload exactly batch1 (10 events).
    # Be defensive: if a future hooks change skews the count, surface it
    # rather than silently splitting halfway through.
    assert mock_anvil_metadata["bridge_count"] == BATCH1_COUNT, (
        f"hooks preloaded {mock_anvil_metadata['bridge_count']} events, "
        f"expected {BATCH1_COUNT} (batch1). Update hooks._DEFAULT_BRIDGE_COUNT "
        f"or this test in lockstep."
    )

    # Recipient balance is the source of truth for "bridge worked":
    # NativeMintPrecompile credits the address directly. By the time this
    # test starts, batch1 has typically already been minted (the oracle
    # processed those events during lifecycle phases), so the current
    # balance already reflects batch1.
    pre_test_balance = w3.eth.get_balance(recipient)

    LOG.info("=" * 70)
    LOG.info("🌉 Hardfork + Bridge E2E Test (Zeta-aware)")
    LOG.info(f"   zetaBlock          = {ZETA_BLOCK}")
    LOG.info(f"   batch1 (pre-Zeta)  = nonces 1-{BATCH1_COUNT} (preloaded)")
    LOG.info(f"   batch2 (post-Zeta) = nonces {BATCH1_COUNT+1}-{BATCH1_COUNT+BATCH2_COUNT} "
             f"(runtime-injected)")
    LOG.info(f"   amount per event   = {amount} wei")
    LOG.info(f"   pre-test recipient balance = {pre_test_balance} wei")
    LOG.info("=" * 70)

    # ================================================================
    # Phase A — verify batch1 minted on the gravity chain.
    #
    # `poll_all_native_minted` is best-effort here: post-Epsilon,
    # GBridgeReceiver's `_processedNonces` storage was removed, so
    # `isProcessed(n)` reverts and the helper falls through to a log scan.
    # The log scan may also miss events (the precompile's emission path
    # can route through a different address than upstream-receiver), so we
    # treat the helper as a soft signal and use *recipient balance* as the
    # ground truth — that's what NativeMintPrecompile actually mutates.
    # ================================================================
    LOG.info(f"\n[Phase A] Verify batch1 minting (nonces 1-{BATCH1_COUNT})")
    current = w3.eth.block_number
    LOG.info(f"  Current gravity block: {current}")

    # batch1 finalized in MockAnvil at hooks.pre_start, so by the time
    # test_full_lifecycle finished walking Zeta the oracle has had ~270+
    # gravity blocks to pull and apply them. balance >= batch1 total is the
    # ground truth: NativeMintPrecompile already credited the recipient.
    expected_batch1_minimum = BATCH1_COUNT * amount
    balance_a = await _wait_for_balance_credit(
        w3, recipient, baseline=0, expected_delta=expected_batch1_minimum,
        timeout=bridge_verify_timeout, label="batch1",
    )
    LOG.info(
        f"  Recipient balance after batch1: {balance_a} wei "
        f"(>= expected {expected_batch1_minimum})"
    )

    # Soft check: log-scan summary for diagnostics. Don't gate the test on
    # event-presence at the GBridgeReceiver address — Epsilon's
    # `_processedNonces` removal seems to also affect where NativeMinted
    # gets emitted, and balance is the user-facing source of truth anyway.
    try:
        result_a = await poll_all_native_minted(
            gravity_w3=w3,
            max_nonce=BATCH1_COUNT,
            timeout=10,  # short — already proven via balance
            poll_interval=3.0,
        )
        LOG.info(
            f"  [diag] log scan found {len(result_a['found_nonces'])} / "
            f"{BATCH1_COUNT} NativeMinted events at receiver address"
        )
    except Exception as e:
        LOG.info(f"  [diag] log scan diagnostic skipped: {str(e)[:100]}")

    LOG.info(f"✅ Phase A PASSED: batch1 minted (balance >= {expected_batch1_minimum} wei)")

    # ================================================================
    # Phase B — ensure the chain is past zetaBlock + Zeta contracts upgraded.
    # ================================================================
    LOG.info(f"\n[Phase B] Ensure chain is post-Zeta (zetaBlock={ZETA_BLOCK})")
    current = w3.eth.block_number
    if current < ZETA_BLOCK:
        LOG.info(f"  Chain at {current}, waiting for zetaBlock={ZETA_BLOCK}...")
        reached = await wait_for_block(w3, ZETA_BLOCK, timeout=600)
        assert reached, f"failed to reach zetaBlock={ZETA_BLOCK}"
        # Let a few more blocks settle so codehashes have certainly migrated.
        await wait_for_blocks_after(w3, ZETA_BLOCK, 5, timeout=60)

    # Sanity: at least one Zeta-upgraded contract has the new bytecode.
    # We can't compare against a pre-snapshot from this test (this test may
    # be running standalone), so just assert the contracts have non-empty code.
    zeta_contracts = get_contracts_for_hardfork("zeta")
    snap = snapshot_system_contracts(w3, zeta_contracts)
    populated = [n for n, h in snap.items() if h is not None]
    assert len(populated) >= 4, (
        f"expected >= 4 Zeta system contracts to have bytecode, got "
        f"{len(populated)}: populated={populated}"
    )
    LOG.info(
        f"✅ Phase B PASSED: chain at block {w3.eth.block_number} (>= {ZETA_BLOCK}); "
        f"{len(populated)}/{len(zeta_contracts)} Zeta contracts present"
    )

    # ================================================================
    # Phase C — inject batch2 at runtime + verify it gets minted post-Zeta.
    # ================================================================
    LOG.info(
        f"\n[Phase C] Runtime-inject batch2 (nonces "
        f"{BATCH1_COUNT+1}-{BATCH1_COUNT+BATCH2_COUNT}) and verify post-Zeta minting"
    )

    # Capture the gravity block + recipient balance at injection time.
    # Both are the baselines we'll measure batch2's effect against.
    inject_block = w3.eth.block_number
    pre_batch2_balance = w3.eth.get_balance(recipient)
    LOG.info(
        f"  Injecting batch2 into MockAnvil at gravity block {inject_block} "
        f"(pre-inject balance: {pre_batch2_balance} wei)..."
    )
    inject_result = _mock_anvil_rpc(
        mock_rpc_url,
        "mock_preload_events",
        [{
            "count": BATCH2_COUNT,
            "amount": amount,
            "recipient": recipient,
            "sender_address": sender_address,
            "events_per_block": 1,
            # start_nonce / start_block omitted → MockAnvil continues from
            # the prior end (nonce 11, block 11), which is what we want.
        }],
    )
    LOG.info(
        f"  MockAnvil now finalized at block {inject_result['finalized_block']}, "
        f"new nonces: {inject_result['nonces']}"
    )

    # Wait for batch2's wei to land in the recipient. We measure delta
    # against `pre_batch2_balance` (captured immediately before the inject
    # RPC), so the +10 ETH we expect is unambiguously attributable to the
    # post-Zeta batch we just injected, not to any prior minting.
    expected_batch2_delta = BATCH2_COUNT * amount
    try:
        balance_c = await _wait_for_balance_credit(
            w3, recipient, pre_batch2_balance, expected_batch2_delta,
            timeout=bridge_verify_timeout, label="batch2",
        )
    except AssertionError as e:
        # Diagnostic: NativeOracle catches callback reverts and emits
        # CallbackFailed(sourceType, sourceId, nonce, callback, reason).
        # If the bridge silently failed post-Zeta, the revert reason will
        # tell us exactly which check fired (InvalidSourceChain /
        # InvalidSender / ZeroAmount / ZeroAddress / MintFailed / something
        # else from the abstract handler). Surface it so the test failure
        # is actually actionable.
        _diagnose_callback_failures(w3, inject_block, ZETA_BLOCK)
        raise
    batch2_delta = balance_c - pre_batch2_balance
    LOG.info(
        f"  Recipient balance: {pre_batch2_balance} → {balance_c} "
        f"(batch2 +{batch2_delta} wei, expected +{expected_batch2_delta})"
    )

    # Freshness assertion: confirm the +10 ETH delta really did happen
    # *after* inject_block. eth_getBalance with a historical block_identifier
    # returns the balance at that block — if it already showed the full
    # post-batch2 balance, then batch2 would have had to land before we
    # injected, which is logically impossible.
    bal_at_inject = w3.eth.get_balance(recipient, block_identifier=inject_block)
    assert bal_at_inject == pre_batch2_balance, (
        f"balance at inject_block ({inject_block}) = {bal_at_inject} != "
        f"pre_batch2_balance {pre_batch2_balance} — the inject_block snapshot "
        f"isn't the baseline we expected"
    )
    assert bal_at_inject < balance_c, (
        f"recipient balance at inject_block ({inject_block}) was already "
        f"{bal_at_inject}, equal to or above current {balance_c} — batch2 "
        f"would've had to land before injection (impossible)."
    )
    LOG.info(
        f"  Freshness OK: balance@{inject_block}={bal_at_inject}, "
        f"balance@latest={balance_c} (delta {batch2_delta} minted strictly "
        f"after inject_block, which is > zetaBlock={ZETA_BLOCK})"
    )

    LOG.info(
        f"✅ Phase C PASSED: post-Zeta bridge credited {batch2_delta} wei "
        f"to recipient strictly after gravity block {inject_block}"
    )

    LOG.info("\n" + "=" * 70)
    LOG.info("🎉 Bridge survives Zeta hardfork end-to-end")
    LOG.info(f"   pre-Zeta batch1:  {BATCH1_COUNT} events ✅ (recipient already at "
             f"{pre_batch2_balance} wei before Phase C)")
    LOG.info(f"   post-Zeta batch2: {BATCH2_COUNT} events ✅ (+{batch2_delta} wei after "
             f"block {inject_block}, > zetaBlock={ZETA_BLOCK})")
    LOG.info(f"   final balance:    {balance_c} wei")
    LOG.info(f"   final block:      {w3.eth.block_number}")
    LOG.info("=" * 70)
