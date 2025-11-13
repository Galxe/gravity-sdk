#!/usr/bin/env python3
"""Extract contract ABI and bytecode for E2E testing"""

import json
import os
from pathlib import Path

def extract_contract_info():
    # Get the build info directory
    build_dir = Path("out")
    
    # Find SimpleToken contract
    simple_token_path = build_dir / "SimpleToken.sol" / "SimpleToken.json"
    
    if not simple_token_path.exists():
        print(f"Contract build file not found at {simple_token_path}")
        return
    
    # Load contract JSON
    with open(simple_token_path, 'r') as f:
        contract_data = json.load(f)
    
    # Extract bytecode
    bytecode = contract_data['bytecode']['object']
    
    # Extract ABI
    abi = contract_data['abi']
    
    # Create output directory
    output_dir = Path("../../..") / "contracts_data"
    output_dir.mkdir(parents=True, exist_ok=True)
    
    # Save contract info
    contract_info = {
        "name": "SimpleToken",
        "bytecode": bytecode,
        "abi": abi
    }
    
    output_file = output_dir / "SimpleToken.json"
    with open(output_file, 'w') as f:
        json.dump(contract_info, f, indent=2)
    
    print(f"Contract info saved to: {output_file}")
    print(f"Bytecode length: {len(bytecode)} characters")
    print(f"ABI functions: {len([abi_item for abi_item in abi if abi_item['type'] == 'function'])}")

if __name__ == "__main__":
    extract_contract_info()