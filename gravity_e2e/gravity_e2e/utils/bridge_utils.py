"""
Bridge Utilities for Gravity E2E Tests

Provides web3.py-based interaction with bridge contracts deployed on Anvil,
and polling utilities for NativeMinted events on the Gravity chain.
"""

import asyncio
import json
import logging
import statistics
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
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

# BatchBridgeCaller — helper contract compiled from /tmp/BatchBridgeCaller.sol
# Batches N bridgeToGravity calls into a single transaction.
BATCH_BRIDGE_CALLER_ABI = [
    {
        "type": "function",
        "name": "batchBridge",
        "inputs": [
            {"name": "gtoken", "type": "address"},
            {"name": "sender", "type": "address"},
            {"name": "amount", "type": "uint256"},
            {"name": "recipient", "type": "address"},
            {"name": "count", "type": "uint256"},
        ],
        "outputs": [],
        "stateMutability": "payable",
    },
    {"stateMutability": "payable", "type": "receive"},
]

BATCH_BRIDGE_CALLER_BYTECODE = (
    "0x6080604052348015600e575f5ffd5b506104c48061001c5f395ff3fe"
    "608060405260043610610021575f3560e01c80633756b5d414610"
    "02c57610028565b3661002857005b5f5ffd5b61004660048036"
    "038101906100419190610210565b610048565b005b8473ffffff"
    "ffffffffffffffffffffffffffffffffff1663095ea7b38583"
    "8661007191906102b4565b6040518363ffffffff1660e01b8152"
    "60040161008e929190610313565b6020604051808303815f875a"
    "f11580156100aa573d5f5f3e3d5ffd5b505050506040513d601f"
    "19601f820116820180604052508101906100ce919061036f565b"
    "505f81346100dc91906103c7565b90505f5f90505b828110156101"
    "76578573ffffffffffffffffffffffffffffffffffffffff1663"
    "3f9418a88387876040518463ffffffff1660e01b815260040161"
    "01279291906103f7565b60206040518083038185885af115801561"
    "0143573d5f5f3e3d5ffd5b50505050506040513d601f19601f82"
    "0116820180604052508101906101689190610463565b50808060"
    "01019150506100e3565b50505050505050565b5f5ffd5b5f73ff"
    "ffffffffffffffffffffffffffffffffffffff82169050919050"
    "565b5f6101ac82610183565b9050919050565b6101bc816101a2"
    "565b81146101c6575f5ffd5b50565b5f813590506101d7816101"
    "b3565b92915050565b5f819050919050565b6101ef816101dd56"
    "5b81146101f9575f5ffd5b50565b5f8135905061020a816101e6"
    "565b92915050565b5f5f5f5f5f60a0868803121561022957610228"
    "61017f565b5b5f610236888289016101c9565b955050602061024"
    "7888289016101c9565b9450506040610258888289016101fc565b"
    "9350506060610269888289016101c9565b925050608061027a88"
    "8289016101fc565b9150509295509295909350565b7f4e487b71"
    "000000000000000000000000000000000000000000000000000000"
    "005f52601160045260245ffd5b5f6102be826101dd565b91506102"
    "c9836101dd565b92508282026102d7816101dd565b91508282048"
    "4148315176102ee576102ed610287565b5b5092915050565b6102"
    "fe816101a2565b82525050565b61030d816101dd565b82525050"
    "565b5f6040820190506103265f8301856102f5565b6103336020"
    "830184610304565b9392505050565b5f8115159050919050565b"
    "61034e8161033a565b8114610358575f5ffd5b50565b5f815190"
    "5061036981610345565b92915050565b5f6020828403121561038"
    "45761038361017f565b5b5f6103918482850161035b565b91505"
    "092915050565b7f4e487b710000000000000000000000000000000"
    "00000000000000000000000005f52601260045260245ffd5b5f61"
    "03d1826101dd565b91506103dc836101dd565b9250826103ec576"
    "103eb61039a565b5b828204905092915050565b5f604082019050"
    "61040a5f830185610304565b61041760208301846102f5565b93"
    "92505050565b5f6fffffffffffffffffffffffffffffffff8216"
    "9050919050565b6104428161041e565b811461044c575f5ffd5b"
    "50565b5f8151905061045d81610439565b92915050565b5f6020"
    "82840312156104785761047761017f565b5b5f610485848285016"
    "1044f565b9150509291505056fea26469706673582212207eb41c"
    "746c3f34e6512ab50367904ed42c2b732fd6d74e17d47a0eb3162"
    "77fde64736f6c634300081e0033"
)

