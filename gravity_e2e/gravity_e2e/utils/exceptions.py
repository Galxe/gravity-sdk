"""
Centralized exception definitions for Gravity E2E Test Framework

This module defines all custom exceptions used throughout the framework,
providing consistent error handling and clear error messages.

Design Notes:
- All exceptions inherit from GravityE2EError for easy catching
- Each exception includes error codes for programmatic handling
- Error codes follow the pattern: CATEGORY-SPECIFIC-NUMBER
- Use these exceptions instead of generic Exception types
"""

from typing import Dict, Any, Optional


class ErrorCodes:
    """Standard error codes for the framework"""

    # Configuration errors (1000-1099)
    INVALID_CONFIG = 1001
    MISSING_REQUIRED_FIELD = 1002
    CONFIG_VALIDATION_FAILED = 1003
    CONFIG_FILE_NOT_FOUND = 1004

    # Transaction errors (2000-2099)
    TRANSACTION_FAILED = 2001
    INSUFFICIENT_FUNDS = 2002
    GAS_LIMIT_EXCEEDED = 2003
    NONCE_TOO_LOW = 2004
    NONCE_TOO_HIGH = 2005
    TRANSACTION_UNDERPRICED = 2006
    INVALID_SIGNATURE = 2007

    # Contract errors (3000-3099)
    CONTRACT_DEPLOYMENT_FAILED = 3001
    CONTRACT_CALL_FAILED = 3002
    INVALID_ABI = 3003
    CONTRACT_NOT_FOUND = 3004
    CONTRACT_EXECUTION_FAILED = 3005
    CONTRACT_REVERTED = 3006

    # Network errors (4000-4099)
    NODE_CONNECTION_FAILED = 4001
    RPC_TIMEOUT = 4002
    INVALID_RESPONSE = 4003
    NETWORK_UNREACHABLE = 4004
    RATE_LIMITED = 4005

    # Test errors (5000-5099)
    TEST_SETUP_FAILED = 5001
    TEST_ASSERTION_FAILED = 5002
    TEST_TIMEOUT = 5003
    TEST_INITIALIZATION_FAILED = 5004
    TEST_CLEANUP_FAILED = 5005

    # Account errors (6000-6099)
    ACCOUNT_CREATION_FAILED = 6001
    ACCOUNT_FUNDING_FAILED = 6002
    INVALID_PRIVATE_KEY = 6003
    ACCOUNT_NOT_FOUND = 6004

    # Event errors (7000-7099)
    EVENT_PARSING_FAILED = 7001
    EVENT_NOT_FOUND = 7002
    INVALID_EVENT_FILTER = 7003
    EVENT_LOG_MALFORMED = 7004


class GravityE2EError(Exception):
    """
    Base exception for the Gravity E2E framework.

    All framework exceptions should inherit from this class
    to enable easy error handling and categorization.
    """

    def __init__(
        self,
        message: str,
        code: int = ErrorCodes.TRANSACTION_FAILED,
        details: Optional[Dict[str, Any]] = None,
        cause: Optional[Exception] = None
    ):
        """
        Initialize the base exception.

        Args:
            message: Human-readable error message
            code: Error code from ErrorCodes class
            details: Additional error context as dictionary
            cause: The original exception that caused this error
        """
        super().__init__(message)
        self.message = message
        self.code = code
        self.details = details or {}
        self.cause = cause

    def to_dict(self) -> Dict[str, Any]:
        """Convert exception to dictionary for JSON serialization."""
        return {
            "error": self.__class__.__name__,
            "message": self.message,
            "code": self.code,
            "details": self.details,
            "cause": str(self.cause) if self.cause else None
        }

    def __str__(self) -> str:
        """String representation with error code."""
        return f"[{self.code}] {self.message}"


# Legacy exceptions for backward compatibility
class GravityError(GravityE2EError):
    """Base exception class for Gravity E2E framework (legacy)"""
    pass


class APIError(GravityE2EError):
    """API call error (legacy)"""
    def __init__(self, message: str, code: int = None):
        super().__init__(message, code or ErrorCodes.INVALID_RESPONSE)
        self.code = code


class GravityConnectionError(GravityE2EError):
    """Connection error"""
    def __init__(self, message: str):
        super().__init__(message, ErrorCodes.NODE_CONNECTION_FAILED)


# Alias for backward compatibility
ConnectionError = GravityConnectionError


class NodeError(GravityE2EError):
    """Node-related error (legacy)"""
    def __init__(self, message: str):
        super().__init__(message, ErrorCodes.NODE_CONNECTION_FAILED)


# New enhanced exceptions
class ConfigurationError(GravityE2EError):
    """Raised when configuration is invalid or missing."""

    def __init__(
        self,
        message: str,
        code: int = ErrorCodes.INVALID_CONFIG,
        config_file: Optional[str] = None,
        field: Optional[str] = None
    ):
        super().__init__(message, code)
        if config_file:
            self.details["config_file"] = config_file
        if field:
            self.details["field"] = field


