"""
Epoch switch test
Tests epoch switching with multiple nodes (node1-node10)
"""

import asyncio
from dataclasses import dataclass
import logging
import random
import signal
import subprocess
from typing import Dict, Set

from gravity_e2e.gravity_e2e.core.client.gravity_client import GravityClient

from ...utils.aptos_identity import AptosIdentity, parse_identity_from_yaml
from ...helpers.test_helpers import RunHelper, TestResult, test_case
from ...core.node_manager import NodeManager
from ...core.client.gravity_http_client import GravityHttpClient

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
        self.should_stop = False
        signal.signal(signal.SIGTERM, self.signal_handler)
        signal.signal(signal.SIGINT, self.signal_handler)

    def signal_handler(self, signum, frame):
        self.should_stop = True
        LOG.info(f"收到信号 {signal.Signals(signum).name}，准备停止...")

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

    async def fuzzy_validator_join_and_leave(self):
        """
        模糊测试：持续随机地让节点加入和离开 validator set
        """
        # validator set not include genesis validators
        validator_set: Set[str] = set()
        pending_joins: Set[str] = set()
        pending_leaves: Set[str] = set()

        # FIXME: hardcoded port
        http_url = "http://127.0.0.1:1031"

        # 初始化 HTTP 客户端
        http_client = GravityHttpClient(base_url=http_url)
        async with http_client:
            # 获取初始 epoch
            try:
                current_epoch = await http_client.get_current_epoch()
                LOG.info(f"初始 epoch: {current_epoch}")
            except Exception as e:
                LOG.error(f"❌ 无法获取初始 epoch: {e}")
                # 如果无法获取初始 epoch，设置为 0 并继续
                raise RuntimeError(f"Failed to get epoch: {e}")

            # 主循环：检查停止标志
            while not self.should_stop:
                try:
                    # 每隔 10 秒检查 epoch 是否切换
                    await asyncio.sleep(10)

                    # 检查 epoch 是否切换
                    try:
                        new_epoch = await http_client.get_current_epoch()
                    except Exception as e:
                        LOG.warning(f"⚠️ 无法获取当前 epoch: {e}，跳过本次检查")
                        raise RuntimeError(f"Failed to get epoch: {e}")

                    if new_epoch == current_epoch:
                        continue

                    if new_epoch < current_epoch:
                        raise RuntimeError(
                            f"Epoch decreased from {current_epoch} to {new_epoch}"
                        )
                    # Epoch 切换了，更新 validator_set
                    LOG.info(f"Epoch 从 {current_epoch} 切换到 {new_epoch}")

                    # 将成功 join 的节点加入 validator_set
                    validator_set.update(pending_joins)
                    if pending_joins:
                        LOG.info(f"节点 {pending_joins} 进入 validator_set")

                    # 将成功 leave 的节点从 validator_set 移除
                    validator_set.difference_update(pending_leaves)
                    if pending_leaves:
                        LOG.info(f"节点 {pending_leaves} 退出 validator_set")

                    # 重置待处理的 join 和 leave
                    pending_joins.clear()
                    pending_leaves.clear()

                    # 更新当前 epoch
                    current_epoch = new_epoch
                    LOG.info(f"当前 validator_set: {validator_set}")

                    # 在每个 epoch 期间执行随机 join 和 leave
                    # 从不在 validator_set 的节点中随机选择 1-3 个节点调用 validator join
                    nodes_not_in_validator = [
                        node for node in self.node_names if node not in validator_set
                    ]

                    if nodes_not_in_validator:
                        # 随机选择 1-3 个节点
                        num_joins = random.randint(
                            1, min(3, len(nodes_not_in_validator))
                        )
                        nodes_to_join = random.sample(nodes_not_in_validator, num_joins)

                        for node_name in nodes_to_join:
                            LOG.info(f"尝试让节点 {node_name} join validator set...")
                            self.validator_join(node_name)
                            pending_joins.add(node_name)
                            LOG.info(
                                f"✅ 节点 {node_name} join 成功，将在下一个 epoch 进入 validator_set"
                            )
                    # 从在 validator_set 的节点中随机选择 1-3 个节点调用 validator leave
                    if validator_set:
                        # 随机选择 1-3 个节点
                        num_leaves = random.randint(1, min(3, len(validator_set)))
                        nodes_to_leave = random.sample(validator_set, num_leaves)

                        for node_name in nodes_to_leave:
                            LOG.info(f"尝试让节点 {node_name} leave validator set...")
                            self.validator_leave(node_name)
                            pending_leaves.add(node_name)
                            LOG.info(
                                f"✅ 节点 {node_name} leave 成功，将在下一个 epoch 退出 validator_set"
                            )
                except Exception as e:
                    raise RuntimeError(f"Failed to fuzzy validator join and leave: {e}")

    async def check_node_block_height(self):
        clients = [self.run_helper.client]
        for i in range(2, 11):
            node_name = f"node{i}"
            # FIXME: hardcoded port
            http_url = f"http://127.0.0.1:{8540 + i}"
            client = GravityClient(rpc_url=http_url, node_id=node_name)
            clients.append(client)
        block_heights = await asyncio.gather(
            *[client.get_block_number() for client in clients]
        )
        max_block_height = max(block_heights)
        LOG.info(f"Max block height: {max_block_height}")
        is_gap_too_large = False
        for i, block_height in enumerate(block_heights):
            LOG.info(f"node{i} block height: {block_height}")
            if block_height + 100 < max_block_height:
                LOG.warning(f"node{i} block height is too low: {block_height}")
                is_gap_too_large = True
        if is_gap_too_large:
            raise RuntimeError(f"Gap between node block heights is too large")


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

        # Step 6: Fuzzy validator join and leave
        LOG.info("\n[Step 6] Fuzzy validator join and leave...")
        await test_context.fuzzy_validator_join_and_leave()
        LOG.info(f"✅ Fuzzy validator join and leave completed successfully")

        # Step 7: Check node block height
        LOG.info("\n[Step 7] Checking node block height...")
        await test_context.check_node_block_height()
        LOG.info(f"✅ Node block height checked successfully")

        test_result.mark_success()

    except Exception as e:
        LOG.error(f"❌ Test failed: {e}")
        test_result.mark_failure(str(e))
        raise
    finally:
        # Cleanup: Stop nodes
        LOG.info("\n[Cleanup] Stopping nodes...")
        test_context.stop_nodes()
