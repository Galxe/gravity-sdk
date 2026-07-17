"""
storage_v2_fresh_sync — SF-enabled fresh nodes sync from block 0 on a
rolling-upgraded network, and an SF validator votes (storage-v2 TC9).

Scenario (design doc: _local/drafts/storage-v2-e2e/sf-fresh-sync-design.md):
the legacy core (node1, node2, vfn1, pfn1) is BORN on v1.7.5, builds
history, and is rolling-upgraded to merge v2.3.0 while staying on the
legacy layout (settings MISSING — TC2's criterion). Five SF nodes then
come up fresh on the new binary and sync the whole chain from 0 —
v1.7.5-era blocks, the upgrade window, the Alpha boundary — under the
static-file changeset layout, covering the SF x non-SF upstream matrix:

    vfn1    <- node1   (legacy vfn <- legacy validator, control)
    pfn1    <- vfn1    (legacy pfn <- legacy vfn, control + tx entry)
    sf_vfn1 <- node2   (SF vfn <- legacy validator)
    sf_vfn2 <- sf_val1 (SF vfn <- SF validator)
    sf_pfn1 <- sf_vfn1 (SF pfn <- SF vfn)
    sf_pfn2 <- vfn1    (SF pfn <- legacy vfn)

sf_val1 joins the validator set via governance-enabled permissionless
join with a stake EQUAL to the genesis validators' (2 ETH): with three
equal-power validators the quorum is ⌊2/3·total⌋+1 = ALL THREE votes, so
every QC necessarily carries sf_val1's signature — and the L3 necessity
probe makes it observable: stopping sf_val1 must FREEZE the chain,
restarting it must resume it.

SF enablement (design §2, Q1): the tested HEAD has no fresh-init SF
switch, so the case enables SF via form D — first start (fresh
init_genesis, legacy settings + block-0 genesis reverts) -> stop ->
`db migrate-changesets` (needs the #391 preflight fix: accepts the
block-0 first entry and migrates the reverts into the SF segment as
entity data) -> restart. The hook is mode-switched ([sf] mode /
GRAVITY_SF_MODE); form B ("flag") slots in once greth wires
--storage.v2 into init_genesis.

Known constraints (design §3):
(1) the runner starts ALL nodes before pytest, so phase 1 stops the five
    SF nodes and wipes their data dirs to restore fresh-init semantics
    (identity/config/binary survive outside data/);
(2) 2 genesis validators = f=0 until the join: every validator's
    swap/stop window freezes the chain. Expected — the TxSender pauses
    around those windows; rolling LIVENESS is TC2's 4-validator coverage,
    not repeated here;
(3) upgrade-window depth (wei-level Alpha reconciliation etc.) is TC2's
    coverage; phase 3 keeps only light health assertions;
(4) ACCOUNT DISCIPLINE: the background TxSender owns a dedicated account
    (funded from the faucet in phase 2, strictly before the sender
    starts); the faucet belongs to FOREGROUND transactions only —
    explicit history txs and the governance recipe (which is faucet-bound:
    governance owner and pool[0] voter) — and sf_val1's staking CLI uses
    its accounts.csv bench account ([faucet_init], funded at suite init).
    Rationale: storage_v2_upgrade separates the sender and explicit
    faucet txs in TIME (its sender is stopped around every explicit
    section); this case's sender runs across phases 3-10, and sharing
    the faucet raced its nonce (live attempt2: "nonce too low" then
    "replacement transaction underpriced" on the phase-3 B-batch txs).
    Separating by ACCOUNT removes the race for every foreground sender
    at once.

Q6 landmine coverage: the A/B anchor sets sample alloc-funded accounts at
early blocks; form D avoids the SF-fresh empty-anchor + block-0 history
index combination by construction (#391 fix migrates entity rows), so a
future form-B run failing here is the design-doc Q6 signal, not a case
bug.

All timeouts are case-internal (no pytest-timeout, repo convention).
"""

import asyncio
import json
import logging
import os
import shutil
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, Optional

try:
    import tomllib
except ImportError:
    import tomli as tomllib

import pytest
from eth_account import Account
from web3 import Web3

import sf_lib
from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.cluster.node import Node, NodeState
from gravity_e2e.helpers import storage_case_lib as lib
from gravity_e2e.helpers import upgrade_lib
from gravity_e2e.helpers.governance import enable_permissionless_join
from gravity_e2e.helpers.node_process import read_node_pid, wait_for_process_exit
from gravity_e2e.helpers.offline_db import (
    SettingsState,
    migrate_changesets,
    read_storage_settings,
)
from gravity_e2e.helpers.storage_anchors import (
    AnchorSet,
    AnchorSpec,
    collect_anchors,
    replay_anchors,
)
from gravity_e2e.helpers.tx_sender import TxSender
from gravity_e2e.utils.transaction_builder import TransactionBuilder

LOG = logging.getLogger(__name__)

TEST_DIR = Path(__file__).resolve().parent
PROJECT_ROOT = TEST_DIR.parent.parent.parent
GENESIS_TOML_PATH = TEST_DIR / "genesis.toml"
TEST_PARAMS_PATH = TEST_DIR / "test_params.toml"

NEW_BINARY_PATH = Path(
    os.environ.get(
        "GRAVITY_NEW_BINARY",
        str(PROJECT_ROOT / "target" / "quick-release" / "gravity_node"),
    )
)

