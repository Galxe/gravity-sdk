"""
Contract utility functions for encoding/decoding contract calls and results

This module provides basic utilities for contract interactions. For most use cases,
prefer using Web3.py's built-in contract.functions.xxx().call() pattern.
"""
import json
import logging
from pathlib import Path
from typing import Any, Dict, List, Optional

LOG = logging.getLogger(__name__)


class ContractUtils:
    """Utility class for contract interaction helpers"""

    # Standard ERC20 function selectors
    ERC20_SELECTORS = {
        'name': '0x06fdde03',
        'symbol': '0x95d89b41',
        'decimals': '0x313ce567',
        'totalSupply': '0x18160ddd',
        'balanceOf': '0x70a08231',
        'transfer': '0xa9059cbb',
        'approve': '0x095ea7b3',
        'transferFrom': '0x23b872dd',
        'allowance': '0xdd62ed3e'
    }

    # Standard function selectors for common contracts
    COMMON_SELECTORS = {
        'getValue': '0x20965255',
        'setValue': '0x55241077',
        'owner': '0x8da5cb5b',
        'renounceOwnership': '0x715018a6',
        'transferOwnership': '0xf2fde38b'
    }

    @staticmethod
    def load_contract_data(contract_name: str, contracts_dir: Path = None) -> Dict:
        """Load contract data from JSON file

        Args:
            contract_name: Name of the contract (e.g., "SimpleStorage")
            contracts_dir: Directory containing contract JSON files

        Returns:
            Contract data dictionary with 'bytecode' and 'abi'
        """
        if contracts_dir is None:
            # Default to gravity_e2e/contracts_data
            contracts_dir = Path(__file__).parent.parent.parent / "contracts_data"

        contract_file = contracts_dir / f"{contract_name}.json"

        if not contract_file.exists():
            raise FileNotFoundError(f"Contract file not found: {contract_file}")

        with open(contract_file, 'r') as f:
            return json.load(f)

    @staticmethod
    def encode_uint256(value: int) -> str:
        """Encode integer as uint256 (32 bytes)"""
        return format(value & 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF, '064x')

    @staticmethod
    def encode_address(address: str) -> str:
        """Encode address as 32 bytes"""
        if not address.startswith("0x"):
            address = "0x" + address
        return address[2:].rjust(64, '0').lower()

    @staticmethod
    def decode_uint256(hex_str: str) -> int:
        """Decode uint256 from hex string"""
        if hex_str.startswith("0x"):
            hex_str = hex_str[2:]
        hex_str = hex_str.rjust(64, '0')
        return int(hex_str, 16) if hex_str else 0

    @staticmethod
    def decode_address(hex_str: str) -> str:
        """Decode address from hex string"""
        if hex_str.startswith("0x"):
            hex_str = hex_str[2:]
        hex_str = hex_str[-40:]
        return "0x" + hex_str.rjust(40, '0')

    @staticmethod
    def validate_address(address: str) -> str:
        """Validate and normalize address"""
        if not address:
            raise ValueError("Address cannot be empty")

        if not isinstance(address, str):
            raise ValueError("Address must be a string")

        if not address.startswith("0x"):
            address = "0x" + address

        if len(address) != 42:
            raise ValueError(f"Invalid address length: {len(address)}")

        return address.lower()


# Convenience functions
def load_contract_data(contract_name: str, contracts_dir: Path = None) -> Dict:
    """Convenience function to load contract data"""
    return ContractUtils.load_contract_data(contract_name, contracts_dir)
