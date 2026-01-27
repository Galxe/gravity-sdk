
import asyncio
import logging
from pathlib import Path
from typing import Tuple, Dict, Any, Optional
from enum import Enum, auto

# Use local client copy
from .client.gravity_client import GravityClient

TransactionReceipt = Dict[str, Any]

LOG = logging.getLogger(__name__)

class NodeState(Enum):
    """Represents the lifecycle state of a node."""
    UNKNOWN = auto()      # Initial state, not yet checked
    STOPPED = auto()      # Node is not running (PID file missing or process dead)
    STARTING = auto()     # Start command issued, waiting for RPC
    RUNNING = auto()      # RPC is responding
    STOPPING = auto()     # Stop command issued
    STALE = auto()        # PID file exists but process is dead or RPC unresponsive
    SYNCING = auto()      # Node is running but catching up (block height < peer)
# Add Live, block height is increasing

class Node:
    """
    Represents a single node in the cluster.
    Maintains internal state and provides lifecycle operations.
    """
    def __init__(self, id: str, rpc_port: int, infra_path: Path, cluster_config_path: Path):
        self.id = id
        self.rpc_port = rpc_port
        self.url = f"http://127.0.0.1:{rpc_port}"
        self.client = GravityClient(self.url, node_id=id)
        self._infra_path = infra_path
        self._cluster_config_path = cluster_config_path
        
        # Paths to control scripts
        self.start_script = self._infra_path / "script" / "start.sh"
        self.stop_script = self._infra_path / "script" / "stop.sh"
        self.pid_file = self._infra_path / "script" / "node.pid"

    def _pid_exists(self) -> bool:
        """Check if process from PID file is alive."""
        if not self.pid_file.exists():
            return False
        try:
            pid = int(self.pid_file.read_text().strip())
            import os
            os.kill(pid, 0)
            return True
        except (ValueError, ProcessLookupError, OSError):
            return False
    
    async def get_txn_receipt(self, txn_hash: str) -> Optional[TransactionReceipt]:
        """
        Fetch the transaction receipt from the node.
        """
        try:
            async with self.client:
                return await self.client.get_transaction_receipt(txn_hash)
        except Exception:
            return None

    async def get_block_number(self) -> int:
        """
        Fetch the current block number from the node.
        Returns block number or raises Exception.
        """
        async with self.client:
            return await self.client.get_block_number()

    async def get_state(self) -> Tuple[NodeState, int]:
        """
        Fetch the current state from the node (live).
        Checks PID and RPC.
        Returns (State, BlockHeight). BlockHeight is -1 if not available.
        """
        # 1. Check RPC first (most reliable for RUNNING)
        rpc_ok = False
        block_height = -1
        try:
            async with self.client:
                block_height = await self.client.get_block_number()
                rpc_ok = block_height >= 0
        except Exception:
            rpc_ok = False

        if rpc_ok:
            return NodeState.RUNNING, block_height

        # 2. If RPC failed, check PID to distinguish STOPPED vs STALE
        pid_alive = self._pid_exists()
        if pid_alive:
            # PID alive but RPC not responding -> Stale or Starting
            return NodeState.STALE, -1
        else:
            return NodeState.STOPPED, -1

    async def start(self) -> bool:
        """
        Start this individual node.
        Returns True if node is now RUNNING.
        """
        if not self.start_script.exists():
            LOG.warning(f"Start script not found for {self.id} (remote node?). Cannot start.")
            return False

        # First, check live state
        current_state, _ = await self.get_state()

        if current_state == NodeState.RUNNING:
            LOG.info(f"Node {self.id} is already running.")
            return True

        if current_state == NodeState.STALE:
            LOG.warning(f"Node {self.id} is STALE (PID alive but RPC down). Stopping first...")
            await self.stop()

        LOG.info(f"Starting node {self.id} (config={self._cluster_config_path})...")

        try:
            # We assume the parent structure if start script exists
            # Call start script
            proc = await asyncio.create_subprocess_exec(
                "bash", str(self.start_script),
                "--config", str(self._cluster_config_path),
                cwd=str(self.start_script.parent.parent),
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE
            )
            
            async def log_stream(stream, level):
                while True:
                    line = await stream.readline()
                    if not line:
                        break
                    decoded = line.decode().strip()
                    if decoded:
                        LOG.log(level, f"[{self.id}] {decoded}")

            # Non-blocking stream logging
            await asyncio.gather(
                log_stream(proc.stdout, logging.INFO),
                log_stream(proc.stderr, logging.WARNING)
            )
            
            returncode = await proc.wait()

            if returncode != 0:
                LOG.error(f"Node {self.id} start script failed with code {returncode}")
                return False

            # Wait for RPC to come up
            if await self.wait_for_rpc(timeout=30):
                LOG.info(f"Node {self.id} started and RPC verified.")
                return True
            else:
                LOG.error(f"Node {self.id} started but RPC never came up.")
                return False

        except Exception as e:
            LOG.error(f"Exception starting node {self.id}: {e}")
            return False

    async def stop(self) -> bool:
        """
        Stop this individual node.
        Returns True if node is now STOPPED.
        """
        if not self.stop_script.exists():
             LOG.warning(f"Stop script not found for {self.id} (remote node?). Cannot stop.")
             return False

        # Check live state
        current_state, _ = await self.get_state()

        if current_state == NodeState.STOPPED:
            LOG.info(f"Node {self.id} is already stopped.")
            return True

        LOG.info(f"Stopping node {self.id}...")

        try:
            proc = await asyncio.create_subprocess_exec(
                "bash", str(self.stop_script),
                cwd=str(self.stop_script.parent.parent),
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE
            )
            stdout, stderr = await proc.communicate()

            if proc.returncode != 0:
                LOG.error(f"Node {self.id} stop script failed: {stderr.decode()}")
                return False

            # Verify stopped
            await asyncio.sleep(1)
            # Re-check live state
            final_state, _ = await self.get_state()

            if final_state == NodeState.STOPPED:
                LOG.info(f"Node {self.id} stopped and verified.")
                return True
            else:
                LOG.warning(f"Node {self.id} stop script succeeded but state is {final_state.name}")
                return False

        except Exception as e:
            LOG.error(f"Exception stopping node {self.id}: {e}")
            return False

    async def restart(self) -> bool:
        """Bounce the node."""
        if not await self.stop():
            return False
        await asyncio.sleep(2) # Grace period
        return await self.start()
        
    def is_running(self) -> bool:
        """Check if node process is running based on PID file."""
        if not self.pid_file.exists():
            return False
        try:
            pid = int(self.pid_file.read_text().strip())
            # Check if process exists (signal 0 does nothing but checks permission/existence)
            import os
            os.kill(pid, 0)
            return True
        except (ValueError, ProcessLookupError, OSError):
            return False

    async def wait_for_rpc(self, timeout: int = 30) -> bool:
        """
        Wait for RPC to become available.
        Polls the get_block_number method until it succeeds or timeout is reached.
        """
        import time
        start_time = time.time()
        while time.time() - start_time < timeout:
            try:
                # We need to use the async context manager to ensure connection logic runs
                async with self.client:
                    # Depending on client impl, get_block_number might be enough
                    bn = await self.client.get_block_number()
                    if bn >= 0:
                        return True
            except Exception:
                # Connection refused or other transient error
                await asyncio.sleep(1)
        return False

    async def wait_for_block_increase(self, timeout: int = 30, delta: int = 1) -> bool:
        """
        Wait for block number to increase by at least `delta`.
        Returns True if progress observed, False if timeout.
        """
        import time
        start_time = time.time()
        
        # Get start height
        start_height = -1
        try:
            async with self.client:
                start_height = await self.client.get_block_number()
        except Exception:
            LOG.warning(f"Node {self.id} RPC unavailable for initial block check")
            return False
            
        target_height = start_height + delta
        LOG.info(f"Node {self.id} current height {start_height}, waiting for {target_height} (timeout={timeout}s)")
        
        while time.time() - start_time < timeout:
            try:
                async with self.client:
                    current = await self.client.get_block_number()
                    if current >= target_height:
                        LOG.info(f"Node {self.id} reached height {current} (progress verified)")
                        return True
            except Exception:
                pass
            
            await asyncio.sleep(1)
            
        LOG.warning(f"Node {self.id} failed to produce {delta} blocks in {timeout}s (started at {start_height})")
        return False
