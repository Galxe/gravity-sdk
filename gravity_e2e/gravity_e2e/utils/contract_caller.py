"""
Contract caller for unified contract interaction
"""
import logging
from typing import Any, Dict, List, Optional, Union
from eth_account import Account
from eth_utils import to_checksum_address

from ..utils.contract_utils import ContractUtils, ABICoder, StructCoder, decode_complex_result
from ..core.client.gravity_client import GravityClient

LOG = logging.getLogger(__name__)


class ContractCaller:
    """Unified contract interaction class"""
    
    def __init__(self, client: GravityClient, contract_address: str, abi: Dict = None):
        """Initialize contract caller
        
        Args:
            client: Gravity RPC client
            contract_address: Contract address
            abi: Contract ABI (optional, for enhanced functionality)
        """
        self.client = client
        self.address = ContractUtils.validate_address(contract_address)
        self.abi = abi or {}
        
        # Extract function selectors from ABI if available
        self._function_selectors = self._build_selectors_from_abi() if abi else {}
        
        # Merge with standard selectors
        self._function_selectors.update(ContractUtils.ERC20_SELECTORS)
        self._function_selectors.update(ContractUtils.COMMON_SELECTORS)
        
        LOG.debug(f"ContractCaller initialized for {self.address}")
    
    def _build_selectors_from_abi(self) -> Dict[str, str]:
        """Build function selectors from contract ABI"""
        selectors = {}
        if not self.abi or not isinstance(self.abi, dict):
            return selectors
        
        # Handle different ABI formats
        functions = None
        if 'functions' in self.abi:
            functions = self.abi['functions']
        elif isinstance(self.abi, list):
            functions = {item['name']: item for item in self.abi if item['type'] == 'function'}
        
        if functions:
            for func_name, func_data in functions.items():
                # For now, use standard selectors for known functions
                # In a full implementation, you'd calculate keccak256 hashes
                if func_name in ContractUtils.ERC20_SELECTORS:
                    selectors[func_name] = ContractUtils.ERC20_SELECTORS[func_name]
                elif func_name in ContractUtils.COMMON_SELECTORS:
                    selectors[func_name] = ContractUtils.COMMON_SELECTORS[func_name]
                else:
                    # Generate simplified selector (not standard keccak256)
                    import hashlib
                    if isinstance(func_data, dict) and 'inputs' in func_data:
                        input_types = [inp['type'] for inp in func_data['inputs']]
                        signature = f"{func_name}({','.join(input_types)})"
                    else:
                        signature = f"{func_name}()"
                    selector_hash = hashlib.sha256(signature.encode()).hexdigest()[:8]
                    selectors[func_name] = f"0x{selector_hash}"
        
        return selectors
    
    async def call(self, method_name: str, *args, **kwargs) -> Any:
        """Make a read-only contract call
        
        Args:
            method_name: Contract method name
            *args: Method arguments
            **kwargs: Additional options:
                - from_: Caller address
                - block: Block number (default: 'latest')
                - return_types: Expected return types for complex decoding
                - use_abi: Whether to use ABI-based decoding (default: True if ABI available)
            
        Returns:
            Decoded method result
        """
        if method_name not in self._function_selectors:
            raise ValueError(f"Method '{method_name}' not found in contract ABI or standard selectors")
        
        # Build call data with enhanced encoding
        if self.abi and kwargs.get('use_abi', True):
            # Use ABI-based encoding for complex types
            from ..utils.contract_utils import encode_complex_call
            call_data = encode_complex_call(method_name, list(args), self.abi)
        else:
            # Use basic encoding
            call_data = ContractUtils.encode_function_call(
                method_name, 
                list(args), 
                self.abi,
                self._function_selectors
            )
        
        # Extract additional options
        from_ = kwargs.get('from_')
        block = kwargs.get('block', 'latest')
        return_types = kwargs.get('return_types')
        
        LOG.debug(f"Calling {method_name} on {self.address} with data: {call_data[:50]}...")
        
        # Make the call
        result = await self.client.call(
            to=self.address,
            data=call_data,
            from_=from_,
            block=block
        )
        
        # Enhanced result decoding
        if return_types:
            # Use provided return types
            decoded_result = ABICoder.decode_multiple_values(result, return_types)
        elif self.abi and kwargs.get('use_abi', True):
            # Use ABI-based decoding
            decoded_result = decode_complex_result(result, method_name, self.abi)
        else:
            # Use basic decoding
            decoded_result = ContractUtils.decode_result(method_name, result, self.abi)
        
        LOG.debug(f"{method_name} result: {decoded_result}")
        return decoded_result
    
    async def send_transaction(self, method_name: str, *args, 
                             from_account: Dict, value: int = 0, 
                             gas_limit: int = None, gas_price: int = None,
                             **kwargs) -> str:
        """Send a transaction that modifies contract state
        
        Args:
            method_name: Contract method name
            *args: Method arguments
            from_account: Account dict with 'private_key' and 'address'
            value: ETH value to send with transaction
            gas_limit: Gas limit (optional, will estimate if not provided)
            gas_price: Gas price (optional, will use current if not provided)
            **kwargs: Additional options
            
        Returns:
            Transaction hash
        """
        if method_name not in self._function_selectors:
            raise ValueError(f"Method '{method_name}' not found in contract ABI or standard selectors")
        
        # Build call data
        call_data = ContractUtils.encode_function_call(
            method_name, 
            list(args), 
            self.abi,
            self._function_selectors
        )
        
        # Get nonce
        nonce = await self.client.get_transaction_count(from_account['address'])
        
        # Get gas price if not provided
        if gas_price is None:
            gas_price = await self.client.get_gas_price()
        
        # Estimate gas if not provided
        if gas_limit is None:
            tx_for_estimation = {
                "to": self.address,
                "data": call_data,
                "from": from_account['address'],
                "value": hex(value) if value else "0x0"
            }
            try:
                gas_limit = await self.client.estimate_gas(tx_for_estimation)
            except Exception as e:
                LOG.warning(f"Gas estimation failed: {e}, using default")
                gas_limit = 200000  # Default for simple calls
        
        # Build transaction
        tx_data = {
            "to": to_checksum_address(self.address),
            "data": call_data,
            "gas": hex(gas_limit),
            "gasPrice": hex(gas_price),
            "nonce": hex(nonce),
            "chainId": hex(await self.client.get_chain_id()),
            "value": hex(value) if value else "0x0"
        }
        
        # Sign transaction
        private_key = from_account['private_key']
        if private_key.startswith('0x'):
            private_key = private_key[2:]
        
        signed_tx = Account.sign_transaction(tx_data, private_key)
        
        LOG.debug(f"Sending transaction for {method_name} from {from_account['address']}")
        
        # Send transaction
        tx_hash = await self.client.send_raw_transaction(signed_tx.raw_transaction)
        
        return tx_hash
    
    async def send_and_wait(self, method_name: str, *args, 
                           from_account: Dict, timeout: int = 120,
                           **kwargs) -> Dict:
        """Send transaction and wait for confirmation
        
        Args:
            method_name: Contract method name
            *args: Method arguments
            from_account: Account dict
            timeout: Wait timeout in seconds
            **kwargs: Additional transaction options
            
        Returns:
            Transaction receipt
        """
        tx_hash = await self.send_transaction(method_name, *args, 
                                           from_account=from_account, 
                                           **kwargs)
        
        LOG.info(f"Transaction sent: {tx_hash}, waiting for confirmation...")
        
        # Wait for receipt
        receipt = await self.client.wait_for_transaction_receipt(tx_hash, timeout=timeout)
        
        if receipt.get("status") != "0x1":
            raise RuntimeError(f"Transaction failed: {tx_hash}")
        
        LOG.info(f"Transaction confirmed in block: {receipt.get('blockNumber')}")
        return receipt
    
    # Convenience methods for common operations
    
    async def get_balance(self, address: str = None) -> int:
        """Get ETH balance of contract or specific address"""
        if address is None:
            return await self.client.get_balance(self.address)
        return await self.client.get_balance(address)
    
    async def get_code(self) -> str:
        """Get contract code"""
        return await self.client.get_code(self.address)
    
    def has_method(self, method_name: str) -> bool:
        """Check if contract has a specific method"""
        return method_name in self._function_selectors
    
    def get_available_methods(self) -> List[str]:
        """Get list of available methods"""
        return list(self._function_selectors.keys())
    
    def __str__(self) -> str:
        return f"ContractCaller(address={self.address}, methods={len(self._function_selectors)})"
    
    def __repr__(self) -> str:
        return self.__str__()


