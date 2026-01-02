# Gravity E2E Test Framework

A comprehensive end-to-end testing framework for the Gravity blockchain network, designed to validate cross-chain functionality, smart contract deployments, consensus mechanisms, randomness, and network operations.

## Overview

The Gravity E2E Test Framework provides a robust, async-first testing infrastructure for validating blockchain operations across different scenarios. It supports multi-node deployments, cross-chain transactions, consensus testing, randomness validation, validator management, and comprehensive test reporting with modular, reusable utilities.

### Key Features

- **Async-First Architecture**: Built on Python 3.8+ async/await for non-blocking operations
- **Test Registry System**: Automatic test discovery with decorator-based registration
- **Modular Design**: Reusable utilities that eliminate code duplication
- **Multi-Node Support**: Test on validator and VFN nodes with cluster configurations
- **Cross-Chain Testing**: Validate deposits and operations (Sepolia ↔ Gravity)
- **Contract Testing**: Comprehensive smart contract deployment and interaction
- **Randomness Testing**: Validate on-chain randomness generation and consumption
- **Consensus Testing**: Epoch consistency and validator reconfiguration tests
- **Node Management**: Automated node deployment, startup, and health monitoring
- **Event Monitoring**: Real-time event polling and verification
- **Retry Mechanisms**: Configurable retry strategies with exponential backoff
- **Code Quality**: Clean, maintainable code with type hints and comprehensive error handling

## Features

- ✅ **Node Management**: Connect to multiple nodes (Validator/VFN) with automatic health checks
- ✅ **Test Registry**: Automatic test discovery and registration system
- ✅ **Account Management**: Dynamic test account creation with automatic funding
- ✅ **Transaction Testing**: ETH transfers with balance verification
- ✅ **Smart Contract Testing**: Deploy and interact with Solidity contracts
- ✅ **ERC20 Token Testing**: Complete ERC20 token functionality verification
- ✅ **Randomness Testing**: On-chain randomness generation and consumption tests
- ✅ **Epoch Testing**: Consensus epoch consistency validation (self-managed)
- ✅ **Validator Testing**: Dynamic validator add/remove operations (self-managed)
- ✅ **Cross-Chain Testing**: Cross-chain deposit and event verification
- ✅ **Test Persistence**: Save and reuse deployed contracts across test runs
- ✅ **Comprehensive Reporting**: JSON-formatted test results with detailed metrics
- ✅ **Pytest Integration**: Full pytest support with async test execution

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

#### Using the CLI

```bash
# List all available tests and suites
python -m gravity_e2e.main --list-tests

# Run all default tests
python -m gravity_e2e.main

# Run specific test suites
python -m gravity_e2e.main --test-suite basic        # Basic ETH transfers
python -m gravity_e2e.main --test-suite contract     # Smart contract deployment
python -m gravity_e2e.main --test-suite erc20        # ERC20 token operations
python -m gravity_e2e.main --test-suite randomness   # Randomness tests
python -m gravity_e2e.main --test-suite cross_chain  # Cross-chain tests
python -m gravity_e2e.main --test-suite epoch        # Epoch consistency tests (self-managed)
python -m gravity_e2e.main --test-suite validator    # Validator management tests (self-managed)

# Run individual tests
python -m gravity_e2e.main --test-suite basic_transfer
python -m gravity_e2e.main --test-suite randomness_smoke

# Test specific nodes or clusters
python -m gravity_e2e.main --cluster cluster1
python -m gravity_e2e.main --node-id node1,node2
python -m gravity_e2e.main --node-type validator

# Customize logging
python -m gravity_e2e.main --log-level DEBUG --log-file test.log
```

#### Using Pytest

```bash
# Run all tests with pytest
pytest

# Run specific test file
pytest gravity_e2e/tests/test_cases/test_basic_transfer.py

# Run tests with specific marker
pytest -m randomness
pytest -m "not slow"

# Run with verbose output
pytest -v -s

# Run and generate coverage report
pytest --cov=gravity_e2e --cov-report=html
```