# ── Orchestration parameters (storage_v2_upgrade lineage) ──
MAX_HEIGHT_GAP = 50
INTER_NODE_WAIT = 120
HEIGHT_CHECK_MAX_RETRIES = 6
HEIGHT_CHECK_RETRY_INTERVAL = 20
TX_INTERVAL = 0.2
TX_RECEIPT_TIMEOUT = 30.0
# Looser than storage_v2_upgrade's floors would suggest: the sender is
# deliberately paused across every swap window and the L3 freeze.
MIN_TX_CONFIRMED = 100
MIN_TX_SUCCESS_RATE = 0.5
# The background sender's dedicated account funding (constraint (4)),
# transferred once in phase 2 before the sender starts. Worst case ~18k
# txs at 21k gas * 100 gwei ≈ 38 ETH; 100 keeps a wide margin.
BG_SENDER_FUND_ETH = 100

# ── Case timeouts (seconds) ──
BLOCK_PROGRESS_TIMEOUT_S = 60
TX_BLOCK_GAP_TIMEOUT_S = 30
STOP_TIMEOUT_S = 90
CATCHUP_TIMEOUT_S = 300
# From-0 sync budget per SF node (the chain is ~30-60 min old by phase 4).
FRESH_SYNC_TIMEOUT_S = 900
RPC_UP_TIMEOUT_S = 120

# ── Storage/history parameters (TC1/TC2 lineage) ──
TRANSFER_AMOUNTS_ETH = (1, 2)
SET_VALUES = (1, 2)
TAIL_TRANSFER_ETH = 3
TAIL_SET_VALUE = 3
MIGRATE_TIMEOUT_S = 900
UNWIND_LINES_MAX = 100

# ── Epoch / governance / join (fuzzy_cluster recipe) ──
# EPOCH_INTERVAL_S / JOIN_STAKE_ETH live in sf_lib and are unit-test
# cross-checked against genesis.toml.tpl (epoch match; equal-power join —
# the L3 prerequisite).
EPOCH_INTERVAL_S = sf_lib.EPOCH_INTERVAL_S
JOIN_STAKE_ETH = sf_lib.JOIN_STAKE_ETH
VOTING_DURATION_S = 5
JOIN_ACTIVE_TIMEOUT_S = 3 * EPOCH_INTERVAL_S + 120

# ── Alpha timeline guards (upgrade_lib) ──
ALPHA_MIN_LEAD_S = 18 * 60
ALPHA_MAX_LEAD_S = 2 * 3600
ALPHA_UPGRADE_MARGIN_S = 5 * 60
ALPHA_ACTIVATION_BUFFER_S = 300

# ── L3 necessity probe ──
FREEZE_SETTLE_S = 10       # let in-flight commits drain after the stop
FREEZE_OBSERVE_S = 30      # window during which heights must not move
RESUME_TIMEOUT_S = 240     # BFT round-timeout backoff makes resume slow


# ---------------------------------------------------------------------------
# Cluster-health helpers (storage_v2_upgrade lineage, trimmed)
# ---------------------------------------------------------------------------


async def get_block_heights(nodes: List[Node]) -> Dict[str, int]:
    async def _get_height(node: Node):
        height = await asyncio.to_thread(lambda: node.w3.eth.block_number)
        return node.id, height

    results = await asyncio.gather(*[_get_height(n) for n in nodes])
    return dict(results)


async def running_nodes(cluster: Cluster) -> List[Node]:
    running = []
    for node in cluster.nodes.values():
        state, _ = await node.get_state()
        if state == NodeState.RUNNING:
            running.append(node)
    return running


async def check_height_gap_ok(cluster: Cluster, max_gap: int = MAX_HEIGHT_GAP) -> bool:
    running = await running_nodes(cluster)
    if len(running) < 2:
        return True
    heights = await get_block_heights(running)
    gap = max(heights.values()) - min(heights.values())
    LOG.info("Block heights: %s | gap=%d (max_allowed=%d)", heights, gap, max_gap)
    return gap < max_gap


async def wait_for_height_gap_ok(
    cluster: Cluster,
    max_retries: int = HEIGHT_CHECK_MAX_RETRIES,
    retry_interval: int = HEIGHT_CHECK_RETRY_INTERVAL,
) -> bool:
    for attempt in range(1, max_retries + 1):
        if await check_height_gap_ok(cluster):
            return True
        if attempt < max_retries:
            await asyncio.sleep(retry_interval)
    return False


async def stop_node_and_wait_exit(node: Node) -> None:
    pid = read_node_pid(node)
    assert pid, f"cannot read {node.id} PID from {node.pid_file} before stop"
    assert await node.stop(), f"failed to stop {node.id}"
    await wait_for_process_exit(pid, STOP_TIMEOUT_S)


def swap_node_binary(node: Node, new_binary: Path) -> None:
    bin_path = node._infra_path / "bin" / "gravity_node"
    LOG.info("[%s] Replacing binary: %s", node.id, bin_path)
    if bin_path.exists():
        bin_path.unlink()
    try:
        os.link(str(new_binary), str(bin_path))
    except OSError:
        shutil.copy2(str(new_binary), str(bin_path))
    assert upgrade_lib.is_upgrade_target(bin_path, new_binary), (
        f"{node.id}: binary swap did not take effect at {bin_path}"
    )


async def restart_node_and_catch_up(cluster: Cluster, node: Node) -> None:
    assert await node.start(), f"failed to start {node.id}"
    assert await node.wait_for_block_increase(
        timeout=CATCHUP_TIMEOUT_S, delta=2
    ), f"{node.id}: no block progress after restart"
    assert await wait_for_height_gap_ok(
        cluster
    ), f"{node.id}: cluster height gap did not close after restart"


async def _wait_until_height(node: Node, target: int, timeout: float) -> int:
    deadline = time.monotonic() + timeout
    head = -1
    while time.monotonic() < deadline:
        try:
            head = node.w3.eth.block_number
            if head >= target:
                return head
        except Exception as exc:  # RPC may flap right after (re)start
            LOG.debug("height poll failed: %s", exc)
        await asyncio.sleep(1)
    raise AssertionError(
        f"node {node.id} did not reach height {target} within {timeout}s "
        f"(last seen {head})"
    )


