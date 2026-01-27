"""
Aptos Identity utilities
Parse Aptos identity from YAML files
"""
import logging
from dataclasses import dataclass
from pathlib import Path

try:
    import yaml
except ImportError:
    yaml = None
    logging.warning("PyYAML not installed. Please install it with: pip install pyyaml")

LOG = logging.getLogger(__name__)


@dataclass
class AptosIdentity:
    """Aptos identity information
    
    Attributes:
        account_address: Account address (hex string)
        account_private_key: Account private key (hex string)
        consensus_private_key: Consensus private key (hex string)
        network_private_key: Network private key (hex string)
        consensus_public_key: Consensus public key (hex string)
        network_public_key: Network public key (hex string)
    """
    account_address: str
    account_private_key: str
    consensus_private_key: str
    network_private_key: str
    consensus_public_key: str
    network_public_key: str
    
    def __post_init__(self):
        """Validate the identity fields"""
        if not self.account_address:
            raise ValueError("account_address cannot be empty")
        if not self.account_private_key:
            raise ValueError("account_private_key cannot be empty")
        if not self.consensus_private_key:
            raise ValueError("consensus_private_key cannot be empty")
        if not self.network_private_key:
            raise ValueError("network_private_key cannot be empty")
        if not self.consensus_public_key:
            raise ValueError("consensus_public_key cannot be empty")
        if not self.network_public_key:
            raise ValueError("network_public_key cannot be empty")


def parse_identity_from_yaml(yaml_path: str | Path) -> AptosIdentity:
    """Parse Aptos identity from a YAML file
    
    Args:
        yaml_path: Path to the identity YAML file (e.g., validator-identity.yaml)
        
    Returns:
        AptosIdentity dataclass instance
        
    Raises:
        ImportError: If PyYAML is not installed
        FileNotFoundError: If the YAML file does not exist
        ValueError: If the YAML file is invalid or missing required fields
    """
    if yaml is None:
        raise ImportError(
            "PyYAML is required to parse identity files. "
            "Please install it with: pip install pyyaml"
        )
    
    yaml_path = Path(yaml_path)
    
    if not yaml_path.exists():
        raise FileNotFoundError(f"Identity YAML file not found: {yaml_path}")
    
    if not yaml_path.is_file():
        raise ValueError(f"Path is not a file: {yaml_path}")
    
    try:
        with open(yaml_path, 'r', encoding='utf-8') as f:
            data = yaml.safe_load(f)
    except yaml.YAMLError as e:
        raise ValueError(f"Failed to parse YAML file {yaml_path}: {e}")
    except Exception as e:
        raise ValueError(f"Failed to read YAML file {yaml_path}: {e}")
    
    if not isinstance(data, dict):
        raise ValueError(f"YAML file {yaml_path} does not contain a dictionary")
    
    # Extract required fields
    account_address = data.get('account_address')
    account_private_key = data.get('account_private_key')
    consensus_private_key = data.get('consensus_private_key')
    network_private_key = data.get('network_private_key')
    consensus_public_key = data.get('consensus_public_key')
    network_public_key = data.get('network_public_key')
    
    # Validate all fields are present
    missing_fields = []
    if account_address is None:
        missing_fields.append('account_address')
    if account_private_key is None:
        missing_fields.append('account_private_key')
    if consensus_private_key is None:
        missing_fields.append('consensus_private_key')
    if network_private_key is None:
        missing_fields.append('network_private_key')
    if consensus_public_key is None:
        missing_fields.append('consensus_public_key')
    if network_public_key is None:
        missing_fields.append('network_public_key')
    
    if missing_fields:
        raise ValueError(
            f"YAML file {yaml_path} is missing required fields: {', '.join(missing_fields)}"
        )
    
    # Convert to strings if they're not already
    account_address = str(account_address).strip()
    account_private_key = str(account_private_key).strip()
    consensus_private_key = str(consensus_private_key).strip()
    network_private_key = str(network_private_key).strip()
    consensus_public_key = str(consensus_public_key).strip()
    network_public_key = str(network_public_key).strip()
    
    try:
        identity = AptosIdentity(
            account_address=account_address,
            account_private_key=account_private_key,
            consensus_private_key=consensus_private_key,
            network_private_key=network_private_key,
            consensus_public_key=consensus_public_key,
            network_public_key=network_public_key
        )
        LOG.debug(f"Successfully parsed identity from {yaml_path}")
        return identity
    except ValueError as e:
        raise ValueError(f"Invalid identity data in {yaml_path}: {e}")