## Test Suites

The framework provides comprehensive test coverage across multiple areas:

### Basic Transfer Tests (`basic` suite)

**Test Files**: `test_basic_transfer.py`

Validates fundamental ETH transfer functionality:
- **basic_transfer**: Basic ETH transfer with balance verification
- **multiple_transfers**: Multiple sequential transfers between accounts
- **insufficient_funds**: Error handling for insufficient balance scenarios

### Smart Contract Tests (`contract` suite)

**Test Files**: `test_contract_deploy.py`

Tests Solidity smart contract deployment and interaction:
- **contract**: Deploy SimpleStorage contract and verify state operations
- **contract_constructor**: Deploy contracts with constructor arguments
- **contract_retry**: Test deployment with retry mechanisms
- Supports contract reuse across test runs
- Automatic contract verification

### ERC20 Token Tests (`erc20` suite)

**Test Files**: `test_erc20.py`

Comprehensive ERC20 token functionality testing:
- **erc20**: Basic ERC20 deployment, metadata verification, and transfers
- **erc20_batch**: Batch transfer operations for stress testing
- **erc20_edge_cases**: Edge case scenarios (zero transfers, self-transfers, etc.)
- Token metadata validation (name, symbol, decimals)
- Total supply verification
- Transfer event monitoring

### Cross-Chain Tests (`cross_chain` suite)

**Test Files**: `test_cross_chain_deposit.py`

Validates cross-chain deposit operations:
- **cross_chain_deposit**: Test deposits from Sepolia to Gravity chain
- Event verification on both chains
- Balance reconciliation
- Cross-chain message passing validation

### Randomness Tests (`randomness` suite)

**Test Files**: `test_randomness_basic.py`, `test_randomness_advanced.py`

Validates on-chain randomness generation and consumption:
- **randomness_basic**: Basic randomness request and consumption
- **randomness_correctness**: Verify randomness quality and distribution
- **randomness_smoke**: Quick smoke test for randomness functionality
- **randomness_reconfiguration**: Test randomness across validator reconfigurations
- **randomness_multi_contract**: Multiple contracts consuming randomness
- **randomness_api_completeness**: Comprehensive API coverage tests
- **randomness_stress**: High-volume randomness consumption stress test

### Epoch Consistency Tests (`epoch` suite) - Self-Managed

**Test Files**: `test_epoch_consistency.py`

Tests consensus epoch progression and consistency:
- **epoch_consistency**: Basic epoch consistency validation
- **epoch_consistency_extended**: Extended epoch testing across multiple validators
- These tests manage their own node lifecycle
- Validate epoch boundaries and state transitions

### Validator Management Tests (`validator` suite) - Self-Managed

**Test Files**: `test_validator_add_remove.py`

Tests dynamic validator set changes:
- **validator_add_remove**: Add and remove validators from the network (immediate startup)
- **validator_add_remove_delayed**: Test with delayed node startup after validator join
- Validate consensus after validator changes
- These tests manage their own cluster setup

## Test Registry System

The framework uses a decorator-based test registry for automatic test discovery:

```python
from gravity_e2e.tests.test_registry import register_test

@register_test("my_test", suite="my_suite")
async def test_my_feature(run_helper, test_result):
    """My test implementation"""
    # Test logic here
    test_result.mark_success(key="value")
```

### Self-Managed Tests

Some tests (epoch, validator) are marked as "self-managed", meaning they handle their own node lifecycle instead of using pre-connected nodes:

```python
@register_test("my_test", suite="my_suite", self_managed=True)
async def test_my_feature(run_helper, test_result):
    # This test will manage its own nodes
    pass
```

## Writing New Tests

### 1. Create Test File

Create a new Python file in `gravity_e2e/tests/test_cases/`:

