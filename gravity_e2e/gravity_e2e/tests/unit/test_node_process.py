"""
Unit tests for gravity_e2e.helpers.node_process.

The helper is tiny on purpose (read a node's PID file, wait for a real
process exit) but it guards a real race: NodeState.STOPPED only means the
PID file is gone, while gravity_node may still be flushing RocksDB. These
tests exercise it hermetically with throwaway child processes.
"""

import asyncio
import subprocess
import sys
import threading
from pathlib import Path
from types import SimpleNamespace

import pytest

from gravity_e2e.helpers.node_process import read_node_pid, wait_for_process_exit


def _stub_node(tmp_path: Path, content=None) -> SimpleNamespace:
    """Anything with a ``pid_file`` attribute quacks like a Node here."""
    pid_file = tmp_path / "node.pid"
    if content is not None:
        pid_file.write_text(content)
    return SimpleNamespace(pid_file=pid_file)


def _spawn_reaped_sleeper(seconds: float) -> subprocess.Popen:
    """Spawn a sleeper child that is reaped as soon as it dies.

    The real gravity_node is started by a start.sh that exits immediately,
    so the node is never our child and its PID vanishes right at exit. A
    plain Popen child instead lingers as a zombie until reaped — and
    ``os.kill(pid, 0)`` succeeds on zombies, which would wedge
    wait_for_process_exit forever. A concurrent ``wait()`` thread restores
    the "PID disappears at death" behavior the helper is written against.
    """
    proc = subprocess.Popen(
        [sys.executable, "-c", f"import time; time.sleep({seconds})"]
    )
    threading.Thread(target=proc.wait, daemon=True).start()
    return proc


# ---------------------------------------------------------------------------
# read_node_pid
# ---------------------------------------------------------------------------


def test_read_node_pid_parses_pid_with_whitespace(tmp_path):
    node = _stub_node(tmp_path, " 12345\n")
    assert read_node_pid(node) == 12345


def test_read_node_pid_missing_file_returns_none(tmp_path):
    node = _stub_node(tmp_path, content=None)
    assert read_node_pid(node) is None


def test_read_node_pid_garbage_returns_none(tmp_path):
    node = _stub_node(tmp_path, "not-a-pid")
    assert read_node_pid(node) is None


# ---------------------------------------------------------------------------
# wait_for_process_exit
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_wait_for_process_exit_returns_once_process_dies():
    proc = _spawn_reaped_sleeper(0.5)
    await wait_for_process_exit(proc.pid, timeout=10)
    # Reaching here without AssertionError is the pass condition.


@pytest.mark.asyncio
async def test_wait_for_process_exit_raises_for_survivor():
    proc = _spawn_reaped_sleeper(30)
    try:
        with pytest.raises(AssertionError):
            await wait_for_process_exit(proc.pid, timeout=1)
    finally:
        proc.kill()


@pytest.mark.asyncio
async def test_wait_for_process_exit_immediate_for_dead_pid():
    proc = subprocess.Popen([sys.executable, "-c", "pass"])
    proc.wait()  # reaped: the PID no longer exists
    start = asyncio.get_event_loop().time()
    await wait_for_process_exit(proc.pid, timeout=10)
    assert asyncio.get_event_loop().time() - start < 5


# ---------------------------------------------------------------------------
# stop_node_and_wait_exit — the defensive stop (live attempt4 regression)
# ---------------------------------------------------------------------------

from gravity_e2e.helpers.node_process import stop_node_and_wait_exit  # noqa: E402


def _stoppable_node(tmp_path: Path, proc: subprocess.Popen, stop_behavior):
    """A Node-shaped stub whose pid file points at a real child process.

    ``stop_behavior`` is an async callable standing in for Node.stop();
    the broken-bookkeeping case returns True WITHOUT touching the process
    (what live attempt4's per-node stop.sh did after the pid file went
    missing: "No PID file found", exit 0, node keeps running).
    """
    pid_file = tmp_path / "node.pid"
    pid_file.write_text(str(proc.pid))
    return SimpleNamespace(id="stub", pid_file=pid_file, stop=stop_behavior)


@pytest.mark.asyncio
async def test_defensive_stop_happy_path(tmp_path):
    proc = _spawn_reaped_sleeper(60)

    async def killing_stop():
        proc.terminate()
        return True

    node = _stoppable_node(tmp_path, proc, killing_stop)
    await stop_node_and_wait_exit(node, timeout=10, grace=5)
    assert proc.poll() is not None


@pytest.mark.asyncio
async def test_defensive_stop_escalates_when_stop_path_is_a_noop(tmp_path):
    """The regression: Node.stop() 'succeeds' (pid-file bookkeeping says
    stopped) but never signals the process. The helper must SIGTERM the
    RECORDED pid itself and clear the stale pid file."""
    proc = _spawn_reaped_sleeper(60)

    async def lying_stop():
        return True  # touches nothing — the attempt4 failure mode

    node = _stoppable_node(tmp_path, proc, lying_stop)
    await stop_node_and_wait_exit(node, timeout=15, grace=1)
    assert proc.poll() is not None, "helper never killed the survivor"
    assert not node.pid_file.exists(), "stale pid file must be dropped"


@pytest.mark.asyncio
async def test_defensive_stop_requires_a_readable_pid(tmp_path):
    async def lying_stop():
        return True

    node = SimpleNamespace(
        id="stub", pid_file=tmp_path / "node.pid", stop=lying_stop
    )
    with pytest.raises(AssertionError, match="cannot read"):
        await stop_node_and_wait_exit(node, timeout=5, grace=1)
