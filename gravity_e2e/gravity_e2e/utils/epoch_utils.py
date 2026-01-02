"""
Epoch consistency testing utilities

This module provides common functions for epoch consistency validation,
reducing duplication across different epoch test scenarios.
"""
import asyncio
import logging
from dataclasses import dataclass
from typing import Dict, List, Optional

from ..core.client.gravity_http_client import GravityHttpClient
from ..core.node_manager import NodeManager

LOG = logging.getLogger(__name__)


@dataclass
class EpochConfig:
    """Configuration for epoch consistency test"""
    num_epochs: int  # Total number of epochs to monitor (e.g., 3 or 10)
    check_interval: int  # Interval between epoch checks in seconds
    epoch_timeout: int = 600  # Timeout per epoch in seconds
    node_startup_delay: int = 10  # Delay after node starts


@dataclass
class EpochData:
    """Data collected for a single epoch"""
    epoch: int
    ledger_info: Dict
    block_round_1: Optional[Dict] = None
    qc_round_1: Optional[Dict] = None


@dataclass
class EpochConsistencyResult:
    """Result of epoch consistency validation"""
    success: bool
    epochs_validated: List[int]
    epoch_data: Dict[int, EpochData]
    error: Optional[str] = None


async def collect_epoch_data(
    http_client: GravityHttpClient,
    config: EpochConfig
) -> Dict[int, EpochData]:
    """
    Collect epoch data for consistency validation.

    Args:
        http_client: HTTP client for Gravity API
        config: Epoch test configuration

    Returns:
        Dictionary mapping epoch number to collected data
    """
    epoch_data: Dict[int, EpochData] = {}
    target_epochs = list(range(1, config.num_epochs + 1))

    for target_epoch in target_epochs:
        LOG.info(f"\n[Epoch {target_epoch}] Waiting for epoch {target_epoch}...")

        # Wait for target epoch
        try:
            current_epoch = await http_client.wait_for_epoch(
                target_epoch=target_epoch,
                timeout=config.epoch_timeout
            )
            LOG.info(f"Reached epoch {current_epoch}")
        except TimeoutError as e:
            LOG.error(f"Timeout waiting for epoch {target_epoch}: {e}")
            raise

        # Get ledger info for this epoch
        LOG.info(f"[Epoch {target_epoch}] Getting ledger info...")
        try:
            ledger_info = await http_client.get_ledger_info_by_epoch(target_epoch)
        except RuntimeError:
            # If current epoch not complete yet, use previous epoch
            LOG.info(f"Epoch {target_epoch} not available yet, using epoch {target_epoch - 1}")
            ledger_info = await http_client.get_ledger_info_by_epoch(target_epoch - 1)

        data = EpochData(
            epoch=target_epoch,
            ledger_info=ledger_info
        )

        LOG.info(f"  Epoch {target_epoch} ledger info:")
        LOG.info(f"    block_number: {ledger_info['block_number']}")
        LOG.info(f"    round: {ledger_info['round']}")
        LOG.info(f"    block_hash: {ledger_info['block_hash']}")

        # For epochs >= 2, get round 1 block and QC
        if target_epoch >= 2:
            LOG.info(f"[Epoch {target_epoch}] Getting round 1 block and QC...")

            # Get block for epoch, round 1
            block_info = await http_client.get_block_by_epoch_round(epoch=target_epoch, round=1)
            data.block_round_1 = block_info

            LOG.info(f"  Epoch {target_epoch}, Round 1 block:")
            LOG.info(f"    block_number: {block_info.get('block_number')}")
            LOG.info(f"    block_id: {block_info['block_id']}")

            # Get QC for epoch, round 1
            qc_info = await http_client.get_qc_by_epoch_round(epoch=target_epoch, round=1)
            data.qc_round_1 = qc_info

            LOG.info(f"  Epoch {target_epoch}, Round 1 QC:")
            LOG.info(f"    certified_block_id: {qc_info['certified_block_id']}")
            LOG.info(f"    commit_info_block_id: {qc_info['commit_info_block_id']}")

        epoch_data[target_epoch] = data

        # Wait before checking next epoch (except for the last one)
        if target_epoch < target_epochs[-1]:
            LOG.info(f"Waiting {config.check_interval} seconds before checking next epoch...")
            await asyncio.sleep(config.check_interval)

    return epoch_data


