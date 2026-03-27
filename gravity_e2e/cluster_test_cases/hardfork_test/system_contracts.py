"""
System contract address registry for Gravity hardfork testing.

Centralizes all system contract addresses, organized per hardfork.
New hardforks inherit all previous contracts and may add new ones
or override addresses.

Source of truth: gravity_chain_core_contracts/src/foundation/SystemAddresses.sol
"""

from typing import Dict


# ── Base system contract addresses (shared across all hardforks) ──────
# These are the fixed system addresses deployed at genesis.
_BASE_SYSTEM_CONTRACTS: Dict[str, str] = {
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


# ── Per-hardfork contract sets ────────────────────────────────────────
# Each hardfork declares which contracts it upgrades.
# The dict values are addresses.

HARDFORK_CONTRACTS: Dict[str, Dict[str, str]] = {
    "gamma": {
        # Gamma upgrades all 11 base system contracts
        **_BASE_SYSTEM_CONTRACTS,
    },
    # Future hardforks:
    # "delta": {
    #     "ContractA": "0x...",
    #     "ContractB": "0x...",
    # },
}


def get_contracts_for_hardfork(hardfork_name: str) -> Dict[str, str]:
    """
    Get the system contract addresses for a specific hardfork.

    Raises KeyError if hardfork is not registered.
    """
    if hardfork_name not in HARDFORK_CONTRACTS:
        available = ", ".join(HARDFORK_CONTRACTS.keys())
        raise KeyError(
            f"Unknown hardfork '{hardfork_name}'. Available: {available}"
        )
    return HARDFORK_CONTRACTS[hardfork_name]
