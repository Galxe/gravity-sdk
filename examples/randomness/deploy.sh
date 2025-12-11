#!/bin/bash
set -e # Exit on error

# --- 1. Configuration
# Set Reth node RPC URL
export RETH_RPC_URL="http://127.0.0.1:8545"

# Set deployer private key
export PRIVATE_KEY="0x...PASTE_YOUR_PRIVATE_KEY..."
# --- End Configuration

echo "--- Deploying Contract to Reth Node ---"
echo "  RPC URL: $RETH_RPC_URL"
echo "  Deployer: (from PRIVATE_KEY)"

# Step 1: Compile the contract
echo "\n[1. Compiling contract...]"
forge build

# Step 2: Deploy the contract
echo "\n[2. Broadcasting deployment transaction...]"
# Use forge create to deploy the RandomDice contract
# --rpc-url: specify the Reth node RPC URL
# --private-key: deployer's private key
# --broadcast: broadcast the transaction to the network
forge create --rpc-url $RETH_RPC_URL \
             --private-key $PRIVATE_KEY \
             examples/randomness/RandomDice.sol:RandomDice \
             --broadcast