import time
from typing import Any, Dict
from datetime import datetime


def hex_to_int(value: str) -> int:
    """Convert hexadecimal string to integer"""
    if isinstance(value, str) and value.startswith("0x"):
        return int(value, 16)
    return int(value)


def int_to_hex(value: int) -> str:
    """Convert integer to hexadecimal string"""
    return hex(value)


def format_timestamp(ts: float = None) -> str:
    """Format timestamp to ISO format"""
    if ts is None:
        ts = time.time()
    return datetime.fromtimestamp(ts).isoformat()


def safe_get(d: Dict, key: str, default: Any = None) -> Any:
    """Safely get value from dictionary"""
    return d.get(key, default)