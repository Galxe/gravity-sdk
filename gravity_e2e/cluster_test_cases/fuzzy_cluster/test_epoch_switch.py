"""
Epoch switch test
Tests epoch switching with multiple nodes (node1-node10)
"""

import asyncio
import logging
import random
import signal
import time

import pytest
from contextlib import AsyncExitStack
from pathlib import Path
from typing import Set

from eth_abi import encode
from eth_account import Account
from web3 import Web3
from gravity_e2e.tests.test_registry import register_test
from gravity_e2e.cluster.manager import Cluster
from gravity_e2e.cluster.node import NodeRole
from gravity_e2e.core.client.gravity_http_client import GravityHttpClient

LOG = logging.getLogger(__name__)

# Fuzzy test duration in seconds.
# Default: 1800s (30 minutes).
# Set to 0 to run indefinitely until signal is received.
FUZZY_TEST_DURATION = 1800

# ── Permissionless-join governance setup ────────────────────────────
# registerValidator/joinValidatorSet are gated on a governance-managed pool
# whitelist unless permissionless join is enabled (defaults to off at
# genesis). The fuzz creates pools dynamically, so flip the flag once via
# the full proposal lifecycle. Genesis pool[0]'s voter is the faucet (see
# genesis.toml) and provides the proposer stake / voting power.
GOVERNANCE = Web3.to_checksum_address("0x00000000000000000000000000000001625F3000")
STAKING = Web3.to_checksum_address("0x00000000000000000000000000000001625F2000")
VALIDATOR_MANAGER = Web3.to_checksum_address("0x00000000000000000000000000000001625F2001")

FAUCET_KEY = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
FAUCET_ADDR = Web3.to_checksum_address("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266")

MAX_UINT128 = (1 << 128) - 1
VOTING_DURATION_SECS = 5  # matches genesis.toml voting_duration_micros (5e6)
PROPOSAL_STATE_SUCCEEDED = 1


def _selector(sig: str) -> bytes:
    return Web3.keccak(text=sig)[:4]


SEL_ADD_EXECUTOR = _selector("addExecutor(address)")
SEL_CREATE_PROPOSAL = _selector("createProposal(address,address[],bytes[],string)")
SEL_VOTE = _selector("vote(address,uint64,uint128,bool)")
SEL_RESOLVE = _selector("resolve(uint64)")
SEL_EXECUTE = _selector("execute(uint64,address[],bytes[])")
SEL_GET_PROPOSAL_STATE = _selector("getProposalState(uint64)")
SEL_GET_POOL = _selector("getPool(uint256)")
SEL_SET_PERMISSIONLESS = _selector("setPermissionlessJoinEnabled(bool)")
SEL_IS_PERMISSIONLESS = _selector("isPermissionlessJoinEnabled()")


def _send_tx(w3: Web3, to: str, data: bytes, sender_key: str, gas: int = 1_000_000) -> dict:
    sender = Account.from_key(sender_key)
    tx = {
        "to": to,
        "data": data,
        "gas": gas,
        "gasPrice": w3.eth.gas_price,
        "nonce": w3.eth.get_transaction_count(sender.address),
        "chainId": w3.eth.chain_id,
        "value": 0,
    }
    signed = sender.sign_transaction(tx)
    tx_hash = w3.eth.send_raw_transaction(signed.raw_transaction)
    return w3.eth.wait_for_transaction_receipt(tx_hash, timeout=60)


