"""
Contract utility functions for encoding/decoding contract calls and results
"""
import json
import logging
import hashlib
import struct
from typing import Any, Dict, List, Optional, Union, Tuple
from pathlib import Path

from ..utils.common import hex_to_int

LOG = logging.getLogger(__name__)


class ContractUtils:
    """Utility class for contract interaction helpers"""
    
    # Standard ERC20 function selectors (commonly used)
    ERC20_SELECTORS = {
        'name': '0x06fdde03',
        'symbol': '0x95d89b41', 
        'decimals': '0x313ce567',
        'totalSupply': '0x18160ddd',
        'balanceOf': '0x70a08231',
        'transfer': '0xa9059cbb',
        'approve': '0x095ea7b3',
        'transferFrom': '0x23b872dd',
        'allowance': '0xdd62ed3e'
    }
    
    # Standard function selectors for common contracts
    COMMON_SELECTORS = {
        'getValue': '0x20965255',
        'setValue': '0x55241077',
        'owner': '0x8da5cb5b',
        'renounceOwnership': '0x715018a6',
        'transferOwnership': '0xf2fde38b'
    }
    
    @staticmethod
    def load_contract_data(contract_name: str, contracts_dir: Path = None) -> Dict:
        """Load contract data from JSON file
        
        Args:
            contract_name: Name of the contract (e.g., "SimpleStorage")
            contracts_dir: Directory containing contract JSON files
            
        Returns:
            Contract data dictionary with 'bytecode' and 'abi'
        """
        if contracts_dir is None:
            # Default to gravity_e2e/contracts_data
            contracts_dir = Path(__file__).parent.parent.parent.parent / "contracts_data"
        
        contract_file = contracts_dir / f"{contract_name}.json"
        
        if not contract_file.exists():
            raise FileNotFoundError(f"Contract file not found: {contract_file}")
        
        with open(contract_file, 'r') as f:
            return json.load(f)
    
    @staticmethod
    def get_function_selector(func_name: str, custom_selectors: Dict[str, str] = None) -> str:
        """Get function selector for a function name
        
        Args:
            func_name: Function name
            custom_selectors: Custom selectors dictionary
            
        Returns:
            Function selector (e.g., "0x06fdde03")
        """
        # Check custom selectors first
        if custom_selectors and func_name in custom_selectors:
            return custom_selectors[func_name]
        
        # Check ERC20 selectors
        if func_name in ContractUtils.ERC20_SELECTORS:
            return ContractUtils.ERC20_SELECTORS[func_name]
        
        # Check common selectors
        if func_name in ContractUtils.COMMON_SELECTORS:
            return ContractUtils.COMMON_SELECTORS[func_name]
        
        # Generate selector using hash (not standard keccak256, but works for basic cases)
        # NOTE: In production, you should use proper keccak256 hashing
        signature = f"{func_name}()"
        # This is a simplified approach - ideally use eth_hashlib.keccak256
        selector_hash = hashlib.sha256(signature.encode()).hexdigest()[:8]
        return f"0x{selector_hash}"
    
    @staticmethod
    def encode_uint256(value: int) -> str:
        """Encode integer as uint256 (32 bytes)"""
        return format(value & 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF, '064x')
    
    @staticmethod
    def encode_address(address: str) -> str:
        """Encode address as 32 bytes"""
        if not address.startswith("0x"):
            address = "0x" + address
        return address[2:].rjust(64, '0').lower()
    
    @staticmethod
    def encode_string(value: str) -> str:
        """Encode string for contract call (simplified)"""
        # This is a simplified string encoding
        # Full implementation should follow ABI specification for strings
        encoded = value.encode('utf-8').hex()
        # Pad to multiple of 32 bytes (64 hex chars)
        padded_length = ((len(encoded) + 63) // 64) * 64
        return encoded.ljust(padded_length, '0')
    
    @staticmethod
    def encode_args(func_name: str, args: List[Any], abi: List[Dict] = None) -> str:
        """Encode function arguments
        
        Args:
            func_name: Function name for determining encoding strategy
            args: List of arguments to encode
            abi: Function ABI for proper parameter encoding
            
        Returns:
            Encoded arguments as hex string
        """
        if not args:
            return ""
        
        encoded_parts = []
        
        # Handle known function signatures
        if func_name in ['balanceOf', 'allowance'] and len(args) == 1:
            # ERC20 balanceOf(address), allowance(address,address) first param
            encoded_parts.append(ContractUtils.encode_address(str(args[0])))
        elif func_name == 'transfer' and len(args) == 2:
            # transfer(address,uint256)
            encoded_parts.append(ContractUtils.encode_address(str(args[0])))
            encoded_parts.append(ContractUtils.encode_uint256(int(args[1])))
        elif func_name == 'approve' and len(args) == 2:
            # approve(address,uint256)
            encoded_parts.append(ContractUtils.encode_address(str(args[0])))
            encoded_parts.append(ContractUtils.encode_uint256(int(args[1])))
        elif func_name == 'transferFrom' and len(args) == 3:
            # transferFrom(address,address,uint256)
            encoded_parts.append(ContractUtils.encode_address(str(args[0])))
            encoded_parts.append(ContractUtils.encode_address(str(args[1])))
            encoded_parts.append(ContractUtils.encode_uint256(int(args[2])))
        elif func_name == 'setValue' and len(args) == 1:
            # SimpleStorage setValue(uint256)
            encoded_parts.append(ContractUtils.encode_uint256(int(args[0])))
        elif func_name == 'transferOwnership' and len(args) == 1:
            # transferOwnership(address)
            encoded_parts.append(ContractUtils.encode_address(str(args[0])))
        else:
            # Generic encoding based on argument types
            for arg in args:
                if isinstance(arg, int):
                    encoded_parts.append(ContractUtils.encode_uint256(arg))
                elif isinstance(arg, str) and arg.startswith('0x'):
                    # Assume it's an address
                    encoded_parts.append(ContractUtils.encode_address(arg))
                elif isinstance(arg, str):
                    # Assume it's a string
                    encoded_parts.append(ContractUtils.encode_string(arg))
                else:
                    raise ValueError(f"Unsupported argument type: {type(arg)}")
        
        return ''.join(encoded_parts)
    
    @staticmethod
    def encode_function_call(func_name: str, args: List[Any] = None, 
                           abi: Dict = None, custom_selectors: Dict[str, str] = None) -> str:
        """Encode complete function call
        
        Args:
            func_name: Function name
            args: Function arguments
            abi: Contract ABI
            custom_selectors: Custom function selectors
            
        Returns:
            Complete call data (selector + encoded arguments)
        """
        # Get function selector
        selector = ContractUtils.get_function_selector(func_name, custom_selectors)
        
        # Remove 0x prefix for concatenation
        selector_data = selector[2:] if selector.startswith("0x") else selector
        
        # Encode arguments
        if args:
            arg_data = ContractUtils.encode_args(func_name, args, abi.get('functions', {}).get(func_name) if abi else None)
        else:
            arg_data = ""
        
        return "0x" + selector_data + arg_data
    
    @staticmethod
    def decode_uint256(hex_str: str) -> int:
        """Decode uint256 from hex string"""
        if hex_str.startswith("0x"):
            hex_str = hex_str[2:]
        # Pad to 64 characters if needed
        hex_str = hex_str.rjust(64, '0')
        return int(hex_str, 16) if hex_str else 0
    
    @staticmethod
    def decode_address(hex_str: str) -> str:
        """Decode address from hex string"""
        if hex_str.startswith("0x"):
            hex_str = hex_str[2:]
        # Take last 40 characters (20 bytes)
        hex_str = hex_str[-40:]
        return "0x" + hex_str.rjust(40, '0')
    
    @staticmethod
    def decode_result(func_name: str, result: str, abi: Dict = None) -> Any:
        """Decode function call result
        
        Args:
            func_name: Function name for determining decoding strategy
            result: Hex result string
            abi: Contract ABI for proper result decoding
            
        Returns:
            Decoded result
        """
        if not result or result == "0x":
            return None
        
        # Handle known return types
        if func_name in ['name', 'symbol']:
            # These should return strings, but we'll return raw hex for now
            # Full implementation would need proper string decoding
            return result
        elif func_name in ['decimals']:
            return ContractUtils.decode_uint256(result)
        elif func_name in ['totalSupply', 'balanceOf', 'allowance', 'getValue']:
            return ContractUtils.decode_uint256(result)
        elif func_name in ['owner']:
            return ContractUtils.decode_address(result)
        else:
            # Default to uint256 decoding
            return ContractUtils.decode_uint256(result)
    
    @staticmethod
    def validate_address(address: str) -> str:
        """Validate and normalize address"""
        if not address:
            raise ValueError("Address cannot be empty")
        
        if not isinstance(address, str):
            raise ValueError("Address must be a string")
        
        if not address.startswith("0x"):
            address = "0x" + address
        
        if len(address) != 42:
            raise ValueError(f"Invalid address length: {len(address)}")
        
        return address.lower()
    
    @staticmethod
    def validate_uint256(value: int) -> int:
        """Validate uint256 value"""
        if not isinstance(value, int):
            raise ValueError("Value must be an integer")
        
        if value < 0:
            raise ValueError("Value must be non-negative")
        
        if value > 2**256 - 1:
            raise ValueError("Value exceeds uint256 maximum")
        
        return value


def load_contract_data(contract_name: str, contracts_dir: Path = None) -> Dict:
    """Convenience function to load contract data"""
    return ContractUtils.load_contract_data(contract_name, contracts_dir)


def encode_function_call(func_name: str, args: List[Any] = None, 
                        abi: Dict = None, custom_selectors: Dict[str, str] = None) -> str:
    """Convenience function to encode function call"""
    return ContractUtils.encode_function_call(func_name, args, abi, custom_selectors)


def decode_result(func_name: str, result: str, abi: Dict = None) -> Any:
    """Convenience function to decode result"""
    return ContractUtils.decode_result(func_name, result, abi)


class ABICoder:
    """Advanced ABI encoding and decoding for complex data types"""
    
    # Basic type sizes (in bytes)
    TYPE_SIZES = {
        'uint8': 1, 'uint16': 2, 'uint24': 3, 'uint32': 4, 'uint40': 5, 'uint48': 6,
        'uint56': 7, 'uint64': 8, 'uint72': 9, 'uint80': 10, 'uint88': 11, 'uint96': 12,
        'uint104': 13, 'uint112': 14, 'uint120': 15, 'uint128': 16, 'uint136': 17,
        'uint144': 18, 'uint152': 19, 'uint160': 20, 'uint168': 21, 'uint176': 22,
        'uint184': 23, 'uint192': 24, 'uint200': 25, 'uint208': 26, 'uint216': 27,
        'uint224': 28, 'uint232': 29, 'uint240': 30, 'uint248': 31, 'uint256': 32,
        'int8': 1, 'int16': 2, 'int24': 3, 'int32': 4, 'int40': 5, 'int48': 6,
        'int56': 7, 'int64': 8, 'int72': 9, 'int80': 10, 'int88': 11, 'int96': 12,
        'int104': 13, 'int112': 14, 'int120': 15, 'int128': 16, 'int136': 17,
        'int144': 18, 'int152': 19, 'int160': 20, 'int168': 21, 'int176': 22,
        'int184': 23, 'int192': 24, 'int200': 25, 'int208': 26, 'int216': 27,
        'int224': 28, 'int232': 29, 'int240': 30, 'int248': 31, 'int256': 32,
        'address': 20, 'bool': 1
    }
    
    @staticmethod
    def parse_type(type_str: str) -> Tuple[str, int, List]:
        """Parse complex type like 'uint256[5][]' into (base_type, size, dimensions)
        
        Returns:
            (base_type, size, dimensions_list)
        """
        # Handle arrays
        if '[' in type_str:
            base_type = type_str.split('[')[0]
            dimensions = []
            temp = type_str
            
            while '[' in temp:
                start = temp.find('[')
                end = temp.find(']')
                if end == -1:
                    break
                    
                dim_str = temp[start+1:end]
                if dim_str == '':
                    dimensions.append(None)  # Dynamic array
                else:
                    dimensions.append(int(dim_str))
                
                temp = temp[end+1:]
            
            return base_type, ABICoder.TYPE_SIZES.get(base_type, 32), dimensions
        
        return type_str, ABICoder.TYPE_SIZES.get(type_str, 32), []
    
    @staticmethod
    def encode_single_value(value: Any, type_str: str) -> str:
        """Encode a single value according to ABI rules"""
        base_type, size, dimensions = ABICoder.parse_type(type_str)
        
        if dimensions:
            # It's an array
            return ABICoder.encode_array(value, base_type, size, dimensions)
        else:
            # It's a single value
            return ABICoder.encode_basic_type(value, base_type)
    
    @staticmethod
    def encode_basic_type(value: Any, type_str: str) -> str:
        """Encode basic types (uint, int, address, bool, string, bytes)"""
        if type_str.startswith('uint'):
            # Unsigned integer
            bits = int(type_str[4:]) if len(type_str) > 4 else 256
            max_val = (1 << bits) - 1
            if value < 0 or value > max_val:
                raise ValueError(f"Value {value} out of range for {type_str}")
            return format(value, '064x')
        
        elif type_str.startswith('int'):
            # Signed integer
            bits = int(type_str[3:]) if len(type_str) > 3 else 256
            if value < 0:
                # Two's complement for negative numbers
                value = (1 << bits) + value
            return format(value & ((1 << bits) - 1), '064x')
        
        elif type_str == 'address':
            # Address (20 bytes)
            if isinstance(value, str):
                if not value.startswith('0x'):
                    value = '0x' + value
                return value[2:].rjust(64, '0').lower()
            raise ValueError("Address must be a string")
        
        elif type_str == 'bool':
            # Boolean
            return format(1 if value else 0, '064x')
        
        elif type_str == 'string':
            # String (dynamic type)
            encoded_str = value.encode('utf-8').hex()
            # Pad to multiple of 64 characters (32 bytes)
            padding_needed = (64 - (len(encoded_str) % 64)) % 64
            encoded_str += '0' * padding_needed
            return encoded_str
        
        elif type_str.startswith('bytes'):
            # Fixed-size bytes
            if type_str == 'bytes':
                # Dynamic bytes
                encoded_bytes = value.hex() if isinstance(value, bytes) else value[2:] if isinstance(value, str) and value.startswith('0x') else str(value)
                # Add length prefix (32 bytes)
                length_hex = format(len(encoded_bytes) // 2, '064x')
                # Pad content to 32-byte boundary
                padding_needed = (64 - (len(encoded_bytes) % 64)) % 64
                return length_hex + encoded_bytes + '0' * padding_needed
            else:
                # Fixed-size bytes (e.g., bytes32)
                size = int(type_str[5:])
                if isinstance(value, bytes):
                    return value.hex().ljust(64, '0')
                elif isinstance(value, str):
                    if value.startswith('0x'):
                        value = value[2:]
                    return value.ljust(64, '0')
        
        else:
            raise ValueError(f"Unsupported type: {type_str}")
    
    @staticmethod
    def encode_array(value: List, base_type: str, size: int, dimensions: List) -> str:
        """Encode array types"""
        if len(dimensions) == 0:
            return ABICoder.encode_basic_type(value, base_type)
        
        if len(dimensions) == 1:
            # One-dimensional array
            encoded_parts = []
            for item in value:
                encoded_parts.append(ABICoder.encode_basic_type(item, base_type))
            return ''.join(encoded_parts)
        
        # Multi-dimensional array
        encoded_parts = []
        for item in value:
            encoded_parts.append(ABICoder.encode_array(item, base_type, size, dimensions[1:]))
        return ''.join(encoded_parts)
    
    @staticmethod
    def decode_single_value(hex_str: str, type_str: str) -> Any:
        """Decode a single value from hex according to ABI rules"""
        if not hex_str or hex_str == "0x":
            return None
        
        # Remove 0x prefix
        if hex_str.startswith('0x'):
            hex_str = hex_str[2:]
        
        base_type, size, dimensions = ABICoder.parse_type(type_str)
        
        if dimensions:
            # It's an array
            return ABICoder.decode_array(hex_str, base_type, size, dimensions)
        else:
            # It's a single value
            return ABICoder.decode_basic_type(hex_str, base_type)
    
    @staticmethod
    def decode_basic_type(hex_str: str, type_str: str) -> Any:
        """Decode basic types"""
        # Ensure we have 64 characters (32 bytes) for most types
        if type_str not in ['string', 'bytes'] and len(hex_str) < 64:
            hex_str = hex_str.rjust(64, '0')
        
        if type_str.startswith('uint'):
            # Unsigned integer
            return int(hex_str[:64], 16)
        
        elif type_str.startswith('int'):
            # Signed integer
            bits = int(type_str[3:]) if len(type_str) > 3 else 256
            value = int(hex_str[:64], 16)
            if value >= (1 << (bits - 1)):  # Negative number in two's complement
                value -= (1 << bits)
            return value
        
        elif type_str == 'address':
            # Address (20 bytes)
            addr_hex = hex_str[-40:]  # Take last 40 characters
            return '0x' + addr_hex.rjust(40, '0')
        
        elif type_str == 'bool':
            # Boolean
            return bool(int(hex_str[:64], 16))
        
        elif type_str == 'string':
            # String - remove trailing zeros
            hex_without_padding = hex_str.rstrip('0')
            if not hex_without_padding:
                return ""
            try:
                return bytes.fromhex(hex_without_padding).decode('utf-8')
            except:
                return hex_without_padding  # Fallback to hex string
        
        elif type_str.startswith('bytes'):
            if type_str == 'bytes':
                # Dynamic bytes - first 32 bytes is length
                length = int(hex_str[:64], 16)
                if length == 0:
                    return b''
                bytes_data = hex_str[64:64 + length * 2]
                return bytes.fromhex(bytes_data)
            else:
                # Fixed-size bytes
                return bytes.fromhex(hex_str[:64])
        
        else:
            # Unknown type, return as hex string
            return '0x' + hex_str
    
    @staticmethod
    def decode_array(hex_str: str, base_type: str, size: int, dimensions: List) -> Any:
        """Decode array types"""
        if len(dimensions) == 0:
            return ABICoder.decode_basic_type(hex_str, base_type)
        
        if len(dimensions) == 1:
            # One-dimensional array
            array_size = dimensions[0]
            if array_size is None:
                # Dynamic array - need to read length from first 32 bytes
                length = int(hex_str[:64], 16)
                hex_str = hex_str[64:]
                array_size = length
            
            item_size = 64  # Each item is 32 bytes in hex
            result = []
            for i in range(array_size):
                start_idx = i * item_size
                end_idx = start_idx + item_size
                if end_idx <= len(hex_str):
                    item_hex = hex_str[start_idx:end_idx]
                    result.append(ABICoder.decode_basic_type(item_hex, base_type))
            return result
        
        # Multi-dimensional array - simplified implementation
        result = []
        # This is a simplified approach - real implementation would need proper ABI layout
        if len(dimensions) >= 2:
            # Estimate array size based on remaining data
            item_count = len(hex_str) // 64
            sub_array_size = dimensions[1] if dimensions[1] else 1
            item_size = sub_array_size * 64
            
            for i in range(item_count):
                start_idx = i * item_size
                end_idx = start_idx + item_size
                if end_idx <= len(hex_str):
                    sub_hex = hex_str[start_idx:end_idx]
                    result.append(ABICoder.decode_array(sub_hex, base_type, size, dimensions[1:]))
        
        return result
    
    @staticmethod
    def decode_multiple_values(hex_str: str, types: List[str]) -> List[Any]:
        """Decode multiple values from concatenated hex string"""
        if not hex_str or hex_str == "0x":
            return [None] * len(types)
        
        if hex_str.startswith('0x'):
            hex_str = hex_str[2:]
        
        results = []
        current_pos = 0
        
        for type_str in types:
            base_type, size, dimensions = ABICoder.parse_type(type_str)
            
            if dimensions:
                # For arrays, we need special handling
                if dimensions[0] is None:
                    # Dynamic array - read length first
                    if current_pos + 64 <= len(hex_str):
                        length = int(hex_str[current_pos:current_pos + 64], 16)
                        # Calculate total size: length (32 bytes) + array items
                        total_size = 64 + (length * 64)  # Each array item is 32 bytes
                        item_hex = hex_str[current_pos:current_pos + total_size]
                        result = ABICoder.decode_single_value(item_hex, type_str)
                        results.append(result)
                        current_pos += total_size
                    else:
                        results.append(None)
                        break
                else:
                    # Fixed-size array
                    item_count = dimensions[0]
                    total_size = item_count * 64  # Each item is 32 bytes
                    if current_pos + total_size <= len(hex_str):
                        item_hex = hex_str[current_pos:current_pos + total_size]
                        result = ABICoder.decode_single_value(item_hex, type_str)
                        results.append(result)
                        current_pos += total_size
                    else:
                        results.append(None)
                        break
            else:
                # Single value (32 bytes)
                if current_pos + 64 <= len(hex_str):
                    item_hex = hex_str[current_pos:current_pos + 64]
                    result = ABICoder.decode_single_value(item_hex, type_str)
                    results.append(result)
                    current_pos += 64
                else:
                    results.append(None)
                    break
        
        return results


class StructCoder:
    """Coder for Solidity structs - handles complex data structures"""
    
    @staticmethod
    def encode_struct(struct_data: Dict, struct_abi: List[Dict]) -> str:
        """Encode a struct based on ABI definition
        
        Args:
            struct_data: Dictionary with struct field values
            struct_abi: List of field definitions from ABI
            
        Returns:
            Encoded hex string
        """
        encoded_parts = []
        
        for field in struct_abi:
            field_name = field['name']
            field_type = field['type']
            
            if field_name not in struct_data:
                raise ValueError(f"Missing field '{field_name}' in struct data")
            
            value = struct_data[field_name]
            
            if field_type.startswith('tuple'):
                # Nested struct
                nested_abi = field.get('components', [])
                encoded_value = StructCoder.encode_struct(value, nested_abi)
            elif field_type.startswith('tuple['):
                # Array of structs
                nested_abi = field.get('components', [])
                encoded_array = []
                for item in value:
                    encoded_array.append(StructCoder.encode_struct(item, nested_abi))
                encoded_value = ''.join(encoded_array)
            else:
                # Basic type
                encoded_value = ABICoder.encode_single_value(value, field_type)
            
            encoded_parts.append(encoded_value)
        
        return ''.join(encoded_parts)
    
    @staticmethod
    def decode_struct(hex_str: str, struct_abi: List[Dict]) -> Dict:
        """Decode struct data based on ABI definition
        
        Args:
            hex_str: Hex string to decode
            struct_abi: List of field definitions from ABI
            
        Returns:
            Dictionary with decoded struct fields
        """
        if hex_str.startswith('0x'):
            hex_str = hex_str[2:]
        
        result = {}
        current_pos = 0
        
        for field in struct_abi:
            field_name = field['name']
            field_type = field['type']
            
            if field_type.startswith('tuple'):
                # Nested struct
                nested_abi = field.get('components', [])
                # Simplified: assume struct fits in remaining bytes
                nested_hex = hex_str[current_pos:]
                result[field_name] = StructCoder.decode_struct(nested_hex, nested_abi)
                # Estimate size (simplified)
                current_pos += len(nested_hex)
            
            elif field_type.startswith('tuple['):
                # Array of structs
                nested_abi = field.get('components', [])
                # This is complex - simplified implementation
                result[field_name] = []  # Placeholder
            
            else:
                # Basic type
                if current_pos + 64 <= len(hex_str):
                    item_hex = hex_str[current_pos:current_pos + 64]
                    result[field_name] = ABICoder.decode_single_value(item_hex, field_type)
                    current_pos += 64
                else:
                    result[field_name] = None
                    break
        
        return result


class MappingDecoder:
    """Helper for decoding mapping-like structures from view functions"""
    
    @staticmethod
    def decode_mapping_result(hex_str: str, key_type: str, value_type: str) -> Any:
        """Decode a mapping access result
        
        This is typically used for view functions that return mapping[key]
        """
        return ABICoder.decode_single_value(hex_str, value_type)
    
    @staticmethod
    def decode_multiple_mappings(hex_str: str, mappings: List[Tuple[str, str]]) -> List[Any]:
        """Decode multiple mapping results
        
        Args:
            hex_str: Concatenated hex results
            mappings: List of (key_type, value_type) tuples
        """
        types = [value_type for _, value_type in mappings]
        return ABICoder.decode_multiple_values(hex_str, types)


# Enhanced convenience functions

def encode_complex_call(func_name: str, args: List[Any], abi: Dict) -> str:
    """Encode function call with complex argument types
    
    Args:
        func_name: Function name
        args: Function arguments
        abi: Function ABI with parameter types
        
    Returns:
        Encoded call data
    """
    # Get function selector
    selector = ContractUtils.get_function_selector(func_name)
    
    if not args:
        return selector
    
    # Get parameter types from ABI
    if isinstance(abi, dict) and 'functions' in abi:
        func_abi = abi['functions'].get(func_name, {})
    elif isinstance(abi, list):
        func_abi = next((item for item in abi if item.get('name') == func_name), {})
    else:
        func_abi = {}
    
    inputs = func_abi.get('inputs', [])
    
    if len(args) != len(inputs):
        raise ValueError(f"Argument count mismatch: expected {len(inputs)}, got {len(args)}")
    
    # Encode each argument according to its type
    encoded_args = ""
    for arg, input_def in zip(args, inputs):
        arg_type = input_def['type']
        
        if arg_type.startswith('tuple'):
            # Struct argument
            components = input_def.get('components', [])
            encoded_value = StructCoder.encode_struct(arg, components)
        else:
            # Basic type
            encoded_value = ABICoder.encode_single_value(arg, arg_type)
        
        encoded_args += encoded_value
    
    return selector + encoded_args


def decode_complex_result(hex_str: str, func_name: str, abi: Dict) -> Any:
    """Decode function call result with complex return types
    
    Args:
        hex_str: Hex result string
        func_name: Function name
        abi: Function ABI with return type
        
    Returns:
        Decoded result
    """
    if not hex_str or hex_str == "0x":
        return None
    
    # Get return type from ABI
    if isinstance(abi, dict) and 'functions' in abi:
        func_abi = abi['functions'].get(func_name, {})
    elif isinstance(abi, list):
        func_abi = next((item for item in abi if item.get('name') == func_name), {})
    else:
        func_abi = {}
    
    outputs = func_abi.get('outputs', [])
    
    if not outputs:
        # Unknown return type, use basic decoding
        return ContractUtils.decode_result(func_name, hex_str)
    
    if len(outputs) == 1:
        # Single return value
        output_type = outputs[0]['type']
        if output_type.startswith('tuple'):
            # Struct return
            components = outputs[0].get('components', [])
            return StructCoder.decode_struct(hex_str, components)
        else:
            # Basic type return
            return ABICoder.decode_single_value(hex_str, output_type)
    else:
        # Multiple return values
        types = [output['type'] for output in outputs]
        return ABICoder.decode_multiple_values(hex_str, types)