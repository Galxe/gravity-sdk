"""
storage_v2_upgrade — v1.7.5 datadir rolling upgrade to greth v2.3.0
(storage-v2 TC2, with the TC3 restart-persistence tail on the same
cluster).

Proves merge-level disk compatibility: history written by a v1.7.5
gravity_node must be read back bit-identically by the v2.3.0 binary after
a rolling upgrade, with service continuity throughout, and must survive
graceful and crash restarts of the upgraded cluster.

Orchestration skeleton: cluster_test_cases/rolling_upgrade (vfn-first
order, height-gap prechecks, sustained tx load with vfn1->node1 failover,
hardfork-timeline wait, per-node stop/swap/start). Storage verification:
the storage-v2 helpers (storage_anchors H1, offline_db H2,
storage_case_lib) in the same consumption pattern as storage_v2_baseline
(TC1).

FORK TIMELINE (operational rule this case both follows and documents):
mainnet activates NO gravity hardforks (genesis/mainnet/genesis.json
carries no alphaTime/betaBlock/gammaBlock/deltaBlock), and greth v2.3.0
gates its behavior changes — system-tx gas exemption + the one-shot
SYSTEM_CALLER balance migration — on the Gravity Alpha fork. Therefore
**Alpha must NOT activate before every node runs the new binary**: an
alphaTime that predates existing v1.7.5 history makes the new binary
re-execute those blocks under exempt semantics and diverge. Symptom
fingerprint (observed live 2026-07-15 with a stale ``alphaTime = 0``
config): gravity_node aborts ~2 s after start with ``panicked at
aptos-core/consensus/src/block_storage/block_store.rs:773 / assertion
`left == right` failed`` on two 32-byte block hashes. Phase 1 and phase 3
guard the timeline; test_params schedules alphaTime as a render-relative
"+NNm" offset comfortably after the worst-case upgrade completion.
Block-number forks (beta early, gamma late) carry no attached semantics
in either binary today — gamma still follows rolling_upgrade's rule of
activating only after the whole fleet is upgraded.

Phases (one test; every phase depends on the previous state — structured
as functions over a shared context so TC4's "migrate-changesets + static
file layout" step can slot in after phase 10 without restructuring):

 1. bootstrap the 6-node cluster on the OLD binary (guards: deployed node
    binaries differ from the upgrade target; Alpha timeline safe), verify
    block production;
 2. build history on v1.7.5 (transfers + AnchorTarget deploy + set()
    calls) and H1-collect anchors covering all six kinds, persist JSON;
 3. rolling upgrade under sustained tx load: vfn1 first (tx failover to
    node1), height-gap + Alpha-margin precheck before every node,
    stop -> wait real process exit -> swap binary -> start;
 4. replay the anchor set against EVERY node — the core "old data read by
    new binary" assertion (a mismatch here is a product bug, never a case
    tolerance);
 5. wait for all nodes to pass the max hardfork block (gamma activates
    only after the whole cluster is on v2.3.0), then stabilize + monitor
    height gaps;
 6. stop the tx sender, assert tx success floor, scan every node's
    execution logs for storage/decode-class errors;
 7. H2 offline on the stopped vfn1 (upgraded-datadir layout facts):
    gravity_storage_settings must be MISSING (the upgrade path must NOT
    write it — only fresh init_genesis does, cf. TC1's PRESENT_LEGACY on
    a fresh v2.3.0 datadir), no changeset static-file segments/.csoff,
    changeset tables non-empty; restart vfn1;
 8. TC3a: graceful stop -> start cycle for every node, rejoin + catch up;
 9. TC3b: kill -9 node3 -> start (crash recovery / pipe consistency
    checks do real work), rejoin + catch up;
10. full anchor replay on every node again, log scan (panics, storage
    errors, unwind-loop ceiling), height-gap check
    (TC4 extension point: migrate-changesets slots in here);
11. Alpha activation tail (conditional — runs when alphaTime is scheduled
    within reach): wait for the chain to cross alphaTime on the fully
    upgraded fleet — the real production sequence "upgrade fleet, then
    open the Alpha gate" — then assert continued block production, the
    gas-exempt semantic (SYSTEM_CALLER balance constant post-activation;
    pre-Alpha it is debited gas_used x basefee every block), replay the
    pre-upgrade anchors once more, and collect + replay a fresh round of
    post-activation anchors.

All timeouts are case-internal (no pytest-timeout, repo convention).
"""

import asyncio
import json
import logging
import math
import os
import shutil
import statistics
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

import upgrade_lib
from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.cluster.node import Node, NodeState
from gravity_e2e.helpers import storage_case_lib as lib
from gravity_e2e.helpers.node_process import read_node_pid, wait_for_process_exit
from gravity_e2e.helpers.offline_db import (
    ACCOUNT_CHANGESETS_TABLE,
    STORAGE_CHANGESETS_TABLE,
    SettingsState,
    count_table_entries,
    inspect_changeset_static_files,
    read_storage_settings,
)
from gravity_e2e.helpers.storage_anchors import (
    AnchorSet,
    AnchorSpec,
    collect_anchors,
    replay_anchors,
)
from gravity_e2e.utils.transaction_builder import TransactionBuilder

LOG = logging.getLogger(__name__)

TEST_DIR = Path(__file__).resolve().parent
PROJECT_ROOT = TEST_DIR.parent.parent.parent
GENESIS_TOML_PATH = TEST_DIR / "genesis.toml"

# New binary to upgrade to (default: current build; same override knob as
# rolling_upgrade).
NEW_BINARY_PATH = Path(
    os.environ.get(
        "GRAVITY_NEW_BINARY",
        str(PROJECT_ROOT / "target" / "quick-release" / "gravity_node"),
    )
)

