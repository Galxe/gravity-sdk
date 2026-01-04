"""
Async retry utility with configurable backoff strategies

This module provides a configurable async retry mechanism for handling
transient failures in network operations, API calls, and other async tasks.

Design Notes:
- Supports exponential backoff with jitter to prevent thundering herd
- Configurable retry conditions based on exception types
- Comprehensive logging for debugging retry attempts
- Type hints for better IDE support
- Context manager support for resource cleanup
"""

import asyncio
import logging
import random
import time
from typing import (
    Any,
    Awaitable,
    Callable,
    Optional,
    Tuple,
    Type,
    Union,
    TypeVar,
    List,
    Dict
)
from .exceptions import GravityE2EError, NodeConnectionError, ErrorCodes

T = TypeVar('T')
LOG = logging.getLogger(__name__)


class RetryState:
    """Tracks retry state for a single operation"""

    def __init__(
        self,
        max_retries: int,
        base_delay: float,
        max_delay: float,
        exponential_base: float,
        jitter: bool
    ):
        self.max_retries = max_retries
        self.base_delay = base_delay
        self.max_delay = max_delay
        self.exponential_base = exponential_base
        self.jitter = jitter
        self.attempt = 0
        self.total_delay = 0.0
        self.last_exception: Optional[Exception] = None
        self.start_time = time.time()

    def should_retry(self) -> bool:
        """Check if we should retry based on attempt count"""
        return self.attempt < self.max_retries

    def next_delay(self) -> float:
        """Calculate next delay with exponential backoff and jitter"""
        # Exponential backoff
        delay = self.base_delay * (self.exponential_base ** self.attempt)

        # Apply jitter if enabled
        if self.jitter:
            # Add random jitter up to ±25% of the delay
            jitter_range = delay * 0.25
            delay += random.uniform(-jitter_range, jitter_range)

        # Cap at max_delay
        delay = min(delay, self.max_delay)

        # Accumulate total delay
        self.total_delay += delay
        return delay

    def record_attempt(self, exception: Exception) -> None:
        """Record a retry attempt"""
        self.attempt += 1
        self.last_exception = exception
        # Note: Don't reset total_delay here - it accumulates across all retries

    def get_summary(self) -> Dict[str, Any]:
        """Get retry summary statistics"""
        return {
            "attempts": self.attempt,
            "max_retries": self.max_retries,
            "total_delay": self.total_delay,
            "duration": time.time() - self.start_time,
            "last_error": str(self.last_exception) if self.last_exception else None
        }


