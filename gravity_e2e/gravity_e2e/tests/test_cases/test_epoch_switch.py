"""
Epoch switch test
Tests epoch switching with multiple nodes (node1-node10)
"""

import asyncio
import logging
import random
import signal
from contextlib import AsyncExitStack
from typing import Dict, Set

from ...utils.aptos_identity import AptosIdentity, parse_identity_from_yaml
from ...utils.validator_utils import (
    ValidatorJoinParams,
    execute_validator_join,
    execute_validator_leave,
    execute_validator_list,
)
from ...helpers.test_helpers import RunHelper, TestResult, test_case
from ...core.node_manager import NodeManager
from ...core.client.gravity_client import GravityClient
from ...core.client.gravity_http_client import GravityHttpClient

LOG = logging.getLogger(__name__)


class TestContext:
    def __init__(self, run_helper: RunHelper):
        self.run_helper = run_helper
        # FIXME: hardcoded port
        self.run_helper.client = GravityClient(
            rpc_url="http://127.0.0.1:8541", node_id="node1"
        )
        self.node_manager = NodeManager()
        # node1-4 are genesis validators
        self.genesis_node_names = [f"node{i}" for i in range(1, 5)]
        # node5-10 are validator candidate nodes
        self.candidate_node_names = [f"node{i}" for i in range(5, 11)]
        self.install_dir = "/tmp"
        self.config_base_dir = (
            self.node_manager.workspace_root
            / "gravity_e2e"
            / "configs"
            / "test_epoch_switch"
        )
        self.node_to_identity: Dict[str, AptosIdentity] = dict()
        self.aptos_address_to_node_name: Dict[str, str] = dict()
        self.node_to_account: Dict[str, Dict] = dict()
        self.node_to_validator_join_params: Dict[str, ValidatorJoinParams] = dict()
        self.should_stop = False
        signal.signal(signal.SIGTERM, self.signal_handler)
        signal.signal(signal.SIGINT, self.signal_handler)

    async def __aenter__(self):
        await self.run_helper.client.__aenter__()
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        await self.run_helper.client.__aexit__(exc_type, exc_val, exc_tb)

    def signal_handler(self, signum, frame):
        self.should_stop = True
        LOG.info(f"收到信号 {signal.Signals(signum).name}，准备停止...")

    def deploy_nodes(self):
        deploy_results = {}

        all_node_names = self.genesis_node_names + self.candidate_node_names
        for node_name in all_node_names:
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

        all_node_names = self.genesis_node_names + self.candidate_node_names
        for node_name in all_node_names:
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
        all_node_names = self.genesis_node_names + self.candidate_node_names
        for node_name in all_node_names:
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

    async def fund_nodes(self):
        faucet_address = self.run_helper.faucet_address()
        faucet_nonce = await self.run_helper.client.get_transaction_count(
            faucet_address
        )
        accounts = await asyncio.gather(
            *[
                self.run_helper.create_test_account(
                    node_name, fund_wei=10**24, nonce=faucet_nonce + i
                )
                for i, node_name in enumerate(self.candidate_node_names)
            ]
        )
        for node_name, account in zip(self.candidate_node_names, accounts):
            self.node_to_account[node_name] = account

    def load_consensus_public_key(self, node_name: str):
        consensus_public_key_path = (
            self.config_base_dir / node_name / "config" / "consensus_public_key"
        )
        with open(consensus_public_key_path, "r") as f:
            consensus_public_key = f.read().strip()
        return consensus_public_key

    async def validator_join(self, node_name: str):
        params = self.node_to_validator_join_params[node_name]
        await execute_validator_join(
            gravity_cli_path=self.node_manager.gravity_cli_path,
            rpc_url=self.run_helper.client.rpc_url,
            params=params,
            start_new_session=True,
        )
        LOG.info(f"✅ Validator {node_name} join command executed successfully")

    async def validator_leave(self, node_name: str):
        params = self.node_to_validator_join_params[node_name]
        await execute_validator_leave(
            gravity_cli_path=self.node_manager.gravity_cli_path,
            rpc_url=self.run_helper.client.rpc_url,
            params=params,
            start_new_session=True,
        )
        LOG.info(f"✅ Validator {node_name} leave command executed successfully")

    async def validator_list(self):
        """
        Get validator list from gravity node
        Returns:
            active_node_names: set of node names in active validator set
            pending_inactive_node_names: set of node names in pending inactive validator set
            pending_active_node_names: set of node names in pending active validator set
        """
        result = await execute_validator_list(
            gravity_cli_path=self.node_manager.gravity_cli_path,
            rpc_url=self.run_helper.client.rpc_url,
            start_new_session=True,
        )

        # Map aptos addresses to node names
        active_node_names = set()
        pending_inactive_node_names = set()
        pending_active_node_names = set()
        
        for aptos_address in result.get_active_aptos_addresses():
            active_node_names.add(self.aptos_address_to_node_name[aptos_address])
        
        for aptos_address in result.get_pending_inactive_aptos_addresses():
            pending_inactive_node_names.add(self.aptos_address_to_node_name[aptos_address])
        
        for aptos_address in result.get_pending_active_aptos_addresses():
            pending_active_node_names.add(self.aptos_address_to_node_name[aptos_address])

        return active_node_names, pending_inactive_node_names, pending_active_node_names

    def init_node_to_identity(self):
        validator_node_names = self.genesis_node_names + self.candidate_node_names
        for node_name in validator_node_names:
            identity_path = (
                self.config_base_dir / node_name / "config" / "validator-identity.yaml"
            )
            identity = parse_identity_from_yaml(identity_path)
            self.node_to_identity[node_name] = identity
            self.aptos_address_to_node_name[identity.account_address] = node_name
            LOG.info(f"Loaded identity for {node_name}: {identity}")

    def init_validator_join_params(self):
        # FIXME: hardcoded port offsets
        validator_network_address_port_offset = 2025
        fullnode_network_address_port_offset = 2125
        for i, node_name in enumerate(self.candidate_node_names):
            validator_network_address_port = validator_network_address_port_offset + i
            fullnode_network_address_port = fullnode_network_address_port_offset + i
            consensus_public_key = self.load_consensus_public_key(node_name)
            identity = self.node_to_identity[node_name]
            account = self.node_to_account[node_name]
            params = ValidatorJoinParams(
                private_key=account["private_key"],
                validator_address=account["address"],
                consensus_public_key=consensus_public_key,
                validator_network_address=f"/ip4/127.0.0.1/tcp/{validator_network_address_port}/noise-ik/{identity.account_address}/handshake/0",
                fullnode_network_address=f"/ip4/127.0.0.1/tcp/{fullnode_network_address_port}/noise-ik/{identity.account_address}/handshake/0",
                aptos_address=identity.account_address,
                moniker=node_name.upper(),
            )
            LOG.info(f"Initialized validator join params for {node_name}: {params}")
            self.node_to_validator_join_params[node_name] = params

    async def fuzzy_validator_join_and_leave(self):
        """
        模糊测试：持续随机地让节点加入和离开 validator set
        """
        # validator set not include genesis validators
        validator_set: Set[str] = set(self.genesis_node_names)
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
            try:
                while not self.should_stop:
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
                    elif new_epoch > current_epoch:
                        LOG.info(f"Epoch 从 {current_epoch} 切换到 {new_epoch}")
                        current_epoch = new_epoch

                    # 将成功 join 的节点加入 validator_set
                    validator_set.update(pending_joins)
                    if pending_joins:
                        LOG.info(f"节点 {pending_joins} 进入 validator_set")

                    # 将成功 leave 的节点从 validator_set 移除
                    validator_set.difference_update(pending_leaves)
                    if pending_leaves:
                        LOG.info(f"节点 {pending_leaves} 退出 validator_set")

                    actual_active_nodes, _, _ = await self.validator_list()
                    if actual_active_nodes != validator_set:
                        raise RuntimeError(
                            f"Actual active nodes: {actual_active_nodes} != expected active nodes: {validator_set}"
                        )

                    # 重置待处理的 join 和 leave
                    pending_joins.clear()
                    pending_leaves.clear()

                    # 更新当前 epoch
                    LOG.info(f"当前 validator_set: {validator_set}")

                    # 在每个 epoch 期间执行随机 join 和 leave
                    # 随机选择 1-3 个候选节点调用 validator join
                    nodes_not_in_validator = [
                        node
                        for node in self.candidate_node_names
                        if node not in validator_set
                    ]

                    if nodes_not_in_validator:
                        # 随机选择 1-3 个节点
                        if len(nodes_not_in_validator) > 1:
                            num_joins = random.randint(
                                1, min(3, len(nodes_not_in_validator))
                            )
                            nodes_to_join = random.sample(
                                nodes_not_in_validator, num_joins
                            )
                        else:
                            nodes_to_join = nodes_not_in_validator

                        for node_name in nodes_to_join:
                            LOG.info(f"尝试让节点 {node_name} join validator set...")
                            await self.validator_join(node_name)
                            pending_joins.add(node_name)
                            LOG.info(
                                f"✅ 节点 {node_name} join 成功，将在下一个 epoch 进入 validator_set"
                            )
                    # 随机选择 1-3 个候选节点调用 validator leave
                    candidate_nodes_in_validator_set = [
                        node
                        for node in validator_set
                        if node not in self.genesis_node_names
                    ]
                    if candidate_nodes_in_validator_set:
                        # 随机选择 1-3 个节点
                        if len(candidate_nodes_in_validator_set) > 1:
                            num_leaves = random.randint(
                                1, min(3, len(candidate_nodes_in_validator_set))
                            )
                            nodes_to_leave = random.sample(
                                candidate_nodes_in_validator_set, num_leaves
                            )
                        else:
                            nodes_to_leave = candidate_nodes_in_validator_set

                        for node_name in nodes_to_leave:
                            LOG.info(f"尝试让节点 {node_name} leave validator set...")
                            await self.validator_leave(node_name)
                            pending_leaves.add(node_name)
                            LOG.info(
                                f"✅ 节点 {node_name} leave 成功，将在下一个 epoch 退出 validator_set"
                            )

                    _, pending_inactive_nodes, pending_active_nodes = (
                        await self.validator_list()
                    )
                    if pending_inactive_nodes != pending_leaves:
                        raise RuntimeError(
                            f"Actual pending inactive nodes: {pending_inactive_nodes} != expected pending inactive nodes: {pending_leaves}"
                        )
                    if pending_active_nodes != pending_joins:
                        raise RuntimeError(
                            f"Actual pending active nodes: {pending_active_nodes} != expected pending active nodes: {pending_joins}"
                        )
            except Exception as e:
                raise RuntimeError(f"Failed to fuzzy validator join and leave: {e}")

    async def check_node_block_height(self):
        clients = []
        for i, node_name in enumerate(
            self.genesis_node_names + self.candidate_node_names
        ):
            # FIXME: hardcoded port
            http_url = f"http://127.0.0.1:{8541 + i}"
            client = GravityClient(rpc_url=http_url, node_id=node_name)
            clients.append(client)
        async with AsyncExitStack() as stack:
            await asyncio.gather(*[stack.enter_async_context(c) for c in clients])
            try:
                while not self.should_stop:
                    await asyncio.sleep(10)
                    all_node_names = self.genesis_node_names + self.candidate_node_names

                    async def get_and_log_block_number(client, node_name):
                        block_height = await client.get_block_number()
                        LOG.info(f"{node_name} block height: {block_height}")
                        return block_height

                    block_heights = await asyncio.gather(
                        *[
                            get_and_log_block_number(client, node_name)
                            for client, node_name in zip(clients, all_node_names)
                        ]
                    )
                    max_block_height = max(block_heights)
                    LOG.info(f"Max block height: {max_block_height}")
                    is_gap_too_large = False
                    for node_name, block_height in zip(all_node_names, block_heights):
                        if block_height + 100 < max_block_height:
                            LOG.warning(
                                f"{node_name} block height is too low: {block_height}"
                            )
                            is_gap_too_large = True
                    if is_gap_too_large:
                        raise RuntimeError(
                            f"Gap between node block heights is too large"
                        )
            except Exception as e:
                raise RuntimeError(f"Failed to check node block height: {e}")


