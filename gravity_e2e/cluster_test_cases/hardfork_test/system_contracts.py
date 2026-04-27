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
    "JWKManager": "0x00000000000000000000000000000001625F4001",
    "OracleRequestQueue": "0x00000000000000000000000000000001625F4002",
}


# Testnet GBridgeReceiver — dynamically deployed by Genesis, address pinned on
# gravity testnet. Override in-test if running against a different environment.
GBRIDGE_RECEIVER_TESTNET: str = "0x595475934ed7d9faa7fca28341c2ce583904a44e"


# ── Per-hardfork contract sets ────────────────────────────────────────
# Each hardfork declares which contracts it upgrades.
# The dict values are addresses.
#
# Alpha / Beta predate the verify framework (they only touched Staking and
# StakePool very early in the testnet lifecycle); they are intentionally not
# listed here.

HARDFORK_CONTRACTS: Dict[str, Dict[str, str]] = {
    # Gamma — 11 base system contracts + dynamic StakePool instances
    "gamma": {
        **_BASE_SYSTEM_CONTRACTS,
    },
    # Delta — targeted fix on top of Gamma (4 contracts).
    "delta": {
        "StakingConfig": _BASE_SYSTEM_CONTRACTS["StakingConfig"],
        "ValidatorManagement": _BASE_SYSTEM_CONTRACTS["ValidatorManagement"],
        "Governance": _BASE_SYSTEM_CONTRACTS["Governance"],
        "NativeOracle": _BASE_SYSTEM_CONTRACTS["NativeOracle"],
    },
    # Epsilon — D3-2 underbonded eviction, eviction call-site move,
    # autoEvictThresholdPct, GBridgeReceiver _processedNonces removal.
    "epsilon": {
        "ValidatorManagement": _BASE_SYSTEM_CONTRACTS["ValidatorManagement"],
        "Reconfiguration": _BASE_SYSTEM_CONTRACTS["Reconfiguration"],
        "ValidatorConfig": _BASE_SYSTEM_CONTRACTS["ValidatorConfig"],
        "GBridgeReceiver": GBRIDGE_RECEIVER_TESTNET,
    },
    # Zeta — Governance initialize + owner, ValidatorManagement whitelist seed,
    # StakePool 2-step role timelock, StakingConfig single-field setters,
    # Reconfiguration DKG snapshot fix, JWKManager non-empty validation.
    "zeta": {
        "Governance": _BASE_SYSTEM_CONTRACTS["Governance"],
        "StakingConfig": _BASE_SYSTEM_CONTRACTS["StakingConfig"],
        "ValidatorManagement": _BASE_SYSTEM_CONTRACTS["ValidatorManagement"],
        "Reconfiguration": _BASE_SYSTEM_CONTRACTS["Reconfiguration"],
        "JWKManager": _BASE_SYSTEM_CONTRACTS["JWKManager"],
    },
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
