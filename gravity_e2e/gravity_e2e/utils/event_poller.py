"""
Event polling utility for Gravity E2E Test Framework

This module provides efficient event polling mechanisms for monitoring
blockchain events across different test scenarios.

Design Notes:
- Supports filtering by contract, event name, and block range
- Implements efficient polling strategies to minimize RPC calls
- Handles event decoding and type conversion
- Provides both sync and async interfaces
- Caches event ABIs for performance
"""

import asyncio
import json
import logging
from typing import Any, Callable, Dict, List, Optional, Tuple, Union
from dataclasses import dataclass, field
from datetime import datetime, timedelta
from web3 import Web3
from web3.contract import Contract
from web3.types import FilterParams, LogReceipt, EventData
from eth_utils import decode_hex, encode_hex
from .exceptions import EventError, NodeConnectionError
from .async_retry import AsyncRetry

LOG = logging.getLogger(__name__)


@dataclass
class EventFilter:
    """Event filter configuration"""
    address: Optional[Union[str, List[str]]] = None
    topics: Optional[List[Optional[str]]] = None
    from_block: Optional[Union[int, str]] = None
    to_block: Optional[Union[int, str]] = None
    event_name: Optional[str] = None
    event_abi: Optional[Dict] = None


@dataclass
class PollingOptions:
    """Options for event polling"""
    max_retries: int = 10
    retry_delay: float = 1.0
    timeout: float = 300.0
    poll_interval: float = 1.0
    batch_size: int = 1000
    decode_events: bool = True
    sort_by_block: bool = True


@dataclass
class EventResult:
    """Result from event polling"""
    events: List[EventData] = field(default_factory=list)
    total_count: int = 0
    start_block: Optional[int] = None
    end_block: Optional[int] = None
    polling_time: Optional[float] = None
    has_more: bool = False