@test_case
async def test_epoch_switch(run_helper: RunHelper, test_result: TestResult):
    """
    Test epoch switching with multiple nodes

    Test steps:
    1. Initialize node to identity for all validator nodes(genesis and candidate)
    2. Deploy all nodes
    3. Start all nodes
    4. Wait for nodes to be ready
    5. Create EVM accounts for candidate nodes
    6. Initialize validator join args for all candidate nodes
    7. Fuzzy validator candidate nodes join and leave
    8. Check node block height gap between all nodes
    """
    LOG.info("=" * 70)
    LOG.info("Test: Epoch Switch Test")
    LOG.info("=" * 70)

    test_context = TestContext(run_helper)
    await test_context.__aenter__()
    try:
        # Step 1: Initialize node to identity and validator join args
        LOG.info("\n[Step 1] Initializing node to identity")
        test_context.init_node_to_identity()
        LOG.info(f"✅ Node to identity initialized successfully")

        # Step 2: Deploy all nodes
        LOG.info("\n[Step 2] Deploying all nodes...")
        test_context.stop_nodes()
        test_context.deploy_nodes()
        LOG.info(f"✅ All nodes deployed successfully")

        # Step 3: Start all nodes
        LOG.info("\n[Step 3] Starting all nodes...")
        test_context.start_nodes()
        LOG.info(f"✅ All nodes started successfully")

        # Step 4: Wait for nodes to be ready
        LOG.info("\n[Step 4] Waiting 10 seconds for nodes to be ready...")
        await asyncio.sleep(10)

        LOG.info("\n✅ All nodes deployed and started successfully!")
        LOG.info("=" * 70)

        # Step 5: Create EVM accounts for candidate nodes
        LOG.info("\n[Step 5] Funding EVM accounts for candidate nodes...")
        await test_context.fund_nodes()
        LOG.info(f"✅ All candidate nodes EVM accounts created successfully")

        # Step 6: Initialize validator join params for all candidate nodes
        LOG.info(
            "\n[Step 6] Initializing validator join params for all candidate nodes..."
        )
        test_context.init_validator_join_params()
        LOG.info(f"✅ Validator join params initialized successfully")

        tasks = []
        # Step 7: Fuzzy validator candidate nodes join and leave
        LOG.info("\n[Step 7] Fuzzy validator candidate nodes join and leave...")
        tasks.append(asyncio.create_task(test_context.fuzzy_validator_join_and_leave()))
        # Step 8: Check node block height gap between all nodes
        LOG.info("\n[Step 8] Checking node block height gap between all nodes...")
        tasks.append(asyncio.create_task(test_context.check_node_block_height()))
        await asyncio.gather(*tasks)

        test_result.mark_success()

    except Exception as e:
        LOG.error(f"❌ Test failed: {e}")
        test_result.mark_failure(str(e))
        raise
    finally:
        # Cleanup: Stop nodes
        LOG.info("\n[Cleanup] Stopping nodes...")
        test_context.stop_nodes()
        await test_context.__aexit__(None, None, None)
