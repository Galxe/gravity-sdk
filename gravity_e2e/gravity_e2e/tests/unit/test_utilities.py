"""
Unit tests for the new utility modules

This module provides unit tests for the utilities created in Phase 2
to ensure they work correctly before refactoring the main tests.
"""

import pytest
import asyncio
import json
from unittest.mock import Mock, AsyncMock, patch, MagicMock
from pathlib import Path
from datetime import datetime
from web3 import Web3
from eth_account import Account

from gravity_e2e.utils.exceptions import (
    GravityE2EError,
    TransactionError,
    ContractError,
    ConfigurationError,
    ErrorCodes
)
from gravity_e2e.utils.async_retry import AsyncRetry, RetryState
from gravity_e2e.utils.config_manager import ConfigManager
from gravity_e2e.utils.transaction_builder import TransactionBuilder, TransactionOptions
from gravity_e2e.utils.event_poller import EventPoller, EventFilter
from gravity_e2e.utils.contract_deployer import ContractDeployer, DeploymentResult


class TestExceptions:
    """Test custom exceptions"""

    def test_base_exception(self):
        """Test base GravityE2EError"""
        error = GravityE2EError("Test error", code=1001)

        assert error.message == "Test error"
        assert error.code == 1001
        assert str(error) == "[1001] Test error"

        error_dict = error.to_dict()
        assert error_dict["error"] == "GravityE2EError"
        assert error_dict["message"] == "Test error"
        assert error_dict["code"] == 1001

    def test_transaction_error(self):
        """Test TransactionError with details"""
        error = TransactionError(
            "Transfer failed",
            tx_hash="0x123",
            from_address="0xabc",
            to_address="0xdef",
            value=1000
        )

        assert error.details["tx_hash"] == "0x123"
        assert error.details["from_address"] == "0xabc"
        assert error.details["to_address"] == "0xdef"
        assert error.details["value"] == 1000

    def test_configuration_error(self):
        """Test ConfigurationError with file info"""
        error = ConfigurationError(
            "Invalid config",
            config_file="/path/to/config.json",
            field="gas_limit"
        )

        assert error.details["config_file"] == "/path/to/config.json"
        assert error.details["field"] == "gas_limit"


class TestAsyncRetry:
    """Test async retry functionality"""

    @pytest.mark.asyncio
    async def test_successful_retry(self):
        """Test successful operation without retry"""
        mock_func = AsyncMock(return_value="success")

        retry = AsyncRetry(max_retries=3)
        result = await retry.execute(mock_func)

        assert result == "success"
        assert mock_func.call_count == 1

    @pytest.mark.asyncio
    async def test_retry_on_failure(self):
        """Test retry on transient failure"""
        mock_func = AsyncMock(side_effect=[ConnectionError("fail"), "success"])

        retry = AsyncRetry(max_retries=3, base_delay=0.01, retry_on=(ConnectionError,))
        result = await retry.execute(mock_func)

        assert result == "success"
        assert mock_func.call_count == 2

    @pytest.mark.asyncio
    async def test_max_retries_exceeded(self):
        """Test failure after max retries"""
        mock_func = AsyncMock(side_effect=ConnectionError("always fails"))

        retry = AsyncRetry(max_retries=2, base_delay=0.01, retry_on=(ConnectionError,))

        with pytest.raises(ConnectionError):
            await retry.execute(mock_func)

        assert mock_func.call_count == 2  # Initial + 1 retry (max_retries=2 means 2 total attempts)

    def test_retry_state(self):
        """Test RetryState functionality"""
        state = RetryState(
            max_retries=5,
            base_delay=1.0,
            max_delay=60.0,
            exponential_base=2.0,
            jitter=False
        )

        assert state.should_retry() == True
        assert state.next_delay() == 1.0

        state.record_attempt(Exception("test"))
        assert state.attempt == 1
        assert state.next_delay() == 2.0  # Exponential backoff

        # Test max delay cap
        state.base_delay = 100
        assert state.next_delay() == 60.0  # Capped at max_delay


class TestConfigManager:
    """Test configuration management"""

    @pytest.fixture
    def temp_config_dir(self, tmp_path):
        """Create temporary config directory"""
        config_dir = tmp_path / "configs"
        config_dir.mkdir()
        return config_dir

    @pytest.fixture
    def config_manager(self, temp_config_dir):
        """Create ConfigManager instance"""
        return ConfigManager(config_dir=temp_config_dir)

    def test_load_config(self, config_manager, temp_config_dir):
        """Test loading configuration"""
        config = {
            "network": {
                "chain_id": 12345,
                "name": "testnet"
            },
            "nodes": {
                "node1": {
                    "host": "localhost",
                    "rpc_port": 8545
                }
            }
        }

        # Create config file
        config_file = temp_config_dir / "test.json"
        with open(config_file, 'w') as f:
            json.dump(config, f)

        # Load config
        loaded = config_manager.load_config("test.json")
        assert loaded == config

    def test_load_missing_config(self, config_manager):
        """Test loading missing configuration file"""
        with pytest.raises(ConfigurationError):
            config_manager.load_config("nonexistent.json")


