# Adding Test Cases Guide

This guide provides detailed instructions on how to add custom test cases to the Gravity E2E test framework, including reusable components, best practices, and complete examples.

## Table of Contents

1. [Test Type Overview](#1-test-type-overview)
2. [Quick Start](#2-quick-start)
3. [Reusable Components](#3-reusable-components)
4. [Complete Examples](#4-complete-examples)
5. [Best Practices](#5-best-practices)
6. [Running Tests](#6-running-tests)

---

## 1. Test Type Overview

The framework supports two types of tests:

### 1.1 Regular Tests (Dependent on External Nodes)

- **Characteristics**: Require pre-started Gravity nodes
- **Use Cases**: Transaction testing, contract deployment, ERC20 operations, Randomness tests
- **Examples**: `test_basic_transfer.py`, `test_erc20.py`, `test_randomness_*.py`

### 1.2 Self-Managed Tests

- **Characteristics**: Tests deploy, start, and stop their own nodes
- **Use Cases**: Tests requiring specific node configurations or multi-node scenarios
- **Examples**: `test_epoch_consistency.py`, `test_validator_add_remove.py`
- **Registration**: Requires `self_managed=True` parameter

---

## 2. Quick Start

### 2.1 Create Test File

Create a new file in the `gravity_e2e/tests/test_cases/` directory:

```python
"""
My custom test
"""
import sys
from pathlib import Path

# Add package path (for absolute imports)
_current_dir = Path(__file__).resolve().parent
_package_root = _current_dir.parent.parent.parent
if str(_package_root) not in sys.path:
    sys.path.insert(0, str(_package_root))

import asyncio
import logging
import pytest

from gravity_e2e.helpers.test_helpers import RunHelper, TestResult, test_case

LOG = logging.getLogger(__name__)


@test_case
async def test_my_feature(run_helper: RunHelper, test_result: TestResult):
    """My feature test"""
    LOG.info("=" * 70)
    LOG.info("Test: My Feature Test")
    LOG.info("=" * 70)

    # Test logic...

    test_result.mark_success(
        custom_field="value"
    )
```

### 2.2 Register Test

Register in `gravity_e2e/tests/test_cases/__init__.py`:

```python
# Import test function
from gravity_e2e.tests.test_cases.test_my_feature import test_my_feature

# Register to test registry
register_test("my_feature", suite="my_suite")(test_my_feature)

# For self-managed tests
register_test("my_self_managed_test", suite="my_suite", self_managed=True)(test_my_self_managed)
```

---

## 3. Reusable Components

### 3.1 Core Components

#### RunHelper - Test Execution Helper

```python
# Create and fund test account
account = await run_helper.create_test_account(
    name="test_account",
    fund_wei=2 * 10**18  # 2 ETH
)
# Returns: {"address": "0x...", "private_key": "0x..."}

# Get RPC client
client = run_helper.client

# Get faucet account (for funding other accounts)
faucet = run_helper.faucet_account
```

#### GravityClient - RPC Client

```python
from gravity_e2e.core.client.gravity_client import GravityClient

# Common methods
block_number = await client.get_block_number()
balance = await client.get_balance(address)
nonce = await client.get_transaction_count(address)
gas_price = await client.get_gas_price()
chain_id = await client.get_chain_id()
block = await client.get_block(block_number, full_transactions=False)
receipt = await client.wait_for_transaction_receipt(tx_hash, timeout=30)
result = await client.call(to=contract_address, data=calldata)
tx_hash = await client.send_raw_transaction(signed_tx)
```

#### GravityHttpClient - HTTP API Client

```python
from gravity_e2e.core.client.gravity_http_client import GravityHttpClient

async with GravityHttpClient(base_url="http://127.0.0.1:1024") as http_client:
    # DKG status
    dkg_status = await http_client.get_dkg_status()
    # Returns: {"epoch": 1, "round": 100, "block_number": 500, "participating_nodes": 4}

    # Get block randomness
    randomness = await http_client.get_randomness(block_number)
    # Returns: "0x..." (32 bytes hex)

    # Wait for specific epoch
    await http_client.wait_for_epoch(target_epoch=5, timeout=600)

    # Get ledger info
    ledger_info = await http_client.get_ledger_info_by_epoch(epoch)

    # Get block information
    block_info = await http_client.get_block_by_epoch_round(epoch=2, round=1)
    qc_info = await http_client.get_qc_by_epoch_round(epoch=2, round=1)
```

### 3.2 Transaction Tools

#### TransactionBuilder - Transaction Builder

```python
from gravity_e2e.utils.transaction_builder import TransactionBuilder, TransactionOptions

builder = TransactionBuilder(web3_or_client, account)

# Build transaction
tx = await builder.build_transaction(
    to="0x...",
    value=1000000000000000000,  # 1 ETH
    data="0x..."  # Optional
)

# Build and send transaction (recommended)
result = await builder.build_and_send_tx(
    to="0x...",
    value=1000000000000000000,
    options=TransactionOptions(gas_limit=21000)
)

if result.success:
    print(f"Tx hash: {result.tx_hash}")
    print(f"Gas used: {result.gas_used}")

# Send ETH
result = await builder.send_ether(
    to="0x...",
    amount_wei=1000000000000000000
)
```

#### Simple Transfer Example

```python
from eth_account import Account

# Sign and send transaction
nonce = await client.get_transaction_count(sender["address"])
gas_price = await client.get_gas_price()
chain_id = await client.get_chain_id()

tx_data = {
    "to": recipient,
    "value": hex(amount_wei),
    "gas": hex(21000),
    "gasPrice": hex(gas_price),
    "nonce": hex(nonce),
    "chainId": hex(chain_id),
}

signed_tx = Account.sign_transaction(tx_data, sender["private_key"])
tx_hash = await client.send_raw_transaction(signed_tx.raw_transaction)
receipt = await client.wait_for_transaction_receipt(tx_hash)
```

### 3.3 Contract Tools

#### ContractDeployer - Contract Deployer

```python
from gravity_e2e.utils.contract_deployer import ContractDeployer, DeploymentOptions

deployer = ContractDeployer(client, account)

# Load contract data from file
contract_data = deployer.load_contract_data("MyContract", contracts_dir)

# Deploy contract
result = await deployer.deploy(
    bytecode=contract_data.bytecode,
    abi=contract_data.abi,
    constructor_args=[arg1, arg2],
    options=DeploymentOptions(gas_limit=3000000)
)

if result.success:
    contract_address = result.contract_address
```

#### ContractUtils - Contract Utilities

```python
from gravity_e2e.utils.contract_utils import ContractUtils

# Load contract data
contract_data = ContractUtils.load_contract_data("SimpleStorage", contracts_dir)

# Encode/decode
encoded = ContractUtils.encode_uint256(12345)
encoded_addr = ContractUtils.encode_address("0x...")
decoded = ContractUtils.decode_uint256("0x...")
decoded_addr = ContractUtils.decode_address("0x...")

# Address validation
normalized = ContractUtils.validate_address(address)
```

### 3.4 Event Tools

#### EventPoller - Event Poller

```python
from gravity_e2e.utils.event_poller import EventPoller, EventFilter

poller = EventPoller(web3)

# Get events
result = await poller.get_events(
    contract=contract,
    event_name="Transfer",
    from_block=100,
    to_block=200
)

print(f"Found {result.total_count} events")
for event in result.events:
    print(event)
```

### 3.5 Retry Tools

#### AsyncRetry - Async Retry

```python
from gravity_e2e.utils.async_retry import AsyncRetry

retry = AsyncRetry(
    max_retries=3,
    base_delay=1.0,
    max_delay=30.0,
    exponential_base=2.0,
    retry_on=(ConnectionError, TimeoutError)
)

result = await retry.execute(async_function, arg1, arg2)
```

### 3.6 Configuration Tools

#### ConfigManager - Configuration Manager

```python
from gravity_e2e.utils.config_manager import load_config, ConfigManager

# Direct loading (recommended)
config = load_config("nodes.json")

# Using ConfigManager (backward compatible)
manager = ConfigManager(config_dir=Path("configs"))
config = manager.load_config("test_accounts.json")
```

### 3.7 Node Management (Self-Managed Tests)

#### NodeManager - Node Manager

```python
from gravity_e2e.core.node_manager import NodeManager

manager = NodeManager()

# Deploy node
success = manager.deploy_node(
    node_name="node1",
    mode="single",  # or "cluster"
    install_dir="/tmp",
    bin_version="quick-release"
)

# Get deployment path
deploy_path = manager.get_node_deploy_path("node1", "/tmp")

# Start node
success = manager.start_node(deploy_path)

# Stop node
success = manager.stop_node(deploy_path, cleanup=True)
```

### 3.8 Specialized Utility Modules

#### Randomness Testing Tools

```python
from gravity_e2e.utils.randomness_utils import (
    RandomDiceHelper,
    deploy_random_dice,
    get_dkg_status_safe,
    get_http_url_from_rpc,
)

# Deploy RandomDice contract
dice = await deploy_random_dice(run_helper, deployer_account)

# Call contract
receipt = await dice.roll_dice(player_account)
result = await dice.get_last_result()  # 1-6
seed = await dice.get_last_seed()
roller, result, seed = await dice.get_latest_roll()
```

#### Epoch Testing Tools

```python
from gravity_e2e.utils.epoch_utils import (
    EpochConfig,
    run_epoch_consistency_test,
    collect_epoch_data,
    validate_epoch_consistency,
)

config = EpochConfig(
    num_epochs=3,
    check_interval=120,
    epoch_timeout=600
)

result = await run_epoch_consistency_test(
    config=config,
    http_url="http://127.0.0.1:1024",
    node_name="node1",
    install_dir="/tmp",
    deploy_node=True
)
```

#### Validator Testing Tools

```python
from gravity_e2e.utils.validator_utils import (
    ValidatorTestConfig,
    ValidatorJoinParams,
    run_validator_add_remove_test,
)

config = ValidatorTestConfig(
    initial_nodes=["node1", "node2"],
    new_validator="node3",
    install_dir="/tmp"
)

result = await run_validator_add_remove_test(
    config=config,
    join_params=ValidatorJoinParams(stake_amount=1000)
)
```

---

## 4. Complete Examples

### 4.1 Regular Test Example

```python
"""
ERC20 Token Test Example
"""
import sys
from pathlib import Path

_current_dir = Path(__file__).resolve().parent
_package_root = _current_dir.parent.parent.parent
if str(_package_root) not in sys.path:
    sys.path.insert(0, str(_package_root))

import asyncio
import logging
import pytest

from gravity_e2e.helpers.test_helpers import RunHelper, TestResult, test_case
from gravity_e2e.utils.contract_deployer import ContractDeployer

LOG = logging.getLogger(__name__)


@test_case
async def test_my_erc20(run_helper: RunHelper, test_result: TestResult):
    """Test ERC20 token functionality"""
    LOG.info("=" * 70)
    LOG.info("Test: My ERC20 Token Test")
    LOG.info("=" * 70)

    # Step 1: Create test accounts
    LOG.info("\n[Step 1] Creating test accounts...")
    deployer = await run_helper.create_test_account("deployer", fund_wei=5 * 10**18)
    recipient = await run_helper.create_test_account("recipient", fund_wei=1 * 10**18)

    LOG.info(f"Deployer: {deployer['address']}")
    LOG.info(f"Recipient: {recipient['address']}")

    # Step 2: Deploy contract
    LOG.info("\n[Step 2] Deploying ERC20 contract...")
    contract_deployer = ContractDeployer(run_helper.client, deployer)

    contracts_dir = Path(__file__).parent.parent.parent.parent / "contracts_data"
    result = await contract_deployer.deploy_from_file(
        "TestToken",
        contracts_dir,
        constructor_args=["MyToken", "MTK", 18, 1000000 * 10**18]
    )

    if not result.success:
        raise RuntimeError(f"Deployment failed: {result.error}")

    contract_address = result.contract_address
    LOG.info(f"Contract deployed at: {contract_address}")

    # Step 3: Verify token info
    LOG.info("\n[Step 3] Verifying token info...")
    # ... Call contract methods to verify ...

    # Step 4: Record results
    test_result.mark_success(
        contract_address=contract_address,
        deployer=deployer["address"],
        recipient=recipient["address"]
    )

    LOG.info("\n" + "=" * 70)
    LOG.info("Test 'My ERC20 Token Test' PASSED!")
    LOG.info("=" * 70)


# Pytest compatibility entry
@pytest.mark.asyncio
async def test_my_erc20_pytest(run_helper: RunHelper, test_result: TestResult):
    """Pytest wrapper"""
    await test_my_erc20(run_helper=run_helper, test_result=test_result)
```

### 4.2 Self-Managed Test Example

```python
"""
Self-Managed Node Test Example
"""
import sys
from pathlib import Path

_current_dir = Path(__file__).resolve().parent
_package_root = _current_dir.parent.parent.parent
if str(_package_root) not in sys.path:
    sys.path.insert(0, str(_package_root))

import asyncio
import logging
import pytest

from gravity_e2e.helpers.test_helpers import RunHelper, TestResult, test_case
from gravity_e2e.core.node_manager import NodeManager
from gravity_e2e.core.client.gravity_http_client import GravityHttpClient

LOG = logging.getLogger(__name__)


@test_case
async def test_my_node_feature(run_helper: RunHelper, test_result: TestResult):
    """Test feature requiring self-managed nodes"""
    LOG.info("=" * 70)
    LOG.info("Test: My Node Feature (Self-Managed)")
    LOG.info("=" * 70)

    node_manager = NodeManager()
    deploy_path = None

    try:
        # Step 1: Deploy node
        LOG.info("\n[Step 1] Deploying node...")
        success = node_manager.deploy_node(
            node_name="node1",
            mode="single",
            install_dir="/tmp",
            bin_version="quick-release"
        )

        if not success:
            raise RuntimeError("Failed to deploy node")

        deploy_path = node_manager.get_node_deploy_path("node1", "/tmp")

        # Step 2: Start node
        LOG.info("\n[Step 2] Starting node...")
        success = node_manager.start_node(deploy_path)

        if not success:
            raise RuntimeError("Failed to start node")

        # Wait for node to be ready
        await asyncio.sleep(10)

        # Step 3: Execute test logic
        LOG.info("\n[Step 3] Running test logic...")
        async with GravityHttpClient("http://127.0.0.1:1024") as http_client:
            # ... Test logic ...
            status = await http_client.get_dkg_status()
            LOG.info(f"DKG Status: {status}")

        # Record results
        test_result.mark_success(
            node_deployed=True,
            dkg_epoch=status.get("epoch", 0)
        )

        LOG.info("\n" + "=" * 70)
        LOG.info("Test 'My Node Feature' PASSED!")
        LOG.info("=" * 70)

    finally:
        # Cleanup: Stop node
        if deploy_path:
            LOG.info("\n[Cleanup] Stopping node...")
            node_manager.stop_node(deploy_path)


# Pytest compatibility entry
@pytest.mark.slow
@pytest.mark.self_managed
@pytest.mark.asyncio
async def test_my_node_feature_pytest(run_helper: RunHelper, test_result: TestResult):
    """Pytest wrapper for self-managed test"""
    await test_my_node_feature(run_helper=run_helper, test_result=test_result)


# Direct execution entry
if __name__ == "__main__":
    logging.basicConfig(
        level=logging.INFO,
        format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
    )

    from gravity_e2e.core.client.gravity_client import GravityClient

    async def run_direct():
        client = GravityClient("http://127.0.0.1:8545", "dummy")
        run_helper = RunHelper(client=client, working_dir="/tmp", faucet_account=None)
        result = await test_my_node_feature(run_helper=run_helper)
        return result

    asyncio.run(run_direct())
```

---

## 5. Best Practices

### 5.1 Test Structure

1. **Clear step division**: Use `[Step N]` format for logging
2. **Detailed logging**: Log key data and state
3. **Exception handling**: Use try-finally to ensure resource cleanup
4. **Result recording**: Use `test_result.mark_success()` to record test data

### 5.2 Naming Conventions

- Test files: `test_<feature>.py`
- Test functions: `test_<feature>_<scenario>`
- Test suites: Use meaningful suite names (e.g., `basic`, `erc20`, `randomness`)

### 5.3 Pytest Markers

```python
@pytest.mark.slow           # Long-running tests
@pytest.mark.self_managed   # Self-managed node tests
@pytest.mark.randomness     # Randomness-related tests
@pytest.mark.epoch          # Epoch-related tests
@pytest.mark.validator      # Validator-related tests
@pytest.mark.cross_chain    # Cross-chain tests
@pytest.mark.asyncio        # Async tests (required)
```

### 5.4 Resource Management

```python
# Use async with for client management
async with GravityHttpClient(url) as client:
    # Automatic cleanup

# Use try-finally for node management
try:
    # Deploy and test
finally:
    # Cleanup nodes
```

### 5.5 Common Mistakes to Avoid

1. **Don't hardcode ports**: Use configuration files or utility functions
2. **Don't forget await**: All async calls must be awaited
3. **Don't ignore exceptions**: Properly handle and log errors
4. **Don't skip cleanup**: Ensure resources are cleaned up after tests

---

## 6. Running Tests

### 6.1 Running via pytest

```bash
cd gravity_e2e

# Run all tests
PYTHONPATH=. pytest gravity_e2e/tests/test_cases/ -v

# Run specific file
PYTHONPATH=. pytest gravity_e2e/tests/test_cases/test_my_feature.py -v

# Run specific test
PYTHONPATH=. pytest gravity_e2e/tests/test_cases/test_my_feature.py::test_my_feature -v

# Filter by marker
PYTHONPATH=. pytest -m "not slow" -v      # Exclude slow tests
PYTHONPATH=. pytest -m randomness -v       # Only randomness tests
PYTHONPATH=. pytest -m self_managed -v     # Only self-managed tests
```

### 6.2 Running via CLI

```bash
cd gravity_e2e

# Run specific test
python -m gravity_e2e.main --test my_feature

# Run test suite
python -m gravity_e2e.main --test-suite my_suite

# List all available tests
python -m gravity_e2e.main --list-tests
```

### 6.3 Direct Execution (Development/Debugging)

```bash
cd gravity_e2e

# Run test file directly
PYTHONPATH=. python gravity_e2e/tests/test_cases/test_my_feature.py
```

---

## Appendix: Directory Structure

```
gravity_e2e/
├── gravity_e2e/
│   ├── core/
│   │   ├── client/
│   │   │   ├── gravity_client.py      # RPC client
│   │   │   └── gravity_http_client.py # HTTP API client
│   │   ├── node_connector.py          # Node connection management
│   │   └── node_manager.py            # Node lifecycle management
│   │
│   ├── helpers/
│   │   ├── test_helpers.py            # RunHelper, @test_case
│   │   └── account_manager.py         # Account management
│   │
│   ├── utils/
│   │   ├── async_retry.py             # Async retry
│   │   ├── config_manager.py          # Configuration loading
│   │   ├── transaction_builder.py     # Transaction building
│   │   ├── contract_deployer.py       # Contract deployment
│   │   ├── contract_utils.py          # Contract utilities
│   │   ├── event_poller.py            # Event polling
│   │   ├── randomness_utils.py         # Randomness testing tools
│   │   ├── epoch_utils.py             # Epoch testing tools
│   │   ├── validator_utils.py         # Validator testing tools
│   │   └── exceptions.py              # Exception definitions
│   │
│   └── tests/
│       ├── test_registry.py           # Test registry
│       ├── conftest.py                # pytest fixtures
│       └── test_cases/
│           ├── __init__.py            # Test registration
│           └── test_*.py              # Test files
│
├── configs/
│   ├── nodes.json                     # Node configuration
│   └── test_accounts.json             # Test accounts
│
└── contracts_data/                    # Compiled contract data
    └── *.json
```

