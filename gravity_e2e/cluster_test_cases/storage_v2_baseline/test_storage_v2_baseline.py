"""
storage_v2_baseline — fresh-node storage layout baseline (storage-v2 TC1).

Asserts explicitly what today is only implicit: a FRESH greth v2.3.0 node
initializes with the LEGACY changeset layout (changesets in database
tables, none in static files) and persists a gravity_storage_settings
Metadata entry saying so. If a future binary flips the default
(StorageSettings::current() -> static files), this case fails loudly
instead of the e2e suite drifting silently. It is also the first
real-binary exercise of the two storage-v2 helpers (storage_anchors /
offline_db) and the control group for the later migration cases (TC2+).

Flow (one test; every later step depends on the previous state):
1. cluster up (runner-managed), block production verified;
2. build history: transfers + AnchorTarget deploy + storage writes with
   events — produces non-empty changesets and anchorable facts;
3. H1: collect anchors covering all six kinds, persist to JSON;
4. graceful stop (SIGTERM via the node's stop.sh);
5. H2 offline (also the real-binary smoke for the db get / stats /
   list --count read-only command surface; migrate-changesets belongs to
   TC4 and is NOT run here):
   settings PRESENT_LEGACY, no changeset segments/sidecars on disk,
   changeset tables non-empty, db stats parseable;
6. restart the node, wait until it serves past the anchored history;
7. H1: replay the anchor set, everything must match.

All timeouts are case-internal (no pytest-timeout, repo convention).
"""

import asyncio
import json
import logging
import time
from pathlib import Path

import pytest
from eth_account import Account
from web3 import Web3

from gravity_e2e.cluster.manager import Cluster, NodeState
from gravity_e2e.helpers import storage_case_lib as lib
from gravity_e2e.helpers.node_process import read_node_pid, wait_for_process_exit
from gravity_e2e.helpers.offline_db import (
    ACCOUNT_CHANGESETS_TABLE,
    STORAGE_CHANGESETS_TABLE,
    SettingsState,
    count_table_entries,
    db_stats,
    inspect_changeset_static_files,
    read_storage_settings,
)
from gravity_e2e.helpers.storage_anchors import (
    AnchorSet,
    collect_anchors,
    replay_anchors,
)
from gravity_e2e.utils.transaction_builder import TransactionBuilder

LOG = logging.getLogger(__name__)

CASE_DIR = Path(__file__).resolve().parent

# Case-internal timeouts (seconds).
FULL_LIVE_TIMEOUT_S = 120
BLOCK_PROGRESS_TIMEOUT_S = 60
TX_BLOCK_GAP_TIMEOUT_S = 30
STOP_TIMEOUT_S = 90
RESTART_TIMEOUT_S = 180
CATCHUP_TIMEOUT_S = 120

# History parameters. Distinct per-step values so historical reads at
# different blocks must return different answers.
TRANSFER_AMOUNTS_ETH = (1, 2)
SET_VALUES = (1, 2)


def _load_anchor_target():
    artifact = json.loads((CASE_DIR / "contracts" / "AnchorTarget.json").read_text())
    return artifact["abi"], artifact["bytecode"]


async def _confirmed_tx_point(node, result, label: str) -> lib.TxPoint:
    """Turn a successful TransactionResult into a TxPoint, then advance one
    block so the next transaction lands strictly later (distinct historical
    sampling points)."""
    assert result.success, f"{label} failed: {result.error}"
    assert result.block_number is not None, f"{label} has no inclusion block"
    point = lib.TxPoint(tx_hash=result.tx_hash, block_number=result.block_number)
    LOG.info("%s confirmed: tx=%s block=%d", label, point.tx_hash, point.block_number)
    assert await node.wait_for_block_increase(
        timeout=TX_BLOCK_GAP_TIMEOUT_S, delta=1
    ), f"no block progress after {label}"
    return point


async def _build_history(node, faucet) -> lib.OnChainHistory:
    """Step 2: transfers + contract deploy + storage writes with events."""
    tb = TransactionBuilder(node.w3, faucet)
    recipient = Account.create()
    LOG.info("History recipient: %s", recipient.address)

    transfers = []
    for amount_eth in TRANSFER_AMOUNTS_ETH:
        result = await tb.send_ether(
            recipient.address, Web3.to_wei(amount_eth, "ether")
        )
        transfers.append(
            await _confirmed_tx_point(node, result, f"transfer {amount_eth} ETH")
        )

    abi, bytecode = _load_anchor_target()
    deploy_result = await tb.deploy_contract(bytecode=bytecode, abi=abi)
    deploy = await _confirmed_tx_point(node, deploy_result, "AnchorTarget deploy")
    contract_addr = deploy_result.tx_receipt["contractAddress"]
    assert contract_addr, "deployment receipt has no contractAddress"
    LOG.info("AnchorTarget deployed at %s", contract_addr)

    sets = []
    for value in SET_VALUES:
        result = await tb.build_and_send_tx(
            to=contract_addr, data=lib.encode_set_call(value)
        )
        sets.append(await _confirmed_tx_point(node, result, f"set({value})"))

    return lib.OnChainHistory(
        faucet=faucet.address,
        recipient=recipient.address,
        contract=contract_addr,
        transfers=transfers,
        deploy=deploy,
        sets=sets,
    )


