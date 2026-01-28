import asyncio
import logging
import sys
if sys.version_info >= (3, 11):
    import tomllib
else:
    import tomli as tomllib  # fallback for older Python
import json
import shutil
import subprocess
from pathlib import Path
from typing import Dict, Optional, List, Tuple

from eth_account import Account
from eth_account.signers.local import LocalAccount

from .node import Node, NodeState

LOG = logging.getLogger(__name__)

# Standard devnet keys (Anvil/Hardhat defaults)
KNOWN_DEV_KEYS = [
    "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80", # 0xf39F...
    "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d", # 0x7099...
    "0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a"  # 0x3C44...
]

class Cluster:
    """
    Unified entry point for interacting with a Gravity Cluster.
    Wraps infrastructure scripts (`cluster/`) and provides RPC access.
    """
    def __init__(self, config_path: Path):
        self.config_path = config_path.resolve()
        if not self.config_path.exists():
            raise FileNotFoundError(f"Cluster config not found: {self.config_path}")
            
        with open(self.config_path, "rb") as f:
            self.config = tomllib.load(f)
        
        # Determine paths
        # Assuming config_path is like .../cluster/cluster.toml
        self.cluster_root = self.config_path.parent
        self.base_dir = Path(self.config["cluster"]["base_dir"])
        
        self.nodes: Dict[str, Node] = self._discover_nodes()
        
        # Cluster control scripts
        self.start_script = self.cluster_root / "start.sh"
        self.stop_script = self.cluster_root / "stop.sh"

    @property
    def faucet(self) -> Optional[LocalAccount]:
        """Returns primary (first) faucet account."""
        f = self.faucets
        return f[0] if f else None

    @property
    def faucets(self) -> List[LocalAccount]:
        """
        Returns all faucet accounts as LocalAccount instances.
        Matches addresses from config with private keys from:
        1. KNOWN_DEV_KEYS (Built-in devnet keys)
        2. genesis.secrets.keys (Config)
        """
        genesis = self.config.get("genesis", {})
        faucet_config = genesis.get("faucet", [])
        
        # Normalize to list
        if isinstance(faucet_config, dict):
            faucets = [faucet_config]
        elif isinstance(faucet_config, list):
            faucets = faucet_config
        else:
            faucets = []
            
        if not faucets:
            return []

        # Gather potential private keys
        candidate_keys = set(KNOWN_DEV_KEYS)
        
        secrets = genesis.get("secrets", {})
        if secrets and "keys" in secrets:
            candidate_keys.update(secrets["keys"])

        # Build address -> LocalAccount mapping
        key_map: Dict[str, LocalAccount] = {}
        for k in candidate_keys:
            try:
                if not k.startswith("0x"):
                    k = "0x" + k
                account = Account.from_key(k)
                key_map[account.address.lower()] = account
            except Exception as e:
                LOG.warning(f"Failed to derive address from key {k[:6]}...: {e}")

        # Match faucet addresses to accounts
        accounts: List[LocalAccount] = []
        for f in faucets:
            addr = f.get("address")
            if addr:
                account = key_map.get(addr.lower())
                if account:
                    accounts.append(account)
                else:
                    LOG.warning(f"Faucet address {addr} has no matching private key")
            
        return accounts

    def _discover_nodes(self) -> Dict[str, Node]:
        nodes = {}
        for node_cfg in self.config.get("nodes", []):
            node_id = node_cfg["id"]
            rpc_port = node_cfg.get("rpc_port")
            
            # Determine where this node lives on disk
            # Inherited from cluster deploy logic: base_dir / node_id
            # Unless explicitly overridden in config (common pattern in this project)
            node_data_dir = node_cfg.get("data_dir")
            if not node_data_dir:
                infra_path = self.base_dir / node_id
            else:
                infra_path = self.base_dir / node_id
            
            nodes[node_id] = Node(id=node_id, rpc_port=rpc_port, infra_path=infra_path, cluster_config_path=self.config_path)
        return nodes

    async def _run_script(self, script: Path, args: List[str] = None) -> bool:
        if not script.exists():
            # If scripts don't exist, we assume it's a remote/unmanaged cluster
            # Log warning but don't fail, just return False
            LOG.warning(f"Script not found: {script} (cluster might be unmanaged)")
            return False
            
        cmd = ["bash", str(script)]
        if args:
            cmd.extend(args)
            
        # We pass CONFIG_FILE env var to scripts as they expect it or default to sibling cluster.toml
        env = {"CONFIG_FILE": str(self.config_path)}
        # Merge with current env
        import os
        full_env = os.environ.copy()
        full_env.update(env)
        
        LOG.info(f"Running cluster script: {script.name} {' '.join(args or [])}")
        try:
            proc = await asyncio.create_subprocess_exec(
                *cmd,
                env=full_env,
                cwd=str(self.cluster_root),
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
                        LOG.log(level, f"[{script.name}] {decoded}")

            # Run stream readers concurrently
            await asyncio.gather(
                log_stream(proc.stdout, logging.INFO),
                log_stream(proc.stderr, logging.WARNING) # Use WARNING for stderr to distinguish, or INFO if script is noisy
            )
            
            returncode = await proc.wait()
            
            if returncode != 0:
                LOG.error(f"{script.name} failed with exit code {returncode}")
                return False
            else:
                LOG.info(f"{script.name} success.")
                return True
        except Exception as e:
            LOG.error(f"Exception running {script.name}: {e}")
            return False

    async def start(self) -> bool:
        """Runs start.sh (starts all nodes)."""
        # start.sh takes --config argument
        return await self._run_script(self.start_script, ["--config", str(self.config_path)])

    async def stop(self) -> bool:
        """Runs stop.sh (stops all nodes)."""
        return await self._run_script(self.stop_script, ["--config", str(self.config_path)])

    def get_node(self, node_id: str) -> Optional[Node]:
        """Get a specific node handle."""
        return self.nodes.get(node_id)

    # ========== Declarative API ==========
    
    async def get_live_nodes(self) -> List[Node]:
        """Returns list of nodes currently in RUNNING state."""
        live = []
        for node in self.nodes.values():
            state, _ = await node.get_state()
            if state == NodeState.RUNNING:
                live.append(node)
        return live

    async def get_dead_nodes(self) -> List[Node]:
        """Returns list of nodes in STOPPED or STALE state."""
        dead = []
        for node in self.nodes.values():
            state, _ = await node.get_state()
            if state in (NodeState.STOPPED, NodeState.STALE, NodeState.UNKNOWN):
                dead.append(node)
        return dead

    async def get_node_status(self, node_id: str) -> Optional[NodeState]:
        """Get current state of a specific node."""
        node = self.nodes.get(node_id)
        if node:
             state, _ = await node.get_state()
             return state
        return None

    async def set_full_live(self, timeout: int = 60) -> bool:
        """
        Ensure ALL nodes are RUNNING. Blocks until converged or timeout.
        Returns True if all nodes are RUNNING.
        """
        import time
        deadline = time.time() + timeout
        
        while time.time() < deadline:
            # Check all nodes
            states = {}
            for node in self.nodes.values():
                state, _ = await node.get_state()
                states[node.id] = state
            
            LOG.info(f"Convergence check: {[(nid, s.name) for nid, s in states.items()]}")

            tasks = []
            all_running = True
            
            for node in self.nodes.values():
                state = states[node.id]
                
                if state != NodeState.RUNNING:
                    all_running = False
                    if state == NodeState.STOPPED:
                        LOG.info(f"Starting stopped node {node.id}...")
                        tasks.append(node.start())
                    elif state == NodeState.STALE:
                        LOG.info(f"Cleaning up stale node {node.id}...")
                        # Restart stale node
                        async def restart_node(n):
                            await n.stop()
                            await n.start()
                        tasks.append(restart_node(node))
                    elif state == NodeState.UNKNOWN:
                        LOG.info(f"Starting unknown node {node.id}...")
                        tasks.append(node.start())
            
            if all_running:
                LOG.info("All nodes are RUNNING.")
                return True
            
            # Execute all start/restart tasks concurrently
            if tasks:
                await asyncio.gather(*tasks)
            
            await asyncio.sleep(2)
        
        LOG.error(f"Failed to converge to full_live within {timeout}s")
        return False

    async def set_all_stopped(self, timeout: int = 60) -> bool:
        """
        Ensure ALL nodes are STOPPED. Blocks until converged or timeout.
        """
        import time
        deadline = time.time() + timeout
        
        while time.time() < deadline:
            all_stopped = True
            
            for node in self.nodes.values():
                state, _ = await node.get_state()
                if state != NodeState.STOPPED:
                    all_stopped = False
                    LOG.info(f"Stopping node {node.id} (state={state.name})...")
                    await node.stop()
            
            if all_stopped:
                LOG.info("All nodes are STOPPED.")
                return True
            
            await asyncio.sleep(2)
        
        LOG.error(f"Failed to stop all nodes within {timeout}s")
        return False

    async def set_live_nodes(self, n: int, timeout: int = 60) -> bool:
        """
        Ensure exactly N nodes are RUNNING. Stops extra or starts missing.
        Blocks until converged or timeout.
        """
        import time
        if n > len(self.nodes):
            LOG.error(f"Requested {n} live nodes but only {len(self.nodes)} exist")
            return False
        
        deadline = time.time() + timeout
        
        while time.time() < deadline:
            current_states = {}
            for node_id, node in self.nodes.items():
                current_states[node_id], _ = await node.get_state()
            
            live = [nid for nid, s in current_states.items() if s == NodeState.RUNNING]
            live_count = len(live)
            
            if live_count == n:
                LOG.info(f"Converged: {n} nodes are RUNNING.")
                return True
            
            node_list = list(self.nodes.values())
            if live_count < n:
                # Need to start more nodes
                dead = [node for node in node_list if current_states[node.id] != NodeState.RUNNING]
                to_start = n - live_count
                for node in dead[:to_start]:
                    LOG.info(f"Starting node {node.id} to reach target {n}...")
                    await node.start()
            else:
                # Need to stop some nodes
                live_nodes = [node for node in node_list if current_states[node.id] == NodeState.RUNNING]
                to_stop = live_count - n
                for node in live_nodes[:to_stop]:
                    LOG.info(f"Stopping node {node.id} to reach target {n}...")
                    await node.stop()
            
            await asyncio.sleep(2)
        
        LOG.error(f"Failed to set {n} live nodes within {timeout}s")
        return False

    async def set_node(self, node_id: str, target_state: NodeState, timeout: int = 30) -> bool:
        """
        Set a specific node to target state (RUNNING or STOPPED).
        Blocks until converged or timeout.
        """
        import time
        node = self.nodes.get(node_id)
        if not node:
            LOG.error(f"Node {node_id} not found")
            return False
        
        if target_state not in (NodeState.RUNNING, NodeState.STOPPED):
            LOG.error(f"Can only set node to RUNNING or STOPPED, got {target_state}")
            return False
        
        deadline = time.time() + timeout
        
        while time.time() < deadline:
            state, _ = await node.get_state()
            
            if state == target_state:
                LOG.info(f"Node {node_id} is now {target_state.name}")
                return True
            
            if target_state == NodeState.RUNNING:
                await node.start()
            else:
                await node.stop()
            
            await asyncio.sleep(2)
        
        LOG.error(f"Failed to set {node_id} to {target_state.name} within {timeout}s")
        return False

    async def check_block_increasing(self, node_id: Optional[str] = None, timeout: int = 30, delta: int = 1) -> bool:
        """
        Check if block height is increasing.
        :param node_id: If specified, check only this node. If None, check ALL currently RUNNING nodes.
        :param timeout: Max time to wait for increase.
        :param delta: Minimum block increase expected.
        :return: True if all unchecked nodes made progress.
        """
        if node_id:
            node = self.get_node(node_id)
            if not node:
                LOG.error(f"Node {node_id} not found")
                return False
            return await node.wait_for_block_increase(timeout=timeout, delta=delta)
        else:
            # Check all live nodes
            live_nodes = await self.get_live_nodes()
            if not live_nodes:
                LOG.warning("No live nodes to check progress for.")
                return False
                
            LOG.info(f"Checking block progress for {len(live_nodes)} nodes: {[n.id for n in live_nodes]}")
            
            results = await asyncio.gather(
                *[n.wait_for_block_increase(timeout=timeout, delta=delta) for n in live_nodes]
            )
            
            success = all(results)
            if success:
                LOG.info("All live nodes made progress.")
            else:
                LOG.error("Some nodes failed to make progress.")
            return success
