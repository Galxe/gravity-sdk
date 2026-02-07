"""
Bridge Utilities for Gravity E2E Tests

Provides web3.py-based interaction with bridge contracts deployed on Anvil,
and polling utilities for NativeMinted events on the Gravity chain.
"""

import asyncio
import json
import logging
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, List, Optional

from web3 import Web3

LOG = logging.getLogger(__name__)

# ============================================================================
# Constants
# ============================================================================

# NativeMinted(address indexed recipient, uint256 amount, uint128 indexed nonce)
NATIVE_MINTED_TOPIC0 = Web3.keccak(
    text="NativeMinted(address,uint256,uint128)"
).hex()

# GBridgeReceiver deterministic address on gravity chain (deployed in genesis)
GBRIDGE_RECEIVER_ADDRESS = "0x595475934ed7d9faa7fca28341c2ce583904a44e"

# Minimal ABIs — only what we need for interactions
MOCK_GTOKEN_ABI = [
    {
        "type": "function",
        "name": "mint",
        "inputs": [
            {"name": "to", "type": "address"},
            {"name": "amount", "type": "uint256"},
        ],
        "outputs": [],
        "stateMutability": "nonpayable",
    },
    {
        "type": "function",
        "name": "approve",
        "inputs": [
            {"name": "spender", "type": "address"},
            {"name": "amount", "type": "uint256"},
        ],
        "outputs": [{"name": "", "type": "bool"}],
        "stateMutability": "nonpayable",
    },
    {
        "type": "function",
        "name": "balanceOf",
        "inputs": [{"name": "account", "type": "address"}],
        "outputs": [{"name": "", "type": "uint256"}],
        "stateMutability": "view",
    },
]

GBRIDGE_SENDER_ABI = [
    {
        "type": "function",
        "name": "bridgeToGravity",
        "inputs": [
            {"name": "amount", "type": "uint256"},
            {"name": "recipient", "type": "address"},
        ],
        "outputs": [{"name": "messageNonce", "type": "uint128"}],
        "stateMutability": "payable",
    },
    {
        "type": "function",
        "name": "calculateBridgeFee",
        "inputs": [
            {"name": "amount", "type": "uint256"},
            {"name": "recipient", "type": "address"},
        ],
        "outputs": [{"name": "requiredFee", "type": "uint256"}],
        "stateMutability": "view",
    },
]

GRAVITY_PORTAL_ABI = [
    {
        "type": "event",
        "name": "MessageSent",
        "anonymous": False,
        "inputs": [
            {"name": "nonce", "type": "uint128", "indexed": True},
            {"name": "block_number", "type": "uint256", "indexed": True},
            {"name": "payload", "type": "bytes", "indexed": False},
        ],
    },
    {
        "type": "function",
        "name": "nonce",
        "inputs": [],
        "outputs": [{"name": "", "type": "uint128"}],
        "stateMutability": "view",
    },
]

BRIDGE_RECEIVER_ABI = [
    {
        "type": "event",
        "name": "NativeMinted",
        "anonymous": False,
        "inputs": [
            {"name": "recipient", "type": "address", "indexed": True},
            {"name": "amount", "type": "uint256", "indexed": False},
            {"name": "nonce", "type": "uint128", "indexed": True},
        ],
    },
    {
        "type": "function",
        "name": "isProcessed",
        "inputs": [{"name": "nonce", "type": "uint128"}],
        "outputs": [{"name": "", "type": "bool"}],
        "stateMutability": "view",
    },
]


# ============================================================================
# Bridge Helper
# ============================================================================