```python
# gravity_e2e/tests/test_cases/test_my_feature.py
import asyncio
import logging
from ...helpers.test_helpers import RunHelper, TestResult, test_case
from ...tests.test_registry import register_test

LOG = logging.getLogger(__name__)

@register_test("my_feature", suite="my_suite")
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

### 2. Register Your Test

Import your test in `gravity_e2e/tests/test_cases/__init__.py`:

```python
# Import your test
from .test_my_feature import test_my_feature

# The test is already registered via the @register_test decorator
# No additional registration needed
```

### 3. Run Your Test

```bash
# Run the specific test
python -m gravity_e2e.main --test-suite my_feature

# Or run the entire suite
python -m gravity_e2e.main --test-suite my_suite

# With pytest
pytest gravity_e2e/tests/test_cases/test_my_feature.py
```

### 4. Available Helper Methods

The `@test_case` decorator automatically:
- Measures execution time
- Handles exceptions
- Logs test start/end
- Tracks test results

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

#### RPC Client (GravityClient)
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

# Get block number
block_number = await run_helper.client.get_block_number()
```

#### Using Utility Modules

##### Transaction Builder
```python
from gravity_e2e.utils.transaction_builder import TransactionBuilder, TransactionOptions
from web3 import Web3

builder = TransactionBuilder(web3, account)
result = await builder.build_and_send_tx(
    to="0x742d35Cc6634C0532925a3b8D4C9db96C4b4Db45",
    options=TransactionOptions(value=Web3.to_wei(1, 'ether')),
    simulate=True  # Simulate before sending
)
```

##### Contract Deployer
```python
from gravity_e2e.utils.contract_deployer import ContractDeployer, DeploymentOptions

deployer = ContractDeployer(web3, account)
result = await deployer.deploy(
    contract_name="SimpleToken",
    constructor_args=["Test Token", "TEST", 1000000],
    options=DeploymentOptions(verify=True)
)
```

##### Event Poller
```python
from gravity_e2e.utils.event_poller import EventPoller

poller = EventPoller(web3)

# Wait for specific event
event = await poller.wait_for_event(
    contract=token_contract,
    event_name="Transfer",
    timeout=60,
    from_address=sender_address
)

# Get all events in range
events = await poller.get_events(
    contract=token_contract,
    event_name="Transfer",
    from_block=1000,
    to_block=2000
)
```