def validate_epoch_consistency(
    epoch_data: Dict[int, EpochData],
    validation_range: List[int]
) -> EpochConsistencyResult:
    """
    Validate epoch consistency across the collected data.

    Validates for each N in validation_range:
    - Epoch N ledger_info.block_number == Epoch N+1 round 1 block.block_number - 1
    - Epoch N+1 round 1 QC commit_info_block_id != Epoch N ledger_info.block_hash

    Args:
        epoch_data: Collected epoch data
        validation_range: List of epoch numbers to validate (e.g., [1, 2] or [1..9])

    Returns:
        EpochConsistencyResult with validation outcome
    """
    LOG.info("\nValidating data consistency...")

    for n in validation_range:
        LOG.info(f"\n[Validation for N={n}]")

        # Get Epoch N ledger_info
        epoch_n_data = epoch_data[n]
        epoch_n_ledger_info = epoch_n_data.ledger_info
        epoch_n_block_number = epoch_n_ledger_info["block_number"]
        epoch_n_block_hash = epoch_n_ledger_info["block_hash"]

        # Get Epoch N+1 round 1 block and QC
        epoch_n_plus_1 = n + 1
        epoch_n_plus_1_data = epoch_data[epoch_n_plus_1]
        epoch_n_plus_1_block = epoch_n_plus_1_data.block_round_1
        epoch_n_plus_1_qc = epoch_n_plus_1_data.qc_round_1

        if epoch_n_plus_1_block is None:
            return EpochConsistencyResult(
                success=False,
                epochs_validated=[],
                epoch_data=epoch_data,
                error=f"Epoch {epoch_n_plus_1} round 1 block data not available"
            )

        epoch_n_plus_1_block_number = epoch_n_plus_1_block.get("block_number")
        if epoch_n_plus_1_block_number is None:
            return EpochConsistencyResult(
                success=False,
                epochs_validated=[],
                epoch_data=epoch_data,
                error=f"Epoch {epoch_n_plus_1} round 1 block has no block_number"
            )

        # Validation 1: Epoch N block_number == Epoch N+1 round 1 block_number - 1
        expected_block_number = epoch_n_plus_1_block_number - 1
        if epoch_n_block_number != expected_block_number:
            return EpochConsistencyResult(
                success=False,
                epochs_validated=[],
                epoch_data=epoch_data,
                error=(
                    f"Block number mismatch for N={n}: "
                    f"Epoch {n} ledger_info.block_number ({epoch_n_block_number}) "
                    f"should equal Epoch {epoch_n_plus_1} round 1 block.block_number - 1 "
                    f"({epoch_n_plus_1_block_number} - 1 = {expected_block_number})"
                )
            )

        LOG.info(f"Validation 1 for N={n} passed: "
                f"Epoch {n} block_number ({epoch_n_block_number}) == "
                f"Epoch {epoch_n_plus_1} round 1 block_number - 1 ({expected_block_number})")

        # Validation 2: Epoch N+1 round 1 commit_info_block_id != Epoch N block_hash
        if epoch_n_plus_1_qc is None:
            return EpochConsistencyResult(
                success=False,
                epochs_validated=[],
                epoch_data=epoch_data,
                error=f"Epoch {epoch_n_plus_1} round 1 QC data not available"
            )

        epoch_n_plus_1_commit_info_block_id = epoch_n_plus_1_qc["commit_info_block_id"]

        # Convert hex strings to compare (remove 0x prefix if present)
        epoch_n_block_hash_clean = epoch_n_block_hash.replace("0x", "").lower()
        epoch_n_plus_1_commit_info_clean = epoch_n_plus_1_commit_info_block_id.replace("0x", "").lower()

        if epoch_n_block_hash_clean == epoch_n_plus_1_commit_info_clean:
            return EpochConsistencyResult(
                success=False,
                epochs_validated=[],
                epoch_data=epoch_data,
                error=(
                    f"Block ID conflict for N={n}: "
                    f"Epoch {n} ledger_info.block_hash ({epoch_n_block_hash}) "
                    f"equals Epoch {epoch_n_plus_1} round 1 QC commit_info_block_id "
                    f"({epoch_n_plus_1_commit_info_block_id})"
                )
            )

        LOG.info(f"Validation 2 for N={n} passed: "
                f"Epoch {n} block_hash ({epoch_n_block_hash}) != "
                f"Epoch {epoch_n_plus_1} round 1 QC commit_info_block_id "
                f"({epoch_n_plus_1_commit_info_block_id})")

    LOG.info(f"\nAll validations passed for N={validation_range}!")

    return EpochConsistencyResult(
        success=True,
        epochs_validated=validation_range,
        epoch_data=epoch_data
    )