class ContractFactory:
    """Factory for creating ContractCaller instances"""
    
    @staticmethod
    async def from_address(contract_address: str, client: GravityClient, 
                          abi: Dict = None) -> ContractCaller:
        """Create ContractCaller from address
        
        Args:
            contract_address: Contract address
            client: Gravity RPC client
            abi: Contract ABI (optional)
            
        Returns:
            ContractCaller instance
        """
        return ContractCaller(client, contract_address, abi)
    
    @staticmethod
    async def from_contract_name(contract_name: str, client: GravityClient,
                                contracts_dir_path: str = None,
                                deployed_address: str = None) -> ContractCaller:
        """Create ContractCaller from contract name
        
        Args:
            contract_name: Name of contract (e.g., "SimpleStorage")
            client: Gravity RPC client
            contracts_dir_path: Path to contracts directory
            deployed_address: Deployed contract address (if None, will try to load from saved results)
            
        Returns:
            ContractCaller instance
        """
        from pathlib import Path
        
        if contracts_dir_path:
            contracts_dir = Path(contracts_dir_path)
        else:
            contracts_dir = Path(__file__).parent.parent.parent.parent / "contracts_data"
        
        # Load contract data
        contract_data = ContractUtils.load_contract_data(contract_name, contracts_dir)
        
        # If no address provided, try to get from saved results
        if deployed_address is None:
            deployed_address = ContractFactory._get_saved_address(contract_name, client)
        
        if not deployed_address:
            raise ValueError(f"No deployed address provided for contract {contract_name}")
        
        return ContractCaller(client, deployed_address, contract_data)
    
    @staticmethod
    def _get_saved_address(contract_name: str, client: GravityClient) -> Optional[str]:
        """Try to get saved contract address from previous test results"""
        # This is a simplified implementation
        # In practice, you'd search through test results or a contract registry
        return None


# Convenience functions for creating common contract callers

async def create_erc20_caller(token_address: str, client: GravityClient) -> ContractCaller:
    """Create a ContractCaller optimized for ERC20 tokens"""
    return ContractCaller(client, token_address)


async def create_storage_caller(storage_address: str, client: GravityClient) -> ContractCaller:
    """Create a ContractCaller optimized for storage contracts"""
    return ContractCaller(client, storage_address)