class AsyncRetry:
    """
    Configurable async retry mechanism with exponential backoff.

    This class provides both a decorator and a context manager interface
    for retrying async operations that might fail transiently.
    """

    def __init__(
        self,
        max_retries: int = 3,
        base_delay: float = 1.0,
        max_delay: float = 60.0,
        exponential_base: float = 2.0,
        jitter: bool = True,
        retry_on: Optional[Tuple[Type[Exception], ...]] = None,
        stop_on: Optional[Tuple[Type[Exception], ...]] = None,
        on_retry: Optional[Callable[[int, Exception, float], Awaitable[None]]] = None
    ):
        """
        Initialize retry configuration.

        Args:
            max_retries: Maximum number of retry attempts (0 means no retries)
            base_delay: Initial delay in seconds before first retry
            max_delay: Maximum delay limit in seconds
            exponential_base: Multiplier for exponential backoff
            jitter: Add random jitter to prevent synchronized retries
            retry_on: Tuple of exception types that should trigger retries
            stop_on: Tuple of exception types that should stop retrying immediately
            on_retry: Async callback called before each retry (attempt, exception, delay)
        """
        self.max_retries = max_retries
        self.base_delay = base_delay
        self.max_delay = max_delay
        self.exponential_base = exponential_base
        self.jitter = jitter
        self.retry_on = retry_on or (GravityE2EError, NodeConnectionError, TimeoutError)
        self.stop_on = stop_on or ()
        self.on_retry = on_retry

    async def execute(
        self,
        func: Callable[..., Awaitable[T]],
        *args,
        **kwargs
    ) -> T:
        """
        Execute an async function with retry logic.

        Args:
            func: The async function to execute
            *args: Positional arguments to pass to func
            **kwargs: Keyword arguments to pass to func

        Returns:
            Result from successful function execution

        Raises:
            The last exception if all retries are exhausted
        """
        state = RetryState(
            self.max_retries,
            self.base_delay,
            self.max_delay,
            self.exponential_base,
            self.jitter
        )

        last_result = None

        while True:
            try:
                # Execute the function
                result = await func(*args, **kwargs)

                # Log success if we retried
                if state.attempt > 0:
                    LOG.info(
                        f"Operation succeeded after {state.attempt} retries "
                        f"(total delay: {state.total_delay:.2f}s)"
                    )

                return result

            except Exception as e:
                # Check if we should stop immediately
                if type(e) in self.stop_on:
                    LOG.error(f"Stopping retry due to stop_on exception: {type(e).__name__}")
                    raise

                # Check if we should retry this exception
                if not self._should_retry(e, state):
                    raise

                # Record the attempt
                state.record_attempt(e)

                # If this is the last attempt, no delay needed
                if not state.should_retry():
                    LOG.error(
                        f"Operation failed after {state.max_retries} attempts. "
                        f"Last error: {type(e).__name__}: {e}"
                    )
                    raise

                # Calculate delay
                delay = state.next_delay()

                # Log retry attempt
                LOG.warning(
                    f"Attempt {state.attempt + 1}/{state.max_retries} failed: {type(e).__name__}: {e}. "
                    f"Retrying in {delay:.2f}s..."
                )

                # Call on_retry callback if provided
                if self.on_retry:
                    try:
                        await self.on_retry(state.attempt, e, delay)
                    except Exception as callback_error:
                        LOG.error(f"Error in retry callback: {callback_error}")

                # Wait before retry
                await asyncio.sleep(delay)

    def _should_retry(self, exception: Exception, state: RetryState) -> bool:
        """
        Determine if an exception should trigger a retry.

        Args:
            exception: The exception that occurred
            state: Current retry state

        Returns:
            True if the operation should be retried
        """
        # Don't retry if we've exceeded max attempts
        if not state.should_retry():
            return False

        # Check if exception type is in retry list
        return any(isinstance(exception, exc_type) for exc_type in self.retry_on)

    def __call__(
        self,
        func: Callable[..., Awaitable[T]]
    ) -> Callable[..., Awaitable[T]]:
        """
        Decorator interface for retrying functions.

        Usage:
            retry = AsyncRetry(max_retries=3)

            @retry
            async def my_function():
                return await some_async_operation()
        """
        async def wrapper(*args, **kwargs):
            return await self.execute(func, *args, **kwargs)

        return wrapper

    async def __aenter__(self):
        """Async context manager entry"""
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Async context manager exit - just pass, actual retry logic is in execute()"""
        return None


class RetryContext:
    """
    Context manager for performing multiple operations with shared retry state.

    This is useful when you need to perform multiple operations and want
    to share retry state between them, or when you need more control over
    the retry process.
    """

    def __init__(self, retry_config: Optional[AsyncRetry] = None):
        """
        Initialize retry context.

        Args:
            retry_config: Optional AsyncRetry configuration, uses defaults if None
        """
        self.retry = retry_config or AsyncRetry()
        self.operations: List[Tuple[str, Callable[..., Awaitable[Any]], tuple, dict]] = []
        self.results: Dict[str, Any] = {}
        self.errors: Dict[str, Exception] = {}

    def add_operation(
        self,
        name: str,
        func: Callable[..., Awaitable[Any]],
        args: tuple = (),
        kwargs: Optional[dict] = None
    ) -> 'RetryContext':
        """
        Add an operation to the context.

        Args:
            name: Unique identifier for the operation
            func: Async function to execute
            args: Positional arguments for the function
            kwargs: Keyword arguments for the function

        Returns:
            Self for method chaining
        """
        self.operations.append((name, func, args, kwargs or {}))
        return self

    async def execute_all(self) -> Dict[str, Any]:
        """
        Execute all added operations with retry logic.

        Each operation is retried independently based on the retry configuration.
        Failed operations are recorded but don't stop other operations from executing.

        Returns:
            Dictionary mapping operation names to their results

        Raises:
            AggregateError if no operations succeeded
        """
        if not self.operations:
            return {}

        successful_count = 0

        for name, func, args, kwargs in self.operations:
            try:
                LOG.info(f"Executing operation: {name}")
                result = await self.retry.execute(func, *args, **kwargs)
                self.results[name] = result
                successful_count += 1
                LOG.info(f"Operation {name} succeeded")
            except Exception as e:
                self.errors[name] = e
                LOG.error(f"Operation {name} failed: {e}")

        if successful_count == 0:
            # Create an aggregated error
            error_messages = [f"{name}: {err}" for name, err in self.errors.items()]
            raise GravityE2EError(
                f"All operations failed:\n" + "\n".join(error_messages),
                code=5003  # TEST_TIMEOUT as a generic failure code
            )

        LOG.info(f"Completed {successful_count}/{len(self.operations)} operations successfully")
        return self.results

    async def execute_all_serial(self) -> Dict[str, Any]:
        """
        Execute all operations in serial order, stopping on first failure.

        Returns:
            Dictionary mapping operation names to their results
        """
        if not self.operations:
            return {}

        results = {}

        for name, func, args, kwargs in self.operations:
            try:
                LOG.info(f"Executing operation: {name} (serial)")
                result = await self.retry.execute(func, *args, **kwargs)
                results[name] = result
                LOG.info(f"Operation {name} succeeded")
            except Exception as e:
                LOG.error(f"Serial execution stopped at {name}: {e}")
                results.update(self.results)  # Include any previous successful results
                raise

        return results

    async def __aenter__(self):
        """Async context manager entry"""
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Async context manager exit"""
        return None


