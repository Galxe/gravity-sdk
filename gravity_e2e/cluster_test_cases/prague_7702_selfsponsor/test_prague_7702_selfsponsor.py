"""
E2E regression for gravity-audit #677 (CRITICAL, deterministic chain halt):
a self-sponsored EIP-7702 transaction must be processed as an ordinary tx, not
panic the node.

Background
----------
This issue actually halted the testnet on 2026-06-09 (block 1400867 -> 1400868).
A *self-sponsored* EIP-7702 (type-4) transaction — one whose sender is also the
`authority` of an authorization tuple in its own `authorization_list` — bumps
the sender's nonce TWICE during execution (once in `deduct_caller`, once in
`apply_eip7702_auth_list`, because `authority == caller`), so the caller's
post-state nonce is `expect + 2`. The pinned grevm (`v2.2.4` / 26b586c) commit
path hard-asserts `assert_eq!(change.info.nonce, expect + 1)` in
`async_commit.rs` and panics. Because the tx is spec-valid and pool-valid, any
funded EOA can submit it; every validator then panics deterministically on the
ordered block (and re-panics on replay) -> permanent network halt.

The fix exists in a newer grevm rev (>= 3c09e7c, which allows a post-nonce in
`[expect+1, expect+1+self_auth_count]`) but is NOT pinned by gravity-sdk, so the
deployed chain remains exposed even though #677 was closed.

This test attacks the node from OUTSIDE over JSON-RPC exactly as a remote
attacker would, and asserts the CORRECT (post-fix) behavior: the node survives
the tx and keeps producing blocks. It is marked xfail until the grevm bump lands.

Prague (EIP-7702) is enabled for this suite via hooks.py:pre_start.
"""

import asyncio
import logging
import time

import pytest
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster

LOG = logging.getLogger(__name__)

# Any non-zero delegate target; its code is irrelevant to the bug.
DELEGATE_TARGET = Web3.to_checksum_address("0x000000000000000000000000000000000000c0de")

# Panic site for #677 (grevm async_commit nonce assertion).
PANIC_MARKERS = ("async_commit", "assertion `left == right`")


def _grep_panic(cluster: Cluster) -> str | None:
    from pathlib import Path

    base = Path(cluster.config["cluster"]["base_dir"])
    for p in (base / "node1" / "logs").rglob("*"):
        if not p.is_file():
            continue
        try:
            with open(p, "r", errors="ignore") as f:
                for line in f:
                    if any(m in line for m in PANIC_MARKERS):
                        return f"{p}: {line.strip()}"
        except OSError:
            continue
    return None


