# Gravity E2E Test Framework

A comprehensive end-to-end testing framework for Gravity Node, designed to validate blockchain functionality through automated tests.

## Overview

The Gravity E2E Test Framework connects to deployed Gravity nodes via JSON-RPC API and performs various tests including basic transactions, smart contract deployment, and token operations. The framework is built with Python using async architecture for efficient concurrent testing.

## Features

- ✅ **Node Management**: Connect to multiple nodes (Validator/VFN) with automatic health checks
- ✅ **Account Management**: Dynamic test account creation with automatic funding
- ✅ **Transaction Testing**: ETH transfers with balance verification
- ✅ **Smart Contract Testing**: Deploy and interact with Solidity contracts
- ✅ **ERC20 Token Testing**: Complete ERC20 token functionality verification
- ✅ **Test Persistence**: Save and reuse deployed contracts across test runs
- ✅ **Comprehensive Reporting**: JSON-formatted test results with detailed metrics

## Prerequisites

- Python 3.8+
- Gravity Node running on port 8545 (or configured port)
- Funded faucet account for test account funding

## Installation

```bash
cd gravity_e2e
pip install -r requirements.txt
```

## Quick Start

### 1. Configure Test Accounts

Edit `configs/test_accounts.json` to include your faucet account:

```json
{
  "faucet": {
    "address": "YOUR_FAUCET_ADDRESS",
    "private_key": "YOUR_FAUCET_PRIVATE_KEY"
  }
}
```

### 2. Configure Node Connection

Edit `configs/nodes.json` to match your node configuration:

```json
{
  "nodes": {
    "local_node": {
      "type": "validator",
      "host": "127.0.0.1",
      "rpc_port": 8545
    }
  }
}
```

### 3. Run Tests

```bash
# Run all tests
python -m gravity_e2e.main

# Run specific test suites
python -m gravity_e2e.main --test-suite basic      # Basic ETH transfers
python -m gravity_e2e.main --test-suite contract   # Smart contract deployment
python -m gravity_e2e.main --test-suite erc20      # ERC20 token operations
```

## Test Cases

### Basic Transfer Tests (`test_basic_transfer.py`)

Validates fundamental ETH transfer functionality:
- Creates test accounts
- Funds accounts from faucet
- Transfers ETH between accounts
- Verifies final balances
- **Test Function**: `test_eth_transfer`

### Smart Contract Tests (`test_contract_deploy.py`)

Tests Solidity smart contract deployment and interaction:
- Deploys SimpleStorage contract
- Reads initial stored value
- Updates storage value
- Verifies value persistence
- Supports contract reuse across test runs
- **Test Function**: `test_simple_storage_deploy`

### ERC20 Token Tests (`test_erc20.py`)

Comprehensive ERC20 token functionality testing:
- Deploys ERC20 token contract (TestToken)
- Verifies token metadata (name, symbol, decimals)
- Checks total supply
- Creates token holder accounts
- Performs token transfers
- **Test Function**: `test_erc20_deploy_and_transfer`

## Writing New Tests

### 1. Create Test File

Create a new Python file in `gravity_e2e/tests/test_cases/`:

```python
# gravity_e2e/tests/test_cases/test_my_feature.py
import asyncio
import logging
from ...helpers.test_helpers import RunHelper, TestResult, test_case

LOG = logging.getLogger(__name__)

@test_case
async def test_my_feature(run_helper: RunHelper, test_result: TestResult):
    """Test my custom feature"""
    LOG.info("Starting my feature test")
    
    # 1. Create test accounts
    account = await run_helper.create_test_account("my_account", fund_wei=1 * 10**18)
    
    # 2. Perform test logic
    # ... your test code here ...
    
    # 3. Mark test success with details
    test_result.mark_success(
        account_address=account["address"],
        custom_metric=42
    )
    
    LOG.info("My feature test completed successfully")
```

### 2. Test Decorator

Use the `@test_case` decorator to automatically:
- Measure execution time
- Handle exceptions
- Log test start/end
- Track test results

### 3. Available Helper Methods

#### Account Management
```python
# Create funded test account
account = await run_helper.create_test_account("account_name", fund_wei=1 * 10**18)

# Account structure:
# {
#     "name": "account_name",
#     "address": "0x...",
#     "private_key": "0x..."
# }
```

#### RPC Client
```python
# Get balance
balance = await run_helper.client.get_balance(address)

# Get transaction count
nonce = await run_helper.client.get_transaction_count(address)

# Send raw transaction
tx_hash = await run_helper.client.send_raw_transaction(raw_tx)

# Wait for transaction receipt
receipt = await run_helper.client.wait_for_transaction_receipt(tx_hash)

# Call contract (read-only)
result = await run_helper.client.call(to=address, data="0x...")
```