# Convenience functions for common retry patterns
async def retry_with_backoff(
    func: Callable[..., Awaitable[T]],
    *args,
    max_retries: int = 3,
    base_delay: float = 1.0,
    max_delay: float = 60.0,
    **kwargs
) -> T:
    """
    Simple retry with exponential backoff.

    Args:
        func: Async function to retry
        max_retries: Maximum retry attempts
        base_delay: Initial delay before first retry
        max_delay: Maximum delay limit
        *args: Function arguments
        **kwargs: Function keyword arguments

    Returns:
        Result from successful function execution
    """
    retry = AsyncRetry(
        max_retries=max_retries,
        base_delay=base_delay,
        max_delay=max_delay
    )
    return await retry.execute(func, *args, **kwargs)


async def retry_connection_errors(
    func: Callable[..., Awaitable[T]],
    *args,
    max_retries: int = 3,
    **kwargs
) -> T:
    """
    Retry specifically for connection-related errors.

    Args:
        func: Async function to retry
        max_retries: Maximum retry attempts
        *args: Function arguments
        **kwargs: Function keyword arguments

    Returns:
        Result from successful function execution
    """
    retry = AsyncRetry(
        max_retries=max_retries,
        base_delay=1.0,
        max_delay=30.0,
        retry_on=(ConnectionError, NodeConnectionError, TimeoutError, OSError)
    )
    return await retry.execute(func, *args, **kwargs)


# Default retry instances for common use cases
default_retry = AsyncRetry()
fast_retry = AsyncRetry(max_retries=2, base_delay=0.5, max_delay=5.0)
slow_retry = AsyncRetry(max_retries=5, base_delay=2.0, max_delay=120.0)
network_retry = AsyncRetry(
    max_retries=3,
    base_delay=1.0,
    max_delay=30.0,
    retry_on=(NodeConnectionError,)
)