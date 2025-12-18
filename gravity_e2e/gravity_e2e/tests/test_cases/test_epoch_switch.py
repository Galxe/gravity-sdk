"""
Epoch switch test
Tests epoch switching with multiple nodes (node1-node10)
"""

import asyncio
from dataclasses import dataclass
import logging
import subprocess
from typing import Dict

from ...utils.aptos_identity import AptosIdentity, parse_identity_from_yaml
from ...helpers.test_helpers import RunHelper, TestResult, test_case
from ...core.node_manager import NodeManager

LOG = logging.getLogger(__name__)


@dataclass
class ValidatorJoinArgs:
    private_key: str
    validator_address: str
    consensus_public_key: str
    validator_network_address: str
    fullnode_network_address: str
    aptos_address: str


class TestContext:
    def __init__(self, run_helper: RunHelper):
        self.run_helper = run_helper
        self.node_manager = NodeManager()
        self.node_names = [f"node{i}" for i in range(5, 11)]  # node5 to node10
        self.install_dir = "/tmp"
        self.config_base_dir = (
            self.node_manager.workspace_root
            / "gravity_e2e"
            / "configs"
            / "test_epoch_switch"
        )
        self.node_to_identity: Dict[str, AptosIdentity] = dict()
        self.node_to_account: Dict[str, Dict] = dict()
        self.node_to_validator_join_args: Dict[str, ValidatorJoinArgs] = dict()

    def deploy_nodes(self):
        deploy_results = {}

        for node_name in self.node_names:
            identity_path = (
                self.config_base_dir / node_name / "config" / "validator-identity.yaml"
            )
            identity = parse_identity_from_yaml(identity_path)
        LOG.info(f"Loaded identity map: {self.node_to_identity}")

        for node_name in self.node_names:
            LOG.info(f"Deploying {node_name}...")
            # 获取该节点的配置目录路径
            node_config_dir = str(self.config_base_dir / node_name / "config")

            deploy_success = self.node_manager.deploy_node(
                node_name=node_name,
                mode="cluster",
                install_dir=self.install_dir,
                bin_version="quick-release",
                recover=False,
                node_config_dir=node_config_dir,
            )

            deploy_results[node_name] = deploy_success

            if not deploy_success:
                LOG.error(f"Failed to deploy {node_name}")
                raise RuntimeError(f"Failed to deploy {node_name}")
            else:
                LOG.info(f"✅ Node {node_name} deployed")

        # 检查所有节点是否部署成功
        failed_nodes = [name for name, success in deploy_results.items() if not success]
        if failed_nodes:
            raise RuntimeError(f"Failed to deploy nodes: {', '.join(failed_nodes)}")

    def start_nodes(self):
        start_results = {}

        for node_name in self.node_names:
            LOG.info(f"Starting {node_name}...")
            node_path = self.node_manager.get_node_deploy_path(
                node_name, self.install_dir
            )
            start_success = self.node_manager.start_node(node_path)

            start_results[node_name] = start_success

            if not start_success:
                LOG.error(f"Failed to start {node_name}")
            else:
                LOG.info(f"✅ Node {node_name} started")

        # 检查所有节点是否启动成功
        failed_start_nodes = [
            name for name, success in start_results.items() if not success
        ]
        if failed_start_nodes:
            raise RuntimeError(
                f"Failed to start nodes: {', '.join(failed_start_nodes)}"
            )

    def stop_nodes(self):
        for node_name in self.node_names:
            try:
                node_path = self.node_manager.get_node_deploy_path(
                    node_name, self.install_dir
                )
                stop_success = self.node_manager.stop_node(node_path)

                if stop_success:
                    LOG.info(f"✅ Node {node_name} stopped")
                else:
                    LOG.warning(f"⚠️  Failed to stop {node_name}")
            except Exception as e:
                LOG.warning(f"⚠️  Error stopping node {node_name}: {e}")

    async def faucet_nodes(self):
        for node_name in self.node_names:
            LOG.info(f"Creating EVM account for {node_name}...")
            account = await self.run_helper.create_test_account(
                node_name, fund_wei=10**24
            )
            LOG.info(f"✅ Created EVM account for {node_name}: {account}")
            self.node_to_account[node_name] = account

    def load_consensus_public_key(self, node_name: str):
        consensus_public_key_path = (
            self.config_base_dir / node_name / "config" / "consensus_public_key"
        )
        with open(consensus_public_key_path, "r") as f:
            consensus_public_key = f.read().strip()
        return consensus_public_key

    def validator_join(self, node_name: str):
        validator_join_args = self.node_to_validator_join_args[node_name]
        join_cmd = [
            str(self.node_manager.gravity_cli_path),
            "validator",
            "join",
            "--rpc-url",
            self.run_helper.client.rpc_url,
            "--private-key",
            validator_join_args.private_key,
            "--stake-amount",
            "10001.0",
            "--validator-address",
            validator_join_args.validator_address,
            "--consensus-public-key",
            validator_join_args.consensus_public_key,
            "--validator-network-address",
            validator_join_args.validator_network_address,
            "--fullnode-network-address",
            validator_join_args.fullnode_network_address,
            "--aptos-address",
            validator_join_args.aptos_address,
        ]
        LOG.info(f"Running command: {' '.join(join_cmd)}")
        result = subprocess.run(join_cmd, capture_output=True, text=True, timeout=60)
        if result.returncode != 0:
            LOG.error(f"Failed to join validator: {result.stderr}")
            raise RuntimeError(f"Failed to join validator: {result.stderr}")
        LOG.info(f"✅ Validator join command executed successfully")
        if result.stdout:
            LOG.info(f"Command output: {result.stdout}")

    def validator_leave(self, node_name: str):
        args = self.node_to_validator_join_args[node_name]
        leave_cmd = [
            str(self.node_manager.gravity_cli_path),
            "validator",
            "leave",
            "--rpc-url",
            self.run_helper.client.rpc_url,
            "--private-key",
            args.private_key,
            "--validator-address",
            args.validator_address,
        ]
        LOG.info(f"Running command: {' '.join(leave_cmd)}")
        result = subprocess.run(leave_cmd, capture_output=True, text=True, timeout=60)
        if result.returncode != 0:
            LOG.error(f"Failed to leave validator: {result.stderr}")
            raise RuntimeError(f"Failed to leave validator: {result.stderr}")
        LOG.info(f"✅ Validator leave command executed successfully")
        if result.stdout:
            LOG.info(f"Command output: {result.stdout}")

    def init_node_to_identity(self):
        for node_name in self.node_names:
            identity_path = (
                self.config_base_dir / node_name / "config" / "validator-identity.yaml"
            )
            identity = parse_identity_from_yaml(identity_path)
            self.node_to_identity[node_name] = identity
            LOG.info(f"Loaded identity for {node_name}: {identity}")

    def init_validator_join_args(self):
        # FIXME: hardcoded port offsets
        validator_network_address_port_offset = 2025
        fullnode_network_address_port_offset = 2125
        for i, node_name in enumerate(self.node_names):
            validator_network_address_port = validator_network_address_port_offset + i
            fullnode_network_address_port = fullnode_network_address_port_offset + i
            consensus_public_key = self.load_consensus_public_key(node_name)
            identity = self.node_to_identity[node_name]
            account = self.node_to_account[node_name]
            args = ValidatorJoinArgs(
                private_key=account["private_key"],
                validator_address=account["address"],
                consensus_public_key=consensus_public_key,
                validator_network_address=f"/ip4/127.0.0.1/tcp/{validator_network_address_port}/noise-ik/{identity.account_address}/handshake/0",
                fullnode_network_address=f"/ip4/127.0.0.1/tcp/{fullnode_network_address_port}/noise-ik/{identity.account_address}/handshake/0",
                aptos_address=identity.account_address,
            )
            LOG.info(f"Initialized validator join args for {node_name}: {args}")
            self.node_to_validator_join_args[node_name] = args