async def _wait_rpc_up(node: Node, timeout: float = RPC_UP_TIMEOUT_S) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        state, _ = await node.get_state()
        if state == NodeState.RUNNING:
            try:
                node.w3.eth.block_number
                return
            except Exception:
                pass
        await asyncio.sleep(1)
    raise AssertionError(f"{node.id}: RPC did not come up within {timeout}s")


def read_alpha_time() -> Optional[int]:
    if not GENESIS_TOML_PATH.exists():
        return None
    with open(GENESIS_TOML_PATH, "rb") as f:
        config = tomllib.load(f)
    value = config.get("genesis", {}).get("hardforks", {}).get("alphaTime")
    return int(value) if value is not None else None


def read_sf_mode() -> str:
    params = {}
    if TEST_PARAMS_PATH.exists():
        with open(TEST_PARAMS_PATH, "rb") as f:
            params = tomllib.load(f)
    return sf_lib.resolve_sf_mode(params, os.environ)


# ---------------------------------------------------------------------------
# History / anchors (storage_v2_upgrade lineage — all txs enter via pfn1)
# ---------------------------------------------------------------------------


def _load_anchor_target():
    artifact = json.loads((TEST_DIR / "contracts" / "AnchorTarget.json").read_text())
    return artifact["abi"], artifact["bytecode"]


async def _confirmed_tx_point(node: Node, result, label: str) -> lib.TxPoint:
    assert result.success, f"{label} failed: {result.error}"
    assert result.block_number is not None, f"{label} has no inclusion block"
    point = lib.TxPoint(tx_hash=result.tx_hash, block_number=result.block_number)
    LOG.info("%s confirmed: tx=%s block=%d", label, point.tx_hash, point.block_number)
    assert await node.wait_for_block_increase(
        timeout=TX_BLOCK_GAP_TIMEOUT_S, delta=1
    ), f"no block progress after {label}"
    return point


async def build_history(node: Node, faucet) -> lib.OnChainHistory:
    """Transfers + AnchorTarget deploy + set() calls, submitted through
    the tx-entry pfn — the production ingress path (pfn -> vfn ->
    validator mempool) is itself under test."""
    tb = TransactionBuilder(node.w3, faucet)
    recipient = Account.create()
    LOG.info("History recipient: %s", recipient.address)

    transfers = []
    for amount_eth in TRANSFER_AMOUNTS_ETH:
        result = await tb.send_ether(recipient.address, Web3.to_wei(amount_eth, "ether"))
        transfers.append(
            await _confirmed_tx_point(node, result, f"transfer {amount_eth} ETH")
        )

    abi, bytecode = _load_anchor_target()
    deploy_result = await tb.deploy_contract(bytecode=bytecode, abi=abi)
    deploy = await _confirmed_tx_point(node, deploy_result, "AnchorTarget deploy")
    contract_addr = deploy_result.tx_receipt["contractAddress"]
    assert contract_addr, "deployment receipt has no contractAddress"

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


async def replay_anchor_file(node: Node, path: Path, stage: str) -> None:
    anchors = AnchorSet.load(path)
    report = await asyncio.to_thread(replay_anchors, node.w3, anchors)
    assert report.ok, f"[{stage}] anchor mismatch on {node.id}:\n{report.summary()}"
    LOG.info(
        "[%s] %s: anchor replay OK (%d/%d matched)",
        stage,
        node.id,
        report.matched,
        report.total,
    )


async def replay_all_batches(ctx, node: Node, stage: str) -> None:
    """A batch (v1.7.5-era) + B batch (post-upgrade/post-Alpha) — replayed
    on an SF node this is THE from-0 correctness evidence: v1.7.5-written
    history served by the static-file read path."""
    await replay_anchor_file(node, ctx.anchors_a_path, f"{stage}/A")
    if ctx.anchors_b_path is not None:
        await replay_anchor_file(node, ctx.anchors_b_path, f"{stage}/B")


def node_log_files(node: Node) -> List[Path]:
    infra = node._infra_path
    files = []
    debug_log = infra / "logs" / "debug.log"
    if debug_log.exists():
        files.append(debug_log)
    for candidate_dir in (infra / "execution_logs", infra / "logs" / "execution_logs"):
        if candidate_dir.is_dir():
            files.extend(sorted(p for p in candidate_dir.iterdir() if p.is_file()))
    return files


def scan_all_node_logs(cluster: Cluster, stage: str) -> None:
    for node in cluster.nodes.values():
        files = node_log_files(node)
        assert files, f"[{stage}] {node.id}: no execution log files found"
        for log_file in files:
            with open(log_file, errors="replace") as fh:
                result = upgrade_lib.scan_log_lines(fh)
            assert result.error_count == 0, (
                f"[{stage}] {node.id}: storage/decode errors in {log_file}:\n"
                f"{result.summary()}"
            )
            assert result.unwind_count <= UNWIND_LINES_MAX, (
                f"[{stage}] {node.id}: unwind lines exceed ceiling "
                f"({result.unwind_count} > {UNWIND_LINES_MAX}) in {log_file}:\n"
                f"{result.summary()}"
            )


# ---------------------------------------------------------------------------
# SF-enable hook (form D today, form B reserved) + fresh-state restore
# ---------------------------------------------------------------------------


def _offline_env(ctx, node: Node):
    env = lib.derive_offline_env(ctx.cluster.base_dir, node.id)
    assert Path(env.binary).is_file(), f"node binary missing: {env.binary}"
    assert Path(env.datadir).is_dir(), f"reth datadir missing: {env.datadir}"
    assert Path(env.chain).is_file(), f"chain spec missing: {env.chain}"
    return env


