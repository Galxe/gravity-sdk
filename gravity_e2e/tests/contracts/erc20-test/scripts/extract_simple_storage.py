#!/usr/bin/env python3
"""Extract SimpleStorage contract from the compiled output"""

import json
import sys

# Read the combined output
with open('out/SimpleStorage.sol/SimpleStorage.json', 'r') as f:
    data = json.load(f)

# Extract bytecode and ABI
bytecode = data['bytecode']['object']  # Bytecode is in the 'object' field
abi = data['abi']

# Create SimpleStorage.json
simple_storage = {
    "name": "SimpleStorage",
    "bytecode": bytecode,
    "abi": abi
}

# Save to contracts_data
import os
output_dir = '../../../contracts_data'
os.makedirs(output_dir, exist_ok=True)

with open(f'{output_dir}/SimpleStorage.json', 'w') as f:
    json.dump(simple_storage, f, indent=2)

print(f"SimpleStorage contract saved to {output_dir}/SimpleStorage.json")
print(f"Bytecode length: {len(bytecode)} characters")
print(f"ABI functions: {len([a for a in abi if a['type'] == 'function'])}")