##### Async Retry
```python
from gravity_e2e.utils.async_retry import AsyncRetry

retry = AsyncRetry(max_retries=3, base_delay=1.0, jitter=True)
result = await retry.execute(unstable_function)

# Or use as decorator
@retry
async def my_function():
    return await some_operation()
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
├── gravity_e2e/                    # Main package
│   ├── core/                      # Core components
│   │   ├── client/                # RPC client implementations
│   │   │   ├── gravity_client.py          # Async Gravity RPC client
│   │   │   └── gravity_http_client.py     # HTTP client with connection pooling
│   │   ├── node_connector.py      # Multi-node connection manager
│   │   └── node_manager.py        # Node deployment and lifecycle
│   │
│   ├── helpers/                   # Test utilities
│   │   ├── test_helpers.py        # Test execution framework (@test_case decorator)
│   │   └── account_manager.py     # Account creation and management
│   │
│   ├── utils/                     # Shared utility modules
│   │   ├── async_retry.py         # Configurable async retry with exponential backoff
│   │   ├── config_manager.py      # Configuration loading
│   │   ├── transaction_builder.py # Transaction building, signing, and sending
│   │   ├── contract_deployer.py   # Contract deployment with verification
│   │   ├── contract_utils.py      # Contract encoding/decoding utilities
│   │   ├── event_poller.py        # Event monitoring and polling
│   │   ├── event_parser.py        # Event parsing and filtering
│   │   ├── randomness_utils.py    # Randomness testing utilities
│   │   ├── epoch_utils.py         # Epoch testing utilities
│   │   ├── validator_utils.py     # Validator testing utilities
│   │   ├── logging.py             # Logging configuration
│   │   ├── common.py              # Common utility functions
│   │   └── exceptions.py          # Centralized exception definitions
│   │
│   ├── tests/                     # Test cases
│   │   ├── test_registry.py       # Test registration system
│   │   ├── conftest.py            # Pytest configuration and fixtures
│   │   ├── test_cases/            # Individual test scenarios
│   │   │   ├── __init__.py        # Test registration imports
│   │   │   ├── test_basic_transfer.py
│   │   │   ├── test_contract_deploy.py
│   │   │   ├── test_erc20.py
│   │   │   ├── test_cross_chain_deposit.py
│   │   │   ├── test_randomness_basic.py
│   │   │   ├── test_randomness_advanced.py
│   │   │   ├── test_epoch_consistency.py
│   │   │   └── test_validator_add_remove.py
│   │   └── unit/                  # Unit tests for utilities
│   │       └── test_utilities.py
│   │
│   ├── main.py                    # CLI entry point
│   └── __main__.py                # Python module execution support
│
├── configs/                       # Configuration files
│   ├── nodes.json                 # Node connection configurations
│   ├── test_accounts.json         # Test account credentials
│   └── cross_chain_config.json    # Cross-chain test configuration
│
├── contracts_data/                # Compiled contract data (bytecode + ABI)
│   ├── SimpleStorage.json
│   ├── TestToken.json
│   └── RandomnessConsumer.json
│
├── tests/                         # Test contracts
│   └── contracts/
│       └── erc20-test/           # Forge project for test contracts
│
├── docs/                          # Documentation
│   └── contract_tools_guide.md   # Contract testing tools guide
│
├── scripts/                       # Helper scripts
│   └── extract_contract.py       # Extract bytecode/ABI from Forge builds
│
├── output/                        # Test results and logs
│   └── test_results.json
│
├── setup.py                       # Package setup
├── requirements.txt               # Python dependencies
├── pytest.ini                     # Pytest configuration
└── README.md                      # This file
```

### Key Components

#### 1. Test Registry System (`tests/test_registry.py`)
- Automatic test discovery using decorators
- Suite-based test organization
- Support for self-managed tests (tests that control their own nodes)
- CLI integration for flexible test execution

#### 2. Core Clients (`core/client/`)
- **GravityClient**: Async RPC client with retry logic and error handling
- **GravityHttpClient**: HTTP client with connection pooling for high-throughput scenarios

#### 3. Utility Modules (`utils/`)
- **Transaction Builder**: Simplified transaction creation with automatic gas estimation
- **Contract Deployer**: Deploy contracts with verification and caching
- **Event Poller**: Efficient event monitoring with filtering
- **Async Retry**: Resilient retry mechanism with exponential backoff
- **Config Manager**: Simple configuration loading from JSON files
- **Validator Utils**: Shared utilities for validator add/remove tests
- **Epoch Utils**: Shared utilities for epoch consistency tests

#### 4. Test Helpers (`helpers/`)
- **RunHelper**: Provides test utilities (account creation, client access)
- **TestAccountManager**: Manages test accounts and funding
- **@test_case decorator**: Automatic timing, error handling, and logging

## Best Practices

