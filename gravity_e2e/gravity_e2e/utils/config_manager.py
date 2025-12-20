"""
Configuration manager with JSON schema validation

This module provides a centralized configuration management system for the Gravity E2E
test framework, supporting JSON schema validation and environment variable overrides.

Design Notes:
- Uses jsonschema for robust configuration validation
- Supports multiple configuration files (nodes, accounts, test-specific)
- Environment variable overrides for CI/CD
- Clear error messages for validation failures
- Type hints and data classes for better IDE support
- Async support for configuration loading from remote sources
"""

import json
import os
import logging
from pathlib import Path
from typing import Any, Dict, List, Optional, Type, TypeVar, Union
from dataclasses import dataclass, field
from datetime import datetime

try:
    import jsonschema
except ImportError:
    jsonschema = None

from .exceptions import ConfigurationError, ErrorCodes

LOG = logging.getLogger(__name__)

T = TypeVar('T')


@dataclass
class ValidationResult:
    """Result of configuration validation"""
    is_valid: bool
    errors: List[str] = field(default_factory=list)
    warnings: List[str] = field(default_factory=list)
    validated_config: Optional[Dict[str, Any]] = None


@dataclass
class NodeConfiguration:
    """Validated node configuration"""
    network: Dict[str, Any]
    nodes: Dict[str, Dict[str, Any]]
    clusters: Optional[Dict[str, Any]] = None


@dataclass
class AccountConfiguration:
    """Validated account configuration"""
    faucet: Dict[str, Any]
    test_accounts: Optional[Dict[str, Dict[str, Any]]] = None
    account_defaults: Optional[Dict[str, Any]] = None


@dataclass
class TestConfiguration:
    """Configuration for specific test execution"""
    test_name: str
    node_id: Optional[str] = None
    timeout_seconds: int = 30
    retry_attempts: int = 3
    gas_limit: int = 210000
    max_fee_per_gas: int = 1000000000