# ── Orchestration parameters (rolling_upgrade lineage) ──
# Maximum allowed block height gap between nodes.
MAX_HEIGHT_GAP = 50
# Wait between upgrading individual nodes. 120s (vs rolling_upgrade's
# 180s) keeps the whole TC2+TC3 flow inside the ~1h budget; the height-gap
# precheck before every node still gates on actual catch-up.
INTER_NODE_WAIT = 120
# Post-hardfork stabilization + monitoring. Shorter than rolling_upgrade's
# 180+600s because TC3 (7 restart cycles with catch-up checks) follows on
# the same cluster and is itself an extended health exercise.
POST_UPGRADE_STABILIZE_WAIT = 60
POST_HARDFORK_MONITOR_DURATION = 300
# Conservative block-rate floor (blocks/sec) for the hardfork-wait TIMEOUT
# only (run 1 observed ~2.6-3.7 blocks/s live; rolling_upgrade's 5 would
# make the timeout fire before a slow chain reaches gamma).
BLOCK_RATE = 2
# Block height gap check retry parameters.
HEIGHT_CHECK_MAX_RETRIES = 6
HEIGHT_CHECK_RETRY_INTERVAL = 20
# Transaction sender parameters.
TX_INTERVAL = 0.2
TX_RECEIPT_TIMEOUT = 30.0
# Sustained-load pass criteria (rolling_upgrade only logged these; the
# floors are deliberately loose — upgrade windows legitimately time out
# some txs — while still catching a broken tx path).
MIN_TX_CONFIRMED = 100
MIN_TX_SUCCESS_RATE = 0.5

# ── Case timeouts (seconds) ──
FULL_LIVE_TIMEOUT_S = 180
BLOCK_PROGRESS_TIMEOUT_S = 60
TX_BLOCK_GAP_TIMEOUT_S = 30
STOP_TIMEOUT_S = 90
CATCHUP_TIMEOUT_S = 300

# ── Storage-verification parameters (TC1 lineage) ──
TRANSFER_AMOUNTS_ETH = (1, 2)
SET_VALUES = (1, 2)
# Which node the H2 offline probe stops (vfn1: zero quorum impact, and as
# the first-upgraded node its datadir has the longest v2.3.0 runtime on
# v1.7.5-born data).
OFFLINE_PROBE_NODE = "vfn1"
# Which node gets the TC3 kill -9 (a genesis validator, so crash recovery
# runs on a consensus-critical datadir).
CRASH_NODE = "node3"
# Unwind-loop ceiling per node per scan: bounded unwinding can be
# legitimate crash recovery, an unbounded stream means the post-restart
# consistency check is looping.
UNWIND_LINES_MAX = 100

# ── Alpha hardfork timeline (see module docstring / upgrade_lib) ──
# SYSTEM_CALLER: greth crates/chainspec/src/gravity.rs — funds the
# per-block system-tx base-fee bill pre-Alpha; gas-exempt from Alpha on.
SYSTEM_CALLER_ADDRESS = "0x00000000000000000000000000000001625f0000"
# Phase-1 guard: a configured alphaTime must be at least this far in the
# future at test start (upgrades typically finish in ~20 min; anything
# tighter risks Alpha activating while v1.7.5 nodes still run).
ALPHA_MIN_LEAD_S = 25 * 60
# Phase-3 guard: refuse to start the next node's upgrade when Alpha would
# activate within this margin.
ALPHA_UPGRADE_MARGIN_S = 5 * 60
# Phase-11: skip the activation tail when alphaTime is further away than
# this at phase start (far-future / production-like schedules).
ALPHA_TAIL_MAX_WAIT_S = 25 * 60
# Phase-11: how many post-activation blocks must show a constant
# SYSTEM_CALLER balance (pre-Alpha it decreases every block).
ALPHA_TAIL_CONFIRM_BLOCKS = 3
# Phase-11: fresh post-activation history parameters.
TAIL_TRANSFER_ETH = 3
TAIL_SET_VALUE = 3


# ---------------------------------------------------------------------------
# Background tx sender (adapted from rolling_upgrade's TxSender)
# ---------------------------------------------------------------------------