async def run_epoch_consistency_test(
    config: EpochConfig,
    http_url: str = "http://127.0.0.1:1024",
    node_name: str = "node1",
    install_dir: str = "/tmp",
    deploy_node: bool = True
) -> EpochConsistencyResult:
    """
    Run a complete epoch consistency test.

    This is the main entry point that handles:
    1. Node deployment and startup (optional)
    2. Epoch data collection
    3. Consistency validation
    4. Cleanup

    Args:
        config: Epoch test configuration
        http_url: HTTP API URL
        node_name: Node name to deploy/start
        install_dir: Installation directory
        deploy_node: Whether to deploy and manage node lifecycle

    Returns:
        EpochConsistencyResult with test outcome
    """
    node_manager = NodeManager() if deploy_node else None
    deploy_path = None

    try:
        if deploy_node and node_manager:
            # Deploy node
            LOG.info(f"\n[Step 1] Deploying {node_name}...")
            deploy_success = node_manager.deploy_node(
                node_name=node_name,
                mode="single",
                install_dir=install_dir,
                bin_version="quick-release",
                recover=False
            )

            if not deploy_success:
                raise RuntimeError(f"Failed to deploy {node_name}")

            deploy_path = node_manager.get_node_deploy_path(node_name, install_dir)
            LOG.info(f"Node {node_name} deployed to {deploy_path}")

            # Start node
            LOG.info(f"\n[Step 2] Starting {node_name}...")
            start_success = node_manager.start_node(deploy_path)

            if not start_success:
                raise RuntimeError(f"Failed to start {node_name}")

            LOG.info(f"Node {node_name} started")

            # Wait for node to be ready
            LOG.info(f"Waiting {config.node_startup_delay} seconds for node to be ready...")
            await asyncio.sleep(config.node_startup_delay)

        # Collect and validate epoch data
        LOG.info(f"\n[Step 3] Monitoring epochs (checking every {config.check_interval} seconds)...")

        async with GravityHttpClient(base_url=http_url) as http_client:
            epoch_data = await collect_epoch_data(http_client, config)

        # Validate consistency
        LOG.info("\n[Step 4] Validating data consistency...")
        validation_range = list(range(1, config.num_epochs))  # N = [1, 2, ..., num_epochs-1]
        result = validate_epoch_consistency(epoch_data, validation_range)

        return result

    except Exception as e:
        LOG.error(f"Test failed: {e}")
        return EpochConsistencyResult(
            success=False,
            epochs_validated=[],
            epoch_data={},
            error=str(e)
        )
    finally:
        # Cleanup: Stop node
        if deploy_node and node_manager and deploy_path:
            LOG.info("\n[Cleanup] Stopping node...")
            try:
                stop_success = node_manager.stop_node(deploy_path)
                if stop_success:
                    LOG.info(f"Node {node_name} stopped")
                else:
                    LOG.warning(f"Failed to stop {node_name}")
            except Exception as e:
                LOG.warning(f"Error stopping node: {e}")