def _assert_history_is_anchorable(anchors: AnchorSet, history: lib.OnChainHistory):
    """Positive controls on the collected expected values: the history must
    have produced real, distinguishable facts — otherwise the replay in
    step 7 would 'pass' on trivially empty anchors."""
    by_kind = {}
    for anchor in anchors.anchors:
        by_kind.setdefault(anchor.kind, []).append(anchor)

    for kind in ("balance", "storage", "transaction", "receipt", "logs", "block_hash"):
        assert by_kind.get(kind), f"no {kind} anchors collected"

    # Storage history: slot 0 at each set() block holds that set's value.
    for point, value in zip(history.sets, SET_VALUES):
        anchor = next(
            a
            for a in by_kind["storage"]
            if a.params["block_number"] == point.block_number
        )
        got = int(anchor.expected, 16)
        assert got == value, (
            f"slot 0 at block {point.block_number}: expected {value}, "
            f"collected {got}"
        )

    # Balance history: recipient accrues the transfers cumulatively.
    cumulative = 0
    for point, amount_eth in zip(history.transfers, TRANSFER_AMOUNTS_ETH):
        cumulative += Web3.to_wei(amount_eth, "ether")
        anchor = next(
            a
            for a in by_kind["balance"]
            if a.params["address"] == history.recipient.lower()
            and a.params["block_number"] == point.block_number
        )
        assert anchor.expected == cumulative, (
            f"recipient balance at block {point.block_number}: expected "
            f"{cumulative}, collected {anchor.expected}"
        )

    # Log history: one ValueSet event per set() call inside the range.
    (logs_anchor,) = by_kind["logs"]
    assert len(logs_anchor.expected) == len(history.sets), (
        f"expected {len(history.sets)} ValueSet logs in "
        f"{logs_anchor.anchor_id}, collected {len(logs_anchor.expected)}"
    )


async def _wait_until_height(node, target: int, timeout: float) -> int:
    """Wait until the node serves a head >= target; returns the head."""
    deadline = time.monotonic() + timeout
    head = -1
    while time.monotonic() < deadline:
        try:
            head = node.w3.eth.block_number
            if head >= target:
                return head
        except Exception as exc:  # RPC may flap right after restart
            LOG.debug("height poll failed: %s", exc)
        await asyncio.sleep(1)
    raise AssertionError(
        f"node {node.id} did not reach height {target} within {timeout}s "
        f"(last seen {head})"
    )