class BridgeHelper:
    """
    Interacts with bridge contracts on Anvil using web3.py.

    Handles: mint GToken → approve → bridgeToGravity → query events.
    """

    def __init__(
        self,
        anvil_rpc_url: str,
        gtoken_address: str,
        portal_address: str,
        sender_address: str,
        deployer_private_key: str,
        deployer_address: str,
    ):
        self.w3 = Web3(Web3.HTTPProvider(anvil_rpc_url))
        assert self.w3.is_connected(), f"Cannot connect to Anvil at {anvil_rpc_url}"

        self.deployer_key = deployer_private_key
        self.deployer_address = Web3.to_checksum_address(deployer_address)

        self.gtoken = self.w3.eth.contract(
            address=Web3.to_checksum_address(gtoken_address),
            abi=MOCK_GTOKEN_ABI,
        )
        self.portal = self.w3.eth.contract(
            address=Web3.to_checksum_address(portal_address),
            abi=GRAVITY_PORTAL_ABI,
        )
        self.sender = self.w3.eth.contract(
            address=Web3.to_checksum_address(sender_address),
            abi=GBRIDGE_SENDER_ABI,
        )

    def mint_and_bridge(self, amount: int, recipient: str) -> int:
        """
        Execute a full bridge flow on Anvil:
        1. Mint GToken to deployer
        2. Approve GBridgeSender to spend
        3. Call bridgeToGravity
        
        Args:
            amount: Amount of GToken to bridge (in wei).
            recipient: Recipient address on gravity chain.
            
        Returns:
            The bridge nonce from GravityPortal.
        """
        recipient = Web3.to_checksum_address(recipient)

        # 1. Mint GToken
        LOG.info(f"  Minting {amount} GTokens...")
        tx = self.gtoken.functions.mint(
            self.deployer_address, amount
        ).build_transaction(self._tx_params())
        self._send_tx(tx)

        # 2. Approve GBridgeSender
        LOG.info(f"  Approving GBridgeSender...")
        tx = self.gtoken.functions.approve(
            self.sender.address, amount
        ).build_transaction(self._tx_params())
        self._send_tx(tx)

        # 3. Get required fee
        fee = self.sender.functions.calculateBridgeFee(amount, recipient).call()
        LOG.info(f"  Required fee: {fee} wei")

        # 4. Bridge
        LOG.info(f"  Calling bridgeToGravity(amount={amount}, recipient={recipient})...")
        tx = self.sender.functions.bridgeToGravity(
            amount, recipient
        ).build_transaction(
            {**self._tx_params(), "value": fee}
        )
        receipt = self._send_tx(tx)

        # 5. Get nonce from portal
        portal_nonce = self.portal.functions.nonce().call()
        LOG.info(f"  Bridge complete. Portal nonce: {portal_nonce}")
        return portal_nonce

    def query_message_sent_events(self, from_block: int = 0) -> list:
        """Query MessageSent events from GravityPortal on Anvil."""
        logs = self.portal.events.MessageSent().get_logs(from_block=from_block)
        return logs

    def _tx_params(self) -> dict:
        """Common transaction parameters."""
        return {
            "from": self.deployer_address,
            "nonce": self.w3.eth.get_transaction_count(self.deployer_address),
            "gas": 500_000,
            "gasPrice": self.w3.eth.gas_price,
            "chainId": self.w3.eth.chain_id,
        }

    def _send_tx(self, tx: dict) -> dict:
        """Sign and send a transaction, wait for receipt."""
        signed = self.w3.eth.account.sign_transaction(tx, self.deployer_key)
        tx_hash = self.w3.eth.send_raw_transaction(signed.raw_transaction)
        receipt = self.w3.eth.wait_for_transaction_receipt(tx_hash, timeout=30)
        if receipt.status != 1:
            raise RuntimeError(
                f"Transaction failed: {tx_hash.hex()}, receipt: {receipt}"
            )
        return receipt


# ============================================================================
# NativeMinted Event Polling (on Gravity Chain)
# ============================================================================


