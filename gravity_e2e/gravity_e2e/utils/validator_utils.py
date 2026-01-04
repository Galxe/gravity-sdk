"""
Validator testing utilities

This module provides common functions for validator add/remove tests,
reducing duplication across different test scenarios.
"""
import asyncio
import logging
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Optional, Tuple

from ..core.client.gravity_http_client import GravityHttpClient
from ..core.node_manager import NodeManager

LOG = logging.getLogger(__name__)


@dataclass
class ValidatorJoinParams:
    """Parameters for joining a validator to the network"""
    private_key: str
    stake_amount: str
    validator_address: str
    consensus_public_key: str
    validator_network_address: str
    fullnode_network_address: str
    aptos_address: str


@dataclass
class ValidatorTestConfig:
    """Configuration for validator tests"""
    node1_name: str = "node1"
    node3_name: str = "node3"
    install_dir: str = "/tmp"
    http_url_node1: str = "http://127.0.0.1:1024"
    http_url_node3: str = "http://127.0.0.1:1026"
    rpc_url: str = "http://127.0.0.1:8545"
    bin_version: str = "quick-release"
    node_startup_delay: int = 10
    validator_change_wait: int = 120  # 2 minutes


# Default validator join parameters (can be overridden per test)
DEFAULT_VALIDATOR_PARAMS = ValidatorJoinParams(
    private_key="0x....",
    stake_amount="10001.0",
    validator_address="0x9B2C25E77a97d3e84DC0Cb7F83fb676ddC4F24b9",
    consensus_public_key="b7a931fa544c2d1d54dee27619edfb70cc801bc599dd7a3f56f641a588cee4600b63e35d0d35fe69f2e454462b0ce9b2",
    validator_network_address="/ip4/127.0.0.1/tcp/2026/noise-ik/99d1c7709b14777edbdbe0c602eb0186ea845ed75b01740726e581215de8625b/handshake/0",
    fullnode_network_address="/ip4/127.0.0.1/tcp/2036/noise-ik/99d1c7709b14777edbdbe0c602eb0186ea845ed75b01740726e581215de8625b/handshake/0",
    aptos_address="99d1c7709b14777edbdbe0c602eb0186ea845ed75b01740726e581215de8625b",
)


async def get_validator_count(
    http_url: str,
    epoch: Optional[int] = None
) -> Tuple[int, int]:
    """
    Get validator count for the specified or current epoch.

    Args:
        http_url: HTTP API URL
        epoch: Specific epoch to query, or None for current epoch

    Returns:
        Tuple of (validator_count, epoch_used)

    Raises:
        AssertionError: If validator count cannot be retrieved
    """
    async with GravityHttpClient(base_url=http_url) as http_client:
        # Get current epoch if not specified
        if epoch is None:
            epoch = await http_client.get_current_epoch()

        LOG.info(f"Querying validator count for epoch {epoch}")

        # Try to get validator count, fall back to previous epoch if needed
        try:
            validator_count_data = await http_client.get_validator_count_by_epoch(epoch)
        except RuntimeError:
            LOG.info(f"Epoch {epoch} not available, trying epoch {epoch - 1}")
            epoch = epoch - 1
            validator_count_data = await http_client.get_validator_count_by_epoch(epoch)

        validator_count = validator_count_data["validator_count"]
        LOG.info(f"Validator count at epoch {epoch}: {validator_count}")

        return validator_count, epoch


async def verify_validator_count(
    http_url: str,
    expected_count: int,
    description: str = ""
) -> int:
    """
    Verify that validator count matches expected value.

    Args:
        http_url: HTTP API URL
        expected_count: Expected validator count
        description: Description for logging

    Returns:
        The epoch used for verification

    Raises:
        AssertionError: If validator count doesn't match
    """
    validator_count, epoch = await get_validator_count(http_url)

    if validator_count != expected_count:
        raise AssertionError(
            f"Expected validator count to be {expected_count}, but got {validator_count}"
            + (f" ({description})" if description else "")
        )

    LOG.info(f"Validation passed: validator count == {expected_count}"
             + (f" ({description})" if description else ""))

    return epoch


def deploy_nodes(
    node_manager: NodeManager,
    config: ValidatorTestConfig
) -> Dict[str, str]:
    """
    Deploy test nodes.

    Args:
        node_manager: NodeManager instance
        config: Test configuration

    Returns:
        Dict mapping node names to deploy paths

    Raises:
        RuntimeError: If deployment fails
    """
    LOG.info(f"Deploying {config.node1_name} and {config.node3_name}...")

    deploy_results = node_manager.deploy_nodes(
        node_names=[config.node1_name, config.node3_name],
        mode="single",
        install_dir=config.install_dir,
        bin_version=config.bin_version,
        recover=False
    )

    if not deploy_results.get(config.node1_name):
        raise RuntimeError(f"Failed to deploy {config.node1_name}")
    if not deploy_results.get(config.node3_name):
        raise RuntimeError(f"Failed to deploy {config.node3_name}")

    node1_path = node_manager.get_node_deploy_path(config.node1_name, config.install_dir)
    node3_path = node_manager.get_node_deploy_path(config.node3_name, config.install_dir)

    LOG.info(f"Nodes deployed: {config.node1_name} -> {node1_path}, {config.node3_name} -> {node3_path}")

    return {
        config.node1_name: node1_path,
        config.node3_name: node3_path
    }