def wipe_node_to_fresh(node: Node) -> None:
    """Restore never-started semantics for a runner-started node: the
    runner's start.sh brings up ALL nodes before pytest (constraint (1)),
    but the SF nodes must fresh-init at the case-controlled first start.
    Wipes <node>/data (reth datadir + consensus/quorumstore DBs) and the
    boot's log files; identity/config/binary live elsewhere and survive."""
    data_dir = node._infra_path / "data"
    assert data_dir.is_dir(), f"{node.id}: data dir missing at {data_dir}"
    shutil.rmtree(data_dir)
    data_dir.mkdir()
    debug_log = node._infra_path / "logs" / "debug.log"
    if debug_log.exists():
        debug_log.unlink()
    for candidate_dir in (
        node._infra_path / "execution_logs",
        node._infra_path / "logs" / "execution_logs",
    ):
        if candidate_dir.is_dir():
            for p in candidate_dir.iterdir():
                if p.is_file():
                    p.unlink()
    LOG.info("[%s] data dir wiped back to fresh", node.id)


async def start_fresh_sf_node(ctx, node_id: str) -> None:
    """First start of an SF node + the SF-enable hook.

    Form D ("migrate"): start (fresh init_genesis writes legacy settings
    + block-0 genesis reverts; the node may sync a handful of blocks —
    they migrate along) -> stop -> `db migrate-changesets` (the #391-fix
    path: accepts the block-0 first entry, migrates the reverts into the
    SF segments as entity rows, flips the ratchet) -> restart -> sync to
    tip. Form B would instead inject sf_lib.sf_start_extra_args() at this
    first start — refused until greth wires it (sf_lib.resolve_sf_mode).
    """
    node = ctx.cluster.get_node(node_id)
    assert node_id in sf_lib.SF_NODE_IDS, f"{node_id} is not an SF node"
    assert ctx.sf_mode == "migrate", f"unsupported sf mode {ctx.sf_mode}"

    LOG.info("[sf-enable/%s] first start (fresh init)...", node_id)
    assert await node.start(), f"failed to start {node_id}"
    await _wait_rpc_up(node)

    LOG.info("[sf-enable/%s] stopping for the offline SF flip...", node_id)
    await stop_node_and_wait_exit(node)
    env = _offline_env(ctx, node)
    assert upgrade_lib.is_upgrade_target(Path(env.binary), NEW_BINARY_PATH), (
        f"{node_id}: SF node must run the new binary (check [sf_source])"
    )
    result = migrate_changesets(env, timeout=MIGRATE_TIMEOUT_S)
    assert result.ok, (
        f"[sf-enable/{node_id}] migrate-changesets failed — without the "
        f"#391 preflight fix every fresh datadir is rejected as 'pruned' "
        f"(block-0 genesis reverts):\n{result.summary()}"
    )
    probe = read_storage_settings(env)
    assert probe.error is None, f"[sf-enable/{node_id}] {probe.summary()}"
    assert probe.state is SettingsState.PRESENT_STATIC_FILES, (
        f"[sf-enable/{node_id}] ratchet not set after migrate "
        f"(state={probe.state})\n" + probe.summary()
    )
    ctx.sf_enable_durations[node_id] = result.duration_s

    LOG.info("[sf-enable/%s] restart under the SF layout...", node_id)
    assert await node.start(), f"failed to restart {node_id} after SF flip"
    await _wait_rpc_up(node)


async def sync_to_tip(ctx, node_id: str) -> None:
    """From-0 catch-up: reach the reference head sampled NOW, then close
    the cluster height gap (gap-convergence, upgrade-case paradigm)."""
    node = ctx.cluster.get_node(node_id)
    reference = ctx.cluster.get_node("node1")
    target = await asyncio.to_thread(lambda: reference.w3.eth.block_number)
    LOG.info("[%s] syncing from 0 to tip (target >= %d)...", node_id, target)
    head = await _wait_until_height(node, target, FRESH_SYNC_TIMEOUT_S)
    assert await wait_for_height_gap_ok(ctx.cluster), (
        f"{node_id}: height gap did not close after from-0 sync"
    )
    LOG.info("[%s] caught up (head=%d)", node_id, head)


# ---------------------------------------------------------------------------
# Context + phases
# ---------------------------------------------------------------------------


@dataclass
class FreshSyncContext:
    cluster: Cluster
    output_dir: Path
    sf_mode: str = "migrate"
    history: Optional[lib.OnChainHistory] = None
    max_history_block: int = 0
    anchors_a_path: Optional[Path] = None
    anchors_b_path: Optional[Path] = None
    tx_sender: Optional[TxSender] = None
    # Constraint (4): the background sender's dedicated account — funded
    # in phase 2, disjoint from the faucet by construction.
    bg_sender_account: Optional[object] = None
    alpha_time: Optional[int] = None
    upgrade_order: List[str] = field(default_factory=list)
    sf_enable_durations: Dict[str, float] = field(default_factory=dict)
    layout_counts: Dict[str, Dict[str, int]] = field(default_factory=dict)
    phase_durations: Dict[str, float] = field(default_factory=dict)

    @property
    def out_dir(self) -> Path:
        return self.output_dir / "storage_v2_fresh_sync"


