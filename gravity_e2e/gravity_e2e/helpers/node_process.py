"""
Node process-exit plumbing shared by storage-v2 cases.

``NodeState.STOPPED`` only means the node's PID file is gone — the per-node
stop.sh deletes it right after sending SIGTERM, while gravity_node is still
flushing and closing RocksDB. Running an offline ``gravity_node db`` command
in that window fails on the still-held RocksDB LOCK file (observed live in
storage_v2_baseline: "While lock file: .../db/state/LOCK: Resource
temporarily unavailable"). The kernel releases the lock at process exit, so
real exit is the correct gate before any offline db work.

Usage (the PID must be read BEFORE stop.sh deletes the PID file)::

    from gravity_e2e.helpers.node_process import (
        read_node_pid, wait_for_process_exit,
    )

    pid = read_node_pid(node)
    assert pid, f"cannot read node PID from {node.pid_file} before stop"
    assert await cluster.set_node(node.id, NodeState.STOPPED, timeout=90)
    await wait_for_process_exit(pid, timeout=90)
    # ... safe to run offline db commands against the node's datadir ...
"""

from __future__ import annotations

import asyncio
import logging
import os
import time
from typing import Optional

LOG = logging.getLogger(__name__)


def read_node_pid(node) -> Optional[int]:
    """Read a node's PID file; None when missing or unparseable.

    Must be called BEFORE the node's stop.sh runs — stop.sh deletes the
    PID file as part of stopping.
    """
    try:
        return int(node.pid_file.read_text().strip())
    except (FileNotFoundError, ValueError):
        return None


async def wait_for_process_exit(pid: int, timeout: float) -> None:
    """Wait until the process is really gone (see module docstring).

    Raises AssertionError when the process is still alive after ``timeout``
    seconds, so pytest cases fail loudly instead of racing the RocksDB lock.
    """
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            LOG.info("gravity_node pid %d fully exited", pid)
            return
        await asyncio.sleep(0.5)
    raise AssertionError(
        f"gravity_node pid {pid} still alive {timeout}s after stop; "
        f"offline db commands would race its RocksDB lock"
    )


async def wait_for_pid_file(
    pid_file, expected_pid: Optional[int] = None, timeout: float = 10.0
) -> int:
    """Wait until ``pid_file`` exists, parses, (optionally) matches
    ``expected_pid``, and points at a LIVE process; returns the pid.

    Root fix for the stop.sh false-success family (attempts 4/8): every
    generated per-node start.sh writes ``script/node.pid`` after
    spawning gravity_node, so a start that "succeeded" without a
    consistent pid file is a broken start — catching it HERE turns the
    later "No PID file found; exit 0" lie into an immediate, attributable
    failure. Raises AssertionError on timeout.
    """
    from pathlib import Path

    pid_file = Path(pid_file)
    deadline = time.monotonic() + timeout
    last_seen = None
    while time.monotonic() < deadline:
        try:
            pid = int(pid_file.read_text().strip())
            last_seen = pid
            if expected_pid is None or pid == expected_pid:
                try:
                    os.kill(pid, 0)
                    return pid
                except ProcessLookupError:
                    pass  # written but already dead — keep waiting/diagnose
        except (FileNotFoundError, ValueError):
            pass
        await asyncio.sleep(0.2)
    raise AssertionError(
        f"pid file {pid_file} not consistent within {timeout}s "
        f"(expected pid {expected_pid}, last seen {last_seen}) — the "
        f"start script did not record a live pid"
    )


async def stop_node_and_wait_exit(
    node,
    timeout: float = 90.0,
    grace: float = 10.0,
    allow_sigkill: bool = False,
    sigkill_grace: float = 30.0,
) -> None:
    """Graceful stop gated on the REAL process, not the pid-file
    bookkeeping.

    The per-node stop.sh stops whatever ``script/node.pid`` points at and
    exits 0 even when that file is missing ("No PID file found") — so
    ``Node.stop()`` can report "stopped and verified" (state == STOPPED
    just means the pid file is gone) while the actual gravity_node keeps
    running. Observed live (storage_v2_fresh_sync attempt4): the recorded
    pid survived a "successful" stop untouched and the offline-db step
    then raced a running node.

    This helper reads the pid BEFORE stopping, gives the stop path
    ``grace`` seconds to take effect, and — when the process is still
    alive — sends SIGTERM to the recorded pid directly (a duplicate
    SIGTERM to an already-stopping process is harmless) before waiting
    out the real exit. It also clears a leftover pid file after a direct
    kill so the next start.sh sees consistent bookkeeping.

    ``allow_sigkill``: opt-in LAST-RESORT escalation for callers whose
    node data has zero value (e.g. a fresh-init datadir about to be
    migrated/wiped). attempt8 live: a ~12s-old gravity_node IGNORED the
    direct SIGTERM for 109s (early-init graceful-shutdown windows are a
    suspected product defect under separate investigation) — with the
    flag set, a SIGTERM ignored for ``sigkill_grace`` seconds is logged
    as a WARNING (product-defect evidence, never silent) and followed by
    SIGKILL. Probe/long-lived stop paths must keep the default False:
    auto-SIGKILL there would mask real shutdown regressions.
    """
    pid = read_node_pid(node)
    assert pid, f"cannot read {node.id} PID from {node.pid_file} before stop"
    stopped = await node.stop()
    if not stopped:
        LOG.warning(
            "%s: Node.stop() reported failure; falling through to the "
            "direct-signal path for pid %d",
            node.id,
            pid,
        )
    direct_signal = False
    try:
        await wait_for_process_exit(pid, grace)
        return
    except AssertionError:
        LOG.warning(
            "%s: pid %d still alive %.0fs after the stop path claimed "
            "success (pid-file bookkeeping broke?); sending SIGTERM "
            "directly",
            node.id,
            pid,
            grace,
        )
    try:
        os.kill(pid, 15)  # signal.SIGTERM
        direct_signal = True
    except ProcessLookupError:
        LOG.info("%s: pid %d exited right before the direct SIGTERM", node.id, pid)
        return
    try:
        await wait_for_process_exit(
            pid, sigkill_grace if allow_sigkill else timeout
        )
    except AssertionError:
        if not allow_sigkill:
            raise
        # PRODUCT-DEFECT EVIDENCE, deliberately loud: a graceful signal
        # was ignored well past any flush window (attempt8: 109s).
        LOG.warning(
            "%s: pid %d IGNORED the direct SIGTERM for %.0fs "
            "(early-init graceful-shutdown defect evidence — separate "
            "investigation); escalating to SIGKILL as the caller marked "
            "this node's data as valueless",
            node.id,
            pid,
            sigkill_grace,
        )
        try:
            os.kill(pid, 9)  # signal.SIGKILL
        except ProcessLookupError:
            pass
        await wait_for_process_exit(pid, timeout)
    if direct_signal:
        # The stop path never saw this process, so its pid file may
        # survive pointing at a dead pid; drop it for clean bookkeeping.
        try:
            if int(node.pid_file.read_text().strip()) == pid:
                node.pid_file.unlink()
                LOG.info(
                    "%s: removed stale pid file after direct signal", node.id
                )
        except (FileNotFoundError, ValueError):
            pass