async def enable_permissionless_join(w3: Web3):
    raw = w3.eth.call({"to": VALIDATOR_MANAGER, "data": SEL_IS_PERMISSIONLESS})
    if int.from_bytes(raw[-32:], "big"):
        LOG.info("Permissionless join already enabled")
        return

    pool0_raw = w3.eth.call(
        {"to": STAKING, "data": SEL_GET_POOL + encode(["uint256"], [0])}
    )
    pool0 = Web3.to_checksum_address("0x" + pool0_raw[-20:].hex())

    receipt = _send_tx(
        w3, GOVERNANCE, SEL_ADD_EXECUTOR + encode(["address"], [FAUCET_ADDR]), FAUCET_KEY
    )
    assert receipt["status"] == 1, f"addExecutor failed: {receipt}"

    enable_call = SEL_SET_PERMISSIONLESS + encode(["bool"], [True])
    create_data = SEL_CREATE_PROPOSAL + encode(
        ["address", "address[]", "bytes[]", "string"],
        [pool0, [VALIDATOR_MANAGER], [enable_call], "fuzzy-permissionless-join"],
    )
    receipt = _send_tx(w3, GOVERNANCE, create_data, FAUCET_KEY)
    assert receipt["status"] == 1, f"createProposal failed: {receipt}"

    proposal_created_topic = Web3.keccak(
        text="ProposalCreated(uint64,address,address,bytes32,string)"
    )
    proposal_id = None
    for log in receipt["logs"]:
        if log["topics"] and bytes(log["topics"][0]) == bytes(proposal_created_topic):
            proposal_id = int.from_bytes(log["topics"][1], "big")
            break
    assert proposal_id is not None, "ProposalCreated event not found"
    LOG.info(f"Created governance proposal {proposal_id} to enable permissionless join")

    vote_data = SEL_VOTE + encode(
        ["address", "uint64", "uint128", "bool"], [pool0, proposal_id, MAX_UINT128, True]
    )
    receipt = _send_tx(w3, GOVERNANCE, vote_data, FAUCET_KEY)
    assert receipt["status"] == 1, f"vote failed: {receipt}"

    await asyncio.sleep(VOTING_DURATION_SECS + 2)

    receipt = _send_tx(
        w3, GOVERNANCE, SEL_RESOLVE + encode(["uint64"], [proposal_id]), FAUCET_KEY
    )
    assert receipt["status"] == 1, f"resolve failed: {receipt}"
    state_raw = w3.eth.call(
        {"to": GOVERNANCE, "data": SEL_GET_PROPOSAL_STATE + encode(["uint64"], [proposal_id])}
    )
    state = int.from_bytes(state_raw[-1:], "big")
    assert state == PROPOSAL_STATE_SUCCEEDED, f"proposal not SUCCEEDED: state={state}"

    exec_data = SEL_EXECUTE + encode(
        ["uint64", "address[]", "bytes[]"],
        [proposal_id, [VALIDATOR_MANAGER], [enable_call]],
    )
    receipt = _send_tx(w3, GOVERNANCE, exec_data, FAUCET_KEY)
    assert receipt["status"] == 1, f"execute failed: {receipt}"

    raw = w3.eth.call({"to": VALIDATOR_MANAGER, "data": SEL_IS_PERMISSIONLESS})
    assert int.from_bytes(raw[-32:], "big"), "permissionless join not enabled after execute"
    LOG.info("Permissionless validator join enabled via governance")