class ConfigManager:
    """
    Manages configuration loading, validation, and saving.

    Provides a unified interface for all configuration operations while
    maintaining backward compatibility with existing configuration formats.
    """

    def __init__(
        self,
        config_dir: Path = None,
        schema_dir: Path = None,
        backup_dir: Path = None
    ):
        """
        Initialize configuration manager.

        Args:
            config_dir: Directory containing configuration files (default: configs/)
            schema_dir: Directory containing JSON schemas (default: configs/schemas/)
            backup_dir: Directory for configuration backups (default: configs/backups/)
        """
        # Resolve default paths relative to the config manager file
        if config_dir is None:
            config_dir = Path(__file__).parent.parent.parent / "configs"
        if schema_dir is None:
            schema_dir = config_dir / "schemas"
        if backup_dir is None:
            backup_dir = config_dir / "backups"

        self.config_dir = Path(config_dir)
        self.schema_dir = Path(schema_dir)
        self.backup_dir = Path(backup_dir)

        # Ensure directories exist
        self.config_dir.mkdir(parents=True, exist_ok=True)
        self.backup_dir.mkdir(parents=True, exist_ok=True)

        # Cache for loaded schemas
        self._schemas: Dict[str, Dict[str, Any]] = {}

        # Cache for loaded configurations
        self._cache: Dict[str, Tuple[datetime, Dict[str, Any]]] = {}

    def _load_schema(self, schema_name: str) -> Dict[str, Any]:
        """Load a JSON schema from file or cache"""
        if schema_name in self._schemas:
            return self._schemas[schema_name]

        schema_file = self.schema_dir / f"{schema_name}_schema.json"
        if not schema_file.exists():
            raise ConfigurationError(
                f"Schema file not found: {schema_file}",
                config_file=str(schema_file)
            )

        try:
            with open(schema_file, 'r') as f:
                schema = json.load(f)
            self._schemas[schema_name] = schema
            return schema
        except json.JSONDecodeError as e:
            raise ConfigurationError(
                f"Invalid JSON in schema file {schema_file}: {e}",
                config_file=str(schema_file),
                code=ErrorCodes.CONFIG_VALIDATION_FAILED
            )

    def _validate_config(
        self,
        config: Dict[str, Any],
        schema: Dict[str, Any],
        config_name: str = "configuration"
    ) -> ValidationResult:
        """Validate configuration against a schema"""
        if jsonschema is None:
            LOG.warning("jsonschema not available, skipping validation")
            return ValidationResult(is_valid=True, validated_config=config)

        validator = jsonschema.Draft7Validator(schema)
        errors = []
        warnings = []

        try:
            jsonschema.validate(config, schema)
            return ValidationResult(is_valid=True, validated_config=config)
        except jsonschema.ValidationError as e:
            # Format validation errors
            for error in e.errors:
                if hasattr(error, 'path') and error.path:
                    path = " -> ".join(str(p) for p in error.path)
                    errors.append(f"'{path}' {error.message}")
                else:
                    errors.append(str(error.message))
        except jsonschema.SchemaError as e:
            errors.append(f"Schema error: {e}")

        return ValidationResult(
            is_valid=False,
            errors=errors,
            warnings=warnings
        )

    def _get_env_override(self, key: str, default: Any = None) -> Any:
        """Get environment variable override"""
        env_key = f"GRAVITY_E2E_{key.upper()}"
        env_value = os.getenv(env_key)

        if env_value is not None:
            # Try to parse as JSON first
            try:
                return json.loads(env_value)
            except json.JSONDecodeError:
                # Return as string
                return env_value

        return default

    def _apply_env_overrides(self, config: Dict[str, Any]) -> Dict[str, Any]:
        """Apply environment variable overrides to configuration"""
        # Deep copy to avoid modifying original
        result = json.loads(json.dumps(config))

        # Apply common overrides
        if isinstance(result, dict):
            # Node configuration overrides
            if 'nodes' in result:
                for node_id, node_config in result['nodes'].items():
                    # Override specific fields
                    node_config['host'] = self._get_env_override(
                        f"NODE_{node_id.upper()}_HOST",
                        node_config.get('host')
                    )
                    node_config['rpc_port'] = self._get_env_override(
                        f"NODE_{node_id.upper()}_RPC_PORT",
                        node_config.get('rpc_port')
                    )
                    node_config['api_port'] = self._get_env_override(
                        f"NODE_{node_id.upper()}_API_PORT",
                        node_config.get('api_port')
                    )

            # Network configuration overrides
            if 'network' in result:
                result['network']['chain_id'] = self._get_env_override(
                    'CHAIN_ID',
                    result['network'].get('chain_id')
                )

            # Test configuration overrides
            for key in ['timeout', 'gas_limit', 'max_retries']:
                override_key = f"TEST_{key.upper()}"
                if override_key in os.environ:
                    try:
                        result[key] = int(os.environ[override_key])
                    except ValueError:
                        LOG.warning(f"Invalid integer value for {override_key}: {os.environ[override_key]}")

        return result

    def load_config(
        self,
        filename: str,
        validate: bool = True,
        schema_name: Optional[str] = None,
        apply_env_overrides: bool = True
    ) -> Dict[str, Any]:
        """
        Load and validate a configuration file.

        Args:
            filename: Configuration filename (e.g., 'nodes.json', 'test_accounts.json')
            validate: Whether to validate against a schema
            schema_name: Schema name to use for validation
            apply_env_overrides: Whether to apply environment variable overrides

        Returns:
            Validated configuration dictionary

        Raises:
            ConfigurationError: If configuration is invalid or missing
        """
        # Check cache first
        cache_key = f"{filename}:{validate}:{schema_name}:{apply_env_overrides}"
        if cache_key in self._cache:
            cached_time, cached_config = self._cache[cache_key]
            # Cache valid for 5 minutes
            if (datetime.now() - cached_time).seconds < 300:
                return cached_config

        config_file = self.config_dir / filename

        if not config_file.exists():
            raise ConfigurationError(
                f"Configuration file not found: {config_file}",
                config_file=str(config_file),
                code=ErrorCodes.CONFIG_FILE_NOT_FOUND
            )

        try:
            with open(config_file, 'r') as f:
                config = json.load(f)
        except json.JSONDecodeError as e:
            raise ConfigurationError(
                f"Invalid JSON in configuration file {config_file}: {e}",
                config_file=str(config_file),
                code=ErrorCodes.CONFIG_VALIDATION_FAILED
            )

        # Apply environment overrides
        if apply_env_overrides:
            config = self._apply_env_overrides(config)

        # Validate against schema if requested
        if validate:
            if schema_name is None:
                # Try to infer schema name from filename
                if filename == 'nodes.json':
                    schema_name = 'nodes'
                elif filename == 'test_accounts.json':
                    schema_name = 'accounts'
                else:
                    schema_name = filename.replace('.json', '')

            try:
                schema = self._load_schema(schema_name)
                validation = self._validate_config(config, schema, filename)

                if not validation.is_valid:
                    error_msg = f"Configuration validation failed for {filename}:\n"
                    error_msg += "\n".join(f"  - {error}" for error in validation.errors)
                    raise ConfigurationError(
                        error_msg,
                        config_file=str(config_file),
                        code=ErrorCodes.CONFIG_VALIDATION_FAILED
                    )
            except FileNotFoundError:
                LOG.warning(f"No schema found for {filename}, skipping validation")

        # Cache the result
        self._cache[cache_key] = (datetime.now(), config)
        return config

    def load_nodes_config(self) -> NodeConfiguration:
        """
        Load and validate node configuration.

        Returns:
            Validated NodeConfiguration object
        """
        config = self.load_config('nodes.json', validate=True, schema_name='nodes')

        # Convert to dataclass
        network = config.get('network', {})
        nodes = config.get('nodes', {})
        clusters = config.get('clusters')

        return NodeConfiguration(
            network=network,
            nodes=nodes,
            clusters=clusters
        )

    def load_accounts_config(self) -> AccountConfiguration:
        """
        Load and validate account configuration.

        Returns:
            Validated AccountConfiguration object
        """
        config = self.load_config('test_accounts.json', validate=True, schema_name='accounts')

        # Convert to dataclass
        faucet = config.get('faucet', {})
        test_accounts = config.get('test_accounts', {})
        account_defaults = config.get('account_defaults', {})

        return AccountConfiguration(
            faucet=faucet,
            test_accounts=test_accounts,
            account_defaults=account_defaults
        )

    def save_config(
        self,
        config: Dict[str, Any],
        filename: str,
        create_backup: bool = True,
        validate: bool = True,
        schema_name: Optional[str] = None,
        format_json: bool = True
    ) -> None:
        """
        Save configuration to file.

        Args:
            config: Configuration dictionary to save
            filename: Target filename
            create_backup: Whether to create a backup of existing file
            validate: Whether to validate before saving
            schema_name: Schema name for validation
            format_json: Whether to format JSON with indentation
        """
        config_file = self.config_dir / filename

        # Validate if requested
        if validate and schema_name:
            try:
                schema = self._load_schema(schema_name)
                validation = self._validate_config(config, schema, filename)

                if not validation.is_valid:
                    raise ConfigurationError(
                        f"Cannot save invalid configuration for {filename}:\n"
                        + "\n".join(f"  - {error}" for error in validation.errors)
                    )
            except FileNotFoundError:
                LOG.warning(f"No schema found for {filename}, skipping validation")

        # Create backup if requested and file exists
        if create_backup and config_file.exists():
            backup_file = self.backup_dir / f"{filename}.backup.{datetime.now().isoformat()}"
            try:
                import shutil
                shutil.copy2(config_file, backup_file)
                LOG.info(f"Created backup: {backup_file}")
            except Exception as e:
                LOG.error(f"Failed to create backup: {e}")

        # Save configuration
        try:
            with open(config_file, 'w') as f:
                if format_json:
                    json.dump(config, f, indent=2, sort_keys=True)
                else:
                    json.dump(config, f)

            LOG.info(f"Saved configuration to {config_file}")
        except Exception as e:
            raise ConfigurationError(
                f"Failed to save configuration to {config_file}: {e}",
                config_file=str(config_file)
            )

        # Clear cache for this file
        keys_to_remove = [k for k in self._cache.keys() if k.startswith(filename + ":")]
        for key in keys_to_remove:
            del self._cache[key]

    def validate_all_configs(self) -> Dict[str, ValidationResult]:
        """
        Validate all configuration files in the directory.

        Returns:
            Dictionary mapping filenames to validation results
        """
        results = {}

        for config_file in self.config_dir.glob('*.json'):
            if not config_file.name.endswith('.backup.'):  # Skip backup files
                try:
                    # Try to infer schema name
                    schema_name = None
                    if config_file.name == 'nodes.json':
                        schema_name = 'nodes'
                    elif config_file.name == 'test_accounts.json':
                        schema_name = 'accounts'

                    config = self.load_config(
                        config_file.name,
                        validate=True,
                        schema_name=schema_name
                    )
                    results[config_file.name] = ValidationResult(
                        is_valid=True,
                        validated_config=config
                    )
                except ConfigurationError as e:
                    results[config_file.name] = ValidationResult(
                        is_valid=False,
                        errors=[str(e)]
                    )
                except Exception as e:
                    results[config_file.name] = ValidationResult(
                        is_valid=False,
                        errors=[f"Unexpected error: {e}"]
                    )

        return results

    def create_test_config(
        self,
        test_name: str,
        node_id: Optional[str] = None,
        **overrides
    ) -> TestConfiguration:
        """
        Create a test configuration with sensible defaults.

        Args:
            test_name: Name of the test
            node_id: Optional node ID to use
            **overrides: Override default values

        Returns:
            TestConfiguration object
        """
        # Apply defaults and overrides
        config = TestConfiguration(
            test_name=test_name,
            node_id=node_id,
            timeout_seconds=overrides.get('timeout_seconds', 30),
            retry_attempts=overrides.get('retry_attempts', 3),
            gas_limit=overrides.get('gas_limit', 210000),
            max_fee_per_gas=overrides.get('max_fee_per_gas', 1000000000)
        )

        return config

    def reload_config(self, filename: str) -> Dict[str, Any]:
        """
        Force reload a configuration file, clearing cache.

        Args:
            filename: Configuration filename to reload

        Returns:
            Reloaded configuration dictionary
        """
        # Clear cache for this file
        keys_to_remove = [k for k in self._cache.keys() if k.startswith(filename + ":")]
        for key in keys_to_remove:
            del self._cache[key]

        # Reload with validation
        return self.load_config(filename)


# Default configuration manager instance
config_manager = ConfigManager()