1. **Use the Test Registry**: Register all tests with `@register_test` decorator for automatic discovery
2. **Use the `@test_case` decorator**: Provides automatic timing, error handling, and logging
3. **Leverage utility modules**: Use TransactionBuilder and ContractDeployer instead of manual RPC calls
4. **Create descriptive test names**: Test names should clearly indicate what is being tested
5. **Include meaningful metrics**: Add relevant data to `test_result.mark_success()` for better reporting
6. **Handle async operations properly**: Always use `await` for async operations
7. **Clean up resources**: The framework handles most cleanup, but close any custom resources
8. **Write deterministic tests**: Tests should produce consistent results given the same input
9. **Use appropriate gas limits**: 200k for deployment, 50k for simple calls, adjust for complex operations
10. **Mark self-managed tests**: Use `self_managed=True` for tests that manage their own nodes
11. **Organize tests into suites**: Group related tests together using the `suite` parameter
12. **Use pytest markers**: Mark slow tests with `@pytest.mark.slow` for selective execution
13. **Validate results thoroughly**: Check transaction receipts, events, and state changes
14. **Log important milestones**: Use LOG.info() for key test steps and results
15. **Handle errors gracefully**: Use try-except blocks for expected failures and test edge cases

### Test Organization Tips

```python
# Good: Clear naming and organization
@register_test("erc20_batch_transfer", suite="erc20")
@test_case
async def test_erc20_batch_transfers(run_helper, test_result):
    """Test batch ERC20 transfers with event verification"""
    # Clear test logic
    pass

# Good: Self-managed test with proper marking
@register_test("epoch_consistency", suite="epoch", self_managed=True)
@test_case
async def test_epoch_consistency(run_helper, test_result):
    """Test epoch boundaries and transitions"""
    # Test manages its own nodes
    pass
```

## Troubleshooting

### Common Issues

1. **"Object of type HexBytes is not JSON serializable"**
   - The framework handles this automatically. Test results are still saved.
   - If you encounter this in your code, use `.hex()` method to convert HexBytes to string.

2. **"Transaction had invalid fields"**
   - Ensure addresses are in checksum format using `Web3.to_checksum_address()`
   - Remove "0x" prefix from private keys before signing
   - Verify all transaction fields are properly formatted (hex strings with "0x" prefix)

3. **"execution reverted"**
   - Check function selectors are correct
   - Verify contract bytecode matches the source
   - Ensure contract is properly deployed
   - Check if you have sufficient balance for the transaction
   - Validate function arguments match the expected types

4. **Connection refused**
   - Verify Gravity Node is running on the configured port
   - Check node configuration in `configs/nodes.json`
   - Ensure firewall rules allow connections
   - Try connecting with curl: `curl -X POST http://localhost:8545 -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'`

5. **Test timeout**
   - Check if the node is syncing or stuck
   - Increase timeout values in pytest.ini or test code
   - Verify network connectivity
   - Check node logs for errors

6. **"Test not found in registry"**
   - Ensure test is imported in `gravity_e2e/tests/test_cases/__init__.py`
   - Check that `@register_test` decorator is applied
   - Verify test name matches what you're passing to CLI

7. **Nonce too low / nonce too high**
   - The framework handles nonce management automatically
   - If issues persist, restart the node or use a fresh account
   - Check for pending transactions: `await client.get_transaction_count(address, 'pending')`

8. **Gas estimation failed**
   - Transaction would fail - check contract logic
   - Ensure sufficient balance for gas costs
   - Try with a fixed gas limit instead of estimation
   - Check if contract has proper permissions/access control

### Debug Mode

Enable debug logging to see detailed information:

```bash
# Via CLI
python -m gravity_e2e.main --log-level DEBUG --log-file debug.log

# In test code
import logging
logging.getLogger('gravity_e2e').setLevel(logging.DEBUG)
```

### Testing Node Health

```bash
# Check node is responding
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'

# Run health check via framework
python -m gravity_e2e.main --test-suite basic_transfer --log-level INFO
```

### Pytest Debugging

```bash
# Run with output capture disabled
pytest -s

# Run with Python debugger on failure
pytest --pdb

# Run specific test with verbose output
pytest -vv gravity_e2e/tests/test_cases/test_basic_transfer.py::test_eth_transfer
```

### Environment Issues

If you encounter import or module errors:
```bash
# Reinstall dependencies
pip install -r requirements.txt --force-reinstall

# Install in development mode
pip install -e .

# Clear Python cache
find . -type d -name __pycache__ -exec rm -rf {} +
find . -type f -name "*.pyc" -delete
```

