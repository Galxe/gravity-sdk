#!/usr/bin/env python3
"""
Verify contract data availability for Gravity E2E tests

This script ensures that all required contract data is available
before running tests. If contracts are missing, it provides
instructions on how to build them.
"""

import os
import sys
import json
import subprocess
from pathlib import Path
from typing import Dict, List, Optional

CONTRACTS_DIR = Path(__file__).parent.parent / "tests" / "contracts"
CONTRACTS_DATA_DIR = Path(__file__).parent.parent / "contracts_data"

# Required contracts for testing
REQUIRED_CONTRACTS = {
    "SimpleStorage": {
        "description": "Simple storage contract for basic deployment tests",
        "source_dir": CONTRACTS_DIR / "erc20-test",
    },
    "SimpleToken": {
        "description": "ERC20 test token for ERC20 functionality tests",
        "source_dir": CONTRACTS_DIR / "erc20-test",
    },
    "RandomDice": {
        "description": "Randomness beacon test contract",
        "source_dir": CONTRACTS_DIR / "erc20-test",
    },
    "CrossChainBridge": {
        "description": "Cross-chain bridge contract for deposit tests",
        "source_dir": CONTRACTS_DIR / "erc20-test",
    }
}

def check_contract_exists(contract_name: str) -> bool:
    """Check if contract data file exists"""
    contract_file = CONTRACTS_DATA_DIR / f"{contract_name}.json"
    return contract_file.exists() and contract_file.stat().st_size > 0

def load_contract_data(contract_name: str) -> Optional[Dict]:
    """Load and validate contract data"""
    contract_file = CONTRACTS_DATA_DIR / f"{contract_name}.json"

    if not contract_file.exists():
        return None

    try:
        with open(contract_file, 'r') as f:
            data = json.load(f)

        # Validate required fields
        required_fields = ['bytecode', 'abi']
        for field in required_fields:
            if field not in data:
                print(f"❌ {contract_name}: Missing '{field}' in contract data")
                return None

        return data
    except json.JSONDecodeError as e:
        print(f"❌ {contract_name}: Invalid JSON in contract file: {e}")
        return None
    except Exception as e:
        print(f"❌ {contract_name}: Error loading contract: {e}")
        return None

def check_forge_installation() -> bool:
    """Check if forge (Foundry) is installed"""
    try:
        result = subprocess.run(
            ["forge", "--version"],
            capture_output=True,
            text=True,
            check=True
        )
        return True
    except (subprocess.CalledProcessError, FileNotFoundError):
        return False

def build_contracts() -> bool:
    """Build contracts using forge"""
    print("\n📦 Building contracts with forge...")

    # Check if forge is installed
    if not check_forge_installation():
        print("❌ Forge (Foundry) is not installed")
        print("Please install Foundry:")
        print("  curl -L https://foundry.paradigm.xyz | bash")
        print("  foundryup")
        return False

    # Change to contract directory
    contract_dir = CONTRACTS_DIR / "erc20-test"
    if not contract_dir.exists():
        print(f"❌ Contract directory not found: {contract_dir}")
        return False

    try:
        # Run forge build
        result = subprocess.run(
            ["forge", "build"],
            cwd=contract_dir,
            capture_output=True,
            text=True,
            check=True
        )
        print("✅ Contracts built successfully")
        return True
    except subprocess.CalledProcessError as e:
        print(f"❌ Failed to build contracts:")
        print(e.stderr)
        return False

def extract_contract_abi(contract_name: str, source_dir: Path) -> bool:
    """Extract ABI from compiled contract"""
    try:
        # The ABI should be in out/<ContractName>.sol/<ContractName>.json
        out_dir = source_dir / "out"

        # Look for contract file
        for contract_file in out_dir.rglob(f"{contract_name}.json"):
            with open(contract_file, 'r') as f:
                contract_data = json.load(f)

            if 'abi' in contract_data and 'bytecode' in contract_data:
                # Save to contracts_data
                output_file = CONTRACTS_DATA_DIR / f"{contract_name}.json"

                # Ensure contracts_data directory exists
                CONTRACTS_DATA_DIR.mkdir(parents=True, exist_ok=True)

                # Write contract data
                with open(output_file, 'w') as f:
                    json.dump({
                        'bytecode': contract_data['bytecode']['object'],
                        'abi': contract_data['abi']
                    }, f, indent=2)

                print(f"✅ Extracted {contract_name} ABI and bytecode")
                return True

        print(f"❌ Could not find compiled contract for {contract_name}")
        return False
    except Exception as e:
        print(f"❌ Error extracting {contract_name}: {e}")
        return False

def main():
    """Main verification function"""
    print("🔍 Verifying Gravity E2E contract data...")

    # Ensure contracts_data directory exists
    CONTRACTS_DATA_DIR.mkdir(parents=True, exist_ok=True)

    missing_contracts = []
    invalid_contracts = []

    # Check each required contract
    for contract_name, info in REQUIRED_CONTRACTS.items():
        print(f"\nChecking {contract_name}...")
        print(f"  {info['description']}")

        if not check_contract_exists(contract_name):
            missing_contracts.append(contract_name)
            print(f"  ❌ Contract data file not found")
            continue

        data = load_contract_data(contract_name)
        if not data:
            invalid_contracts.append(contract_name)
            continue

        print(f"  ✅ Contract data valid (ABI: {len(data['abi'])} functions)")

    # Note: Some contracts (RandomDice, CrossChainBridge) might be in different locations
    # They are marked as optional for now since they're used by specific tests
    optional_contracts = {"RandomDice", "CrossChainBridge"}
    critical_missing = [c for c in missing_contracts if c not in optional_contracts]

    # Report results
    if not critical_missing and not invalid_contracts:
        if missing_contracts:
            print(f"\n✅ All critical contracts are valid!")
            print(f"⚠️  Optional contracts missing: {', '.join(missing_contracts)}")
            print("   These contracts may be loaded dynamically or from other locations")
        else:
            print("\n✅ All contract data is valid and ready!")
        return 0

    # Provide fix instructions
    print("\n⚠️  Contract issues detected:")

    if missing_contracts:
        print(f"\nMissing contracts: {', '.join(missing_contracts)}")
        print("\nTo fix this:")
        print("1. Ensure you have Foundry installed:")
        print("   curl -L https://foundry.paradigm.xyz | bash")
        print("   foundryup")
        print("\n2. Build contracts:")
        print(f"   cd {CONTRACTS_DIR / 'erc20-test'}")
        print("   forge build")
        print("\n3. Extract contract data:")
        for contract in missing_contracts:
            print(f"   python scripts/extract_contract.py {contract}")

    if invalid_contracts:
        print(f"\nInvalid contracts: {', '.join(invalid_contracts)}")
        print("\nPlease rebuild these contracts.")

    # Offer to automatically build and extract
    print("\nWould you like to automatically build contracts? (y/n)")
    try:
        response = input().strip().lower()
        if response in ['y', 'yes']:
            if build_contracts():
                print("\nExtracting contract ABIs...")
                success_count = 0
                for contract_name in missing_contracts:
                    source_dir = REQUIRED_CONTRACTS[contract_name]['source_dir']
                    if extract_contract_abi(contract_name, source_dir):
                        success_count += 1

                if success_count == len(missing_contracts):
                    print("\n✅ All contracts extracted successfully!")
                    return 0
                else:
                    print(f"\n⚠️  Only {success_count}/{len(missing_contracts)} contracts extracted")
    except KeyboardInterrupt:
        print("\nOperation cancelled by user")

    return 1

if __name__ == "__main__":
    sys.exit(main())