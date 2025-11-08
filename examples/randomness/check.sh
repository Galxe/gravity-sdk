#!/bin/bash
set -e # Exit on error
set -o pipefail

# --- 1. Configuration
# Set Reth node RPC URL
export RETH_RPC_URL="http://127.0.0.1:8545"

# Set private key for transaction signing
export PRIVATE_KEY="0x047a5466f6f9e08c8bcc56213d6530d517c1ef126eefbbdf85ffe8d893ed0e9f"

# Set deployed contract address
# Replace with your actual contract address after deployment
export CONTRACT_ADDRESS="0x2BB0961D1b7f928FB3dF4d90A1A825d55e2F4e1A"
# --- End Configuration 

# --- 2. Environment Check
echo "=========================================="
echo " RETH Node Random Number Verification"
echo "=========================================="
echo ""
echo "=========================================="
echo "Step 1: Environment Check"
echo "=========================================="

# Check if contract address is set
if [[ "$CONTRACT_ADDRESS" == "0x...PASTE_YOUR_ADDRESS_HERE..." ]]; then
    echo "  ❌ Error: Please set a valid contract address first"
    exit 1
fi
echo "CHECK_CONTRACT_ADDRESS  ✅ "
echo "   Contract Address: $CONTRACT_ADDRESS"
echo ""

# Check if jq tool is installed
if ! command -v jq &> /dev/null; then
    echo "  ❌ Error: jq tool is not installed"
    exit 1
fi
echo "CHECK_JQ_COMMAND  ✅ "
echo "   jq tool is installed"
echo ""

echo "=========================================="
echo "Step 2: Connection Info"
echo "=========================================="
echo "   RPC URL: $RETH_RPC_URL"
echo "   Private Key: ${PRIVATE_KEY:0:10}...${PRIVATE_KEY: -4}"

# --- 3. Call rollDice() function
echo ""
echo "=========================================="
echo "Step 3: Call rollDice() Function"
echo "=========================================="

# Send transaction to call rollDice()
# This will trigger the contract to generate a random number
TX_OUTPUT_JSON=$(cast send $CONTRACT_ADDRESS "rollDice()" \
    --rpc-url $RETH_RPC_URL \
    --private-key $PRIVATE_KEY \
    --json)

# Extract transaction details from JSON output
BLOCK_NUMBER_HEX=$(echo "$TX_OUTPUT_JSON" | jq -r .blockNumber)
TX_HASH=$(echo "$TX_OUTPUT_JSON" | jq -r .transactionHash)
BLOCK_NUMBER=$(cast --to-dec $BLOCK_NUMBER_HEX)

echo "CHECK_TX_OUTPUT_JSON  ✅ "
echo "   Transaction Hash: $TX_HASH"
echo "   Block Number: $BLOCK_NUMBER (hex: $BLOCK_NUMBER_HEX)"
echo ""

# --- 4. Fetch random seed and block information
echo "=========================================="
echo "Step 4: Fetch Random Seed & Block Info"
echo "=========================================="

# 4a. Read the last seed used from the contract
SEED_FROM_CONTRACT_HEX=$(cast call $CONTRACT_ADDRESS "lastSeedUsed()" \
    --rpc-url $RETH_RPC_URL)
SEED_FROM_CONTRACT=$(cast --to-dec $SEED_FROM_CONTRACT_HEX)
echo "CHECK_SEED_FROM_CONTRACT  ✅ "
echo "   Seed from Contract: $SEED_FROM_CONTRACT"
echo "   (hex: $SEED_FROM_CONTRACT_HEX)"
echo ""

# 4b. Read difficulty and mixHash (prevrandao) from the block
BLOCK_JSON=$(cast block $BLOCK_NUMBER --rpc-url $RETH_RPC_URL --json)
DIFFICULTY_FROM_NODE_HEX=$(echo $BLOCK_JSON | jq -r .difficulty)
PREVRANDAO_FROM_NODE_HEX=$(echo $BLOCK_JSON | jq -r .mixHash)
DIFFICULTY_FROM_NODE=$(cast --to-dec $DIFFICULTY_FROM_NODE_HEX)
PREVRANDAO_FROM_NODE=$(cast --to-dec $PREVRANDAO_FROM_NODE_HEX)
echo "CHECK_DIFFICULTY_FROM_NODE  ✅ "
echo "   Block Difficulty: $DIFFICULTY_FROM_NODE"
echo "   (hex: $DIFFICULTY_FROM_NODE_HEX)"
echo ""
echo "   Block mixHash (prevrandao): $PREVRANDAO_FROM_NODE"
echo "   (hex: $PREVRANDAO_FROM_NODE_HEX)"

# --- 5. Display key data comparison
echo ""
echo "=========================================="
echo "Step 5: Key Data Comparison"
echo "=========================================="
echo "  A) Seed from Contract:  $SEED_FROM_CONTRACT"
echo "  B) Block Difficulty:    $DIFFICULTY_FROM_NODE"
echo "  C) Block mixHash:       $PREVRANDAO_FROM_NODE"
echo "=========================================="

# --- 6. Verification
echo ""
echo "=========================================="
echo "Step 6: Verification"
echo "=========================================="
all_good=true

# Verification 1: Contract seed vs Block difficulty
echo ""
echo "Verification 1: Contract Seed == Block Difficulty?"
echo "   Contract Seed:   $SEED_FROM_CONTRACT"
echo "   Block Difficulty: $DIFFICULTY_FROM_NODE"
if [ "$SEED_FROM_CONTRACT" != "$DIFFICULTY_FROM_NODE" ]; then
    echo "  ❌ Mismatch! RETH may not correctly implement the DIFFICULTY opcode"
    all_good=false
else
    echo "  ✅ Match! Contract successfully read the block's difficulty value"
fi

# Verification 2: Difficulty vs Prevrandao
echo ""
echo "Verification 2: Difficulty == mixHash (prevrandao)?"
echo "   Block Difficulty: $DIFFICULTY_FROM_NODE"
echo "   Block mixHash:    $PREVRANDAO_FROM_NODE"
if [ "$DIFFICULTY_FROM_NODE" != "$PREVRANDAO_FROM_NODE" ]; then
    echo "  ⚠️  Mismatch! In PoS chains, difficulty should map to prevrandao"
    all_good=false
else
    echo "  ✅ Match! Difficulty correctly maps to prevrandao"
fi

echo ""
if $all_good; then
    echo "=========================================="
    echo "  🎉 All checks passed! RETH correctly implements randomness!"
    echo "=========================================="
else
    echo "=========================================="
    echo "  ⚠️  Some checks failed. Please verify RETH configuration"
    echo "=========================================="
fi

# Read the final dice roll result
RESULT_HEX=$(cast call $CONTRACT_ADDRESS "lastRollResult()" --rpc-url $RETH_RPC_URL)
RESULT=$(cast --to-dec $RESULT_HEX)
echo ""
echo "=========================================="
echo "🎲 Final Dice Result: $RESULT (Range: 1-6)"
echo "=========================================="