class TransactionError(GravityE2EError):
    """Raised when a blockchain transaction fails."""

    def __init__(
        self,
        message: str,
        code: int = ErrorCodes.TRANSACTION_FAILED,
        tx_hash: Optional[str] = None,
        from_address: Optional[str] = None,
        to_address: Optional[str] = None,
        value: Optional[int] = None,
        gas_limit: Optional[int] = None,
        cause: Optional[Exception] = None
    ):
        super().__init__(message, code, cause=cause)
        if tx_hash:
            self.details["tx_hash"] = tx_hash
        if from_address:
            self.details["from_address"] = from_address
        if to_address:
            self.details["to_address"] = to_address
        if value is not None:
            self.details["value"] = value
        if gas_limit is not None:
            self.details["gas_limit"] = gas_limit


class ContractError(GravityE2EError):
    """Raised when contract interaction fails."""

    def __init__(
        self,
        message: str,
        code: int = ErrorCodes.CONTRACT_EXECUTION_FAILED,
        contract_address: Optional[str] = None,
        contract_name: Optional[str] = None,
        method: Optional[str] = None,
        revert_reason: Optional[str] = None
    ):
        super().__init__(message, code)
        if contract_address:
            self.details["contract_address"] = contract_address
        if contract_name:
            self.details["contract_name"] = contract_name
        if method:
            self.details["method"] = method
        if revert_reason:
            self.details["revert_reason"] = revert_reason


class NodeConnectionError(GravityE2EError):
    """Raised when connection to a Gravity node fails."""

    def __init__(
        self,
        message: str,
        code: int = ErrorCodes.NODE_CONNECTION_FAILED,
        node_url: Optional[str] = None,
        node_id: Optional[str] = None,
        timeout: Optional[float] = None
    ):
        super().__init__(message, code)
        if node_url:
            self.details["node_url"] = node_url
        if node_id:
            self.details["node_id"] = node_id
        if timeout is not None:
            self.details["timeout"] = timeout


class AccountError(GravityE2EError):
    """Raised when account operations fail."""

    def __init__(
        self,
        message: str,
        code: int = ErrorCodes.ACCOUNT_CREATION_FAILED,
        address: Optional[str] = None,
        operation: Optional[str] = None
    ):
        super().__init__(message, code)
        if address:
            self.details["address"] = address
        if operation:
            self.details["operation"] = operation


class TestError(GravityE2EError):
    """Raised when test execution fails."""

    def __init__(
        self,
        message: str,
        code: int = ErrorCodes.TEST_ASSERTION_FAILED,
        test_name: Optional[str] = None,
        test_file: Optional[str] = None,
        assertion: Optional[str] = None
    ):
        super().__init__(message, code)
        if test_name:
            self.details["test_name"] = test_name
        if test_file:
            self.details["test_file"] = test_file
        if assertion:
            self.details["assertion"] = assertion


class EventError(GravityE2EError):
    """Raised when event processing fails."""

    def __init__(
        self,
        message: str,
        code: int = ErrorCodes.EVENT_PARSING_FAILED,
        event_name: Optional[str] = None,
        contract_address: Optional[str] = None,
        block_number: Optional[int] = None
    ):
        super().__init__(message, code)
        if event_name:
            self.details["event_name"] = event_name
        if contract_address:
            self.details["contract_address"] = contract_address
        if block_number is not None:
            self.details["block_number"] = block_number


# Convenience functions for creating common exceptions
def wrap_exception(
    original_exception: Exception,
    context: str,
    error_type: type = GravityE2EError,
    code: Optional[int] = None
) -> GravityE2EError:
    """
    Wrap a generic exception in a framework exception with context.

    Args:
        original_exception: The original exception to wrap
        context: Description of what was happening when error occurred
        error_type: The type of GravityE2EError to create
        code: Optional error code (defaults to error_type's default)

    Returns:
        A GravityE2EError with additional context
    """
    if code is None:
        error = error_type(
            f"{context}: {str(original_exception)}",
            cause=original_exception
        )
    else:
        error = error_type(
            f"{context}: {str(original_exception)}",
            code=code,
            cause=original_exception
        )
    return error


# Async error handling utilities
async def handle_async_error(
    coro,
    context: str,
    error_map: Optional[Dict[type, type]] = None
) -> Any:
    """
    Execute a coroutine and convert exceptions to framework exceptions.

    Args:
        coro: The coroutine to execute
        context: Description of the operation
        error_map: Mapping of exception types to framework exception types

    Returns:
        The result of the coroutine

    Raises:
        GravityE2EError: Wrapped exception with context
    """
    try:
        return await coro
    except Exception as e:
        # Check if we have a specific mapping
        if error_map and type(e) in error_map:
            framework_error = error_map[type(e)]
            raise framework_error(
                f"{context}: {str(e)}",
                cause=e
            ) from e
        # Otherwise wrap in generic error
        raise wrap_exception(e, context) from e