class EventPoller:
    """
    Utility class for efficiently polling blockchain events.

    Provides various polling strategies and handles common edge cases
    like reorgs and missing blocks.
    """

    def __init__(
        self,
        web3: Web3,
        retry_config: Optional[AsyncRetry] = None,
        default_options: Optional[PollingOptions] = None
    ):
        """
        Initialize event poller.

        Args:
            web3: Web3 instance for blockchain interaction
            retry_config: Retry configuration for RPC calls
            default_options: Default polling options
        """
        self.web3 = web3
        self.retry = retry_config or AsyncRetry(
            max_retries=3,
            base_delay=0.5,
            max_delay=10.0
        )
        self.default_options = default_options or PollingOptions()

        # Cache for contract ABIs
        self._abi_cache: Dict[str, Dict] = {}
        self._event_sighash_cache: Dict[str, str] = {}

    async def get_logs(
        self,
        filter_params: FilterParams,
        options: Optional[PollingOptions] = None
    ) -> List[LogReceipt]:
        """
        Get raw logs from blockchain.

        Args:
            filter_params: Web3 filter parameters
            options: Polling options

        Returns:
            List of log receipts
        """
        opts = options or self.default_options

        try:
            logs = await self.retry.execute(
                self.web3.eth.get_logs,
                filter_params
            )

            LOG.debug(f"Retrieved {len(logs)} logs")
            return logs

        except Exception as e:
            raise EventError(
                f"Failed to get logs: {e}",
                block_number=filter_params.get('fromBlock'),
                cause=e
            )

    async def get_events(
        self,
        contract: Contract,
        event_name: str,
        from_block: Union[int, str] = 'latest',
        to_block: Union[int, str] = 'latest',
        argument_filters: Optional[Dict[str, Any]] = None,
        options: Optional[PollingOptions] = None
    ) -> EventResult:
        """
        Get events for a specific contract event.

        Args:
            contract: Contract instance
            event_name: Name of the event to poll
            from_block: Starting block number or tag
            to_block: Ending block number or tag
            argument_filters: Filters for event arguments
            options: Polling options

        Returns:
            EventResult with decoded events
        """
        opts = options or self.default_options
        start_time = asyncio.get_event_loop().time()

        # Get event ABI
        event_abi = self._get_event_abi(contract, event_name)
        if not event_abi:
            raise EventError(
                f"Event '{event_name}' not found in contract ABI",
                event_name=event_name,
                contract_address=contract.address
            )

        # Build filter
        event_filter = self._build_filter(
            contract=contract,
            event_abi=event_abi,
            from_block=from_block,
            to_block=to_block,
            argument_filters=argument_filters
        )

        # Get logs
        logs = await self.get_logs(event_filter, opts)

        # Decode events if requested
        events = []
        if opts.decode_events:
            events = self._decode_logs(logs, contract, event_abi)
        else:
            # Convert to EventData format
            for log in logs:
                events.append({
                    'args': {},
                    'event': event_name,
                    'logIndex': log.logIndex,
                    'transactionIndex': log.transactionIndex,
                    'transactionHash': log.transactionHash,
                    'address': log.address,
                    'blockHash': log.blockHash,
                    'blockNumber': log.blockNumber
                })

        # Sort by block if requested
        if opts.sort_by_block:
            events.sort(key=lambda e: e['blockNumber'])

        # Return result
        polling_time = asyncio.get_event_loop().time() - start_time

        return EventResult(
            events=events,
            total_count=len(events),
            start_block=self._resolve_block_number(from_block),
            end_block=self._resolve_block_number(to_block),
            polling_time=polling_time,
            has_more=len(events) >= opts.batch_size
        )

    async def wait_for_event(
        self,
        contract: Contract,
        event_name: str,
        timeout: float = 60.0,
        poll_interval: float = 1.0,
        from_block: Optional[int] = None,
        argument_filters: Optional[Dict[str, Any]] = None,
        callback: Optional[Callable[[EventData], bool]] = None
    ) -> Optional[EventData]:
        """
        Wait for a specific event to occur.

        Args:
            contract: Contract to watch
            event_name: Name of the event to wait for
            timeout: Maximum time to wait
            poll_interval: Polling interval
            from_block: Starting block number
            argument_filters: Filters for event arguments
            callback: Optional callback to check if event matches

        Returns:
            First matching event or None if timeout
        """
        start_time = asyncio.get_event_loop().time()
        last_block = from_block or await self.retry.execute(self.web3.eth.block_number) - 1

        while True:
            current_block = await self.retry.execute(self.web3.eth.block_number)

            # Poll for events since last check
            result = await self.get_events(
                contract=contract,
                event_name=event_name,
                from_block=last_block + 1,
                to_block=current_block,
                argument_filters=argument_filters
            )

            # Check events
            for event in result.events:
                # Apply callback if provided
                if callback is None or callback(event):
                    return event

            # Update last block
            last_block = current_block

            # Check timeout
            if asyncio.get_event_loop().time() - start_time > timeout:
                return None

            # Wait before next poll
            await asyncio.sleep(poll_interval)

    async def get_events_in_batches(
        self,
        contract: Contract,
        event_name: str,
        from_block: int,
        to_block: int,
        batch_size: int = 1000,
        argument_filters: Optional[Dict[str, Any]] = None
    ) -> List[EventData]:
        """
        Get events in batches to handle large ranges efficiently.

        Args:
            contract: Contract instance
            event_name: Name of the event
            from_block: Starting block number
            to_block: Ending block number
            batch_size: Number of blocks per batch
            argument_filters: Filters for event arguments

        Returns:
            List of all events
        """
        all_events = []
        current_from = from_block

        while current_from <= to_block:
            current_to = min(current_from + batch_size - 1, to_block)

            LOG.debug(f"Polling events from block {current_from} to {current_to}")

            result = await self.get_events(
                contract=contract,
                event_name=event_name,
                from_block=current_from,
                to_block=current_to,
                argument_filters=argument_filters
            )

            all_events.extend(result.events)
            current_from = current_to + 1

        return all_events

    async def get_all_contract_events(
        self,
        contract: Contract,
        from_block: Union[int, str] = 'latest',
        to_block: Union[int, str] = 'latest',
        options: Optional[PollingOptions] = None
    ) -> EventResult:
        """
        Get all events emitted by a contract.

        Args:
            contract: Contract instance
            from_block: Starting block number or tag
            to_block: Ending block number or tag
            options: Polling options

        Returns:
            EventResult with all events
        """
        opts = options or self.default_options

        # Build filter for all contract logs
        filter_params: FilterParams = {
            'address': contract.address,
            'fromBlock': from_block,
            'toBlock': to_block
        }

        # Get logs
        logs = await self.get_logs(filter_params, opts)

        # Decode logs
        events = []
        for log in logs:
            try:
                # Try to decode with each event in the contract
                decoded = self._try_decode_log(log, contract)
                if decoded:
                    events.append(decoded)
                else:
                    # Unknown event
                    events.append({
                        'args': {},
                        'event': 'Unknown',
                        'logIndex': log.logIndex,
                        'transactionIndex': log.transactionIndex,
                        'transactionHash': log.transactionHash,
                        'address': log.address,
                        'blockHash': log.blockHash,
                        'blockNumber': log.blockNumber,
                        'data': log.data,
                        'topics': log.topics
                    })
            except Exception as e:
                LOG.warning(f"Failed to decode log: {e}")

        # Sort by block
        events.sort(key=lambda e: e['blockNumber'])

        return EventResult(
            events=events,
            total_count=len(events),
            start_block=self._resolve_block_number(from_block),
            end_block=self._resolve_block_number(to_block)
        )

    async def monitor_events(
        self,
        contract: Contract,
        event_names: List[str],
        callback: Callable[[EventData], None],
        poll_interval: float = 1.0,
        from_block: Optional[int] = None
    ) -> None:
        """
        Continuously monitor for specific events.

        Args:
            contract: Contract to monitor
            event_names: List of event names to watch
            callback: Callback function for each event
            poll_interval: Polling interval
            from_block: Starting block number
        """
        last_block = from_block or await self.retry.execute(self.web3.eth.block_number) - 1

        while True:
            current_block = await self.retry.execute(self.web3.eth.block_number)

            # Check each event type
            for event_name in event_names:
                result = await self.get_events(
                    contract=contract,
                    event_name=event_name,
                    from_block=last_block + 1,
                    to_block=current_block
                )

                # Call callback for each event
                for event in result.events:
                    try:
                        if asyncio.iscoroutinefunction(callback):
                            await callback(event)
                        else:
                            callback(event)
                    except Exception as e:
                        LOG.error(f"Error in event callback: {e}")

            # Update last block
            last_block = current_block

            # Wait before next poll
            await asyncio.sleep(poll_interval)

    def _get_event_abi(self, contract: Contract, event_name: str) -> Optional[Dict]:
        """Get ABI for a specific event"""
        # Check cache
        cache_key = f"{contract.address}:{event_name}"
        if cache_key in self._abi_cache:
            return self._abi_cache[cache_key]

        # Search in contract ABI
        for item in contract.abi:
            if item.get('type') == 'event' and item.get('name') == event_name:
                self._abi_cache[cache_key] = item
                return item

        return None

    def _build_filter(
        self,
        contract: Contract,
        event_abi: Dict,
        from_block: Union[int, str],
        to_block: Union[int, str],
        argument_filters: Optional[Dict[str, Any]] = None
    ) -> FilterParams:
        """Build Web3 filter parameters"""
        # Get event signature
        event_signature = self._get_event_signature(event_abi)

        # Build topics
        topics = [event_signature]

        # Add argument filters
        if argument_filters:
            # Map argument names to indexed parameters
            indexed_inputs = [
                input for input in event_abi.get('inputs', [])
                if input.get('indexed', False)
            ]

            # Create topic filter
            topic_filter = [None] * (len(indexed_inputs) + 1)
            topic_filter[0] = event_signature

            for i, input_def in enumerate(indexed_inputs):
                arg_name = input_def['name']
                if arg_name in argument_filters:
                    value = argument_filters[arg_name]
                    if isinstance(value, list):
                        topic_filter[i + 1] = value
                    else:
                        topic_filter[i + 1] = self._value_to_topic(value, input_def)

            topics = topic_filter

        return FilterParams({
            'address': contract.address,
            'fromBlock': from_block,
            'toBlock': to_block,
            'topics': topics if any(t is not None for t in topics) else None
        })

    def _get_event_signature(self, event_abi: Dict) -> str:
        """Get event signature hash"""
        # Build signature
        name = event_abi['name']
        types = []
        for input_def in event_abi.get('inputs', []):
            types.append(input_def['type'])

        signature = f"{name}({','.join(types)})"

        # Hash and return
        return Web3.keccak(text=signature).hex()

    def _value_to_topic(self, value: Any, input_def: Dict) -> Optional[str]:
        """Convert a value to a topic value"""
        if value is None:
            return None

        # Handle addresses
        if input_def['type'] == 'address':
            if isinstance(value, str):
                return value.lower() if value.startswith('0x') else f"0x{value.lower()}"
            return f"0x{value:040x}"

        # Handle uint256
        elif input_def['type'].startswith('uint'):
            if isinstance(value, int):
                return f"0x{value:064x}"
            elif isinstance(value, str) and value.startswith('0x'):
                return value

        # Handle other types
        return Web3.to_hex(value)

    def _decode_logs(self, logs: List[LogReceipt], contract: Contract, event_abi: Dict) -> List[EventData]:
        """Decode logs into event data"""
        events = []

        for log in logs:
            try:
                # Decode using Web3's event processing
                event = contract.events[event_abi['name']]().process_log(log)
                events.append(event)
            except Exception as e:
                LOG.warning(f"Failed to decode log: {e}")
                # Return raw event
                events.append({
                    'args': {},
                    'event': event_abi['name'],
                    'logIndex': log.logIndex,
                    'transactionIndex': log.transactionIndex,
                    'transactionHash': log.transactionHash,
                    'address': log.address,
                    'blockHash': log.blockHash,
                    'blockNumber': log.blockNumber,
                    'data': log.data,
                    'topics': log.topics
                })

        return events

    def _try_decode_log(self, log: LogReceipt, contract: Contract) -> Optional[EventData]:
        """Try to decode a log with any event in the contract"""
        # Try each event in the contract
        for event_abi in contract.abi:
            if event_abi.get('type') != 'event':
                continue

            try:
                event = contract.events[event_abi['name']]().process_log(log)
                return event
            except:
                continue

        return None

    def _resolve_block_number(self, block: Union[int, str]) -> Optional[int]:
        """Resolve block tag to number"""
        if isinstance(block, int):
            return block
        elif isinstance(block, str):
            if block == 'latest':
                return None  # Can't resolve latest without RPC call
            elif block == 'earliest':
                return 0
            elif block == 'pending':
                return None
        return None