async def phase_1_bootstrap_legacy_core(ctx: FreshSyncContext) -> None:
    LOG.info("[Phase 1] Bootstrap: legacy core up on v1.7.5, SF nodes wiped...")
    assert NEW_BINARY_PATH.exists(), (
        f"New binary not found at {NEW_BINARY_PATH}. Build it first or set "
        f"GRAVITY_NEW_BINARY."
    )

    # Binary posture guards: legacy nodes must NOT already run the target
    # (deploy copies binaries — size-fallback predicate), SF nodes MUST.
    for node_id in sf_lib.LEGACY_NODE_IDS:
        deployed = ctx.cluster.get_node(node_id)._infra_path / "bin" / "gravity_node"
        assert deployed.exists(), f"{node_id}: deployed binary missing: {deployed}"
        assert not upgrade_lib.is_upgrade_target(deployed, NEW_BINARY_PATH), (
            f"{node_id}: deployed binary IS (a copy of) the upgrade target — "
            f"check test_params.toml [source]"
        )
    for node_id in sf_lib.SF_NODE_IDS:
        deployed = ctx.cluster.get_node(node_id)._infra_path / "bin" / "gravity_node"
        assert deployed.exists(), f"{node_id}: deployed binary missing: {deployed}"
        assert upgrade_lib.is_upgrade_target(deployed, NEW_BINARY_PATH), (
            f"{node_id}: SF node must be deployed with the NEW binary — "
            f"check test_params.toml [sf_source]"
        )

    ctx.alpha_time = read_alpha_time()
    alpha_error = upgrade_lib.alpha_preflight_error(
        ctx.alpha_time, time.time(), ALPHA_MIN_LEAD_S, max_lead_s=ALPHA_MAX_LEAD_S
    )
    assert alpha_error is None, f"[Phase 1] {alpha_error}"

    # Constraint (1): the runner started everything; SF nodes go back to
    # never-started (their minute on the new binary is erased with the
    # wipe, so the case-controlled fresh init below is the real one).
    for node_id in sf_lib.SF_NODE_IDS:
        node = ctx.cluster.get_node(node_id)
        state, _ = await node.get_state()
        if state == NodeState.RUNNING:
            await stop_node_and_wait_exit(node)
        wipe_node_to_fresh(node)

    for node_id in sf_lib.LEGACY_NODE_IDS:
        node = ctx.cluster.get_node(node_id)
        state, _ = await node.get_state()
        assert state == NodeState.RUNNING, (
            f"{node_id}: expected RUNNING after the runner start, got {state}"
        )
    assert await ctx.cluster.check_block_increasing(
        timeout=BLOCK_PROGRESS_TIMEOUT_S
    ), "block production not working on the v1.7.5 core"
    LOG.info(
        "[Phase 1] legacy core live on v1.7.5; SF nodes fresh; alphaTime=%s",
        ctx.alpha_time,
    )


async def phase_2_history_and_anchors_a(ctx: FreshSyncContext) -> None:
    LOG.info("[Phase 2] v1.7.5-era history + anchor batch A (via %s)...",
             sf_lib.TX_ENTRY_NODE)
    entry = ctx.cluster.get_node(sf_lib.TX_ENTRY_NODE)
    faucet = ctx.cluster.faucet
    assert faucet, "faucet account not configured (genesis.faucet)"

    ctx.history = await build_history(entry, faucet)
    ctx.max_history_block = max(
        p.block_number
        for p in (*ctx.history.transfers, ctx.history.deploy, *ctx.history.sets)
    )
    await _wait_until_height(
        entry, ctx.max_history_block + 1, BLOCK_PROGRESS_TIMEOUT_S
    )
    spec = lib.build_anchor_spec(ctx.history)
    anchors = collect_anchors(
        entry.w3,
        spec,
        meta={
            "case": "storage_v2_fresh_sync",
            "stage": "v1.7.5-era",
            "node": entry.id,
            "max_history_block": ctx.max_history_block,
        },
    )
    lib.assert_history_is_anchorable(
        anchors,
        ctx.history,
        transfer_amounts_wei=[
            Web3.to_wei(amount, "ether") for amount in TRANSFER_AMOUNTS_ETH
        ],
        set_values=SET_VALUES,
    )
    ctx.anchors_a_path = ctx.out_dir / "anchors_a.json"
    anchors.save(ctx.anchors_a_path)
    LOG.info("[Phase 2] %d A-batch anchors saved to %s", len(anchors),
             ctx.anchors_a_path)

    # Constraint (4): fund the background sender's dedicated account NOW,
    # while no concurrent sender exists — after this, the faucet nonce is
    # foreground-only property and the two lanes can never race.
    ctx.bg_sender_account = Account.create()
    tb = TransactionBuilder(entry.w3, faucet)
    await _confirmed_tx_point(
        entry,
        await tb.send_ether(
            ctx.bg_sender_account.address,
            Web3.to_wei(BG_SENDER_FUND_ETH, "ether"),
        ),
        f"background-sender funding {BG_SENDER_FUND_ETH} ETH",
    )
    LOG.info(
        "[Phase 2] background sender account funded: %s",
        ctx.bg_sender_account.address,
    )

    # Cross >= 1 epoch on v1.7.5 so the from-0 sync replays an epoch
    # boundary out of v1.7.5-era history (PR #779 regression surface).
    LOG.info("[Phase 2] crossing an epoch boundary (%ds)...", EPOCH_INTERVAL_S + 30)
    await asyncio.sleep(EPOCH_INTERVAL_S + 30)
    assert await ctx.cluster.check_block_increasing(
        timeout=BLOCK_PROGRESS_TIMEOUT_S
    ), "block production stalled across the epoch crossing"