def start_node(
    node_manager: NodeManager,
    node_path: str,
    node_name: str
) -> None:
    """
    Start a node.

    Args:
        node_manager: NodeManager instance
        node_path: Path to node deployment
        node_name: Node name for logging

    Raises:
        RuntimeError: If start fails
    """
    LOG.info(f"Starting {node_name}...")

    if not node_manager.start_node(node_path):
        raise RuntimeError(f"Failed to start {node_name}")

    LOG.info(f"Node {node_name} started")


def stop_nodes(
    node_manager: NodeManager,
    node_paths: Dict[str, str]
) -> None:
    """
    Stop nodes with error handling.

    Args:
        node_manager: NodeManager instance
        node_paths: Dict mapping node names to paths
    """
    LOG.info("Stopping nodes...")

    for node_name, node_path in node_paths.items():
        try:
            if node_manager.stop_node(node_path):
                LOG.info(f"Node {node_name} stopped")
            else:
                LOG.warning(f"Failed to stop {node_name}")
        except Exception as e:
            LOG.warning(f"Error stopping {node_name}: {e}")


async def execute_validator_join(
    gravity_cli_path: Path,
    rpc_url: str,
    params: ValidatorJoinParams,
    timeout: int = 60
) -> str:
    """
    Execute validator join command.

    Args:
        gravity_cli_path: Path to gravity_cli binary
        rpc_url: RPC URL
        params: Validator join parameters
        timeout: Command timeout in seconds

    Returns:
        Command output

    Raises:
        RuntimeError: If command fails
        asyncio.TimeoutError: If command times out
    """
    join_cmd = [
        str(gravity_cli_path),
        "validator", "join",
        "--rpc-url", rpc_url,
        "--private-key", params.private_key,
        "--stake-amount", params.stake_amount,
        "--validator-address", params.validator_address,
        "--consensus-public-key", params.consensus_public_key,
        "--validator-network-address", params.validator_network_address,
        "--fullnode-network-address", params.fullnode_network_address,
        "--aptos-address", params.aptos_address,
    ]

    LOG.info(f"Executing validator join command...")
    LOG.debug(f"Command: {' '.join(join_cmd)}")

    try:
        process = await asyncio.create_subprocess_exec(
            *join_cmd,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE
        )

        stdout, stderr = await asyncio.wait_for(
            process.communicate(),
            timeout=timeout
        )

        stdout_str = stdout.decode() if stdout else ""
        stderr_str = stderr.decode() if stderr else ""

        if process.returncode != 0:
            LOG.error(f"Failed to join validator: {stderr_str}")
            raise RuntimeError(f"Failed to join validator: {stderr_str}")

        LOG.info("Validator join command executed successfully")
        if stdout_str:
            LOG.debug(f"Command output: {stdout_str}")

        return stdout_str

    except asyncio.TimeoutError:
        LOG.error(f"Validator join command timed out after {timeout} seconds")
        raise


async def execute_validator_leave(
    gravity_cli_path: Path,
    rpc_url: str,
    params: ValidatorJoinParams,
    timeout: int = 60
) -> str:
    """
    Execute validator leave command.

    Args:
        gravity_cli_path: Path to gravity_cli binary
        rpc_url: RPC URL
        params: Validator parameters (uses private_key and validator_address)
        timeout: Command timeout in seconds

    Returns:
        Command output

    Raises:
        RuntimeError: If command fails
        asyncio.TimeoutError: If command times out
    """
    leave_cmd = [
        str(gravity_cli_path),
        "validator", "leave",
        "--rpc-url", rpc_url,
        "--private-key", params.private_key,
        "--validator-address", params.validator_address,
    ]

    LOG.info(f"Executing validator leave command...")
    LOG.debug(f"Command: {' '.join(leave_cmd)}")

    try:
        process = await asyncio.create_subprocess_exec(
            *leave_cmd,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE
        )

        stdout, stderr = await asyncio.wait_for(
            process.communicate(),
            timeout=timeout
        )

        stdout_str = stdout.decode() if stdout else ""
        stderr_str = stderr.decode() if stderr else ""

        if process.returncode != 0:
            LOG.error(f"Failed to leave validator: {stderr_str}")
            raise RuntimeError(f"Failed to leave validator: {stderr_str}")

        LOG.info("Validator leave command executed successfully")
        if stdout_str:
            LOG.debug(f"Command output: {stdout_str}")

        return stdout_str

    except asyncio.TimeoutError:
        LOG.error(f"Validator leave command timed out after {timeout} seconds")
        raise