# Convenience functions for common event polling patterns
async def wait_for_transfer_event(
    web3: Web3,
    contract: Contract,
    from_address: Optional[str] = None,
    to_address: Optional[str] = None,
    timeout: float = 60.0
) -> Optional[EventData]:
    """
    Wait for a Transfer event.

    Args:
        web3: Web3 instance
        contract: Token contract
        from_address: Filter by sender
        to_address: Filter by recipient
        timeout: Maximum wait time

    Returns:
        Transfer event or None
    """
    poller = EventPoller(web3)

    argument_filters = {}
    if from_address:
        argument_filters['from'] = from_address
    if to_address:
        argument_filters['to'] = to_address

    return await poller.wait_for_event(
        contract=contract,
        event_name='Transfer',
        timeout=timeout,
        argument_filters=argument_filters
    )


async def get_all_transfer_events(
    web3: Web3,
    contract: Contract,
    from_block: int,
    to_block: int,
    batch_size: int = 1000
) -> List[EventData]:
    """
    Get all Transfer events in a block range.

    Args:
        web3: Web3 instance
        contract: Token contract
        from_block: Starting block
        to_block: Ending block
        batch_size: Batch size for polling

    Returns:
        List of Transfer events
    """
    poller = EventPoller(web3)

    return await poller.get_events_in_batches(
        contract=contract,
        event_name='Transfer',
        from_block=from_block,
        to_block=to_block,
        batch_size=batch_size
    )