#### Transaction Building
```python
from eth_account import Account

# Build transaction
tx_data = {
    "to": "0x...",
    "value": hex(amount),
    "gas": hex(gas_limit),
    "gasPrice": hex(gas_price),
    "nonce": hex(nonce),
    "chainId": hex(chain_id)
}

# Sign transaction
signed_tx = Account.sign_transaction(tx_data, private_key)
```

### 4. Register New Test

Add your test to `gravity_e2e/main.py`:

```python
# Import your test
from .tests.test_cases.test_my_feature import test_my_feature

# Add to run_test_module function
async def run_test_module(module_name: str, run_helper: RunHelper):
    if module_name == "cases.my_feature":
        result = await test_my_feature(run_helper=run_helper)
        test_results.append(result)
```

### 5. Run Your Test

```bash
python -m gravity_e2e.main --test-suite my_feature
```

## Smart Contract Development

### Adding New Contracts

1. Create contract in `tests/contracts/erc20-test/src/`
2. Build with Forge:
   ```bash
   cd tests/contracts/erc20-test
   forge build
   ```
3. Extract bytecode and ABI:
   ```bash
   python scripts/extract_contract.py YourContract
   ```
4. Use in your test by loading from `contracts_data/YourContract.json`

### Example Contract Test

```python
import json
from pathlib import Path

# Load contract data
CONTRACTS_DIR = Path(__file__).parent.parent.parent.parent / "contracts_data"
with open(CONTRACTS_DIR / "YourContract.json", 'r') as f:
    contract_data = json.load(f)
    bytecode = contract_data["bytecode"]
    abi = contract_data["abi"]

# Deploy contract
deploy_tx = {
    "data": bytecode,
    "gas": hex(200000),
    "gasPrice": hex(gas_price),
    "nonce": hex(nonce),
    "chainId": hex(chain_id)
}
```

## Configuration

### Test Accounts (`configs/test_accounts.json`)
```json
{
  "faucet": {
    "address": "0x...",
    "private_key": "0x..."
  },
  "test_accounts": {
    "account_name": {
      "address": "0x...",
      "private_key": "0x..."
    }
  }
}
```

### Node Configuration (`configs/nodes.json`)
```json
{
  "network": {
    "name": "gravity-local",
    "chain_id": 1337
  },
  "nodes": {
    "node_id": {
      "type": "validator|vfn",
      "host": "127.0.0.1",
      "rpc_port": 8545,
      "capabilities": ["full", "mining"]
    }
  }
}
```

## Test Results

Test results are saved in `output/test_results.json`:

```json
{
  "test_name": "cases.contract",
  "success": true,
  "start_time": "2025-11-12T23:30:47.534Z",
  "end_time": "2025-11-12T23:30:50.615Z",
  "details": {
    "contract_address": "0x56844ceb3a3ee99f22c982c60fbb9b7b69c00eb0",
    "deployment_gas_used": 160008,
    "initial_value": 42,
    "updated_value": 12345
  }
}
```

## Architecture

```
gravity_e2e/
├── gravity_e2e/          # Main package
│   ├── core/            # Core components
│   │   ├── client/      # Gravity RPC client
│   │   └── node_connector.py  # Node connection manager
│   ├── helpers/         # Test utilities
│   │   ├── test_helpers.py    # Test execution framework
│   │   └── account_manager.py # Account management
│   ├── tests/           # Test cases
│   │   └── test_cases/
│   └── utils/           # Common utilities
├── configs/             # Configuration files
├── contracts_data/      # Compiled contract data
└── output/             # Test results
```

## Best Practices

1. **Use the `@test_case` decorator** for all test functions
2. **Create descriptive test names** that explain what is being tested
3. **Include meaningful metrics** in `test_result.mark_success()`
4. **Handle async operations properly** with `await`
5. **Clean up resources** if needed (though the framework handles most cleanup)
6. **Write deterministic tests** that produce the same results given the same input
7. **Use appropriate gas limits** for transactions (200k for deployment, 50k for simple calls)

## Troubleshooting

### Common Issues

1. **"Object of type HexBytes is not JSON serializable"**
   - The framework handles this automatically. Test results are still saved.

2. **"Transaction had invalid fields"**
   - Ensure addresses are in checksum format using `to_checksum_address()`
   - Remove "0x" prefix from private keys before signing

3. **"execution reverted"**
   - Check function selectors are correct
   - Verify contract bytecode matches the source
   - Ensure contract is properly deployed

4. **Connection refused**
   - Verify Gravity Node is running on the configured port
   - Check node configuration in `configs/nodes.json`

### Debug Mode

Enable debug logging:
```python
import logging
logging.basicConfig(level=logging.DEBUG)
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass
5. Submit a pull request

## License

This project is part of the Gravity SDK and follows the same license terms.