@pytest.mark.xfail(
    reason=(
        "gravity-audit #677: a self-sponsored EIP-7702 tx makes the caller "
        "post-nonce = expect+2, but the pinned grevm v2.2.4 async_commit "
        "hard-asserts expect+1 and panics -> deterministic chain halt. Remove "
        "this xfail once grevm is bumped to >= 3c09e7c and relocked."
    ),
    strict=False,
)
@pytest.mark.asyncio
async def test_self_sponsored_7702_does_not_halt_node(cluster: Cluster):
    # ---- 1. Node live, Prague active, producing blocks --------------------
    assert await cluster.set_full_live(timeout=60), "Cluster failed to become live"
    node = cluster.get_node("node1")
    assert node, "node1 not found in cluster config"
    assert await cluster.check_block_increasing(timeout=30), "Node not producing blocks"

    faucet = cluster.faucet
    assert faucet, "Faucet account not configured"
    w3 = node.w3
    chain_id = w3.eth.chain_id
    h0 = node.get_block_number()
    LOG.info(f"Node live at height {h0}, chain_id={chain_id}, sender={faucet.address}")

    # ---- 2. Build the self-sponsored EIP-7702 (type-4) transaction --------
    # authority == sender, and authorization.nonce == tx.nonce + 1 (the value the
    # caller nonce holds after deduct_caller), so the auth tuple is APPLIED and
    # bumps the caller nonce a second time (-> post-nonce = nonce + 2).
    nonce = w3.eth.get_transaction_count(faucet.address)
    gas_price = w3.eth.gas_price
    auth = faucet.sign_authorization(
        {"chainId": chain_id, "address": DELEGATE_TARGET, "nonce": nonce + 1}
    )
    tx = {
        "type": 4,
        "chainId": chain_id,
        "nonce": nonce,
        "to": faucet.address,  # self-sponsored
        "value": 0,
        "gas": 200000,
        "maxFeePerGas": int(gas_price * 2),
        "maxPriorityFeePerGas": gas_price,
        "data": b"",
        "authorizationList": [auth],
    }
    signed = faucet.sign_transaction(tx)
    LOG.info(f"Submitting self-sponsored 7702 tx (nonce={nonce}, auth.nonce={nonce+1})...")
    tx_hash = None
    try:
        # Do not wait for a receipt yet: on the buggy build the node panics while
        # executing this tx, so no receipt would ever return.
        tx_hash = w3.eth.send_raw_transaction(signed.raw_transaction)
        LOG.info(f"Submitted: {tx_hash.hex()}")
    except Exception as e:
        # CRITICAL guard against a false negative: if the node REJECTS the tx
        # before execution (e.g. "transaction type not supported" because Prague
        # is not actually active in the genesis the node booted from), then the
        # #677 code path was never reached. In that case the node trivially
        # "survives" — but that proves nothing about the bug. Fail hard so this
        # is fixed in the harness rather than silently passing.
        msg = str(e)
        if "transaction type not supported" in msg or "not supported" in msg:
            pytest.fail(
                "Self-sponsored EIP-7702 tx was REJECTED before execution "
                f"({msg!r}). Prague/EIP-7702 is not active in the genesis the "
                "node booted from, so the #677 code path was never exercised. "
                "This is a harness setup error (see hooks.py:pre_start — the "
                "DEPLOYED <base_dir>/genesis.json must carry pragueTime=0), NOT "
                "evidence that the node is healthy."
            )
        # Any other send error may just be the node already dying mid-execution
        # (the bug reproducing); fall through to the health/panic checks below.
        LOG.warning(f"send raced the node (it may already be dying): {e}")

    # ---- 3. The node must stay healthy ------------------------------------
    LOG.info("Polling block height for ~30s...")
    heights: list[int] = []
    rpc_failures = 0
    deadline = time.time() + 30
    while time.time() < deadline:
        try:
            heights.append(node.get_block_number())
        except Exception:
            rpc_failures += 1
        await asyncio.sleep(2)

    max_height = max(heights) if heights else h0
    process_alive = node.is_running()
    panic_line = _grep_panic(cluster)
    advanced = max_height - h0
    LOG.info(
        f"After 7702 tx: samples={heights}, advanced={advanced}, "
        f"rpc_failures={rpc_failures}, process_alive={process_alive}, panic={panic_line}"
    )

    node_healthy = (
        process_alive
        and rpc_failures == 0
        and advanced >= 1
        and panic_line is None
    )
    assert node_healthy, (
        "Node did not survive a self-sponsored EIP-7702 tx (gravity-audit #677): "
        f"process_alive={process_alive}, rpc_failures={rpc_failures}, "
        f"height_advanced={advanced}, panic_line={panic_line}"
    )

    # The node is alive — but on a build that simply DROPPED the tx (never
    # executed it) the node would also look alive. To prove the #677 code path
    # was actually exercised and handled correctly, require positive evidence of
    # execution: a receipt for the tx AND the sender nonce advanced by exactly 2
    # (one bump in deduct_caller + one in apply_eip7702_auth_list, because
    # authority == caller). Without this, "node survived" is not evidence of a fix.
    assert tx_hash is not None, (
        "Node stayed alive but the 7702 tx was never accepted (no hash). Cannot "
        "conclude #677 is fixed without the tx actually being processed."
    )
    receipt = w3.eth.wait_for_transaction_receipt(tx_hash, timeout=30)
    post_nonce = w3.eth.get_transaction_count(faucet.address)
    LOG.info(
        f"Tx mined in block {receipt.blockNumber}, status={receipt.status}; "
        f"sender nonce {nonce} -> {post_nonce} (expected {nonce + 2})"
    )
    assert post_nonce == nonce + 2, (
        "Self-sponsored EIP-7702 tx did not double-bump the caller nonce "
        f"(got {post_nonce}, expected {nonce + 2}). The #677 code path "
        "(authority == caller applying its own auth tuple) was not exercised, "
        "so a green result here would be a false negative."
    )
    LOG.info(
        "Node remained healthy AND correctly applied the self-sponsored 7702 tx "
        "(nonce +2, no panic) — #677 fixed."
    )
