"""
Event parsing utility for Gravity E2E Test Framework

This module provides utilities for parsing blockchain events using Web3 contract ABIs
instead of hardcoded byte parsing.

Design Notes:
- Uses Web3 contract event processing for reliable parsing
- Supports custom event ABIs
- Provides backward compatibility with existing code
- Handles both indexed and non-indexed parameters
"""

import logging
from typing import Any, Dict, List, Optional, Union
from web3 import Web3
from web3.contract import Contract
from eth_utils import to_checksum_address

LOG = logging.getLogger(__name__)


class EventParser:
    """
    Utility class for parsing blockchain events using contract ABIs.

    This class provides methods to parse raw transaction logs into structured
    event data using Web3's built-in event processing capabilities.
    """

    @staticmethod
    def parse_deposit_gravity_event(receipt_logs: List[Dict], contract_abi: List[Dict]) -> Optional[Dict[str, Any]]:
        """
        Parse DepositGravityEvent from transaction receipt logs.

        Args:
            receipt_logs: List of log dictionaries from transaction receipt
            contract_abi: ABI of the contract that emits the event

        Returns:
            Parsed event data or None if not found

        Example:
            event = EventParser.parse_deposit_gravity_event(
                receipt.logs,
                gravity_bridge_abi
            )
            if event:
                print(f"User: {event['args']['user']}")
                print(f"Amount: {event['args']['amount']}")
        """
        try:
            # Create a temporary contract instance for event processing
            # We don't need an actual address, just the ABI
            temp_contract = Contract(
                address="0x0000000000000000000000000000000000000000",
                abi=contract_abi
            )

            # Find DepositGravity event in logs
            for log in receipt_logs:
                try:
                    # Process the log using Web3's event processing
                    # Look for any event that matches our log topics
                    for event_abi in contract_abi:
                        if event_abi.get('type') == 'event' and event_abi.get('name') == 'DepositGravity':
                            # Create event object and process log
                            event = getattr(temp_contract.events, event_abi['name'])()
                            processed = event.process_log(log)

                            # Verify this is the DepositGravity event by checking expected fields
                            if hasattr(processed.args, 'user') and hasattr(processed.args, 'amount'):
                                return processed
                except Exception as e:
                    LOG.debug(f"Failed to process log with ABI: {e}")
                    continue

            return None

        except Exception as e:
            LOG.error(f"Error parsing DepositGravity event: {e}")
            return None

    @staticmethod
    def parse_cross_chain_deposit_processed_event(
        receipt_logs: List[Dict],
        contract_abi: List[Dict]
    ) -> List[Dict[str, Any]]:
        """
        Parse CrossChainDepositProcessed events from transaction receipt logs.

        Args:
            receipt_logs: List of log dictionaries from transaction receipt
            contract_abi: ABI of the contract that emits the events

        Returns:
            List of parsed events

        Example:
            events = EventParser.parse_cross_chain_deposit_processed_event(
                receipt.logs,
                monitor_contract_abi
            )
            for event in events:
                print(f"Sender: {event['args']['sender']}")
                print(f"Success: {event['args']['success']}")
        """
        events = []

        try:
            # Create a temporary contract instance
            temp_contract = Contract(
                address="0x0000000000000000000000000000000000000000",
                abi=contract_abi
            )

            # Find CrossChainDepositProcessed events
            for log in receipt_logs:
                try:
                    for event_abi in contract_abi:
                        if event_abi.get('type') == 'event' and event_abi.get('name') == 'CrossChainDepositProcessed':
                            event = getattr(temp_contract.events, event_abi['name'])()
                            processed = event.process_log(log)
                            events.append(processed)
                except Exception as e:
                    LOG.debug(f"Failed to process log with ABI: {e}")
                    continue

        except Exception as e:
            LOG.error(f"Error parsing CrossChainDepositProcessed events: {e}")

        return events

    @staticmethod
    def create_deposit_gravity_abi() -> List[Dict]:
        """
        Create a minimal ABI for DepositGravityEvent.

        Returns:
            ABI definition for DepositGravity event
        """
        return [
            {
                "anonymous": False,
                "inputs": [
                    {"indexed": False, "name": "user", "type": "address"},
                    {"indexed": False, "name": "amount", "type": "uint256"},
                    {"indexed": False, "name": "targetAddress", "type": "address"},
                    {"indexed": False, "name": "blockNumber", "type": "uint256"}
                ],
                "name": "DepositGravity",
                "type": "event"
            }
        ]

    @staticmethod
    def create_cross_chain_deposit_abi() -> List[Dict]:
        """
        Create a minimal ABI for CrossChainDepositProcessed event.

        Returns:
            ABI definition for CrossChainDepositProcessed event
        """
        return [
            {
                "anonymous": False,
                "inputs": [
                    {"indexed": True, "name": "sender", "type": "address"},
                    {"indexed": True, "name": "targetAddress", "type": "address"},
                    {"indexed": False, "name": "amount", "type": "uint256"},
                    {"indexed": False, "name": "blockNumber", "type": "uint256"},
                    {"indexed": False, "name": "success", "type": "bool"},
                    {"indexed": False, "name": "errorMessage", "type": "string"},
                    {"indexed": False, "name": "issuer", "type": "string"},
                    {"indexed": False, "name": "onchainBlockNumber", "type": "uint256"}
                ],
                "name": "CrossChainDepositProcessed",
                "type": "event"
            }
        ]

    @staticmethod
    def verify_deposit_event(
        event_data: Dict[str, Any],
        expected_sender: str,
        expected_amount: int,
        expected_target: str
    ) -> bool:
        """
        Verify that a parsed deposit event matches expected values.

        Args:
            event_data: Parsed event data
            expected_sender: Expected sender address
            expected_amount: Expected amount
            expected_target: Expected target address

        Returns:
            True if all values match
        """
        try:
            args = event_data.get('args', {})

            # Convert addresses to checksum format for comparison
            actual_sender = to_checksum_address(args.get('user', '0x'))
            actual_target = to_checksum_address(args.get('targetAddress', '0x'))
            expected_sender_checksum = to_checksum_address(expected_sender)
            expected_target_checksum = to_checksum_address(expected_target)

            # Check all fields match
            return (
                actual_sender.lower() == expected_sender_checksum.lower() and
                actual_target.lower() == expected_target_checksum.lower() and
                args.get('amount') == expected_amount
            )
        except Exception as e:
            LOG.error(f"Error verifying deposit event: {e}")
            return False

    @staticmethod
    def extract_legacy_deposit_event(raw_log: Dict) -> Optional[Dict[str, Any]]:
        """
        Extract deposit event using legacy parsing for backward compatibility.

        This method maintains the original hardcoded parsing logic as a fallback
        when ABI parsing is not available.

        Args:
            raw_log: Raw log dictionary from transaction receipt

        Returns:
            Parsed event data or None if parsing failed
        """
        try:
            data_hex = raw_log.get('data', '')[2:]  # Remove '0x' prefix

            if len(data_hex) < 256:  # Need at least 128 bytes (256 hex chars)
                return None

            # Extract values based on known structure
            # user(32 bytes) + amount(32 bytes) + targetAddress(32 bytes) + blockNumber(32 bytes)
            deposit_event = {
                "user": "0x" + data_hex[24:64],  # User address (last 20 bytes)
                "amount": int(data_hex[64:128], 16),  # Amount (32 bytes)
                "targetAddress": "0x" + data_hex[152:192],  # Target address (last 20 bytes)
                "blockNumber": int(data_hex[192:256], 16)  # Block number (32 bytes)
            }

            return deposit_event

        except Exception as e:
            LOG.error(f"Error in legacy event parsing: {e}")
            return None