async def poll_native_minted(
    gravity_w3: Web3,
    nonce: int,
    timeout: float = 120.0,
    poll_interval: float = 2.0,
    receiver_address: str = GBRIDGE_RECEIVER_ADDRESS,
) -> Optional[Dict[str, Any]]:
    """
    Poll gravity chain for NativeMinted event matching a specific nonce.

    Uses eth_getLogs with topic filtering on the indexed nonce for
    precise matching. Scans incrementally from current block forward.

    Args:
        gravity_w3: Web3 instance connected to gravity chain.
        nonce: The bridge nonce to match (indexed in NativeMinted event).
        timeout: Maximum seconds to wait.
        poll_interval: Seconds between polls.
        receiver_address: GBridgeReceiver contract address.

    Returns:
        Dict with {recipient, amount, nonce, block_number, tx_hash}
        or None if timeout.
    """
    receiver = gravity_w3.eth.contract(
        address=Web3.to_checksum_address(receiver_address),
        abi=BRIDGE_RECEIVER_ABI,
    )

    start_block = gravity_w3.eth.block_number
    start_time = time.time()
    last_scanned_block = max(0, start_block - 1)

    # Encode nonce as bytes32 topic (uint128, left-padded to 32 bytes)
    nonce_topic = "0x" + hex(nonce)[2:].zfill(64)

    LOG.info(
        f"  Polling NativeMinted (nonce={nonce}) from block {start_block}, "
        f"timeout={timeout}s..."
    )

    while time.time() - start_time < timeout:
        current_block = gravity_w3.eth.block_number

        if current_block > last_scanned_block:
            try:
                logs = gravity_w3.eth.get_logs(
                    {
                        "address": Web3.to_checksum_address(receiver_address),
                        "fromBlock": last_scanned_block + 1,
                        "toBlock": current_block,
                        "topics": [
                            NATIVE_MINTED_TOPIC0,
                            None,  # topic[1] = recipient (any)
                            nonce_topic,  # topic[2] = nonce (exact)
                        ],
                    }
                )

                if logs:
                    log = logs[0]
                    event = receiver.events.NativeMinted().process_log(log)
                    result = {
                        "recipient": event.args.recipient,
                        "amount": event.args.amount,
                        "nonce": event.args.nonce,
                        "block_number": log["blockNumber"],
                        "tx_hash": log["transactionHash"].hex(),
                    }
                    LOG.info(
                        f"  NativeMinted found at block {result['block_number']}: "
                        f"recipient={result['recipient']}, "
                        f"amount={result['amount']}, "
                        f"nonce={result['nonce']}"
                    )
                    return result

            except Exception as e:
                LOG.debug(f"  Poll error (will retry): {e}")

            last_scanned_block = current_block

        await asyncio.sleep(poll_interval)

    # Timeout fallback: check isProcessed as last resort
    try:
        is_done = receiver.functions.isProcessed(nonce).call()
        if is_done:
            LOG.warning(
                f"  Nonce {nonce} isProcessed=True but NativeMinted event "
                f"not found in logs (block range may have been missed)"
            )
    except Exception:
        pass

    return None


# ============================================================================
# Bridge Stats
# ============================================================================


@dataclass
class BridgeStats:
    """Accumulates bridge test statistics across iterations."""

    total: int = 0
    success: int = 0
    failed: int = 0
    latencies: List[float] = field(default_factory=list)
    nonces: List[int] = field(default_factory=list)
    total_bridged: int = 0

    def record(self, nonce: int, latency: float, amount: int) -> None:
        """Record a successful bridge iteration."""
        self.total += 1
        self.success += 1
        self.latencies.append(latency)
        self.nonces.append(nonce)
        self.total_bridged += amount

    def record_failure(self) -> None:
        """Record a failed bridge iteration."""
        self.total += 1
        self.failed += 1

    def report(self) -> str:
        """Generate a summary report."""
        if not self.latencies:
            msg = "No successful bridge transactions."
            LOG.info(msg)
            return msg

        avg_lat = sum(self.latencies) / len(self.latencies)
        min_lat = min(self.latencies)
        max_lat = max(self.latencies)

        # Check nonce continuity
        nonces_sorted = sorted(self.nonces)
        is_continuous = all(
            nonces_sorted[i] + 1 == nonces_sorted[i + 1]
            for i in range(len(nonces_sorted) - 1)
        )

        report = (
            f"\n{'=' * 60}\n"
            f"  Bridge E2E Test Report\n"
            f"{'=' * 60}\n"
            f"  Total rounds:     {self.total}\n"
            f"  Successful:       {self.success}\n"
            f"  Failed:           {self.failed}\n"
            f"  Success rate:     {self.success / self.total * 100:.1f}%\n"
            f"  \n"
            f"  Latency (bridge → NativeMinted):\n"
            f"    Average:        {avg_lat:.1f}s\n"
            f"    Min:            {min_lat:.1f}s\n"
            f"    Max:            {max_lat:.1f}s\n"
            f"  \n"
            f"  Total bridged:    {self.total_bridged} wei\n"
            f"  Nonce range:      {nonces_sorted[0]} → {nonces_sorted[-1]}\n"
            f"  Nonces continuous: {'✓' if is_continuous else '✗'}\n"
            f"{'=' * 60}\n"
        )
        LOG.info(report)
        return report
