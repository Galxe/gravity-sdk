"""
Staking Testing Utilities

This module provides shared utilities for staking-related E2E tests:
- System contract addresses
- ABI definitions for Staking, StakePool, ValidatorManagement
- Helper functions for pool creation and common operations
"""

import time
import logging
from typing import Optional

from gravity_e2e.utils.transaction_builder import TransactionBuilder, TransactionOptions, run_sync

LOG = logging.getLogger(__name__)

# ============================================================================
# SYSTEM CONTRACT ADDRESSES
# These are fixed addresses deployed at genesis
# ============================================================================
STAKING_PROXY_ADDRESS = "0x00000000000000000000000000000001625f2000"
VALIDATOR_MANAGER_ADDRESS = "0x00000000000000000000000000000001625f2001"

# ============================================================================
# ABI DEFINITIONS
# ============================================================================

# Staking.sol ABI (Factory)
STAKING_ABI = [
    {
        "inputs": [
            {"internalType": "address", "name": "owner", "type": "address"},
            {"internalType": "address", "name": "staker", "type": "address"},
            {"internalType": "address", "name": "operator", "type": "address"},
            {"internalType": "address", "name": "voter", "type": "address"},
            {"internalType": "uint64", "name": "lockedUntil", "type": "uint64"}
        ],
        "name": "createPool",
        "outputs": [{"internalType": "address", "name": "pool", "type": "address"}],
        "stateMutability": "payable",
        "type": "function"
    },
    {
        "inputs": [{"internalType": "address", "name": "pool", "type": "address"}],
        "name": "isPool",
        "outputs": [{"internalType": "bool", "name": "", "type": "bool"}],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [{"internalType": "address", "name": "pool", "type": "address"}],
        "name": "getPoolActiveStake",
        "outputs": [{"internalType": "uint256", "name": "", "type": "uint256"}],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [{"internalType": "address", "name": "pool", "type": "address"}],
        "name": "getPoolVotingPowerNow",
        "outputs": [{"internalType": "uint256", "name": "", "type": "uint256"}],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [],
        "name": "getMinimumStake",
        "outputs": [{"internalType": "uint256", "name": "", "type": "uint256"}],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "anonymous": False,
        "inputs": [
            {"indexed": True, "internalType": "address", "name": "creator", "type": "address"},
            {"indexed": True, "internalType": "address", "name": "pool", "type": "address"},
            {"indexed": True, "internalType": "address", "name": "owner", "type": "address"},
            {"indexed": False, "internalType": "address", "name": "staker", "type": "address"},
            {"indexed": False, "internalType": "uint256", "name": "poolIndex", "type": "uint256"}
        ],
        "name": "PoolCreated",
        "type": "event"
    }
]