async def phase_3_rolling_upgrade(ctx: FreshSyncContext) -> None:
    LOG.info("[Phase 3] Rolling upgrade of the legacy core to %s ...",
             NEW_BINARY_PATH)
    ctx.upgrade_order = upgrade_lib.build_upgrade_order(
        list(sf_lib.LEGACY_NODE_IDS), first="vfn1"
    )
    LOG.info("[Phase 3] Upgrade order: %s", ctx.upgrade_order)

    for i, node_id in enumerate(ctx.upgrade_order):
        node = ctx.cluster.get_node(node_id)
        assert await wait_for_height_gap_ok(ctx.cluster), (
            f"height gap too large before upgrading {node_id}"
        )
        if ctx.alpha_time is not None:
            remaining = ctx.alpha_time - time.time()
            assert remaining > ALPHA_UPGRADE_MARGIN_S, (
                f"alphaTime activates in {remaining:.0f}s (< margin "
                f"{ALPHA_UPGRADE_MARGIN_S}s) but {node_id} is not upgraded "
                f"yet — widen the [hardforks.alphaTime] schedule and re-render"
            )
        # Constraint (2): with 2 validators every validator swap freezes
        # the chain, and pfn1 is the sender's ingress — pause across
        # every swap window instead of burning failures.
        ctx.tx_sender.pause()
        LOG.info("[Phase 3] Upgrading %d/%d: %s", i + 1, len(ctx.upgrade_order),
                 node_id)
        await stop_node_and_wait_exit(node)
        swap_node_binary(node, NEW_BINARY_PATH)
        assert await node.start(), f"failed to start {node_id} after swap"
        await _wait_rpc_up(node)
        ctx.tx_sender.resume()
        assert await node.wait_for_block_increase(
            timeout=CATCHUP_TIMEOUT_S, delta=2
        ), f"{node_id}: no block progress after upgrade"
        if i < len(ctx.upgrade_order) - 1:
            await asyncio.sleep(INTER_NODE_WAIT)

    completion_error = upgrade_lib.upgrade_completion_error(
        ctx.alpha_time, time.time()
    )
    assert completion_error is None, f"[Phase 3] {completion_error}"

    # Light Alpha handling (depth is TC2's): wait out the crossing, then
    # health + A-batch replay on the legacy control.
    wait_s = upgrade_lib.alpha_tail_wait_s(ctx.alpha_time, time.time())
    if wait_s is not None:
        LOG.info("[Phase 3] waiting for the Alpha crossing (~%.0fs)...", wait_s)
        node = ctx.cluster.get_node("node1")
        deadline = time.monotonic() + wait_s + ALPHA_ACTIVATION_BUFFER_S
        crossed = False
        while time.monotonic() < deadline:
            head = await asyncio.to_thread(lambda: node.w3.eth.get_block("latest"))
            if head["timestamp"] >= ctx.alpha_time:
                crossed = True
                break
            await asyncio.sleep(2)
        assert crossed, (
            f"chain never crossed alphaTime={ctx.alpha_time} — production "
            f"stalled around activation?"
        )
        assert await ctx.cluster.check_block_increasing(
            timeout=BLOCK_PROGRESS_TIMEOUT_S, delta=2
        ), "block production stalled after Alpha activation"
    assert await wait_for_height_gap_ok(ctx.cluster), (
        "height gap did not close after the upgrade tail"
    )
    await replay_anchor_file(
        ctx.cluster.get_node("vfn1"), ctx.anchors_a_path, "post-upgrade"
    )

    # Anchor batch B: fresh post-upgrade/post-Alpha history via pfn1.
    entry = ctx.cluster.get_node(sf_lib.TX_ENTRY_NODE)
    tb = TransactionBuilder(entry.w3, ctx.cluster.faucet)
    tail_recipient = Account.create()
    transfer = await _confirmed_tx_point(
        entry,
        await tb.send_ether(
            tail_recipient.address, Web3.to_wei(TAIL_TRANSFER_ETH, "ether")
        ),
        f"post-upgrade transfer {TAIL_TRANSFER_ETH} ETH",
    )
    set_point = await _confirmed_tx_point(
        entry,
        await tb.build_and_send_tx(
            to=ctx.history.contract, data=lib.encode_set_call(TAIL_SET_VALUE)
        ),
        f"post-upgrade set({TAIL_SET_VALUE})",
    )
    await _wait_until_height(
        entry, set_point.block_number + 1, BLOCK_PROGRESS_TIMEOUT_S
    )
    b_spec = AnchorSpec(
        balances=[(tail_recipient.address, transfer.block_number)],
        storage_slots=[
            (ctx.history.contract, lib.VALUE_SLOT, set_point.block_number)
        ],
        tx_hashes=[transfer.tx_hash, set_point.tx_hash],
        block_numbers=sorted({transfer.block_number, set_point.block_number}),
        log_ranges=[
            (set_point.block_number, set_point.block_number, ctx.history.contract)
        ],
    )
    b_anchors = collect_anchors(
        entry.w3,
        b_spec,
        meta={"case": "storage_v2_fresh_sync", "stage": "post-upgrade",
              "node": entry.id},
    )
    ctx.anchors_b_path = ctx.out_dir / "anchors_b.json"
    b_anchors.save(ctx.anchors_b_path)
    await replay_anchor_file(
        ctx.cluster.get_node("vfn1"), ctx.anchors_b_path, "post-upgrade-fresh"
    )
    LOG.info("[Phase 3] upgrade tail complete; anchor batches A+B on disk")


async def phase_4_sf_first_batch(ctx: FreshSyncContext) -> None:
    LOG.info("[Phase 4] SF fullnodes, first batch: %s then sf_pfn1 ...",
             sf_lib.SF_FIRST_BATCH)
    for node_id in sf_lib.SF_FIRST_BATCH:
        await start_fresh_sf_node(ctx, node_id)
        await sync_to_tip(ctx, node_id)
    # SF <- SF chain: sf_pfn1 syncs FROM sf_vfn1 (its only upstream), so
    # an SF node is exercised as a sync SERVER here, not just a client.
    await start_fresh_sf_node(ctx, "sf_pfn1")
    await sync_to_tip(ctx, "sf_pfn1")


