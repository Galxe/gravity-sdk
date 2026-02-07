"""
Anvil Manager for Gravity E2E Bridge Tests

Manages Anvil process lifecycle and bridge contract deployment
via forge script for local testing.
"""

import logging
import os
import re
import signal
import socket
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

LOG = logging.getLogger(__name__)

# Anvil default deployer (Account 0)
ANVIL_PRIVATE_KEY = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
ANVIL_DEPLOYER = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"

# Deterministic contract addresses (Anvil deployer nonce 0/1/2)
DEFAULT_GTOKEN_ADDRESS = "0x5FbDB2315678afecb367f032d93F642f64180aa3"
DEFAULT_PORTAL_ADDRESS = "0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512"
DEFAULT_SENDER_ADDRESS = "0x9fE46736679d2D9a65F0992F2272dE9f3c7fa6e0"


@dataclass
class BridgeContracts:
    """Bridge contract addresses and connection info."""

    rpc_url: str
    gtoken_address: str
    portal_address: str
    sender_address: str
    deployer_private_key: str
    deployer_address: str


class AnvilManager:
    """
    Manages Anvil process lifecycle and bridge contract deployment.

    Usage:
        mgr = AnvilManager()
        mgr.start(port=8546)
        contracts = mgr.deploy_bridge_contracts(contracts_dir)
        # ... run tests ...
        mgr.stop()
    """

    def __init__(self):
        self._process: Optional[subprocess.Popen] = None
        self._port: int = 8546
        self._rpc_url: str = ""

    @property
    def rpc_url(self) -> str:
        return self._rpc_url

    @property
    def is_running(self) -> bool:
        return self._process is not None and self._process.poll() is None

    def start(self, port: int = 8546, block_time: int = None) -> None:
        """
        Start Anvil local testnet.

        Args:
            port: Port to run Anvil on.
            block_time: Block time in seconds. None = auto-mine (instant).
        """
        if self.is_running:
            LOG.warning("Anvil already running, stopping first...")
            self.stop()

        # Check port availability
        if self._is_port_in_use(port):
            LOG.warning(f"Port {port} in use, killing existing process...")
            self._kill_port(port)
            time.sleep(1)

        self._port = port
        self._rpc_url = f"http://localhost:{port}"

        cmd = ["anvil", "--port", str(port)]
        if block_time is not None:
            cmd.extend(["--block-time", str(block_time)])
            LOG.info(f"Starting Anvil on port {port} (block-time: {block_time}s)...")
        else:
            LOG.info(f"Starting Anvil on port {port} (auto-mine mode)...")

        self._process = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

        # Wait for Anvil to be ready
        if not self._wait_for_ready(timeout=10):
            self.stop()
            raise RuntimeError("Anvil failed to start within timeout")

        LOG.info(f"Anvil running at {self._rpc_url} (PID: {self._process.pid})")

    def set_interval_mining(self, interval: int = 1) -> None:
        """
        Switch Anvil to interval mining mode (timed block production).

        Uses evm_setIntervalMining RPC. Keeps all existing state intact.

        Args:
            interval: Block interval in seconds.
        """
        if not self.is_running:
            raise RuntimeError("Anvil is not running")

        import requests
        resp = requests.post(
            self._rpc_url,
            json={
                "jsonrpc": "2.0",
                "method": "evm_setIntervalMining",
                "params": [interval],
                "id": 1,
            },
            timeout=5,
        )
        resp.raise_for_status()
        result = resp.json()
        if "error" in result:
            raise RuntimeError(f"evm_setIntervalMining failed: {result['error']}")
        LOG.info(f"Switched Anvil to interval mining: {interval}s per block")

    def deploy_bridge_contracts(self, contracts_dir: Path) -> BridgeContracts:
        """
        Deploy bridge contracts using forge script.

        Args:
            contracts_dir: Path to gravity_chain_core_contracts repo.

        Returns:
            BridgeContracts with deployed addresses.
        """
        if not self.is_running:
            raise RuntimeError("Anvil is not running")

        LOG.info("Deploying bridge contracts via forge script...")

        env = os.environ.copy()
        env["PRIVATE_KEY"] = ANVIL_PRIVATE_KEY

        result = subprocess.run(
            [
                "forge",
                "script",
                "script/DeployBridgeLocal.s.sol:DeployBridgeLocal",
                "--rpc-url",
                self._rpc_url,
                "--broadcast",
            ],
            cwd=str(contracts_dir),
            env=env,
            capture_output=True,
            text=True,
        )

        if result.returncode != 0:
            LOG.error(f"Forge deploy failed:\n{result.stderr}")
            raise RuntimeError(f"Bridge contract deployment failed: {result.stderr}")

        # Parse addresses from forge output
        contracts = self._parse_deploy_output(result.stdout + result.stderr)
        LOG.info(
            f"Contracts deployed: "
            f"GToken={contracts.gtoken_address}, "
            f"Portal={contracts.portal_address}, "
            f"Sender={contracts.sender_address}"
        )
        return contracts

    def stop(self) -> None:
        """Stop Anvil process."""
        if self._process is not None:
            LOG.info("Stopping Anvil...")
            try:
                self._process.send_signal(signal.SIGTERM)
                self._process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self._process.kill()
                self._process.wait()
            finally:
                self._process = None
            LOG.info("Anvil stopped")

    def _parse_deploy_output(self, output: str) -> BridgeContracts:
        """Parse contract addresses from forge script output."""
        gtoken = self._extract_address(output, "MockGToken deployed at:")
        portal = self._extract_address(output, "GravityPortal deployed at:")
        sender = self._extract_address(output, "GBridgeSender deployed at:")

        # Fall back to deterministic addresses if parsing fails
        return BridgeContracts(
            rpc_url=self._rpc_url,
            gtoken_address=gtoken or DEFAULT_GTOKEN_ADDRESS,
            portal_address=portal or DEFAULT_PORTAL_ADDRESS,
            sender_address=sender or DEFAULT_SENDER_ADDRESS,
            deployer_private_key=ANVIL_PRIVATE_KEY,
            deployer_address=ANVIL_DEPLOYER,
        )

    @staticmethod
    def _extract_address(output: str, prefix: str) -> Optional[str]:
        """Extract an Ethereum address following a prefix in text."""
        pattern = re.escape(prefix) + r"\s*(0x[0-9a-fA-F]{40})"
        match = re.search(pattern, output)
        return match.group(1) if match else None

    def _wait_for_ready(self, timeout: int = 10) -> bool:
        """Wait for Anvil RPC to accept connections."""
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                with socket.create_connection(("127.0.0.1", self._port), timeout=1):
                    return True
            except (ConnectionRefusedError, OSError):
                time.sleep(0.5)
        return False

    @staticmethod
    def _is_port_in_use(port: int) -> bool:
        """Check if a port is in use."""
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=1):
                return True
        except (ConnectionRefusedError, OSError):
            return False

    @staticmethod
    def _kill_port(port: int) -> None:
        """Kill process using a port."""
        subprocess.run(
            ["fuser", "-k", f"{port}/tcp"],
            check=False,
            capture_output=True,
        )