@pytest.mark.asyncio
async def test_storage_v2_baseline(cluster: Cluster, output_dir: Path):
    node = cluster.get_node("node1")
    assert node, "node1 not found in cluster config"

    # ── Step 1: cluster live, block production verified ──
    LOG.info("[Step 1] Waiting for cluster to be fully live...")
    assert await cluster.set_full_live(
        timeout=FULL_LIVE_TIMEOUT_S
    ), "cluster failed to become fully live"
    assert await node.wait_for_block_increase(
        timeout=BLOCK_PROGRESS_TIMEOUT_S, delta=2
    ), "block production not progressing"

    faucet = cluster.faucet
    assert faucet, "faucet account not configured (genesis.faucet)"

    # ── Step 2: build anchorable history ──
    LOG.info("[Step 2] Building on-chain history...")
    history = await _build_history(node, faucet)
    max_history_block = max(
        p.block_number for p in (*history.transfers, history.deploy, *history.sets)
    )

    # ── Step 3: H1 collection ──
    LOG.info("[Step 3] Collecting storage anchors...")
    # Balance/slot anchors must sample HISTORICAL blocks: make sure head is
    # strictly past everything the history touched.
    head = await _wait_until_height(
        node, max_history_block + 1, BLOCK_PROGRESS_TIMEOUT_S
    )
    spec = lib.build_anchor_spec(history)
    anchors = collect_anchors(
        node.w3,
        spec,
        meta={
            "case": "storage_v2_baseline",
            "node": node.id,
            "head_at_collection": head,
            "max_history_block": max_history_block,
        },
    )
    _assert_history_is_anchorable(anchors, history)
    anchors_path = output_dir / "storage_v2_baseline" / "anchors.json"
    anchors.save(anchors_path)
    LOG.info("[Step 3] %d anchors saved to %s", len(anchors), anchors_path)

    # ── Step 4: graceful stop ──
    LOG.info("[Step 4] Stopping node gracefully...")
    node_pid = read_node_pid(node)
    assert node_pid, f"cannot read node PID from {node.pid_file} before stop"
    assert await cluster.set_node(
        node.id, NodeState.STOPPED, timeout=STOP_TIMEOUT_S
    ), "node did not stop gracefully"
    # STOPPED is PID-file based; wait for the actual process exit so the
    # offline db commands below never race the node's RocksDB lock (see
    # gravity_e2e.helpers.node_process for the RocksDB LOCK evidence).
    await wait_for_process_exit(node_pid, STOP_TIMEOUT_S)

    # ── Step 5: H2 offline assertions ──
    LOG.info("[Step 5] Running offline db assertions...")
    env = lib.derive_offline_env(cluster.base_dir, node.id)
    assert Path(env.binary).is_file(), f"node binary missing: {env.binary}"
    assert Path(env.datadir).is_dir(), f"reth datadir missing: {env.datadir}"
    assert Path(env.chain).is_file(), f"chain spec missing: {env.chain}"

    # 5a. Fresh init persists the settings entry with the legacy layout
    # (greth init.rs writes StorageSettings::current() on first startup) —
    # PRESENT_LEGACY, explicitly not MISSING.
    probe = read_storage_settings(env)
    assert probe.error is None, probe.summary()
    assert probe.state is SettingsState.PRESENT_LEGACY, probe.summary()
    assert probe.settings.get("changesets_in_static_files", False) is False
    LOG.info("[Step 5] storage settings: %s", probe.settings)

    # 5b. Legacy layout on disk: no changeset segment files, no .csoff
    # sidecars under the node's static-files dir.
    layout = inspect_changeset_static_files(env.datadir, env.static_files_dir)
    assert layout.exists, f"static-files dir missing: {layout.static_files_dir}"
    assert not layout.has_segment_files, (
        f"fresh legacy node has changeset segment files: "
        f"{layout.account_segments + layout.storage_segments}"
    )
    assert not layout.has_sidecar_files, (
        f"fresh legacy node has .csoff sidecars: "
        f"{layout.account_sidecars + layout.storage_sidecars}"
    )

    # 5c. Positive control: the history really wrote changesets into the
    # database tables (and the list --count command surface works).
    for table in (ACCOUNT_CHANGESETS_TABLE, STORAGE_CHANGESETS_TABLE):
        count = count_table_entries(env, table)
        assert count.error is None, count.summary()
        assert count.count > 0, (
            f"{table} is empty on a legacy-layout node with history:\n"
            + count.summary()
        )
        LOG.info("[Step 5] %s entries: %d", table, count.count)

    # 5d. db stats must run and parse; entry counts are NOT asserted (the
    # gravity RocksDB backend reports placeholder zeros) — recorded in the
    # log as evidence only.
    stats = db_stats(env)
    assert stats.error is None, stats.summary()
    LOG.info(
        "[Step 5] db stats parsed %d tables; changeset rows (placeholder "
        "counts): %s=%s %s=%s",
        len(stats.entries),
        ACCOUNT_CHANGESETS_TABLE,
        stats.entries.get(ACCOUNT_CHANGESETS_TABLE),
        STORAGE_CHANGESETS_TABLE,
        stats.entries.get(STORAGE_CHANGESETS_TABLE),
    )

    # ── Step 6: restart and catch up ──
    LOG.info("[Step 6] Restarting node...")
    assert await cluster.set_node(
        node.id, NodeState.RUNNING, timeout=RESTART_TIMEOUT_S
    ), "node did not come back after restart"
    assert await node.wait_for_block_increase(
        timeout=BLOCK_PROGRESS_TIMEOUT_S, delta=2
    ), "no block progress after restart"
    await _wait_until_height(node, max_history_block + 1, CATCHUP_TIMEOUT_S)

    # ── Step 7: H1 replay ──
    LOG.info("[Step 7] Replaying anchors against the restarted node...")
    reloaded = AnchorSet.load(anchors_path)
    assert len(reloaded) == len(anchors), "anchor set changed across save/load"
    report = replay_anchors(node.w3, reloaded)
    assert report.ok, report.summary()
    LOG.info(
        "[Step 7] Anchor replay OK: %d/%d matched", report.matched, report.total
    )