class TestTransactionBuilder:
    """Test transaction building"""

    @pytest.fixture
    def mock_web3(self):
        """Create mock Web3 instance"""
        web3 = Mock()
        web3.eth.chain_id = 12345
        web3.eth.gas_price = 20000000000
        web3.eth.get_transaction_count = Mock(return_value=0)
        web3.eth.estimate_gas = Mock(return_value=21000)
        return web3

    @pytest.fixture
    def test_account(self):
        """Create test account"""
        return Account.create()

    def test_build_transaction(self, mock_web3, test_account):
        """Test building a transaction"""
        builder = TransactionBuilder(mock_web3, test_account)

        # Mock get_nonce to use the async method
        with patch.object(builder, 'get_nonce', return_value=0):
            asyncio.run(
                self._test_build_transaction_async(builder, test_account)
            )

    async def _test_build_transaction_async(self, builder, test_account):
        """Async part of build_transaction test"""
        tx = await builder.build_transaction(
            to="0x742d35Cc6634C0532925a3b8D4C9db96C4b4Db45",
            value=1000000000000000000  # 1 ETH
        )

        assert tx['to'] == "0x742d35Cc6634C0532925a3b8D4C9db96C4b4Db45"
        assert tx['value'] == 1000000000000000000
        assert tx['nonce'] == 0
        assert 'chainId' in tx
        assert 'gas' in tx

    def test_transaction_options(self, mock_web3, test_account):
        """Test transaction with custom options"""
        options = TransactionOptions(
            gas_limit=50000,
            gas_price=30000000000,
            value=500000000000000000
        )

        builder = TransactionBuilder(mock_web3, test_account, default_options=options)

        with patch.object(builder, 'get_nonce', return_value=1):
            asyncio.run(
                self._test_transaction_options_async(builder)
            )

    async def _test_transaction_options_async(self, builder):
        """Async part of transaction options test"""
        tx = await builder.build_transaction(
            to="0x742d35Cc6634C0532925a3b8D4C9db96C4b4Db45"
        )

        assert tx['gas'] == 50000
        assert tx['gasPrice'] == 30000000000
        assert tx['value'] == 500000000000000000


class TestEventPoller:
    """Test event polling"""

    @pytest.fixture
    def mock_web3(self):
        """Create mock Web3 instance"""
        web3 = Mock()
        web3.eth.get_logs = AsyncMock(return_value=[])
        web3.eth.contract = Mock()
        return web3

    def test_get_events(self, mock_web3):
        """Test getting events"""
        # Mock contract
        mock_contract = Mock()
        mock_contract.address = "0x123"
        mock_contract.abi = [
            {"type": "event", "name": "Transfer", "inputs": []}
        ]

        poller = EventPoller(mock_web3)

        with patch.object(poller, '_get_event_abi', return_value={"name": "Transfer"}):
            asyncio.run(
                self._test_get_events_async(poller, mock_contract)
            )

    async def _test_get_events_async(self, poller, mock_contract):
        """Async part of get_events test"""
        result = await poller.get_events(
            contract=mock_contract,
            event_name="Transfer",
            from_block=100,
            to_block=200
        )

        assert result.total_count == 0
        assert result.events == []
        assert result.start_block == 100
        assert result.end_block == 200


class TestContractDeployer:
    """Test contract deployment"""

    @pytest.fixture
    def mock_web3(self):
        """Create mock Web3 instance"""
        web3 = Mock()
        web3.eth.chain_id = AsyncMock(return_value=12345)
        web3.eth.get_code = AsyncMock(return_value="0x123456")
        web3.eth.contract = Mock()
        return web3

    @pytest.fixture
    def test_account(self):
        """Create test account"""
        return Account.create()

    @pytest.fixture
    def temp_contracts_dir(self, tmp_path):
        """Create temporary contracts directory"""
        contracts_dir = tmp_path / "contracts_data"
        contracts_dir.mkdir()
        return contracts_dir

    def test_load_contract_data(self, temp_contracts_dir):
        """Test loading contract data"""
        # Create contract file
        contract_data = {
            "bytecode": "0x123456",
            "abi": [{"type": "function", "name": "foo"}]
        }

        contract_file = temp_contracts_dir / "TestContract.json"
        with open(contract_file, 'w') as f:
            json.dump(contract_data, f)

        # Load it
        deployer = ContractDeployer(Mock(), Account.create())
        loaded = deployer.load_contract_data("TestContract", temp_contracts_dir)

        assert loaded.bytecode == "0x123456"
        assert loaded.abi == [{"type": "function", "name": "foo"}]

    def test_deployment_result(self):
        """Test DeploymentResult creation"""
        result = DeploymentResult(
            success=True,
            contract_address="0xabc123",
            transaction_hash="0xdef456",
            gas_used=21000
        )

        assert result.success == True
        assert result.contract_address == "0xabc123"
        assert result.transaction_hash == "0xdef456"
        assert result.gas_used == 21000

    def test_missing_contract_file(self, temp_contracts_dir):
        """Test handling missing contract file"""
        deployer = ContractDeployer(Mock(), Account.create())

        with pytest.raises(ContractError) as exc_info:
            deployer.load_contract_data("MissingContract", temp_contracts_dir)

        assert "not found" in str(exc_info.value)


# Test integration between utilities
class TestUtilityIntegration:
    """Test integration between different utilities"""

    def test_exception_with_retry(self):
        """Test using custom exceptions with retry"""
        retry = AsyncRetry(
            max_retries=2,
            retry_on=(TransactionError,)
        )

        async def failing_func():
            raise TransactionError("Database connection lost")

        with pytest.raises(TransactionError):
            asyncio.run(retry.execute(failing_func))