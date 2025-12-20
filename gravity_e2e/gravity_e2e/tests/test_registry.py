"""
Test Registry for Gravity E2E Test Framework

This module provides a centralized registry for test cases, enabling automatic
test discovery and reducing boilerplate code in main.py.

Usage:
    from .test_registry import register_test, get_test, list_tests

    @register_test("basic_transfer")
    async def test_eth_transfer(run_helper, test_result):
        ...

    # Or register with suite
    @register_test("randomness_smoke", suite="randomness")
    async def test_randomness_smoke(run_helper, test_result):
        ...
"""

import logging
from typing import Callable, Dict, List, Optional, Set, Awaitable, Any

LOG = logging.getLogger(__name__)


class TestRegistry:
    """
    Central registry for all test cases.
    
    Provides registration, lookup, and categorization of tests.
    """
    
    def __init__(self):
        self._tests: Dict[str, Callable] = {}
        self._suites: Dict[str, Set[str]] = {}
        self._self_managed_tests: Set[str] = set()
    
    def register(
        self,
        name: str,
        func: Callable,
        suite: Optional[str] = None,
        self_managed: bool = False
    ) -> None:
        """
        Register a test function.
        
        Args:
            name: Unique test name (e.g., "basic_transfer")
            func: The async test function
            suite: Optional suite name for grouping tests
            self_managed: If True, test manages its own nodes
        """
        if name in self._tests:
            LOG.warning(f"Test '{name}' already registered, overwriting")
        
        self._tests[name] = func
        
        if suite:
            if suite not in self._suites:
                self._suites[suite] = set()
            self._suites[suite].add(name)
        
        if self_managed:
            self._self_managed_tests.add(name)
        
        LOG.debug(f"Registered test: {name}" + (f" (suite: {suite})" if suite else ""))
    
    def get(self, name: str) -> Optional[Callable]:
        """Get a test function by name."""
        return self._tests.get(name)
    
    def get_suite_tests(self, suite_name: str) -> List[str]:
        """Get all test names in a suite."""
        return list(self._suites.get(suite_name, set()))
    
    def list_all(self) -> List[str]:
        """List all registered test names."""
        return list(self._tests.keys())
    
    def list_suites(self) -> List[str]:
        """List all registered suite names."""
        return list(self._suites.keys())
    
    def is_self_managed(self, name: str) -> bool:
        """Check if a test manages its own nodes."""
        return name in self._self_managed_tests
    
    def get_available_choices(self) -> List[str]:
        """Get all available choices for --test-suite argument."""
        choices = ["all"]
        choices.extend(sorted(self._suites.keys()))
        choices.extend(sorted(self._tests.keys()))
        return list(dict.fromkeys(choices))  # Remove duplicates while preserving order


# Global registry instance
_registry = TestRegistry()


def register_test(
    name: str,
    suite: Optional[str] = None,
    self_managed: bool = False
) -> Callable:
    """
    Decorator to register a test function.
    
    Args:
        name: Unique test name
        suite: Optional suite name for grouping
        self_managed: If True, test manages its own nodes
    
    Returns:
        Decorator function
    
    Example:
        @register_test("basic_transfer")
        async def test_eth_transfer(run_helper, test_result):
            ...
        
        @register_test("randomness_smoke", suite="randomness")
        async def test_randomness_smoke(run_helper, test_result):
            ...
    """
    def decorator(func: Callable) -> Callable:
        _registry.register(name, func, suite, self_managed)
        return func
    return decorator


def get_test(name: str) -> Optional[Callable]:
    """Get a test function by name."""
    return _registry.get(name)


def get_suite_tests(suite_name: str) -> List[str]:
    """Get all test names in a suite."""
    return _registry.get_suite_tests(suite_name)


def list_tests() -> List[str]:
    """List all registered test names."""
    return _registry.list_all()


def list_suites() -> List[str]:
    """List all registered suite names."""
    return _registry.list_suites()


def is_self_managed(name: str) -> bool:
    """Check if a test manages its own nodes."""
    return _registry.is_self_managed(name)


def get_available_choices() -> List[str]:
    """Get all available choices for --test-suite argument."""
    return _registry.get_available_choices()


def get_tests_to_run(test_suite: str) -> List[str]:
    """
    Get the list of test names to run based on test_suite argument.
    
    Args:
        test_suite: The test suite name or "all"
    
    Returns:
        List of test names to run
    """
    if test_suite == "all":
        # Return default tests (not self-managed)
        return [name for name in _registry.list_all() 
                if not _registry.is_self_managed(name)]
    
    # Check if it's a suite name
    suite_tests = _registry.get_suite_tests(test_suite)
    if suite_tests:
        return suite_tests
    
    # Check if it's a single test name
    if _registry.get(test_suite):
        return [test_suite]
    
    return []

