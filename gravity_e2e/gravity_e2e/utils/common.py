import time
from typing import Any, Dict
from datetime import datetime


def hex_to_int(value: str) -> int:
    """将十六进制字符串转换为整数"""
    if isinstance(value, str) and value.startswith("0x"):
        return int(value, 16)
    return int(value)


def int_to_hex(value: int) -> str:
    """将整数转换为十六进制字符串"""
    return hex(value)


def format_timestamp(ts: float = None) -> str:
    """格式化时间戳"""
    if ts is None:
        ts = time.time()
    return datetime.fromtimestamp(ts).isoformat()


def safe_get(d: Dict, key: str, default: Any = None) -> Any:
    """安全获取字典值"""
    return d.get(key, default)