async def offline_sf_probe_and_restart(ctx: FreshSyncContext, node_id: str,
                                        stage: str) -> None:
    node = ctx.cluster.get_node(node_id)
    await stop_node_and_wait_exit(node)
    env = _offline_env(ctx, node)
    ctx.layout_counts[f"{stage}/{node_id}"] = lib.assert_sf_layout(
        env, f"{stage}/{node_id}"
    )
    await restart_node_and_catch_up(ctx.cluster, node)


async def phase_5_layout_probes(ctx: FreshSyncContext) -> None:
    LOG.info("[Phase 5] Offline layout probes: SF mirror + legacy controls...")
    # SF nodes: TC1's exact mirror (settings PRESENT_STATIC_FILES, both
    # segment kinds + sidecars, tables == 0) + G5 restart persistence.
    for node_id in ("sf_vfn1", "sf_pfn1", "sf_pfn2"):
        await offline_sf_probe_and_restart(ctx, node_id, "Phase 5")

    # Legacy controls: upgraded v1.7.5 datadirs — TC2's criterion
    # (settings MISSING, no SF files, populated tables). Same cluster,
    # same chain height, both layouts hold: the anti-false-green pair.
    for node_id in ("vfn1", "pfn1"):
        node = ctx.cluster.get_node(node_id)
        if node_id == sf_lib.TX_ENTRY_NODE:
            ctx.tx_sender.pause()
        await stop_node_and_wait_exit(node)
        env = _offline_env(ctx, node)
        ctx.layout_counts[f"Phase 5/{node_id}"] = lib.assert_upgraded_legacy_layout(
            env, f"Phase 5/{node_id}"
        )
        await restart_node_and_catch_up(ctx.cluster, node)
        if node_id == sf_lib.TX_ENTRY_NODE:
            ctx.tx_sender.resume()


async def phase_6_anchor_replays(ctx: FreshSyncContext) -> None:
    LOG.info("[Phase 6] A+B anchor replays on the SF fullnodes...")
    for node_id in ("sf_vfn1", "sf_pfn1", "sf_pfn2"):
        await replay_all_batches(ctx, ctx.cluster.get_node(node_id), "sf-from-0")


async def phase_7_sf_val1_join(ctx: FreshSyncContext) -> None:
    LOG.info("[Phase 7] sf_val1: from-0 sync + governance join (equal power)...")
    await start_fresh_sf_node(ctx, "sf_val1")
    await sync_to_tip(ctx, "sf_val1")

    entry = ctx.cluster.get_node(sf_lib.TX_ENTRY_NODE)
    faucet = ctx.cluster.faucet
    await enable_permissionless_join(
        entry.w3,
        faucet_key=Web3.to_hex(faucet.key),
        faucet_address=faucet.address,
        voting_duration_s=VOTING_DURATION_S,
    )

    # L3 prerequisite: equal power. JOIN_STAKE_ETH == the genesis
    # validators' 2e18 wei (unit-test cross-checked against the tpl), so
    # the 3-validator quorum becomes ALL votes.
    await ctx.cluster.validator_join(
        "sf_val1", stake_amount=JOIN_STAKE_ETH, moniker="SF_VAL1"
    )

    # L1: active in the next epoch(s). (The manager API exposes set
    # membership, not voting power — power reality is what L2/L3 prove.)
    deadline = time.monotonic() + JOIN_ACTIVE_TIMEOUT_S
    active = False
    while time.monotonic() < deadline:
        validator_set = await ctx.cluster.validator_list()
        if any(n.id == "sf_val1" for n in validator_set.active):
            active = True
            break
        await asyncio.sleep(15)
    assert active, (
        f"sf_val1 not in the active validator set within "
        f"{JOIN_ACTIVE_TIMEOUT_S}s of the join"
    )
    LOG.info("[Phase 7] L1: sf_val1 active")

    # L2: the chain keeps producing across >= 1 full epoch with sf_val1 in
    # the set. With three equal-power validators the quorum is all three
    # votes — sustained production IS proof its votes are counted.
    assert await ctx.cluster.check_block_increasing(
        timeout=BLOCK_PROGRESS_TIMEOUT_S, delta=2
    ), "chain stalled right after sf_val1 became active"
    await asyncio.sleep(EPOCH_INTERVAL_S + 15)
    assert await ctx.cluster.check_block_increasing(
        timeout=BLOCK_PROGRESS_TIMEOUT_S, delta=2
    ), "chain stalled within an epoch of sf_val1 joining"
    assert await wait_for_height_gap_ok(ctx.cluster), (
        "height gap did not stay closed with sf_val1 active"
    )
    LOG.info("[Phase 7] L2: chain healthy across an epoch with sf_val1 voting")


async def phase_8_sf_vfn2_matrix_close(ctx: FreshSyncContext) -> None:
    LOG.info("[Phase 8] sf_vfn2 (SF vfn <- SF validator): the last matrix cell...")
    await start_fresh_sf_node(ctx, "sf_vfn2")
    await sync_to_tip(ctx, "sf_vfn2")
    await offline_sf_probe_and_restart(ctx, "sf_vfn2", "Phase 8")
    await replay_all_batches(ctx, ctx.cluster.get_node("sf_vfn2"), "sf-from-sf")


