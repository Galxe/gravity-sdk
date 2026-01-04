"""
Common utility functions for Gravity E2E tests
"""
import time
from datetime import datetime


def hex_to_int(value: str) -> int:
    """Convert hexadecimal string to integer"""
    if isinstance(value, str) and value.startswith("0x"):
        return int(value, 16)
    return int(value)


def format_timestamp(ts: float = None) -> str:
    """Format timestamp to ISO format"""
    if ts is None:
        ts = time.time()
    return datetime.fromtimestamp(ts).isoformat()
