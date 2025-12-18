"""
Validator add and remove test
Tests adding and removing validators from the validator set
"""
import asyncio
import logging
import subprocess
from typing import Dict, Optional
from pathlib import Path

from ...helpers.test_helpers import RunHelper, TestResult, test_case
from ...core.client.gravity_http_client import GravityHttpClient
from ...core.node_manager import NodeManager

LOG = logging.getLogger(__name__)


@test_case
async def test_validator_add_remove(
    run_helper: RunHelper,
    test_result: TestResult
):
    """
    Test adding and removing validators
    
    Test steps:
    1. Deploy node1 and node3
    2. Start node1
    3. Wait 1 minute and verify validator count == 1
    4. Add validator (node3) using gravity_cli
    5. Start node3
    6. Wait 2 minutes and verify validator count == 2
    7. Remove validator (node3) using gravity_cli
    8. Wait 2 minutes and verify validator count == 1
    9. Stop nodes
    """
    LOG.info("=" * 70)
    LOG.info("Test: Validator Add and Remove Test")
    LOG.info("=" * 70)
    
    node_manager = NodeManager()
    node1_name = "node1"
    node3_name = "node3"
    install_dir = "/tmp"
    http_url = "http://127.0.0.1:1024"
    rpc_url = "http://127.0.0.1:8545"
    
    # Validator join parameters
    private_key = "0x...."
    stake_amount = "10001.0"
    validator_address = "0x9B2C25E77a97d3e84DC0Cb7F83fb676ddC4F24b9"
    consensus_public_key = "b7a931fa544c2d1d54dee27619edfb70cc801bc599dd7a3f56f641a588cee4600b63e35d0d35fe69f2e454462b0ce9b2"
    validator_network_address = "/ip4/127.0.0.1/tcp/2026/noise-ik/99d1c7709b14777edbdbe0c602eb0186ea845ed75b01740726e581215de8625b/handshake/0"
    fullnode_network_address = "/ip4/127.0.0.1/tcp/2036/noise-ik/99d1c7709b14777edbdbe0c602eb0186ea845ed75b01740726e581215de8625b/handshake/0"
    aptos_address = "99d1c7709b14777edbdbe0c602eb0186ea845ed75b01740726e581215de8625b"
    
    try:
        # Step 1: Deploy node1 and node3
        LOG.info("\n[Step 1] Deploying node1 and node3...")
        deploy_results = node_manager.deploy_nodes(
            node_names=[node1_name, node3_name],
            mode="single",
            install_dir=install_dir,
            bin_version="quick-release",
            recover=False
        )
        
        if not deploy_results.get(node1_name):
            raise RuntimeError(f"Failed to deploy {node1_name}")
        if not deploy_results.get(node3_name):
            raise RuntimeError(f"Failed to deploy {node3_name}")
        
        LOG.info(f"✅ Node {node1_name} and {node3_name} deployed")
        
        # Step 2: Start node1
        LOG.info("\n[Step 2] Starting node1...")
        node1_path = node_manager.get_node_deploy_path(node1_name, install_dir)
        start_success = node_manager.start_node(node1_path)
        
        if not start_success:
            raise RuntimeError(f"Failed to start {node1_name}")
        
        LOG.info(f"✅ Node {node1_name} started")
        
        # Wait a bit for node to be ready
        LOG.info("Waiting 10 seconds for node to be ready...")
        await asyncio.sleep(10)
        
        # Step 3: Wait 10 seconds and verify validator count == 1
        LOG.info("\n[Step 3] Waiting 10 seconds and verifying validator count == 1...")
        await asyncio.sleep(10)  # Wait 10 seconds
        
        # Ensure HTTP client is created within the async context
        http_client = GravityHttpClient(base_url=http_url)
        async with http_client:
            # Get current epoch
            current_epoch = await http_client.get_current_epoch()
            LOG.info(f"Current epoch: {current_epoch}")
            
            # Try to get validator count for current epoch, if it fails, use previous epoch
            # (current epoch may not have completed yet)
            try:
                validator_count_data = await http_client.get_validator_count_by_epoch(current_epoch)
            except RuntimeError:
                # If current epoch is not available, use previous epoch
                LOG.info(f"Current epoch {current_epoch} not available, trying epoch {current_epoch - 1}")
                validator_count_data = await http_client.get_validator_count_by_epoch(current_epoch - 1)
            
            validator_count = validator_count_data["validator_count"]
            
            if validator_count != 1:
                raise AssertionError(
                    f"Expected validator count to be 1, but got {validator_count}"
                )
            
            LOG.info(f"✅ Validation passed: validator count == 1")
        
        # Step 4: Add validator (node3) using gravity_cli
        LOG.info("\n[Step 4] Adding validator (node3) using gravity_cli...")
        gravity_cli_path = node_manager.gravity_cli_path
        
        join_cmd = [
            str(gravity_cli_path),
            "validator", "join",
            "--rpc-url", rpc_url,
            "--private-key", private_key,
            "--stake-amount", stake_amount,
            "--validator-address", validator_address,
            "--consensus-public-key", consensus_public_key,
            "--validator-network-address", validator_network_address,
            "--fullnode-network-address", fullnode_network_address,
            "--aptos-address", aptos_address,
        ]
        
        LOG.info(f"Running command: {' '.join(join_cmd)}")
        result = subprocess.run(
            join_cmd,
            capture_output=True,
            text=True,
            timeout=60
        )
        
        if result.returncode != 0:
            LOG.error(f"Failed to join validator: {result.stderr}")
            raise RuntimeError(f"Failed to join validator: {result.stderr}")
        
        LOG.info(f"✅ Validator join command executed successfully")
        if result.stdout:
            LOG.info(f"Command output: {result.stdout}")
        
        # Step 5: Start node3
        LOG.info("\n[Step 5] Starting node3...")
        node3_path = node_manager.get_node_deploy_path(node3_name, install_dir)
        start_success = node_manager.start_node(node3_path)
        
        if not start_success:
            raise RuntimeError(f"Failed to start {node3_name}")
        
        LOG.info(f"✅ Node {node3_name} started")
        
        # Step 6: Wait 2 minutes and verify validator count == 2
        LOG.info("\n[Step 6] Waiting 2 minutes and verifying validator count == 2...")
        await asyncio.sleep(120)  # Wait 2 minutes
        
        # Wait for epoch to potentially change
        http_client = GravityHttpClient(base_url=http_url)
        async with http_client:
            # Get current epoch (may have changed)
            current_epoch = await http_client.get_current_epoch()
            LOG.info(f"Current epoch after adding validator: {current_epoch}")
            
            # Try to get validator count for current epoch, if it fails, use previous epoch
            try:
                validator_count_data = await http_client.get_validator_count_by_epoch(current_epoch)
            except RuntimeError:
                # If current epoch is not available, use previous epoch
                LOG.info(f"Current epoch {current_epoch} not available, trying epoch {current_epoch - 1}")
                validator_count_data = await http_client.get_validator_count_by_epoch(current_epoch - 1)
            
            validator_count = validator_count_data["validator_count"]
            
            if validator_count != 2:
                raise AssertionError(
                    f"Expected validator count to be 2, but got {validator_count}"
                )
            
            LOG.info(f"✅ Validation passed: validator count == 2")
        
        # Step 7: Remove validator (node3) using gravity_cli
        LOG.info("\n[Step 7] Removing validator (node3) using gravity_cli...")
        
        leave_cmd = [
            str(gravity_cli_path),
            "validator", "leave",
            "--rpc-url", rpc_url,
            "--private-key", private_key,
            "--validator-address", validator_address,
        ]
        
        LOG.info(f"Running command: {' '.join(leave_cmd)}")
        result = subprocess.run(
            leave_cmd,
            capture_output=True,
            text=True,
            timeout=60
        )
        
        if result.returncode != 0:
            LOG.error(f"Failed to leave validator: {result.stderr}")
            raise RuntimeError(f"Failed to leave validator: {result.stderr}")
        
        LOG.info(f"✅ Validator leave command executed successfully")
        if result.stdout:
            LOG.info(f"Command output: {result.stdout}")
        
        # Step 8: Wait 2 minutes and verify validator count == 1
        LOG.info("\n[Step 8] Waiting 2 minutes and verifying validator count == 1...")
        await asyncio.sleep(120)  # Wait 2 minutes
        
        # Wait for epoch to potentially change
        http_client = GravityHttpClient(base_url=http_url)
        async with http_client:
            # Get current epoch (may have changed)
            current_epoch = await http_client.get_current_epoch()
            LOG.info(f"Current epoch after removing validator: {current_epoch}")
            
            # Try to get validator count for current epoch, if it fails, use previous epoch
            try:
                validator_count_data = await http_client.get_validator_count_by_epoch(current_epoch)
            except RuntimeError:
                # If current epoch is not available, use previous epoch
                LOG.info(f"Current epoch {current_epoch} not available, trying epoch {current_epoch - 1}")
                validator_count_data = await http_client.get_validator_count_by_epoch(current_epoch - 1)
            
            validator_count = validator_count_data["validator_count"]
            
            if validator_count != 1:
                raise AssertionError(
                    f"Expected validator count to be 1, but got {validator_count}"
                )
            
            LOG.info(f"✅ Validation passed: validator count == 1")
        
        LOG.info("\n✅ All validations passed!")
        LOG.info("=" * 70)
        
        test_result.mark_success(
            initial_validator_count=1,
            after_add_validator_count=2,
            after_remove_validator_count=1
        )
        
    except Exception as e:
        LOG.error(f"❌ Test failed: {e}")
        test_result.mark_failure(str(e))
        raise
    finally:
        # Cleanup: Stop nodes
        LOG.info("\n[Cleanup] Stopping nodes...")
        try:
            node1_path = node_manager.get_node_deploy_path(node1_name, install_dir)
            node3_path = node_manager.get_node_deploy_path(node3_name, install_dir)
            
            stop1_success = node_manager.stop_node(node1_path)
            stop3_success = node_manager.stop_node(node3_path)
            
            if stop1_success:
                LOG.info(f"✅ Node {node1_name} stopped")
            else:
                LOG.warning(f"⚠️  Failed to stop {node1_name}")
            
            if stop3_success:
                LOG.info(f"✅ Node {node3_name} stopped")
            else:
                LOG.warning(f"⚠️  Failed to stop {node3_name}")
        except Exception as e:
            LOG.warning(f"⚠️  Error stopping nodes: {e}")