class TxSender:
    """Continuously sends txs to a target node, with a fallback target for
    the window where the primary (vfn1) is itself being upgraded."""

    def __init__(
        self, cluster: Cluster, faucet, primary_node_id: str, fallback_node_id: str
    ):
        self.cluster = cluster
        self.faucet = faucet
        self.primary_node_id = primary_node_id
        self.fallback_node_id = fallback_node_id
        self.recipient = Account.create().address

        self._use_fallback = False
        self._stop_event = asyncio.Event()
        self._task: Optional[asyncio.Task] = None

        self.total_sent = 0
        self.total_confirmed = 0
        self.total_failed = 0
        self.total_timeout = 0
        self.latencies: List[float] = []

    @property
    def _current_node_id(self) -> str:
        return self.fallback_node_id if self._use_fallback else self.primary_node_id

    @property
    def _current_w3(self) -> Web3:
        return self.cluster.get_node(self._current_node_id).w3

    def set_fallback(self, use_fallback: bool):
        old = self._current_node_id
        self._use_fallback = use_fallback
        LOG.info("TxSender target: %s -> %s", old, self._current_node_id)

    async def _send_loop(self):
        chain_id = await asyncio.to_thread(lambda: self._current_w3.eth.chain_id)
        gas_price = Web3.to_wei("100", "gwei")
        nonce = await asyncio.to_thread(
            lambda: self._current_w3.eth.get_transaction_count(
                self.faucet.address, "pending"
            )
        )
        LOG.info(
            "TxSender started: target=%s nonce=%d recipient=%s",
            self._current_node_id,
            nonce,
            self.recipient,
        )

        while not self._stop_event.is_set():
            w3 = self._current_w3
            tx = {
                "nonce": nonce,
                "to": self.recipient,
                "value": 0,
                "gas": 21000,
                "gasPrice": gas_price,
                "chainId": chain_id,
            }
            try:
                signed = w3.eth.account.sign_transaction(tx, self.faucet.key)
                send_time = time.monotonic()
                tx_hash = await asyncio.to_thread(
                    lambda: w3.eth.send_raw_transaction(signed.raw_transaction)
                )
                self.total_sent += 1
                nonce += 1

                confirmed = False
                while time.monotonic() - send_time < TX_RECEIPT_TIMEOUT:
                    try:
                        receipt = await asyncio.to_thread(
                            lambda: w3.eth.get_transaction_receipt(tx_hash)
                        )
                        if receipt:
                            self.latencies.append(time.monotonic() - send_time)
                            self.total_confirmed += 1
                            confirmed = True
                            break
                    except Exception:
                        pass
                    await asyncio.sleep(0.1)

                if not confirmed:
                    self.total_timeout += 1
                    LOG.warning("TxSender: tx %s... timed out", tx_hash.hex()[:10])
                    try:
                        nonce = await asyncio.to_thread(
                            lambda: self._current_w3.eth.get_transaction_count(
                                self.faucet.address, "pending"
                            )
                        )
                        LOG.info("TxSender: nonce re-synced to %d", nonce)
                    except Exception:
                        pass

            except Exception as e:
                self.total_failed += 1
                LOG.warning(
                    "TxSender: send failed (%s): %s", self._current_node_id, e
                )
                await asyncio.sleep(1)
                try:
                    nonce = await asyncio.to_thread(
                        lambda: self._current_w3.eth.get_transaction_count(
                            self.faucet.address, "pending"
                        )
                    )
                except Exception:
                    pass
                continue

            await asyncio.sleep(TX_INTERVAL)

    def start(self):
        self._task = asyncio.create_task(self._send_loop())

    async def stop(self):
        self._stop_event.set()
        if self._task:
            await self._task
            self._task = None

    @property
    def success_rate(self) -> float:
        return self.total_confirmed / self.total_sent if self.total_sent else 0.0

    def log_stats(self):
        LOG.info("=" * 60)
        LOG.info("TRANSACTION STATISTICS")
        LOG.info("=" * 60)
        LOG.info("Total Sent:      %d", self.total_sent)
        LOG.info("Confirmed:       %d", self.total_confirmed)
        LOG.info("Failed (send):   %d", self.total_failed)
        LOG.info("Timed Out:       %d", self.total_timeout)
        if self.total_sent:
            LOG.info("Success Rate:    %.1f%%", self.success_rate * 100)
        if self.latencies:
            sorted_lat = sorted(self.latencies)

            def percentile(p: float) -> float:
                k = (len(sorted_lat) - 1) * (p / 100.0)
                f, c = math.floor(k), math.ceil(k)
                if f == c:
                    return sorted_lat[int(k)]
                return sorted_lat[f] + (sorted_lat[c] - sorted_lat[f]) * (k - f)

            LOG.info("Min/Avg/Max:     %.4fs / %.4fs / %.4fs",
                     sorted_lat[0], statistics.mean(sorted_lat), sorted_lat[-1])
            LOG.info("P50/P90/P99:     %.4fs / %.4fs / %.4fs",
                     percentile(50), percentile(90), percentile(99))
        LOG.info("=" * 60)


# ---------------------------------------------------------------------------
# Cluster-wide height helpers (rolling_upgrade lineage)
# ---------------------------------------------------------------------------


async def get_block_heights(nodes: List[Node]) -> Dict[str, int]:
    """Block heights of the given nodes, concurrently; raises on failure."""

    async def _get_height(node: Node):
        height = await asyncio.to_thread(lambda: node.w3.eth.block_number)
        return node.id, height

    results = await asyncio.gather(*[_get_height(n) for n in nodes])
    return dict(results)


async def check_height_gap_ok(cluster: Cluster, max_gap: int = MAX_HEIGHT_GAP) -> bool:
    running = []
    for node in cluster.nodes.values():
        state, _ = await node.get_state()
        if state == NodeState.RUNNING:
            running.append(node)
    if len(running) < 2:
        LOG.info("Less than 2 running nodes, height gap check trivially passes")
        return True

    heights = await get_block_heights(running)
    gap = max(heights.values()) - min(heights.values())
    LOG.info("Block heights: %s | gap=%d (max_allowed=%d)", heights, gap, max_gap)
    if gap >= max_gap:
        LOG.warning("Block height gap %d >= %d", gap, max_gap)
        return False
    return True


async def wait_for_height_gap_ok(
    cluster: Cluster,
    max_retries: int = HEIGHT_CHECK_MAX_RETRIES,
    retry_interval: int = HEIGHT_CHECK_RETRY_INTERVAL,
) -> bool:
    for attempt in range(1, max_retries + 1):
        if await check_height_gap_ok(cluster):
            return True
        if attempt < max_retries:
            LOG.info(
                "Height gap too large, retrying in %ds (attempt %d/%d)...",
                retry_interval,
                attempt,
                max_retries,
            )
            await asyncio.sleep(retry_interval)
    LOG.error("Block height gap still too large after %d retries", max_retries)
    return False