# StakePool.sol ABI
STAKE_POOL_ABI = [
    {
        "inputs": [],
        "name": "addStake",
        "outputs": [],
        "stateMutability": "payable",
        "type": "function"
    },
    {
        "inputs": [{"internalType": "uint256", "name": "amount", "type": "uint256"}],
        "name": "unstake",
        "outputs": [],
        "stateMutability": "nonpayable",
        "type": "function"
    },
    {
        "inputs": [{"internalType": "address", "name": "recipient", "type": "address"}],
        "name": "withdrawAvailable",
        "outputs": [{"internalType": "uint256", "name": "amount", "type": "uint256"}],
        "stateMutability": "nonpayable",
        "type": "function"
    },
    {
        "inputs": [
            {"internalType": "uint256", "name": "amount", "type": "uint256"},
            {"internalType": "address", "name": "recipient", "type": "address"}
        ],
        "name": "unstakeAndWithdraw",
        "outputs": [{"internalType": "uint256", "name": "withdrawn", "type": "uint256"}],
        "stateMutability": "nonpayable",
        "type": "function"
    },
    {
        "inputs": [{"internalType": "uint64", "name": "durationMicros", "type": "uint64"}],
        "name": "renewLockUntil",
        "outputs": [],
        "stateMutability": "nonpayable",
        "type": "function"
    },
    {
        "inputs": [{"internalType": "address", "name": "newOperator", "type": "address"}],
        "name": "setOperator",
        "outputs": [],
        "stateMutability": "nonpayable",
        "type": "function"
    },
    {
        "inputs": [{"internalType": "address", "name": "newVoter", "type": "address"}],
        "name": "setVoter",
        "outputs": [],
        "stateMutability": "nonpayable",
        "type": "function"
    },
    {
        "inputs": [{"internalType": "address", "name": "newStaker", "type": "address"}],
        "name": "setStaker",
        "outputs": [],
        "stateMutability": "nonpayable",
        "type": "function"
    },
    {
        "inputs": [],
        "name": "activeStake",
        "outputs": [{"internalType": "uint256", "name": "", "type": "uint256"}],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [],
        "name": "getStaker",
        "outputs": [{"internalType": "address", "name": "", "type": "address"}],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [],
        "name": "getOperator",
        "outputs": [{"internalType": "address", "name": "", "type": "address"}],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [],
        "name": "getVoter",
        "outputs": [{"internalType": "address", "name": "", "type": "address"}],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [],
        "name": "getLockedUntil",
        "outputs": [{"internalType": "uint64", "name": "", "type": "uint64"}],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [],
        "name": "isLocked",
        "outputs": [{"internalType": "bool", "name": "", "type": "bool"}],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [],
        "name": "getTotalPending",
        "outputs": [{"internalType": "uint256", "name": "", "type": "uint256"}],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [],
        "name": "getClaimableAmount",
        "outputs": [{"internalType": "uint256", "name": "", "type": "uint256"}],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [],
        "name": "getVotingPowerNow",
        "outputs": [{"internalType": "uint256", "name": "", "type": "uint256"}],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "anonymous": False,
        "inputs": [
            {"indexed": True, "internalType": "address", "name": "pool", "type": "address"},
            {"indexed": False, "internalType": "uint256", "name": "amount", "type": "uint256"}
        ],
        "name": "StakeAdded",
        "type": "event"
    },
    {
        "anonymous": False,
        "inputs": [
            {"indexed": True, "internalType": "address", "name": "pool", "type": "address"},
            {"indexed": False, "internalType": "uint256", "name": "amount", "type": "uint256"},
            {"indexed": False, "internalType": "uint64", "name": "lockedUntil", "type": "uint64"}
        ],
        "name": "Unstaked",
        "type": "event"
    },
    {
        "anonymous": False,
        "inputs": [
            {"indexed": True, "internalType": "address", "name": "pool", "type": "address"},
            {"indexed": False, "internalType": "address", "name": "oldOperator", "type": "address"},
            {"indexed": False, "internalType": "address", "name": "newOperator", "type": "address"}
        ],
        "name": "OperatorChanged",
        "type": "event"
    }
]


# ============================================================================
# HELPER FUNCTIONS
# ============================================================================

def get_current_time_micros() -> int:
    """Get current time in microseconds (Gravity uses microseconds for timestamps)"""
    return int(time.time() * 1_000_000)


async def create_stake_pool(
    tx_builder: TransactionBuilder,
    staking_contract,
    owner: str,
    staker: str,
    operator: str,
    voter: str,
    locked_until: int,
    initial_stake_wei: int
) -> Optional[str]:
    """
    Create a new stake pool via the Staking factory.
    
    Args:
        tx_builder: TransactionBuilder instance for the creator
        staking_contract: Web3 contract instance for Staking
        owner: Pool owner address
        staker: Staker address (funds management)
        operator: Operator address (validator operations)
        voter: Voter address (governance)
        locked_until: Lock expiration timestamp in microseconds
        initial_stake_wei: Initial stake amount in wei
    
    Returns:
        The pool address if successful, None otherwise.
    """
    LOG.info(f"Creating stake pool with initial stake: {initial_stake_wei / 10**18} ETH")
    
    result = await tx_builder.build_and_send_tx(
        to=STAKING_PROXY_ADDRESS,
        data=staking_contract.encode_abi('createPool', [
            owner, staker, operator, voter, locked_until
        ]),
        value=initial_stake_wei,
        options=TransactionOptions(gas_limit=5_000_000)
    )
    
    if not result.success:
        LOG.error(f"Failed to create pool: {result.error}")
        return None
    
    # Parse PoolCreated event to get pool address
    w3 = tx_builder.web3
    receipt = w3.eth.get_transaction_receipt(result.tx_hash)
    logs = staking_contract.events.PoolCreated().process_receipt(receipt)
    
    if not logs:
        LOG.error("PoolCreated event not found in receipt")
        return None
    
    pool_address = logs[0]['args']['pool']
    LOG.info(f"Stake pool created at: {pool_address}")
    return pool_address


def get_staking_contract(w3):
    """Get a Web3 contract instance for the Staking factory."""
    return w3.eth.contract(address=STAKING_PROXY_ADDRESS, abi=STAKING_ABI)


def get_pool_contract(w3, pool_address: str):
    """Get a Web3 contract instance for a StakePool."""
    return w3.eth.contract(address=pool_address, abi=STAKE_POOL_ABI)