class EpochSwitchTestContext:
    """
    Context for epoch switch test, using the declarative Cluster API.

    Args:
        cluster: The Cluster instance to test against.
        duration: Test duration in seconds. Default is 1800s (30 minutes).
                  Set to 0 to run indefinitely until signal is received.
    """

    def __init__(self, cluster: Cluster, duration: int = 1800):
        self.cluster = cluster
        self.duration = duration
        self.start_time = time.monotonic()

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

        self._signal_received = False
        signal.signal(signal.SIGTERM, self.signal_handler)
        signal.signal(signal.SIGINT, self.signal_handler)

    def signal_handler(self, signum, frame):
        self._signal_received = True
        LOG.info(f"Received signal {signal.Signals(signum).name}, stopping...")

    @property
    def elapsed_time(self) -> float:
        """Get elapsed time in seconds since test started."""
        return time.monotonic() - self.start_time

    @property
    def should_stop(self) -> bool:
        """
        Check if the test should stop.

        Returns True if:
        - A signal was received (SIGTERM or SIGINT)
        - Duration > 0 and elapsed time exceeds duration
        """
        if self._signal_received:
            return True
        if self.duration > 0 and self.elapsed_time >= self.duration:
            LOG.info(f"Test duration ({self.duration}s) reached, stopping...")
            return True
        return False

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
        Fuzzy test: Continuously and randomly make nodes join and leave the validator set
        """
        validator_set, pending_joins, pending_leaves = await self.validator_list()

        # Get HTTP port from first node
        first_node = list(self.cluster.nodes.values())[0]
        # FIXME: derive http_port from config if available
        # Initialize HTTP client
        http_client = GravityHttpClient(
            self.cluster.get_node(self.genesis_node_names[0]).http_url
        )
        async with http_client:
            # Get initial epoch
            try:
                current_epoch = await http_client.get_current_epoch()
                LOG.info(f"Initial epoch: {current_epoch}")
            except Exception as e:
                LOG.error(f"❌ Failed to get initial epoch: {e}")
                # If unable to get initial epoch, raise error
                raise RuntimeError(f"Failed to get epoch: {e}")

            # Main loop: check stop signal
            try:
                while not self.should_stop:
                    # Check for epoch switch every 10 seconds
                    await asyncio.sleep(10)

                    # Check if epoch switched
                    try:
                        new_epoch = await http_client.get_current_epoch()
                    except Exception as e:
                        LOG.warning(
                            f"⚠️ Failed to get current epoch: {e}, skipping this check"
                        )
                        raise RuntimeError(f"Failed to get epoch: {e}")

                    if new_epoch == current_epoch:
                        continue

                    if new_epoch < current_epoch:
                        raise RuntimeError(
                            f"Epoch decreased from {current_epoch} to {new_epoch}"
                        )
                    elif new_epoch > current_epoch:
                        LOG.info(f"Epoch switched from {current_epoch} to {new_epoch}")
                        current_epoch = new_epoch

                    # Add successfully joined nodes to validator_set
                    validator_set.update(pending_joins)
                    if pending_joins:
                        LOG.info(f"Nodes {pending_joins} entered validator_set")

                    # Remove successfully left nodes from validator_set
                    validator_set.difference_update(pending_leaves)
                    if pending_leaves:
                        LOG.info(f"Nodes {pending_leaves} exited validator_set")

                    actual_active_nodes, _, _ = await self.validator_list()
                    if actual_active_nodes != validator_set:
                        raise RuntimeError(
                            f"Actual active nodes: {actual_active_nodes} != expected active nodes: {validator_set}"
                        )

                    # Reset pending joins and leaves
                    pending_joins.clear()
                    pending_leaves.clear()

                    # Show current validator_set
                    LOG.info(f"Current validator_set: {validator_set}")

                    # Perform random join and leave during each epoch
                    # Randomly select 1-3 candidate nodes to call validator join
                    nodes_not_in_validator = [
                        node
                        for node in self.candidate_node_names
                        if node not in validator_set
                    ]

                    if nodes_not_in_validator:
                        # Randomly select 1-3 nodes
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
                            LOG.info(
                                f"Attempting to let node {node_name} join validator set..."
                            )
                            await self.validator_join(node_name)
                            pending_joins.add(node_name)
                            LOG.info(
                                f"✅ Node {node_name} joined successfully, will enter validator_set in the next epoch"
                            )
                    # Randomly select 1-3 candidate nodes to call validator leave
                    candidate_nodes_in_validator_set = [
                        node
                        for node in validator_set
                        if node not in self.genesis_node_names
                    ]
                    if candidate_nodes_in_validator_set:
                        # Randomly select 1-3 nodes
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
                            LOG.info(
                                f"Attempting to let node {node_name} leave validator set..."
                            )
                            await self.validator_leave(node_name)
                            pending_leaves.add(node_name)
                            LOG.info(
                                f"✅ Node {node_name} left successfully, will exit validator_set in the next epoch"
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

        try:
            while not self.should_stop:
                await asyncio.sleep(10)

                async def get_and_log_block_number(node_name):
                    node = self.cluster.get_node(node_name)
                    # Run sync web3 call in a thread to avoid blocking the loop
                    block_height = await asyncio.to_thread(
                        lambda: node.w3.eth.block_number
                    )
                    LOG.info(f"{node_name} block height: {block_height}")
                    return block_height

                block_heights = await asyncio.gather(
                    *[
                        get_and_log_block_number(node_name)
                        for node_name in all_node_names
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
                    raise RuntimeError(f"Gap between node block heights is too large")
        except Exception as e:
            raise RuntimeError(f"Failed to check node block height: {e}")


@pytest.mark.asyncio
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
    if FUZZY_TEST_DURATION > 0:
        LOG.info(
            f"Test duration: {FUZZY_TEST_DURATION}s ({FUZZY_TEST_DURATION / 60:.1f} minutes)"
        )
    else:
        LOG.info("Test duration: indefinite (until signal received)")
    LOG.info("=" * 70)

    test_context = EpochSwitchTestContext(cluster, duration=FUZZY_TEST_DURATION)
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

        # Step 2.5: Enable permissionless join so dynamically created pools
        # can register without a per-pool governance whitelist entry
        LOG.info("\n[Step 2.5] Enabling permissionless validator join...")
        await enable_permissionless_join(test_context.web3)

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