def get_max_hardfork_block() -> int:
    """Max hardfork block from the rendered genesis.toml; 0 when none."""
    if not GENESIS_TOML_PATH.exists():
        LOG.warning("genesis.toml not found at %s", GENESIS_TOML_PATH)
        return 0
    with open(GENESIS_TOML_PATH, "rb") as f:
        config = tomllib.load(f)
    hardforks = config.get("genesis", {}).get("hardforks", {})
    # Only block-number forks gate the timeline (alphaTime is a timestamp).
    block_forks = {k: v for k, v in hardforks.items() if k.endswith("Block")}
    if not block_forks:
        LOG.info("No block hardforks configured in genesis.toml")
        return 0
    max_block = max(block_forks.values())
    LOG.info("Hardfork config: %s | max hardfork block: %d", hardforks, max_block)
    return max_block


def read_alpha_time() -> Optional[int]:
    """alphaTime (unix seconds) from the rendered genesis.toml; None when
    unset (the mainnet posture: Alpha never activates)."""
    if not GENESIS_TOML_PATH.exists():
        return None
    with open(GENESIS_TOML_PATH, "rb") as f:
        config = tomllib.load(f)
    value = config.get("genesis", {}).get("hardforks", {}).get("alphaTime")
    return int(value) if value is not None else None


# ---------------------------------------------------------------------------
# Node lifecycle helpers
# ---------------------------------------------------------------------------


async def stop_node_and_wait_exit(node: Node) -> None:
    """Graceful stop plus real-process-exit wait (frees ports + RocksDB)."""
    pid = read_node_pid(node)
    assert pid, f"cannot read {node.id} PID from {node.pid_file} before stop"
    assert await node.stop(), f"failed to stop {node.id}"
    await wait_for_process_exit(pid, STOP_TIMEOUT_S)


def _is_upgrade_target(binary: Path, new_binary: Path) -> bool:
    """True when ``binary`` is the upgrade target: same inode (hardlink) or
    an identical-size copy (the cross-device fallback in swap_node_binary
    preserves size and mtime via copy2)."""
    return os.path.samefile(binary, new_binary) or (
        Path(binary).stat().st_size == new_binary.stat().st_size
    )


def swap_node_binary(node: Node, new_binary: Path) -> None:
    """Replace the node's hardlinked binary with the upgrade target."""
    bin_path = node._infra_path / "bin" / "gravity_node"
    LOG.info("[%s] Replacing binary: %s", node.id, bin_path)
    if bin_path.exists():
        bin_path.unlink()
    try:
        os.link(str(new_binary), str(bin_path))
    except OSError:
        # Cross-device (e.g. tmpfs base_dir): fall back to a copy.
        shutil.copy2(str(new_binary), str(bin_path))
    assert _is_upgrade_target(bin_path, new_binary), (
        f"{node.id}: binary swap did not take effect at {bin_path}"
    )


async def upgrade_node(node: Node, new_binary: Path) -> None:
    """Upgrade a single node: stop, wait real exit, swap binary, start."""
    LOG.info("[%s] Stopping node...", node.id)
    await stop_node_and_wait_exit(node)
    swap_node_binary(node, new_binary)
    LOG.info("[%s] Starting node with new binary...", node.id)
    assert await node.start(), f"failed to start {node.id} after upgrade"
    LOG.info("[%s] Upgrade complete", node.id)


async def restart_node_and_catch_up(cluster: Cluster, node: Node) -> None:
    """Start a stopped node and require rejoin: RPC up, blocks advancing,
    height gap back inside the ceiling."""
    assert await node.start(), f"failed to start {node.id}"
    assert await node.wait_for_block_increase(
        timeout=CATCHUP_TIMEOUT_S, delta=2
    ), f"{node.id}: no block progress after restart"
    assert await wait_for_height_gap_ok(
        cluster
    ), f"{node.id}: cluster height gap did not close after restart"


async def _wait_until_height(node: Node, target: int, timeout: float) -> int:
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


# ---------------------------------------------------------------------------
# Storage verification helpers
# ---------------------------------------------------------------------------


def _load_anchor_target():
    artifact = json.loads((TEST_DIR / "contracts" / "AnchorTarget.json").read_text())
    return artifact["abi"], artifact["bytecode"]


async def _confirmed_tx_point(node: Node, result, label: str) -> lib.TxPoint:
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


async def build_history(node: Node, faucet) -> lib.OnChainHistory:
    """Transfers + contract deploy + storage writes with events, on the
    OLD binary — this is the history whose readability the case proves."""
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


async def replay_anchors_on_all_nodes(
    cluster: Cluster, anchors: AnchorSet, stage: str
) -> None:
    """Replay the anchor set against EVERY node; any mismatch fails.

    A post-upgrade mismatch means the new binary misreads history the old
    binary wrote — a product bug this case exists to catch. Do not weaken.
    """
    for node in cluster.nodes.values():
        report = await asyncio.to_thread(replay_anchors, node.w3, anchors)
        assert report.ok, f"[{stage}] anchor mismatch on {node.id}:\n{report.summary()}"
        LOG.info(
            "[%s] %s: anchor replay OK (%d/%d matched)",
            stage,
            node.id,
            report.matched,
            report.total,
        )


def node_log_files(node: Node) -> List[Path]:
    """The node's execution log files: stdout/stderr capture (debug.log,
    where reth logs ERROR-level lines and panics land) plus the reth
    file logs (log.file.filter=info) under execution_logs/ — deploy.sh
    puts them at <node>/execution_logs (or <node>/logs/execution_logs
    with a custom cluster.log_dir); both locations are globbed."""
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
    """Scan every node's execution logs: zero storage/decode-class errors,
    unwind mentions below the loop ceiling."""
    for node in cluster.nodes.values():
        files = node_log_files(node)
        assert files, f"[{stage}] {node.id}: no execution log files found"
        for log_file in files:
            with open(log_file, errors="replace") as fh:
                result = upgrade_lib.scan_log_lines(fh)
            LOG.info(
                "[%s] %s: scanned %s: %d lines, %d errors, %d unwind",
                stage,
                node.id,
                log_file.name,
                result.lines_scanned,
                result.error_count,
                result.unwind_count,
            )
            assert result.error_count == 0, (
                f"[{stage}] {node.id}: storage/decode errors in {log_file}:\n"
                f"{result.summary()}"
            )
            assert result.unwind_count <= UNWIND_LINES_MAX, (
                f"[{stage}] {node.id}: unwind lines exceed ceiling "
                f"({result.unwind_count} > {UNWIND_LINES_MAX}) in {log_file} — "
                f"possible unwind loop:\n{result.summary()}"
            )


