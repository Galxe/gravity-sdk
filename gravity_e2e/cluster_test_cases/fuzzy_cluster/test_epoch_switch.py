"""
Epoch switch test
Tests epoch switching with multiple nodes (node1-node10)
"""

import asyncio
import logging
import random
import signal
from contextlib import AsyncExitStack
from pathlib import Path
from typing import Set

from web3 import Web3
from gravity_e2e.tests.test_registry import register_test
from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.cluster.node import NodeRole
from gravity_e2e.core.client.gravity_http_client import GravityHttpClient

LOG = logging.getLogger(__name__)


class EpochSwitchTestContext:
    """
    Context for epoch switch test, using the declarative Cluster API.
    """

    def __init__(self, cluster: Cluster):
        self.cluster = cluster

        self.genesis_node_names = []
        self.candidate_node_names = []
        for node_name, node in cluster.nodes.items():
            if node.role == NodeRole.GENESIS:
                self.genesis_node_names.append(node_name)
            elif node.role == NodeRole.VALIDATOR:
                self.candidate_node_names.append(node_name)

        first_genesis = self.genesis_node_names[0]
        self.rpc_url = self.cluster.nodes[first_genesis].url
        self.web3 = Web3(Web3.HTTPProvider(self.rpc_url))

        self.should_stop = False
        signal.signal(signal.SIGTERM, self.signal_handler)
        signal.signal(signal.SIGINT, self.signal_handler)

    def signal_handler(self, signum, frame):
        self.should_stop = True
        LOG.info(f"收到信号 {signal.Signals(signum).name}，准备停止...")

    async def validator_join(self, node_name: str):
        """Join a node to the validator set using Cluster API."""
        await self.cluster.validator_join(
            node_id=node_name,
            moniker=node_name.upper(),
        )
        LOG.info(f"✅ Validator {node_name} join command executed successfully")

    async def validator_leave(self, node_name: str):
        """Remove a node from the validator set using Cluster API."""
        await self.cluster.validator_leave(node_id=node_name)
        LOG.info(f"✅ Validator {node_name} leave command executed successfully")

    async def validator_list(self):
        """
        Get validator list from gravity node using Cluster API.
        Returns:
            active_node_names: set of node names in active validator set
            pending_inactive_node_names: set of node names in pending inactive validator set
            pending_active_node_names: set of node names in pending active validator set
        """
        validator_set = await self.cluster.validator_list()

        active_node_names = {n.id for n in validator_set.active}
        pending_inactive_node_names = {n.id for n in validator_set.pending_inactive}
        pending_active_node_names = {n.id for n in validator_set.pending_active}

        return active_node_names, pending_inactive_node_names, pending_active_node_names

    async def fuzzy_validator_join_and_leave(self):
        """
        模糊测试：持续随机地让节点加入和离开 validator set
        """
        validator_set, pending_joins, pending_leaves = await self.validator_list()

        # Get HTTP port from first node
        first_node = list(self.cluster.nodes.values())[0]
        # FIXME: derive http_port from config if available
        # 初始化 HTTP 客户端
        http_client = GravityHttpClient(
            self.cluster.get_node(self.genesis_node_names[0]).http_url
        )
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
        all_node_names = self.genesis_node_names + self.candidate_node_names
        clients = []
        for node_name in all_node_names:
            node = self.cluster.get_node(node_name)
            client = GravityClient(rpc_url=node.url, node_id=node_name)
            clients.append(client)

        async with AsyncExitStack() as stack:
            await asyncio.gather(*[stack.enter_async_context(c) for c in clients])
            try:
                while not self.should_stop:
                    await asyncio.sleep(10)

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


@register_test("epoch_switch", suite="epoch_switch", self_managed=True)
async def test_epoch_switch(cluster: Cluster):
    """
    Test epoch switching with multiple nodes using declarative Cluster API.

    Test steps:
    1. Ensure all nodes are running (set_full_live)
    2. Create EVM accounts for candidate nodes
    3. Fuzzy validator candidate nodes join and leave (lazy init validator join params)
    4. Check node block height gap between all nodes
    """
    LOG.info("=" * 70)
    LOG.info("Test: Epoch Switch Test (Declarative API)")
    LOG.info("=" * 70)

    test_context = EpochSwitchTestContext(cluster)
    try:
        # Step 1: Ensure all nodes are running using declarative API
        LOG.info("\n[Step 1] Ensuring all nodes are running (set_full_live)...")
        assert await cluster.set_full_live(
            timeout=120
        ), "Failed to bring all nodes to RUNNING"

        # Log current state
        live_nodes = await cluster.get_live_nodes()
        LOG.info(
            f"✅ All {len(live_nodes)} nodes are RUNNING: {[n.id for n in live_nodes]}"
        )

        # Step 2: Log candidate nodes info
        LOG.info("\n[Step 2] Candidate nodes for fuzzy testing:")
        for node_name in test_context.candidate_node_names:
            node = cluster.get_node(node_name)
            LOG.info(f"  {node_name}: role={node.role.value}")
        LOG.info(f"✅ {len(test_context.candidate_node_names)} candidate nodes ready")

        tasks = []
        # Step 3: Fuzzy validator candidate nodes join and leave
        LOG.info("\n[Step 3] Fuzzy validator candidate nodes join and leave...")
        tasks.append(asyncio.create_task(test_context.fuzzy_validator_join_and_leave()))
        # Step 4: Check node block height gap between all nodes
        LOG.info("\n[Step 4] Checking node block height gap between all nodes...")
        tasks.append(asyncio.create_task(test_context.check_node_block_height()))
        await asyncio.gather(*tasks)

        LOG.info("✅ Epoch switch test completed successfully")

    except Exception as e:
        LOG.error(f"❌ Test failed: {e}")
        raise