# Default batch size for BatchBridgeCaller (bridges per transaction)
BATCH_BRIDGE_CHUNK_SIZE = 100



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
        self.w3 = Web3(Web3.HTTPProvider(
            anvil_rpc_url,
            request_kwargs={"timeout": 300},
        ))
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

    def batch_mint_and_bridge(
        self,
        count: int,
        amount: int,
        recipient: str,
        interval: float = 0.0,
    ) -> List[int]:
        """
        Pre-load Anvil with N bridge transactions efficiently.

        Uses a BatchBridgeCaller contract to batch 100 bridgeToGravity
        calls per transaction, reducing 20K RPC calls to ~200.

        Args:
            count: Number of bridge transactions to send.
            amount: Amount per bridge transaction (in wei).
            recipient: Recipient address on gravity chain.
            interval: Seconds between batch calls (0 = as fast as possible).

        Returns:
            List of bridge nonces (1..count).
        """
        recipient = Web3.to_checksum_address(recipient)
        total_amount = amount * count
        chunk_size = BATCH_BRIDGE_CHUNK_SIZE  # 100 bridges per tx

        # 1. Deploy BatchBridgeCaller contract
        LOG.info("  Batch: deploying BatchBridgeCaller...")
        BatchCaller = self.w3.eth.contract(
            abi=BATCH_BRIDGE_CALLER_ABI,
            bytecode=BATCH_BRIDGE_CALLER_BYTECODE,
        )
        deploy_tx = BatchCaller.constructor().build_transaction(self._tx_params())
        deploy_tx["gas"] = 2_000_000  # deployment needs more gas
        receipt = self._send_tx(deploy_tx)
        batch_caller_addr = receipt.contractAddress
        LOG.info(f"  Batch: BatchBridgeCaller deployed at {batch_caller_addr}")

        batch_caller = self.w3.eth.contract(
            address=batch_caller_addr,
            abi=BATCH_BRIDGE_CALLER_ABI,
        )

        # 2. Mint all GTokens directly to the batch caller
        LOG.info(f"  Batch: minting {total_amount} GTokens to batch caller ({count} × {amount})...")
        tx = self.gtoken.functions.mint(
            batch_caller_addr, total_amount
        ).build_transaction(self._tx_params())
        self._send_tx(tx)

        # 3. Get fee per bridge
        fee = self.sender.functions.calculateBridgeFee(amount, recipient).call()
        total_fee = fee * count
        LOG.info(f"  Batch: fee per bridge = {fee} wei, total = {total_fee} wei")

        # 4. Fund the batch caller with ETH for fees
        LOG.info(f"  Batch: funding batch caller with {total_fee} wei ETH...")
        fund_tx = {
            **self._tx_params(),
            "to": batch_caller_addr,
            "value": total_fee,
        }
        self._send_tx(fund_tx)

        # 5. Call batchBridge in chunks of `chunk_size` (auto-mine mode)
        num_chunks = (count + chunk_size - 1) // chunk_size
        LOG.info(
            f"  Batch: executing {count} bridges in {num_chunks} batch calls "
            f"({chunk_size} per call, auto-mine mode)..."
        )
        t0 = time.time()
        bridges_done = 0

        for i in range(num_chunks):
            remaining = count - bridges_done
            this_chunk = min(chunk_size, remaining)
            chunk_fee = fee * this_chunk

            tx = batch_caller.functions.batchBridge(
                self.gtoken.address,
                self.sender.address,
                amount,
                recipient,
                this_chunk,
            ).build_transaction({
                **self._tx_params(),
                "gas": 500_000 * this_chunk,
                "value": chunk_fee,
            })
            receipt = self._send_tx(tx)
            bridges_done += this_chunk

            if (i + 1) % 10 == 0 or (i + 1) == num_chunks:
                elapsed = time.time() - t0
                LOG.info(
                    f"  Batch: chunk {i + 1}/{num_chunks} done — "
                    f"{bridges_done}/{count} bridges ({elapsed:.1f}s)"
                )

        elapsed = time.time() - t0
        portal_nonce = self.portal.functions.nonce().call()
        LOG.info(
            f"  Batch complete: {bridges_done} bridges in {elapsed:.1f}s "
            f"({num_chunks} txns). Portal nonce = {portal_nonce}"
        )

        return list(range(1, count + 1))

    def query_message_sent_events(self, from_block: int = 0) -> list:
        """Query MessageSent events from GravityPortal on Anvil."""
        logs = self.portal.events.MessageSent().get_logs(from_block=from_block)
        return logs

    def query_message_sent_timestamps(self, from_block: int = 0) -> Dict[int, int]:
        """
        Query MessageSent events and return a mapping of nonce → block timestamp.

        Fetches block timestamps for each unique block containing MessageSent
        events and maps them back to the nonce in each event.

        Returns:
            Dict mapping bridge nonce to the unix timestamp of the Anvil block
            in which the MessageSent event was emitted.
        """
        events = self.query_message_sent_events(from_block=from_block)
        nonce_to_timestamp: Dict[int, int] = {}
        block_ts_cache: Dict[int, int] = {}  # block_number → timestamp

        for evt in events:
            block_num = evt["blockNumber"]
            nonce_val = evt.args.nonce

            if block_num not in block_ts_cache:
                block = self.w3.eth.get_block(block_num)
                block_ts_cache[block_num] = block["timestamp"]

            nonce_to_timestamp[nonce_val] = block_ts_cache[block_num]

        LOG.info(
            f"  Fetched timestamps for {len(nonce_to_timestamp)} MessageSent events "
            f"across {len(block_ts_cache)} blocks"
        )
        return nonce_to_timestamp

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
        receipt = self.w3.eth.wait_for_transaction_receipt(tx_hash, timeout=120)
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


