"""
Unit tests for Node.force_kill (crash simulation via SIGKILL).

Hermetic: a throwaway sleeper process stands in for gravity_node, its PID
written to a real pid_file under tmp_path. No cluster infrastructure is
started; Node is constructed directly with dummy ports.
"""

import subprocess
import sys
import threading
import time
from pathlib import Path

import pytest

from gravity_e2e.cluster.node import Node, NodeRole


def _make_node(tmp_path: Path) -> Node:
    infra = tmp_path / "node1"
    (infra / "script").mkdir(parents=True)
    return Node(
        id="node1",
        rpc_port=59999,  # nothing listens: get_state() sees RPC down
        infra_path=infra,
        cluster_config_path=tmp_path / "cluster.toml",
        role=NodeRole.GENESIS,
        http_port=59998,
        p2p_port=59997,
        vfn_port=59996,
    )


def _spawn_reaped_sleeper(seconds: float) -> subprocess.Popen:
    """Child whose PID disappears at death (concurrent reaper thread) —
    matches the real gravity_node arrangement, which is never our child."""
    proc = subprocess.Popen(
        [sys.executable, "-c", f"import time; time.sleep({seconds})"]
    )
    threading.Thread(target=proc.wait, daemon=True).start()
    return proc


@pytest.mark.asyncio
async def test_force_kill_kills_process_and_removes_pid_file(tmp_path):
    node = _make_node(tmp_path)
    proc = _spawn_reaped_sleeper(60)
    node.pid_file.write_text(f"{proc.pid}\n")

    assert await node.force_kill() is True

    # The process must be gone (poll briefly for the reaper thread).
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline and proc.poll() is None:
        time.sleep(0.05)
    assert proc.poll() is not None, "sleeper survived SIGKILL"
    assert not node.pid_file.exists(), "pid file must be cleaned up"


@pytest.mark.asyncio
async def test_force_kill_without_pid_file_returns_false(tmp_path):
    node = _make_node(tmp_path)
    assert await node.force_kill() is False


@pytest.mark.asyncio
async def test_force_kill_with_stale_pid_returns_true_and_cleans_up(tmp_path):
    node = _make_node(tmp_path)
    proc = subprocess.Popen([sys.executable, "-c", "pass"])
    proc.wait()  # reaped: PID no longer exists
    node.pid_file.write_text(f"{proc.pid}\n")

    # Process already dead == the desired post-condition holds.
    assert await node.force_kill() is True
    assert not node.pid_file.exists()


@pytest.mark.asyncio
async def test_force_kill_with_garbage_pid_file_returns_false(tmp_path):
    node = _make_node(tmp_path)
    node.pid_file.write_text("not-a-pid")
    assert await node.force_kill() is False
