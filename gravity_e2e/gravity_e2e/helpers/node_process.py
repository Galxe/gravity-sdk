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