async def poll_all_native_minted(
    gravity_w3: Web3,
    max_nonce: int,
    timeout: float = 600.0,
    poll_interval: float = 3.0,
    receiver_address: str = GBRIDGE_RECEIVER_ADDRESS,
) -> Dict[str, Any]:
    """
    Poll gravity chain until all nonces 1..max_nonce have NativeMinted events.

    Uses isProcessed(max_nonce) as a fast-path check, then scans logs
    for all events to collect detailed data.

    Args:
        gravity_w3: Web3 instance connected to gravity chain.
        max_nonce: The highest expected nonce (assumes 1..max_nonce).
        timeout: Maximum seconds to wait.
        poll_interval: Seconds between polls.
        receiver_address: GBridgeReceiver contract address.

    Returns:
        Dict with:
            - events: list of {recipient, amount, nonce, block_number, tx_hash}
            - found_nonces: set of nonces found
            - missing_nonces: set of nonces NOT found
            - processing_time: seconds from start to all-found
    """
    receiver = gravity_w3.eth.contract(
        address=Web3.to_checksum_address(receiver_address),
        abi=BRIDGE_RECEIVER_ABI,
    )

    expected_nonces = set(range(1, max_nonce + 1))
    start_time = time.time()
    start_block = 0  # Scan from genesis to catch all events

    LOG.info(
        f"  Polling for {max_nonce} NativeMinted events "
        f"(nonces 1→{max_nonce}), timeout={timeout}s..."
    )

    first_iter = True  # one-time extended diagnostics on first poll

    while time.time() - start_time < timeout:
        # Current gravity block height (for CI log correlation)
        try:
            cur_block = gravity_w3.eth.block_number
        except Exception:
            cur_block = -1

        # Fast-path: check if the highest nonce is processed
        try:
            is_last_done = receiver.functions.isProcessed(max_nonce).call()
        except Exception:
            is_last_done = False

        if is_last_done:
            LOG.info(
                f"  isProcessed({max_nonce})=True after "
                f"{time.time() - start_time:.1f}s — scanning logs..."
            )
            break

        # One-time diagnostic on first iteration
        if first_iter:
            first_iter = False
            try:
                is_first = receiver.functions.isProcessed(1).call()
                LOG.info(f"  [diag] isProcessed(1)={is_first}, gravity block={cur_block}")
            except Exception as e:
                LOG.warning(f"  [diag] isProcessed(1) call failed: {e}")
            # Log receiver contract code size (sanity check contract exists)
            try:
                code = gravity_w3.eth.get_code(
                    Web3.to_checksum_address(receiver_address)
                )
                LOG.info(
                    f"  [diag] GBridgeReceiver code size = {len(code)} bytes "
                    f"at {receiver_address}"
                )
            except Exception as e:
                LOG.warning(f"  [diag] get_code failed: {e}")

        # Progress check: check a few nonces to report progress
        checkpoints = [1, max_nonce // 4, max_nonce // 2, 3 * max_nonce // 4, max_nonce]
        processed_count = 0
        for n in checkpoints:
            if n < 1 or n > max_nonce:
                continue
            try:
                if receiver.functions.isProcessed(n).call():
                    processed_count += 1
            except Exception:
                pass

        elapsed = time.time() - start_time
        LOG.info(
            f"  Waiting... {elapsed:.0f}s elapsed, "
            f"checkpoints processed: {processed_count}/5, "
            f"gravity block: {cur_block}"
        )

        await asyncio.sleep(poll_interval)
    else:
        # Timeout reached without isProcessed(max_nonce) being True
        LOG.warning(f"  Timeout after {timeout}s waiting for isProcessed({max_nonce})")
        # Dump extra diagnostics on timeout
        try:
            final_block = gravity_w3.eth.block_number
            is_first_done = receiver.functions.isProcessed(1).call()
            LOG.warning(
                f"  [timeout-diag] gravity block={final_block}, "
                f"isProcessed(1)={is_first_done}, "
                f"blocks since start={final_block - start_block}"
            )
        except Exception as e:
            LOG.warning(f"  [timeout-diag] failed: {e}")

    processing_time = time.time() - start_time

    # Now scan all logs to collect event details
    current_block = gravity_w3.eth.block_number
    all_events = []
    found_nonces = set()
    gravity_block_numbers = set()  # unique blocks for timestamp lookup

    # Scan in chunks to avoid huge responses
    CHUNK_SIZE = 5000
    scan_from = start_block
    while scan_from <= current_block:
        scan_to = min(scan_from + CHUNK_SIZE, current_block)
        try:
            logs = gravity_w3.eth.get_logs(
                {
                    "address": Web3.to_checksum_address(receiver_address),
                    "fromBlock": scan_from,
                    "toBlock": scan_to,
                    "topics": [NATIVE_MINTED_TOPIC0],
                }
            )
            for log_entry in logs:
                event = receiver.events.NativeMinted().process_log(log_entry)
                nonce_val = event.args.nonce
                if nonce_val in expected_nonces:
                    found_nonces.add(nonce_val)
                    blk_num = log_entry["blockNumber"]
                    all_events.append(
                        {
                            "recipient": event.args.recipient,
                            "amount": event.args.amount,
                            "nonce": nonce_val,
                            "block_number": blk_num,
                            "tx_hash": log_entry["transactionHash"].hex(),
                        }
                    )
                    # Collect unique block numbers for timestamp lookup
                    gravity_block_numbers.add(blk_num)
        except Exception as e:
            LOG.warning(f"  Log scan error for blocks {scan_from}-{scan_to}: {e}")

        scan_from = scan_to + 1

    # Batch-fetch block timestamps for all unique gravity blocks
    gravity_block_ts: Dict[int, int] = {}
    for blk_num in gravity_block_numbers:
        try:
            block = gravity_w3.eth.get_block(blk_num)
            gravity_block_ts[blk_num] = block["timestamp"]
        except Exception as e:
            LOG.warning(f"  Failed to fetch gravity block {blk_num} timestamp: {e}")

    # Enrich events with block_timestamp
    for evt in all_events:
        evt["block_timestamp"] = gravity_block_ts.get(evt["block_number"])

    missing_nonces = expected_nonces - found_nonces
    LOG.info(
        f"  Log scan complete: {len(found_nonces)}/{max_nonce} events found, "
        f"{len(missing_nonces)} missing"
    )
    if missing_nonces and len(missing_nonces) <= 20:
        LOG.info(f"  Missing nonces: {sorted(missing_nonces)}")

    return {
        "events": sorted(all_events, key=lambda e: e["nonce"]),
        "found_nonces": found_nonces,
        "missing_nonces": missing_nonces,
        "processing_time": processing_time,
    }


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
        median_lat = statistics.median(self.latencies)
        p95_lat = (
            sorted(self.latencies)[int(len(self.latencies) * 0.95)]
            if len(self.latencies) >= 2
            else max_lat
        )

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
            f"  Latency (MessageSent → NativeMinted):\n"
            f"    Min:            {min_lat:.1f}s\n"
            f"    Max:            {max_lat:.1f}s\n"
            f"    Average:        {avg_lat:.1f}s\n"
            f"    Median:         {median_lat:.1f}s\n"
            f"    P95:            {p95_lat:.1f}s\n"
            f"  \n"
            f"  Total bridged:    {self.total_bridged} wei\n"
            f"  Nonce range:      {nonces_sorted[0]} → {nonces_sorted[-1]}\n"
            f"  Nonces continuous: {'✓' if is_continuous else '✗'}\n"
            f"{'=' * 60}\n"
        )
        LOG.info(report)
        return report