@dataclass
class ValidatorTestResult:
    """Result of a validator add/remove test"""
    success: bool
    initial_validator_count: int = 1
    after_add_validator_count: int = 2
    after_remove_validator_count: int = 1
    error: Optional[str] = None
    delayed_startup: bool = False


async def run_validator_add_remove_test(
    config: ValidatorTestConfig,
    validator_params: ValidatorJoinParams,
    delayed_node3_start: bool = False,
    pre_node3_start_delay: int = 0,
    post_node3_start_delay: int = 120,
    verification_http_url_after_add: Optional[str] = None
) -> ValidatorTestResult:
    """
    Run a complete validator add/remove test.

    Args:
        config: Test configuration
        validator_params: Validator join parameters
        delayed_node3_start: Whether to delay node3 start after join
        pre_node3_start_delay: Seconds to wait before starting node3 (if delayed)
        post_node3_start_delay: Seconds to wait after starting node3
        verification_http_url_after_add: HTTP URL to use for verification after add
            (defaults to node1's URL, can be set to node3's URL for delayed tests)

    Returns:
        ValidatorTestResult with test outcome
    """
    node_manager = NodeManager()
    node_paths = {}

    # Default verification URL
    if verification_http_url_after_add is None:
        verification_http_url_after_add = config.http_url_node1

    try:
        # Step 1: Deploy nodes
        LOG.info("\n[Step 1] Deploying nodes...")
        node_paths = deploy_nodes(node_manager, config)

        # Step 2: Start node1
        LOG.info("\n[Step 2] Starting node1...")
        start_node(node_manager, node_paths[config.node1_name], config.node1_name)

        # Wait for node to be ready
        LOG.info(f"Waiting {config.node_startup_delay} seconds for node to be ready...")
        await asyncio.sleep(config.node_startup_delay)

        # Step 3: Verify initial validator count == 1
        LOG.info("\n[Step 3] Verifying initial validator count == 1...")
        await asyncio.sleep(10)  # Additional wait
        await verify_validator_count(config.http_url_node1, 1, "initial state")

        # Step 4: Add validator using gravity_cli
        LOG.info("\n[Step 4] Adding validator (node3) using gravity_cli...")
        await execute_validator_join(
            node_manager.gravity_cli_path,
            config.rpc_url,
            validator_params
        )

        # Step 5/6: Handle node3 startup (immediate or delayed)
        if delayed_node3_start:
            # Delayed startup scenario
            if pre_node3_start_delay > 0:
                LOG.info(f"\n[Step 5] Waiting {pre_node3_start_delay} seconds before starting node3...")
                await asyncio.sleep(pre_node3_start_delay)

            LOG.info("\n[Step 6] Starting node3...")
            start_node(node_manager, node_paths[config.node3_name], config.node3_name)

            LOG.info(f"\n[Step 7] Waiting {post_node3_start_delay} seconds after starting node3...")
            await asyncio.sleep(post_node3_start_delay)

            LOG.info("\n[Step 8] Verifying validator count == 2...")
        else:
            # Immediate startup scenario
            LOG.info("\n[Step 5] Starting node3...")
            start_node(node_manager, node_paths[config.node3_name], config.node3_name)

            LOG.info(f"\n[Step 6] Waiting {config.validator_change_wait} seconds and verifying validator count == 2...")
            await asyncio.sleep(config.validator_change_wait)

        await verify_validator_count(verification_http_url_after_add, 2, "after add")

        # Remove validator
        step_num = 9 if delayed_node3_start else 7
        LOG.info(f"\n[Step {step_num}] Removing validator (node3) using gravity_cli...")
        await execute_validator_leave(
            node_manager.gravity_cli_path,
            config.rpc_url,
            validator_params
        )

        # Wait and verify count back to 1
        step_num += 1
        LOG.info(f"\n[Step {step_num}] Waiting {config.validator_change_wait} seconds and verifying validator count == 1...")
        await asyncio.sleep(config.validator_change_wait)

        await verify_validator_count(config.http_url_node1, 1, "after remove")

        LOG.info("\nAll validations passed!")

        return ValidatorTestResult(
            success=True,
            initial_validator_count=1,
            after_add_validator_count=2,
            after_remove_validator_count=1,
            delayed_startup=delayed_node3_start
        )

    except Exception as e:
        LOG.error(f"Test failed: {e}")
        return ValidatorTestResult(
            success=False,
            error=str(e),
            delayed_startup=delayed_node3_start
        )
    finally:
        # Cleanup
        LOG.info("\n[Cleanup] Stopping nodes...")
        stop_nodes(node_manager, node_paths)