# ---------------------------------------------------------------------------
# Context + phases
# ---------------------------------------------------------------------------


@dataclass
class UpgradeContext:
    cluster: Cluster
    output_dir: Path
    history: Optional[lib.OnChainHistory] = None
    max_history_block: int = 0
    anchors: Optional[AnchorSet] = None
    anchors_path: Optional[Path] = None
    tx_sender: Optional[TxSender] = None
    upgrade_order: List[str] = field(default_factory=list)
    alpha_time: Optional[int] = None
    phase_durations: Dict[str, float] = field(default_factory=dict)


async def phase_1_bootstrap_old_chain(ctx: UpgradeContext) -> None:
    LOG.info("[Phase 1] Bootstrapping cluster on the OLD binary...")
    assert NEW_BINARY_PATH.exists(), (
        f"New binary not found at {NEW_BINARY_PATH}. Build it first or set "
        f"GRAVITY_NEW_BINARY."
    )
    # Guard: the deployed (old) binaries must NOT already be the upgrade
    # target — otherwise the case would silently prove nothing.
    for node in ctx.cluster.nodes.values():
        deployed = node._infra_path / "bin" / "gravity_node"
        assert deployed.exists(), f"{node.id}: deployed binary missing: {deployed}"
        assert not os.path.samefile(deployed, NEW_BINARY_PATH), (
            f"{node.id}: deployed binary IS the upgrade target "
            f"({NEW_BINARY_PATH}) — check test_params.toml [source]"
        )

    # Guard: Alpha timeline. Alpha gates v2.3.0 behavior changes; it must
    # not activate before the fleet finishes upgrading (module docstring).
    ctx.alpha_time = read_alpha_time()
    alpha_error = upgrade_lib.alpha_preflight_error(
        ctx.alpha_time, time.time(), ALPHA_MIN_LEAD_S
    )
    assert alpha_error is None, f"[Phase 1] {alpha_error}"
    LOG.info(
        "[Phase 1] Alpha timeline: %s",
        "not scheduled (never activates)"
        if ctx.alpha_time is None
        else f"alphaTime={ctx.alpha_time} (in {ctx.alpha_time - time.time():.0f}s)",
    )

    assert await ctx.cluster.set_full_live(
        timeout=FULL_LIVE_TIMEOUT_S
    ), "failed to bring all nodes RUNNING on the old binary"
    live = await ctx.cluster.get_live_nodes()
    LOG.info("[Phase 1] All %d nodes running: %s", len(live), [n.id for n in live])
    assert await ctx.cluster.check_block_increasing(
        timeout=BLOCK_PROGRESS_TIMEOUT_S
    ), "block production not working on the old binary"


async def phase_2_history_and_anchors(ctx: UpgradeContext) -> None:
    LOG.info("[Phase 2] Building on-chain history on v1.7.5...")
    node = ctx.cluster.get_node("node1")
    faucet = ctx.cluster.faucet
    assert faucet, "faucet account not configured (genesis.faucet)"

    ctx.history = await build_history(node, faucet)
    ctx.max_history_block = max(
        p.block_number
        for p in (*ctx.history.transfers, ctx.history.deploy, *ctx.history.sets)
    )

    # Balance/slot anchors must sample HISTORICAL blocks: make sure head is
    # strictly past everything the history touched.
    head = await _wait_until_height(
        node, ctx.max_history_block + 1, BLOCK_PROGRESS_TIMEOUT_S
    )
    spec = lib.build_anchor_spec(ctx.history)
    ctx.anchors = collect_anchors(
        node.w3,
        spec,
        meta={
            "case": "storage_v2_upgrade",
            "node": node.id,
            "binary": "old",
            "head_at_collection": head,
            "max_history_block": ctx.max_history_block,
        },
    )
    lib.assert_history_is_anchorable(
        ctx.anchors,
        ctx.history,
        transfer_amounts_wei=[
            Web3.to_wei(amount, "ether") for amount in TRANSFER_AMOUNTS_ETH
        ],
        set_values=SET_VALUES,
    )
    ctx.anchors_path = ctx.output_dir / "storage_v2_upgrade" / "anchors.json"
    ctx.anchors.save(ctx.anchors_path)
    LOG.info(
        "[Phase 2] %d anchors collected on the old binary, saved to %s",
        len(ctx.anchors),
        ctx.anchors_path,
    )


