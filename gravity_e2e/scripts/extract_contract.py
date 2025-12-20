#!/usr/bin/env python3
"""
Extract contract ABI and bytecode from forge build output

Usage: python extract_contract.py <ContractName>
"""

import sys
import json
import os
from pathlib import Path

# Directories
CONTRACTS_DIR = Path(__file__).parent.parent / "tests" / "contracts"
CONTRACTS_DATA_DIR = Path(__file__).parent.parent / "contracts_data"

def extract_contract(contract_name: str):
    """Extract contract ABI and bytecode"""
    # Look in erc20-test directory
    contract_dir = CONTRACTS_DIR / "erc20-test"
    out_dir = contract_dir / "out"

    if not out_dir.exists():
        print(f"❌ Build output directory not found: {out_dir}")
        print("Please run 'forge build' first")
        return 1

    # Find contract in build output
    contract_file = None
    for root, dirs, files in os.walk(out_dir):
        for file in files:
            if file == f"{contract_name}.json":
                contract_file = Path(root) / file
                break
        if contract_file:
            break

    if not contract_file:
        print(f"❌ Contract {contract_name} not found in build output")
        print("Available contracts:")
        for root, dirs, files in os.walk(out_dir):
            for file in files:
                if file.endswith('.json'):
                    print(f"  - {file[:-5]}")
        return 1

    # Load contract data
    with open(contract_file, 'r') as f:
        contract_data = json.load(f)

    # Extract ABI and bytecode
    if 'abi' not in contract_data or 'bytecode' not in contract_data:
        print(f"❌ Invalid contract format for {contract_name}")
        return 1

    # Prepare output
    output = {
        "abi": contract_data['abi'],
        "bytecode": contract_data['bytecode'].get('object', '')
    }

    # Ensure contracts_data directory exists
    CONTRACTS_DATA_DIR.mkdir(parents=True, exist_ok=True)

    # Save to file
    output_file = CONTRACTS_DATA_DIR / f"{contract_name}.json"
    with open(output_file, 'w') as f:
        json.dump(output, f, indent=2)

    print(f"✅ Extracted {contract_name} to {output_file}")
    print(f"   ABI: {len(output['abi'])} functions")
    print(f"   Bytecode: {len(output['bytecode'])} characters")

    return 0

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: python extract_contract.py <ContractName>")
        sys.exit(1)

    contract_name = sys.argv[1]
    sys.exit(extract_contract(contract_name))