## CLI Reference

The framework provides a comprehensive CLI for running tests:

```bash
python -m gravity_e2e.main [OPTIONS]
```

### Options

- `--nodes-config PATH`: Path to nodes configuration file (default: `configs/nodes.json`)
- `--accounts-config PATH`: Path to accounts configuration file (default: `configs/test_accounts.json`)
- `--test-suite NAME`: Test suite or individual test to run (default: `all`)
  - Available suites: `basic`, `contract`, `erc20`, `cross_chain`, `randomness`, `epoch`, `validator`
  - Can also specify individual test names
- `--list-tests`: List all available tests and suites
- `--cluster NAME`: Test a specific cluster defined in nodes.json
- `--node-id IDS`: Comma-separated list of node IDs to test
- `--node-type TYPE`: Test all nodes of specific type (`validator` or `vfn`)
- `--log-level LEVEL`: Logging level (`DEBUG`, `INFO`, `WARNING`, `ERROR`) (default: `INFO`)
- `--log-file PATH`: Path to log file (optional)
- `--output-dir PATH`: Output directory for test results (default: `output`)

### Examples

```bash
# List all available tests
python -m gravity_e2e.main --list-tests

# Run all default tests
python -m gravity_e2e.main

# Run specific suite
python -m gravity_e2e.main --test-suite randomness

# Run individual test
python -m gravity_e2e.main --test-suite randomness_smoke

# Test specific cluster
python -m gravity_e2e.main --cluster mainnet --test-suite basic

# Test specific nodes
python -m gravity_e2e.main --node-id node1,node2 --test-suite erc20

# Test all validators
python -m gravity_e2e.main --node-type validator

# Debug mode with log file
python -m gravity_e2e.main --log-level DEBUG --log-file debug.log --test-suite contract
```

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Follow the code style and best practices outlined above
4. Add tests for new functionality
5. Register new tests using `@register_test` decorator
6. Update documentation as needed
7. Ensure all tests pass (`pytest && python -m gravity_e2e.main --test-suite all`)
8. Commit your changes with descriptive messages
9. Push to your branch
10. Submit a pull request

### Code Style

- Follow PEP 8 guidelines
- Use type hints for function parameters and return values
- Write docstrings for public functions and classes
- Keep functions focused and concise
- Use meaningful variable and function names
- Add comments for complex logic

## Technology Stack

- **Python 3.8+**: Async/await support required
- **Web3.py**: Ethereum/blockchain interaction
- **aiohttp**: Async HTTP client for RPC communication
- **eth-account**: Transaction signing and account management
- **Pydantic**: Configuration validation and data modeling
- **pytest**: Testing framework with async support
- **pytest-asyncio**: Async test execution

## Project Status

The Gravity E2E Test Framework is actively maintained and provides comprehensive test coverage for:
- ✅ Basic blockchain operations (transfers, balance checks)
- ✅ Smart contract deployment and interaction
- ✅ ERC20 token functionality
- ✅ Cross-chain deposits and messaging
- ✅ On-chain randomness generation and consumption
- ✅ Consensus epoch consistency
- ✅ Dynamic validator set changes
- ✅ Multi-node testing (validator and VFN)

## License

This project is part of the Gravity SDK and follows the same license terms.

## Support

For issues, questions, or contributions:
- Open an issue on GitHub
- Check existing documentation in the `docs/` directory
- Review test examples in `gravity_e2e/tests/test_cases/`
- Run `python -m gravity_e2e.main --list-tests` to see all available tests

## Related Documentation

- [Gravity SDK Architecture](../book/docs/architecture.md)
- [Contract Testing Tools Guide](docs/contract_tools_guide.md)
- [Node Deployment Instructions](../deploy_utils/readme.md)

---

**Happy Testing!** 🚀