async def phase_3_rolling_upgrade(ctx: UpgradeContext) -> None:
    LOG.info("[Phase 3] Rolling upgrade to %s ...", NEW_BINARY_PATH)
    ctx.upgrade_order = upgrade_lib.build_upgrade_order(
        list(ctx.cluster.nodes.keys()), first="vfn1"
    )
    LOG.info("[Phase 3] Upgrade order: %s", ctx.upgrade_order)

    for i, node_id in enumerate(ctx.upgrade_order):
        node = ctx.cluster.get_node(node_id)
        LOG.info(
            "[Phase 3] Upgrading node %d/%d: %s (role=%s)",
            i + 1,
            len(ctx.upgrade_order),
            node_id,
            node.role.value,
        )

        assert await wait_for_height_gap_ok(ctx.cluster), (
            f"height gap too large before upgrading {node_id}"
        )

        # Alpha-margin guard: never start another upgrade if Alpha would
        # activate mid-flight — v1.7.5 nodes cannot execute post-Alpha
        # semantics (see module docstring for the failure fingerprint).
        if ctx.alpha_time is not None:
            remaining = ctx.alpha_time - time.time()
            assert remaining > ALPHA_UPGRADE_MARGIN_S, (
                f"alphaTime activates in {remaining:.0f}s (< margin "
                f"{ALPHA_UPGRADE_MARGIN_S}s) but {node_id} is not upgraded "
                f"yet — schedule alphaTime later (test_params '+NNm')"
            )

        if node_id == ctx.tx_sender.primary_node_id:
            ctx.tx_sender.set_fallback(True)
        await upgrade_node(node, NEW_BINARY_PATH)
        if node_id == ctx.tx_sender.primary_node_id:
            ctx.tx_sender.set_fallback(False)

        if i < len(ctx.upgrade_order) - 1:
            LOG.info("[Phase 3] Waiting %ds before next upgrade...", INTER_NODE_WAIT)
            await asyncio.sleep(INTER_NODE_WAIT)

    LOG.info("[Phase 3] All nodes upgraded")


async def phase_4_replay_anchors_post_upgrade(ctx: UpgradeContext) -> None:
    LOG.info("[Phase 4] Replaying anchors on every upgraded node...")
    reloaded = AnchorSet.load(ctx.anchors_path)
    assert len(reloaded) == len(ctx.anchors), "anchor set changed across save/load"
    await replay_anchors_on_all_nodes(ctx.cluster, reloaded, stage="post-upgrade")


async def phase_5_hardfork_and_health(ctx: UpgradeContext) -> None:
    max_hardfork_block = get_max_hardfork_block()
    if max_hardfork_block > 0:
        timeout_secs = (max_hardfork_block / BLOCK_RATE) + 300
        LOG.info(
            "[Phase 5] Waiting for all nodes past hardfork block %d (timeout=%ds)...",
            max_hardfork_block,
            int(timeout_secs),
        )
        wait_start = time.monotonic()
        all_past = False
        while time.monotonic() - wait_start < timeout_secs:
            await asyncio.sleep(10)
            heights = await get_block_heights(list(ctx.cluster.nodes.values()))
            min_h, max_h = min(heights.values()), max(heights.values())
            gap = max_h - min_h
            LOG.info(
                "[Phase 5 @%ds] min=%d max=%d gap=%d target=%d",
                int(time.monotonic() - wait_start),
                min_h,
                max_h,
                gap,
                max_hardfork_block,
            )
            assert gap < MAX_HEIGHT_GAP, (
                f"height gap {gap} >= {MAX_HEIGHT_GAP} while waiting for hardfork"
            )
            if min_h > max_hardfork_block:
                all_past = True
                break
        assert all_past, (
            f"timed out waiting for hardfork block {max_hardfork_block} "
            f"after {int(timeout_secs)}s"
        )
        LOG.info("[Phase 5] All nodes past hardfork block %d", max_hardfork_block)
    else:
        LOG.info("[Phase 5] No block hardforks configured, skipping hardfork wait")

    LOG.info("[Phase 5] Stabilizing %ds...", POST_UPGRADE_STABILIZE_WAIT)
    await asyncio.sleep(POST_UPGRADE_STABILIZE_WAIT)

    LOG.info("[Phase 5] Health monitoring %ds...", POST_HARDFORK_MONITOR_DURATION)
    monitor_start = time.monotonic()
    checks = 0
    while time.monotonic() - monitor_start < POST_HARDFORK_MONITOR_DURATION:
        await asyncio.sleep(10)
        checks += 1
        heights = await get_block_heights(list(ctx.cluster.nodes.values()))
        gap = max(heights.values()) - min(heights.values())
        assert gap < MAX_HEIGHT_GAP, (
            f"height gap {gap} >= {MAX_HEIGHT_GAP} during post-upgrade monitoring"
        )
    LOG.info("[Phase 5] Health monitoring done (%d checks, all healthy)", checks)


async def phase_6_tx_stats_and_log_scan(ctx: UpgradeContext) -> None:
    LOG.info("[Phase 6] Stopping tx sender and checking sustained load...")
    await ctx.tx_sender.stop()
    ctx.tx_sender.log_stats()
    assert ctx.tx_sender.total_confirmed >= MIN_TX_CONFIRMED, (
        f"only {ctx.tx_sender.total_confirmed} txs confirmed during the run "
        f"(need >= {MIN_TX_CONFIRMED}) — the sustained load was not sustained"
    )
    assert ctx.tx_sender.success_rate >= MIN_TX_SUCCESS_RATE, (
        f"tx success rate {ctx.tx_sender.success_rate:.1%} below floor "
        f"{MIN_TX_SUCCESS_RATE:.0%} "
        f"({ctx.tx_sender.total_confirmed}/{ctx.tx_sender.total_sent})"
    )
    LOG.info("[Phase 6] Scanning node logs (post-upgrade window)...")
    scan_all_node_logs(ctx.cluster, stage="post-upgrade")


