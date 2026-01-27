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
from typing import Dict, Set

from gravity_e2e.tests.test_registry import register_test
from gravity_e2e.helpers.test_helpers import RunHelper, TestResult, test_case
from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.cluster.node import NodeRole
from gravity_e2e.utils.validator_utils import (
    ValidatorJoinParams,
    execute_validator_join,
    execute_validator_leave,
    execute_validator_list,
)
from gravity_e2e.core.client.gravity_client import GravityClient
from gravity_e2e.core.client.gravity_http_client import GravityHttpClient

LOG = logging.getLogger(__name__)


class EpochSwitchTestContext:
    """
    Context for epoch switch test, using the declarative Cluster API.
    """

    def __init__(self, cluster: Cluster, run_helper: RunHelper):
        self.cluster = cluster
        self.run_helper = run_helper

        self.genesis_node_names = []
        self.candidate_node_names = []
        for node_name, node in cluster.nodes.items():
            if node.role == NodeRole.GENESIS:
                self.genesis_node_names.append(node_name)
            elif node.role == NodeRole.VALIDATOR:
                self.candidate_node_names.append(node_name)

        self.run_helper.client = GravityClient(
            rpc_url=f"{self.cluster.nodes[self.genesis_node_names[0]].url}",
            node_id=self.genesis_node_names[0],
        )

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

    async def validator_join(self, node_name: str):
        params = self.get_validator_join_params(node_name)
        await execute_validator_join(
            gravity_cli_path=self.cluster.gravity_cli_path,
            rpc_url=self.run_helper.client.rpc_url,
            params=params,
            start_new_session=True,
        )
        LOG.info(f"✅ Validator {node_name} join command executed successfully")

    async def validator_leave(self, node_name: str):
        params = self.get_validator_join_params(node_name)
        await execute_validator_leave(
            gravity_cli_path=self.cluster.gravity_cli_path,
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
        gravity_cli_path = self.cluster.cluster_root.parent / "gravity-cli"
        result = await execute_validator_list(
            gravity_cli_path=gravity_cli_path,
            rpc_url=self.run_helper.client.rpc_url,
            start_new_session=True,
        )

        # Build address to node name mapping from cluster nodes
        aptos_address_to_node_name = {}
        for node_name, node in self.cluster.nodes.items():
            aptos_address_to_node_name[node.account_address] = node_name

        # Map aptos addresses to node names
        active_node_names = set()
        pending_inactive_node_names = set()
        pending_active_node_names = set()

        for aptos_address in result.get_active_aptos_addresses():
            if aptos_address in aptos_address_to_node_name:
                active_node_names.add(aptos_address_to_node_name[aptos_address])

        for aptos_address in result.get_pending_inactive_aptos_addresses():
            if aptos_address in aptos_address_to_node_name:
                pending_inactive_node_names.add(
                    aptos_address_to_node_name[aptos_address]
                )

        for aptos_address in result.get_pending_active_aptos_addresses():
            if aptos_address in aptos_address_to_node_name:
                pending_active_node_names.add(aptos_address_to_node_name[aptos_address])

        return active_node_names, pending_inactive_node_names, pending_active_node_names

    def get_validator_join_params(self, node_name: str) -> ValidatorJoinParams:
        """
        Get validator join params for a node (lazy initialization).
        Uses node.p2p_port for validator_network_address and node.vfn_port for fullnode_network_address.
        """
        if node_name in self.node_to_validator_join_params:
            return self.node_to_validator_join_params[node_name]

        node = self.cluster.get_node(node_name)
        account = self.node_to_account[node_name]
        params = ValidatorJoinParams(
            private_key=account["private_key"],
            validator_address=account["address"],
            consensus_public_key=node.consensus_public_key,
            validator_network_address=f"/ip4/127.0.0.1/tcp/{node.p2p_port}/noise-ik/{node.identity.network_public_key}/handshake/0",
            fullnode_network_address=f"/ip4/127.0.0.1/tcp/{node.vfn_port}/noise-ik/{node.identity.network_public_key}/handshake/0",
            aptos_address=node.account_address,
            moniker=node_name.upper(),
        )
        LOG.info(f"Initialized validator join params for {node_name}: {params}")
        self.node_to_validator_join_params[node_name] = params
        return params

    async def fuzzy_validator_join_and_leave(self):
        """
        模糊测试：持续随机地让节点加入和离开 validator set
        """
        # validator set not include genesis validators
        validator_set: Set[str] = set(self.genesis_node_names)
        pending_joins: Set[str] = set()
        pending_leaves: Set[str] = set()

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
@test_case
async def test_epoch_switch(
    cluster: Cluster, run_helper: RunHelper, test_result: TestResult
):
    """
    Test epoch switching with multiple nodes using declarative Cluster API.

    Test steps:
    1. Ensure all nodes are running (set_full_live)
    2. Wait for nodes to be ready
    3. Create EVM accounts for candidate nodes
    4. Fuzzy validator candidate nodes join and leave (lazy init validator join params)
    5. Check node block height gap between all nodes
    """
    LOG.info("=" * 70)
    LOG.info("Test: Epoch Switch Test (Declarative API)")
    LOG.info("=" * 70)

    test_context = EpochSwitchTestContext(cluster, run_helper)
    await test_context.__aenter__()
    try:
        # Step 1: Ensure all nodes are running using declarative API
        LOG.info("\n[Step 1] Ensuring all nodes are running (set_full_live)...")
        if not await cluster.set_full_live(timeout=120):
            test_result.error = "Failed to bring all nodes to RUNNING"
            return

        # Log current state
        live_nodes = await cluster.get_live_nodes()
        LOG.info(
            f"✅ All {len(live_nodes)} nodes are RUNNING: {[n.id for n in live_nodes]}"
        )

        # Step 2: Wait for nodes to be ready
        LOG.info("\n[Step 2] Waiting 10 seconds for nodes to be ready...")
        await asyncio.sleep(10)
        LOG.info("\n✅ All nodes ready!")
        LOG.info("=" * 70)

        # Step 3: Create EVM accounts for candidate nodes
        LOG.info("\n[Step 3] Funding EVM accounts for candidate nodes...")
        await test_context.fund_nodes()
        LOG.info("✅ All candidate nodes EVM accounts created successfully")

        tasks = []
        # Step 4: Fuzzy validator candidate nodes join and leave
        LOG.info("\n[Step 4] Fuzzy validator candidate nodes join and leave...")
        tasks.append(asyncio.create_task(test_context.fuzzy_validator_join_and_leave()))
        # Step 5: Check node block height gap between all nodes
        LOG.info("\n[Step 5] Checking node block height gap between all nodes...")
        tasks.append(asyncio.create_task(test_context.check_node_block_height()))
        await asyncio.gather(*tasks)

        test_result.mark_success()

    except Exception as e:
        LOG.error(f"❌ Test failed: {e}")
        test_result.mark_failure(str(e))
        raise
    finally:
        await test_context.__aexit__(None, None, None)
