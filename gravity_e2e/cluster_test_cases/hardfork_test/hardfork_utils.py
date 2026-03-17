"""
Reusable utilities for hardfork E2E testing.

Provides helper functions for:
- Waiting for specific block heights
- Verifying system contract bytecode changes
- Sending light transaction pressure
- Injecting hardfork config into genesis.json

These utilities are designed to be reusable across different hardfork tests
(gamma, delta, epsilon, etc.).
"""

import asyncio
import json
import logging
import time
from pathlib import Path
from typing import Dict, List, Optional, Tuple

from eth_account import Account
from web3 import Web3

LOG = logging.getLogger(__name__)


# ── Gamma hardfork system contract addresses ─────────────────────────
# These are the fixed system contract addresses upgraded by the gamma hardfork.
# Source: gravity-reth/crates/ethereum/evm/src/hardfork/gamma.rs GAMMA_SYSTEM_UPGRADES
GAMMA_SYSTEM_CONTRACTS = {
    "StakingConfig": "0x00000000000000000000000000000001625F1001",
    "ValidatorConfig": "0x00000000000000000000000000000001625F1002",
    "GovernanceConfig": "0x00000000000000000000000000000001625F1004",
    "Staking": "0x00000000000000000000000000000001625F2000",
    "ValidatorManagement": "0x00000000000000000000000000000001625F2001",
    "Reconfiguration": "0x00000000000000000000000000000001625F2003",
    "Blocker": "0x00000000000000000000000000000001625F2004",
    "PerformanceTracker": "0x00000000000000000000000000000001625F2005",
    "Governance": "0x00000000000000000000000000000001625F3000",
    "NativeOracle": "0x00000000000000000000000000000001625F4000",
    "OracleRequestQueue": "0x00000000000000000000000000000001625F4002",
}


async def wait_for_block(
    w3: Web3, target_block: int, timeout: int = 300, poll_interval: float = 1.0
) -> bool:
    """
    Wait until the node reaches a specific block height.
    Returns True if target reached, False on timeout.
    """
    start = time.monotonic()
    while time.monotonic() - start < timeout:
        try:
            current = w3.eth.block_number
            if current >= target_block:
                LOG.info(f"✅ Reached block {current} (target: {target_block})")
                return True
            if int(time.monotonic() - start) % 10 == 0 and int(time.monotonic() - start) > 0:
                LOG.info(f"  ⏳ Current block: {current}, waiting for {target_block}...")
        except Exception:
            pass
        await asyncio.sleep(poll_interval)
    LOG.error(f"❌ Timed out waiting for block {target_block} (timeout={timeout}s)")
    return False


async def wait_for_blocks_after(
    w3: Web3, start_block: int, delta: int, timeout: int = 120
) -> bool:
    """
    Wait for `delta` more blocks after `start_block`.
    """
    target = start_block + delta
    LOG.info(f"Waiting for {delta} blocks after {start_block} (target: {target})...")
    return await wait_for_block(w3, target, timeout=timeout)


def get_contract_code_hashes(
    w3: Web3, addresses: Dict[str, str]
) -> Dict[str, Optional[str]]:
    """
    Get code hashes for a set of contract addresses.
    Returns {name: hex_code_hash_or_None}.
    """
    result = {}
    for name, addr in addresses.items():
        try:
            code = w3.eth.get_code(Web3.to_checksum_address(addr))
            if code and len(code) > 0:
                code_hash = Web3.keccak(code).hex()
                result[name] = code_hash
            else:
                result[name] = None
        except Exception as e:
            LOG.warning(f"  Failed to get code for {name} ({addr}): {e}")
            result[name] = None
    return result


def snapshot_system_contracts(w3: Web3) -> Dict[str, Optional[str]]:
    """
    Take a snapshot of code hashes for all gamma system contracts.
    """
    return get_contract_code_hashes(w3, GAMMA_SYSTEM_CONTRACTS)


def compare_snapshots(
    before: Dict[str, Optional[str]], after: Dict[str, Optional[str]]
) -> Tuple[List[str], List[str], List[str]]:
    """
    Compare two contract code hash snapshots.
    Returns (changed, unchanged, missing).
    """
    changed = []
    unchanged = []
    missing = []
    for name in before:
        if before[name] is None and after.get(name) is None:
            missing.append(name)
        elif before[name] != after.get(name):
            changed.append(name)
        else:
            unchanged.append(name)
    return changed, unchanged, missing


async def send_eth_transfers(
    w3: Web3, sender_account, num_txns: int = 10, amount_wei: int = 1000
) -> Tuple[int, int]:
    """
    Send simple ETH transfers as light pressure.
    Returns (successful_count, failed_count).
    """
    success_count = 0
    fail_count = 0
    chain_id = w3.eth.chain_id

    for i in range(num_txns):
        try:
            receiver = Account.create()
            nonce = w3.eth.get_transaction_count(sender_account.address)

            tx = {
                "to": receiver.address,
                "value": amount_wei,
                "gas": 21000,
                "gasPrice": w3.eth.gas_price,
                "nonce": nonce,
                "chainId": chain_id,
            }

            signed = sender_account.sign_transaction(tx)
            tx_hash = w3.eth.send_raw_transaction(signed.raw_transaction)
            receipt = w3.eth.wait_for_transaction_receipt(tx_hash, timeout=30)

            if receipt["status"] == 1:
                success_count += 1
            else:
                fail_count += 1
                LOG.warning(f"  TX {i} reverted")
        except Exception as e:
            fail_count += 1
            LOG.warning(f"  TX {i} failed: {e}")

    LOG.info(f"  Transfers: {success_count} ok, {fail_count} failed (out of {num_txns})")
    return success_count, fail_count


def inject_hardfork_config(
    genesis_path: Path, hardfork_name: str, block_number: int
):
    """
    Inject a gravity hardfork block number into genesis.json.
    Generic function for any hardfork (gamma, delta, etc.).
    """
    with open(genesis_path) as f:
        genesis = json.load(f)

    if "config" not in genesis:
        genesis["config"] = {}
    if "gravityHardforks" not in genesis["config"]:
        genesis["config"]["gravityHardforks"] = {}

    key = f"{hardfork_name}Block"
    genesis["config"]["gravityHardforks"][key] = block_number

    with open(genesis_path, "w") as f:
        json.dump(genesis, f, indent=2)

    LOG.info(f"Injected {key}={block_number} into {genesis_path}")