async def phase_7_offline_db_assertions(ctx: UpgradeContext) -> None:
    LOG.info("[Phase 7] H2 offline assertions on %s ...", OFFLINE_PROBE_NODE)
    node = ctx.cluster.get_node(OFFLINE_PROBE_NODE)
    await stop_node_and_wait_exit(node)

    env = lib.derive_offline_env(ctx.cluster.base_dir, node.id)
    assert Path(env.binary).is_file(), f"node binary missing: {env.binary}"
    assert Path(env.datadir).is_dir(), f"reth datadir missing: {env.datadir}"
    assert Path(env.chain).is_file(), f"chain spec missing: {env.chain}"
    # The offline commands run the node's own (post-swap) binary — i.e. the
    # NEW binary probing the upgraded datadir.
    assert _is_upgrade_target(Path(env.binary), NEW_BINARY_PATH)

    # 7a. THE upgraded-datadir criterion: gravity_storage_settings must be
    # MISSING. v1.7.5 predates the settings entry and only a fresh v2.3.0
    # init_genesis writes it (TC1 asserts PRESENT_LEGACY there); the
    # upgrade path must not have written one behind our back. PRESENT_*
    # here means the upgrade mutated storage metadata — a product bug.
    probe = read_storage_settings(env)
    assert probe.error is None, probe.summary()
    assert probe.state is SettingsState.MISSING, (
        "upgraded datadir has a gravity_storage_settings entry "
        f"(state={probe.state}) — the upgrade path must not write settings; "
        "only fresh init_genesis does. Product bug unless proven otherwise.\n"
        + probe.summary()
    )
    LOG.info("[Phase 7] storage settings: MISSING (as required)")

    # 7b. Legacy layout on disk: no changeset segment files, no .csoff
    # sidecars under the node's static-files dir.
    layout = inspect_changeset_static_files(env.datadir, env.static_files_dir)
    assert layout.exists, f"static-files dir missing: {layout.static_files_dir}"
    assert not layout.has_segment_files, (
        f"upgraded legacy node has changeset segment files: "
        f"{layout.account_segments + layout.storage_segments}"
    )
    assert not layout.has_sidecar_files, (
        f"upgraded legacy node has .csoff sidecars: "
        f"{layout.account_sidecars + layout.storage_sidecars}"
    )

    # 7c. Positive control: the v1.7.5-written history + upgrade-window
    # traffic really live in the changeset tables.
    for table in (ACCOUNT_CHANGESETS_TABLE, STORAGE_CHANGESETS_TABLE):
        count = count_table_entries(env, table)
        assert count.error is None, count.summary()
        assert count.count > 0, (
            f"{table} is empty on an upgraded legacy-layout node:\n"
            + count.summary()
        )
        LOG.info("[Phase 7] %s entries: %d", table, count.count)

    LOG.info("[Phase 7] Restarting %s ...", node.id)
    await restart_node_and_catch_up(ctx.cluster, node)


async def phase_8_graceful_restart_cycle(ctx: UpgradeContext) -> None:
    LOG.info("[Phase 8] TC3a: graceful stop -> start cycle for every node...")
    for node_id in ctx.upgrade_order:
        node = ctx.cluster.get_node(node_id)
        LOG.info("[Phase 8] Restarting %s ...", node_id)
        await stop_node_and_wait_exit(node)
        await restart_node_and_catch_up(ctx.cluster, node)
    LOG.info("[Phase 8] All nodes survived a graceful restart")


async def phase_9_crash_restart(ctx: UpgradeContext) -> None:
    LOG.info("[Phase 9] TC3b: crash-restarting %s (kill -9)...", CRASH_NODE)
    node = ctx.cluster.get_node(CRASH_NODE)
    assert await node.force_kill(), f"failed to force-kill {CRASH_NODE}"
    # Unclean shutdown on purpose: the next start must run recovery (pipe
    # consistency checks) and still rejoin.
    await restart_node_and_catch_up(ctx.cluster, node)
    LOG.info("[Phase 9] %s recovered from SIGKILL", CRASH_NODE)


async def phase_10_final_verification(ctx: UpgradeContext) -> None:
    LOG.info("[Phase 10] Final verification...")
    reloaded = AnchorSet.load(ctx.anchors_path)
    await replay_anchors_on_all_nodes(ctx.cluster, reloaded, stage="post-restart")
    scan_all_node_logs(ctx.cluster, stage="post-restart")
    assert await check_height_gap_ok(ctx.cluster), "final height gap check failed"
    # TC4 extension point: run `db migrate-changesets` on a stopped node
    # here and assert the static-file layout flip (settings ->
    # PRESENT_STATIC_FILES, tables emptied, segments + .csoff present).


async def _system_caller_balance(node: Node, block_number: int) -> int:
    return await asyncio.to_thread(
        lambda: node.w3.eth.get_balance(
            Web3.to_checksum_address(SYSTEM_CALLER_ADDRESS), block_number
        )
    )