async def phase_9_l3_necessity_probe(ctx: FreshSyncContext) -> None:
    """L3: stop sf_val1 -> the chain MUST freeze (3 equal-power
    validators, quorum = all votes) -> restart -> the chain resumes and
    sf_val1 is still active. Direct black-box proof that its vote is a
    necessary condition for progress — the only probe that catches
    'listed active but not actually signing' false greens. The offline SF
    probe rides inside the freeze window (G5 for the validator role)."""
    LOG.info("[Phase 9] L3 necessity probe: stopping sf_val1 ...")
    ctx.tx_sender.pause()
    node = ctx.cluster.get_node("sf_val1")
    await stop_node_and_wait_exit(node)

    await asyncio.sleep(FREEZE_SETTLE_S)
    observers = [
        ctx.cluster.get_node(node_id) for node_id in ("node1", "node2")
    ]
    before = await get_block_heights(observers)
    await asyncio.sleep(FREEZE_OBSERVE_S)
    after = await get_block_heights(observers)
    assert before == after, (
        f"chain kept producing without sf_val1's vote — quorum is NOT "
        f"all-three, the equal-power premise failed (stake/voting-power "
        f"drift?): {before} -> {after}"
    )
    LOG.info("[Phase 9] chain frozen for %ds without sf_val1 (as required)",
             FREEZE_OBSERVE_S)

    env = _offline_env(ctx, node)
    ctx.layout_counts["Phase 9/sf_val1"] = lib.assert_sf_layout(
        env, "Phase 9/sf_val1"
    )

    LOG.info("[Phase 9] restarting sf_val1; chain must resume...")
    assert await node.start(), "failed to restart sf_val1"
    await _wait_rpc_up(node)
    assert await ctx.cluster.check_block_increasing(
        timeout=RESUME_TIMEOUT_S, delta=2
    ), "chain did not resume after sf_val1 came back (BFT recovery failed)"
    validator_set = await ctx.cluster.validator_list()
    assert any(n.id == "sf_val1" for n in validator_set.active), (
        "sf_val1 dropped out of the active set across the restart"
    )
    # sf_vfn2 froze with its upstream; it must follow the resumed chain.
    await sync_to_tip(ctx, "sf_vfn2")
    ctx.tx_sender.resume()
    LOG.info("[Phase 9] L3 proven: no sf_val1 vote -> no progress; "
             "restart -> recovery")


async def phase_10_wrapup(ctx: FreshSyncContext) -> None:
    LOG.info("[Phase 10] Wrap-up: load floors, final replays, log scan...")
    await ctx.tx_sender.stop()
    ctx.tx_sender.log_stats()
    assert ctx.tx_sender.total_confirmed >= MIN_TX_CONFIRMED, (
        f"only {ctx.tx_sender.total_confirmed} txs confirmed during the run "
        f"(need >= {MIN_TX_CONFIRMED})"
    )
    assert ctx.tx_sender.success_rate >= MIN_TX_SUCCESS_RATE, (
        f"tx success rate {ctx.tx_sender.success_rate:.1%} below floor "
        f"{MIN_TX_SUCCESS_RATE:.0%} "
        f"({ctx.tx_sender.total_confirmed}/{ctx.tx_sender.total_sent})"
    )
    for node in ctx.cluster.nodes.values():
        await replay_all_batches(ctx, node, "final")
    scan_all_node_logs(ctx.cluster, stage="final")
    assert await check_height_gap_ok(ctx.cluster), "final height gap check failed"


# ---------------------------------------------------------------------------
# The test
# ---------------------------------------------------------------------------


async def _run_phase(ctx: FreshSyncContext, phase) -> None:
    start = time.monotonic()
    await phase(ctx)
    ctx.phase_durations[phase.__name__] = time.monotonic() - start


@pytest.mark.asyncio
async def test_storage_v2_fresh_sync(cluster: Cluster, output_dir: Path):
    LOG.info("=" * 70)
    LOG.info("Test: storage_v2_fresh_sync (TC9: SF fresh nodes from 0 + voting)")
    LOG.info("New binary: %s", NEW_BINARY_PATH)
    LOG.info("=" * 70)

    ctx = FreshSyncContext(cluster=cluster, output_dir=output_dir)
    ctx.sf_mode = read_sf_mode()
    ctx.out_dir.mkdir(parents=True, exist_ok=True)
    LOG.info("SF enable mode: %s", ctx.sf_mode)

    try:
        await _run_phase(ctx, phase_1_bootstrap_legacy_core)
        await _run_phase(ctx, phase_2_history_and_anchors_a)

        # Constraint (4): the sender runs on its DEDICATED account, never
        # the faucet — the faucet nonce belongs to foreground txs (B-batch
        # history, the faucet-bound governance recipe).
        assert ctx.bg_sender_account is not None, "phase 2 must fund the sender"
        assert ctx.bg_sender_account.address != cluster.faucet.address
        ctx.tx_sender = TxSender(
            cluster,
            ctx.bg_sender_account,
            primary_node_id=sf_lib.TX_ENTRY_NODE,
            tx_interval=TX_INTERVAL,
            receipt_timeout=TX_RECEIPT_TIMEOUT,
        )
        ctx.tx_sender.start()
        LOG.info("Background tx sender started (all load via %s, account %s)",
                 sf_lib.TX_ENTRY_NODE, ctx.bg_sender_account.address)

        await _run_phase(ctx, phase_3_rolling_upgrade)
        await _run_phase(ctx, phase_4_sf_first_batch)
        await _run_phase(ctx, phase_5_layout_probes)
        await _run_phase(ctx, phase_6_anchor_replays)
        await _run_phase(ctx, phase_7_sf_val1_join)
        await _run_phase(ctx, phase_8_sf_vfn2_matrix_close)
        await _run_phase(ctx, phase_9_l3_necessity_probe)
        await _run_phase(ctx, phase_10_wrapup)
    finally:
        if ctx.tx_sender:
            await ctx.tx_sender.stop()
        LOG.info("Phase durations: %s", {
            name: f"{seconds:.0f}s"
            for name, seconds in ctx.phase_durations.items()
        })
        LOG.info("SF enable (migrate) seconds: %s", {
            k: f"{v:.1f}" for k, v in ctx.sf_enable_durations.items()
        })

    LOG.info("storage_v2_fresh_sync PASSED")
