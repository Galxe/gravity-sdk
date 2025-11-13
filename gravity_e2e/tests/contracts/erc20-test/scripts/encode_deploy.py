#!/usr/bin/env python3
"""Encode constructor parameters for SimpleToken deployment"""

from eth_abi import encode

# Constructor parameters
name = "TestToken"
symbol = "TEST"
initial_supply = 1000000 * 10**18  # 1 million tokens with 18 decimals

# Constructor ABI
constructor_abi = {
    "inputs": [
        {"internalType": "string", "name": "name", "type": "string"},
        {"internalType": "string", "name": "symbol", "type": "string"},
        {"internalType": "uint256", "name": "initialSupply", "type": "uint256"}
    ],
    "stateMutability": "nonpayable",
    "type": "constructor"
}

# Encode parameters
encoded = encode(
    [item["type"] for item in constructor_abi["inputs"]],
    [name, symbol, initial_supply]
)

# Print encoded data
print("Encoded constructor data:")
print(encoded.hex())
print(f"Length: {len(encoded.hex())} characters")

# Also save to a file
import json
import sys
sys.path.append("../../..")

CONTRACTS_DIR = "../../../contracts_data"
with open(f"{CONTRACTS_DIR}/deployment_data.json", "w") as f:
    json.dump({
        "constructor_data": encoded.hex(),
        "parameters": {
            "name": name,
            "symbol": symbol,
            "initialSupply": str(initial_supply)
        }
    }, f, indent=2)

print(f"Saved to: {CONTRACTS_DIR}/deployment_data.json")