async def phase_11_alpha_activation_tail(ctx: UpgradeContext) -> None:
    """Conditional: exercise the production sequence "upgrade the whole
    fleet first, then let Alpha activate" (mainnet has Alpha unscheduled;
    v2.3.0 gates system-tx gas exemption on it — see module docstring)."""
    wait_s = upgrade_lib.alpha_tail_wait_s(
        ctx.alpha_time, time.time(), ALPHA_TAIL_MAX_WAIT_S
    )
    if wait_s is None:
        LOG.info(
            "[Phase 11] Alpha activation tail skipped (alphaTime=%s)",
            ctx.alpha_time,
        )
        return

    node = ctx.cluster.get_node("node1")
    LOG.info(
        "[Phase 11] Waiting for the chain to cross alphaTime=%d (~%.0fs away)...",
        ctx.alpha_time,
        wait_s,
    )
    # Activation = first block whose timestamp >= alphaTime. Poll the head;
    # generous buffer past the wall-clock ETA for block cadence.
    deadline = time.monotonic() + wait_s + 300
    activation_head = None
    while time.monotonic() < deadline:
        head = await asyncio.to_thread(lambda: node.w3.eth.get_block("latest"))
        if head["timestamp"] >= ctx.alpha_time:
            activation_head = int(head["number"])
            break
        await asyncio.sleep(2)
    assert activation_head is not None, (
        f"chain never produced a block with timestamp >= alphaTime="
        f"{ctx.alpha_time} (head timestamp lagging?)"
    )
    LOG.info("[Phase 11] Alpha active as of block <= %d", activation_head)

    # Liveness across the activation boundary.
    assert await ctx.cluster.check_block_increasing(
        timeout=BLOCK_PROGRESS_TIMEOUT_S, delta=2
    ), "block production stalled after Alpha activation"
    assert await wait_for_height_gap_ok(ctx.cluster), (
        "height gap did not stay closed after Alpha activation"
    )

    # Gas-exempt semantic: pre-Alpha every block debits SYSTEM_CALLER by
    # gas_used * basefee (verified live on v1.7.5); post-activation the
    # balance must stop moving. (v2.3.0 additionally zeroes it via the
    # one-shot Alpha migration — logged, not asserted, to avoid coupling
    # the case to that implementation detail.)
    confirm_from = activation_head + 1
    await _wait_until_height(
        node, confirm_from + ALPHA_TAIL_CONFIRM_BLOCKS, BLOCK_PROGRESS_TIMEOUT_S * 2
    )
    balances = {
        n: await _system_caller_balance(node, n)
        for n in range(confirm_from, confirm_from + ALPHA_TAIL_CONFIRM_BLOCKS + 1)
    }
    LOG.info("[Phase 11] SYSTEM_CALLER balances post-activation: %s", balances)
    assert len(set(balances.values())) == 1, (
        f"SYSTEM_CALLER balance still moving after Alpha activation "
        f"(gas exemption not in effect?): {balances}"
    )

    # Pre-upgrade history must still read identically post-Alpha.
    reloaded = AnchorSet.load(ctx.anchors_path)
    await replay_anchors_on_all_nodes(ctx.cluster, reloaded, stage="post-alpha")

    # Fresh post-activation history: one transfer + one set() on the
    # existing contract, anchored and replayed on every node.
    faucet = ctx.cluster.faucet
    tb = TransactionBuilder(node.w3, faucet)
    tail_recipient = Account.create()
    transfer = await _confirmed_tx_point(
        node,
        await tb.send_ether(
            tail_recipient.address, Web3.to_wei(TAIL_TRANSFER_ETH, "ether")
        ),
        f"post-alpha transfer {TAIL_TRANSFER_ETH} ETH",
    )
    set_point = await _confirmed_tx_point(
        node,
        await tb.build_and_send_tx(
            to=ctx.history.contract, data=lib.encode_set_call(TAIL_SET_VALUE)
        ),
        f"post-alpha set({TAIL_SET_VALUE})",
    )
    await _wait_until_height(
        node, set_point.block_number + 1, BLOCK_PROGRESS_TIMEOUT_S
    )
    tail_spec = AnchorSpec(
        balances=[(tail_recipient.address, transfer.block_number)],
        storage_slots=[(ctx.history.contract, lib.VALUE_SLOT, set_point.block_number)],
        tx_hashes=[transfer.tx_hash, set_point.tx_hash],
        block_numbers=sorted({transfer.block_number, set_point.block_number}),
        log_ranges=[
            (set_point.block_number, set_point.block_number, ctx.history.contract)
        ],
    )
    tail_anchors = collect_anchors(
        node.w3,
        tail_spec,
        meta={"case": "storage_v2_upgrade", "stage": "post-alpha", "node": node.id},
    )
    tail_anchors.save(ctx.output_dir / "storage_v2_upgrade" / "anchors_post_alpha.json")
    await replay_anchors_on_all_nodes(
        ctx.cluster, tail_anchors, stage="post-alpha-fresh"
    )
    LOG.info("[Phase 11] Alpha activation tail complete")


# ---------------------------------------------------------------------------
# The test
# ---------------------------------------------------------------------------


async def _run_phase(ctx: UpgradeContext, phase) -> None:
    start = time.monotonic()
    await phase(ctx)
    ctx.phase_durations[phase.__name__] = time.monotonic() - start


@pytest.mark.asyncio
async def test_storage_v2_upgrade(cluster: Cluster, output_dir: Path):
    LOG.info("=" * 70)
    LOG.info("Test: storage_v2_upgrade (v1.7.5 datadir -> v2.3.0, TC2+TC3)")
    LOG.info("New binary: %s", NEW_BINARY_PATH)
    LOG.info("=" * 70)

    ctx = UpgradeContext(cluster=cluster, output_dir=output_dir)
    try:
        await _run_phase(ctx, phase_1_bootstrap_old_chain)
        await _run_phase(ctx, phase_2_history_and_anchors)

        ctx.tx_sender = TxSender(
            cluster, cluster.faucet, primary_node_id="vfn1", fallback_node_id="node1"
        )
        ctx.tx_sender.start()
        LOG.info("Background transaction sender started (target: vfn1)")

        await _run_phase(ctx, phase_3_rolling_upgrade)
        await _run_phase(ctx, phase_4_replay_anchors_post_upgrade)
        await _run_phase(ctx, phase_5_hardfork_and_health)
        await _run_phase(ctx, phase_6_tx_stats_and_log_scan)
        await _run_phase(ctx, phase_7_offline_db_assertions)
        await _run_phase(ctx, phase_8_graceful_restart_cycle)
        await _run_phase(ctx, phase_9_crash_restart)
        await _run_phase(ctx, phase_10_final_verification)
        await _run_phase(ctx, phase_11_alpha_activation_tail)
    finally:
        # Idempotent: already stopped in phase 6 on the happy path.
        if ctx.tx_sender:
            await ctx.tx_sender.stop()
        LOG.info("Phase durations: %s", {
            name: f"{seconds:.0f}s" for name, seconds in ctx.phase_durations.items()
        })

    LOG.info("storage_v2_upgrade PASSED")
