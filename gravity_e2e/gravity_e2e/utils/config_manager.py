"""
Configuration loading utilities for Gravity E2E tests
"""
import json
import logging
from pathlib import Path
from typing import Any, Dict, Optional

from .exceptions import ConfigurationError, ErrorCodes

LOG = logging.getLogger(__name__)


def load_config(
    filename: str,
    config_dir: Path = None
) -> Dict[str, Any]:
    """
    Load a JSON configuration file.

    Args:
        filename: Configuration filename (e.g., 'nodes.json', 'test_accounts.json')
        config_dir: Directory containing config files (default: configs/)

    Returns:
        Configuration dictionary

    Raises:
        ConfigurationError: If file not found or invalid JSON
    """
    if config_dir is None:
        config_dir = Path(__file__).parent.parent.parent / "configs"

    config_file = config_dir / filename

    if not config_file.exists():
        raise ConfigurationError(
            f"Configuration file not found: {config_file}",
            config_file=str(config_file),
            code=ErrorCodes.CONFIG_FILE_NOT_FOUND
        )

    try:
        with open(config_file, 'r') as f:
            return json.load(f)
    except json.JSONDecodeError as e:
        raise ConfigurationError(
            f"Invalid JSON in configuration file {config_file}: {e}",
            config_file=str(config_file),
            code=ErrorCodes.CONFIG_VALIDATION_FAILED
        )


class ConfigManager:
    """
    Simple configuration manager for backward compatibility.

    For new code, prefer using load_config() directly.
    """

    def __init__(self, config_dir: Path = None):
        if config_dir is None:
            config_dir = Path(__file__).parent.parent.parent / "configs"
        self.config_dir = Path(config_dir)

    def load_config(
        self,
        filename: str,
        validate: bool = True,
        schema_name: Optional[str] = None,
        apply_env_overrides: bool = True
    ) -> Dict[str, Any]:
        """Load configuration file (extra params ignored for compatibility)"""
        return load_config(filename, self.config_dir)
