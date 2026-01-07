"""
Test cases package initialization.

This module registers all test cases with the test registry for automatic
discovery and execution through the CLI.

Note: For pytest compatibility, imports use absolute paths.
"""

import sys
from pathlib import Path

# Add package to path for absolute imports
_current_dir = Path(__file__).resolve().parent
_package_root = _current_dir.parent.parent.parent
if str(_package_root) not in sys.path:
    sys.path.insert(0, str(_package_root))

from gravity_e2e.tests.test_registry import register_test

# Import test functions using absolute imports
# Basic transfer tests
from gravity_e2e.tests.test_cases.test_basic_transfer import (
    test_eth_transfer,
    test_multiple_transfers,
    test_transfer_with_insufficient_funds,
)

# Contract tests
from gravity_e2e.tests.test_cases.test_contract_deploy import (
    test_simple_storage_deploy,
    test_contract_with_constructor,
    test_contract_deployment_with_retry,
)

# ERC20 tests
from gravity_e2e.tests.test_cases.test_erc20 import (
    test_erc20_deploy_and_transfer,
    test_erc20_batch_transfers,
    test_erc20_edge_cases,
)

# Cross-chain tests
from gravity_e2e.tests.test_cases.test_cross_chain_deposit import test_cross_chain_gravity_deposit

# Randomness tests
from gravity_e2e.tests.test_cases.test_randomness_basic import (
    test_randomness_basic_consumption,
    test_randomness_correctness,
)

from gravity_e2e.tests.test_cases.test_randomness_advanced import (
    test_randomness_smoke,
    test_randomness_reconfiguration,
    test_randomness_multi_contract,
    test_randomness_api_completeness,
    test_randomness_stress,
)

# Epoch consistency tests (both scenarios now in one file)
from gravity_e2e.tests.test_cases.test_epoch_consistency import (
    test_epoch_consistency,
    test_epoch_consistency_extended,
    test_epoch_consistency_slow,
    test_epoch_consistency_fast,
)

# Validator tests (both scenarios now in one file)
from gravity_e2e.tests.test_cases.test_validator_add_remove import (
    test_validator_add_remove,
    test_validator_add_remove_immediate,
    test_validator_add_remove_delayed,
)

# Cluster Operations tests (Self-managed)
from gravity_e2e.tests.test_cases.test_cluster_ops import (
    test_cluster_lifecycle,
    test_fault_tolerance,
)

# Epoch switch tests
from gravity_e2e.tests.test_cases.test_epoch_switch import (
    test_epoch_switch,
)


# Register all tests with the registry
# Basic transfer tests
register_test("basic_transfer", suite="basic")(test_eth_transfer)
register_test("multiple_transfers", suite="basic")(test_multiple_transfers)
register_test("insufficient_funds", suite="basic")(test_transfer_with_insufficient_funds)

# Contract tests
register_test("contract", suite="contract")(test_simple_storage_deploy)
register_test("contract_constructor", suite="contract")(test_contract_with_constructor)
register_test("contract_retry", suite="contract")(test_contract_deployment_with_retry)

# ERC20 tests
register_test("erc20", suite="erc20")(test_erc20_deploy_and_transfer)
register_test("erc20_batch", suite="erc20")(test_erc20_batch_transfers)
register_test("erc20_edge_cases", suite="erc20")(test_erc20_edge_cases)

# Cross-chain tests
register_test("cross_chain_deposit", suite="cross_chain")(test_cross_chain_gravity_deposit)

# Randomness tests
register_test("randomness_basic", suite="randomness")(test_randomness_basic_consumption)
register_test("randomness_correctness", suite="randomness")(test_randomness_correctness)
register_test("randomness_smoke", suite="randomness")(test_randomness_smoke)
register_test("randomness_reconfiguration", suite="randomness")(test_randomness_reconfiguration)
register_test("randomness_multi_contract", suite="randomness")(test_randomness_multi_contract)
register_test("randomness_api_completeness", suite="randomness")(test_randomness_api_completeness)
register_test("randomness_stress", suite="randomness")(test_randomness_stress)

# Epoch consistency tests (self-managed)
register_test("epoch_consistency", suite="epoch", self_managed=True)(test_epoch_consistency)
register_test("epoch_consistency_extended", suite="epoch", self_managed=True)(test_epoch_consistency_extended)

# Validator tests (self-managed)
register_test("validator_add_remove", suite="validator", self_managed=True)(test_validator_add_remove)
register_test("validator_add_remove_delayed", suite="validator", self_managed=True)(test_validator_add_remove_delayed)

# Cluster Ops tests (self-managed)
register_test("cluster_lifecycle", suite="cluster_ops", self_managed=True)(test_cluster_lifecycle)
register_test("cluster_fault_tolerance", suite="cluster_ops", self_managed=True)(test_fault_tolerance)
# Epoch switch tests (self-managed)
register_test("epoch_switch", suite="epoch_switch", self_managed=True)(test_epoch_switch)


# Define default test list for "all" suite
DEFAULT_TESTS = [
    "basic_transfer",
    "contract",
    "erc20",
    "cross_chain_deposit",
]

# Define which tests are included in "all" by default
__all__ = [
    # Test functions
    'test_eth_transfer',
    'test_multiple_transfers',
    'test_transfer_with_insufficient_funds',
    'test_simple_storage_deploy',
    'test_contract_with_constructor',
    'test_contract_deployment_with_retry',
    'test_erc20_deploy_and_transfer',
    'test_erc20_batch_transfers',
    'test_erc20_edge_cases',
    'test_cross_chain_gravity_deposit',
    'test_randomness_basic_consumption',
    'test_randomness_correctness',
    'test_randomness_smoke',
    'test_randomness_reconfiguration',
    'test_randomness_multi_contract',
    'test_randomness_api_completeness',
    'test_randomness_stress',
    'test_epoch_consistency',
    'test_epoch_consistency_extended',
    'test_epoch_consistency_slow',
    'test_epoch_consistency_fast',
    'test_validator_add_remove',
    'test_validator_add_remove_immediate',
    'test_validator_add_remove_immediate',
    'test_validator_add_remove_delayed',
    'test_cluster_lifecycle',
    'test_fault_tolerance',
    'test_epoch_switch',
    # Default test list
    'DEFAULT_TESTS',
]