@test_case
async def test_epoch_switch(run_helper: RunHelper, test_result: TestResult):
    """
    Test epoch switching with multiple nodes

    Test steps:
    1. Deploy node5-node10
    2. Start node5-node10
    3. Wait for nodes to be ready
    4. Create EVM accounts for node5-10
    """
    LOG.info("=" * 70)
    LOG.info("Test: Epoch Switch Test")
    LOG.info("=" * 70)

    test_context = TestContext(run_helper)
    try:
        # Step 1: Deploy node1-node10
        LOG.info("\n[Step 1] Deploying node5-node10...")
        test_context.deploy_nodes()
        LOG.info(f"✅ All nodes (node5-node10) deployed successfully")

        # Step 2: Start node1-node10
        LOG.info("\n[Step 2] Starting node5-node10...")
        test_context.start_nodes()
        LOG.info(f"✅ All nodes (node1-node10) started successfully")

        # Step 3: Wait for nodes to be ready
        LOG.info("\n[Step 3] Waiting 10 seconds for nodes to be ready...")
        await asyncio.sleep(10)

        LOG.info("\n✅ All nodes deployed and started successfully!")
        LOG.info("=" * 70)

        # node1-4 are genesis validators, create evm accounts for node5-10
        LOG.info("\n[Step 4] Creating EVM accounts for node5-10...")
        await test_context.faucet_nodes()
        LOG.info(f"✅ All nodes (node5-10) EVM accounts created successfully")

        # Step 5: Initialize node to identity and validator join args
        LOG.info("\n[Step 5] Initializing node to identity and validator join args...")
        test_context.init_node_to_identity()
        test_context.init_validator_join_args()
        LOG.info(
            f"✅ Node to identity and validator join args initialized successfully"
        )

        test_result.mark_success()

    except Exception as e:
        LOG.error(f"❌ Test failed: {e}")
        test_result.mark_failure(str(e))
        raise
    finally:
        # Cleanup: Stop nodes
        LOG.info("\n[Cleanup] Stopping nodes...")
        test_context.stop_nodes()
