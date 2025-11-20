"""
Shared contracts management for reusing deployed contracts across tests
"""
import json
import logging
import time
from datetime import datetime
from pathlib import Path
from typing import Dict, List, Optional, Callable, Any

from eth_account import Account
from ..utils.contract_utils import ContractUtils
from ..utils.contract_caller import ContractCaller

LOG = logging.getLogger(__name__)


class ContractRegistry:
    """Registry for managing deployed contracts across test sessions"""
    
    def __init__(self, registry_file: str = None):
        """Initialize contract registry
        
        Args:
            registry_file: Path to registry file for persisting contract info
        """
        if registry_file is None:
            # Default registry file in output directory
            registry_file = Path(__file__).parent.parent.parent.parent / "output" / "contract_registry.json"
        
        self.registry_file = Path(registry_file)
        self.registry_file.parent.mkdir(parents=True, exist_ok=True)
        
        self._contracts = {}
        self._load_registry()
        
        LOG.debug(f"ContractRegistry initialized with {len(self._contracts)} contracts")
    
    def _load_registry(self):
        """Load contract registry from file"""
        if self.registry_file.exists():
            try:
                with open(self.registry_file, 'r') as f:
                    self._contracts = json.load(f)
                LOG.info(f"Loaded {len(self._contracts)} contracts from registry")
            except Exception as e:
                LOG.warning(f"Failed to load contract registry: {e}, starting fresh")
                self._contracts = {}
        else:
            LOG.info("No existing registry found, starting fresh")
    
    def _save_registry(self):
        """Save contract registry to file"""
        try:
            with open(self.registry_file, 'w') as f:
                json.dump(self._contracts, f, indent=2)
            LOG.debug(f"Saved contract registry to {self.registry_file}")
        except Exception as e:
            LOG.error(f"Failed to save contract registry: {e}")
    
    def get_contract(self, name: str) -> Optional[Dict]:
        """Get contract info from registry
        
        Args:
            name: Contract name or key
            
        Returns:
            Contract info dict or None if not found
        """
        return self._contracts.get(name)
    
    def register_contract(self, name: str, contract_info: Dict):
        """Register a contract in the registry
        
        Args:
            name: Contract name/key
            contract_info: Contract information dict
        """
        contract_info['registered_at'] = datetime.now().isoformat()
        self._contracts[name] = contract_info
        self._save_registry()
        LOG.info(f"Registered contract '{name}' at {contract_info.get('address')}")
    
    def unregister_contract(self, name: str):
        """Remove contract from registry"""
        if name in self._contracts:
            address = self._contracts[name].get('address', 'unknown')
            del self._contracts[name]
            self._save_registry()
            LOG.info(f"Unregistered contract '{name}' at {address}")
    
    def list_contracts(self) -> List[str]:
        """List all registered contract names"""
        return list(self._contracts.keys())
    
    def clear_registry(self):
        """Clear all contracts from registry"""
        self._contracts.clear()
        self._save_registry()
        LOG.info("Cleared contract registry")


class SharedContracts:
    """Manager for sharing contracts across test cases"""
    
    def __init__(self, run_helper, registry_file: str = None):
        """Initialize shared contracts manager
        
        Args:
            run_helper: RunHelper instance for accessing client and utilities
            registry_file: Optional custom registry file path
        """
        self.run_helper = run_helper
        self.client = run_helper.client
        self.registry = ContractRegistry(registry_file)
        self._deployed_contracts = {}
        
        LOG.info("SharedContracts manager initialized")
    
    async def get_or_deploy(self, contract_name: str, deploy_func: Callable, 
                           force_redeploy: bool = False) -> Dict:
        """Get existing contract or deploy new one
        
        Args:
            contract_name: Name/key for the contract
            deploy_func: Async function that deploys the contract
            force_redeploy: Whether to force redeploy even if exists
            
        Returns:
            Contract information dict with address, abi, etc.
        """
        # Check if we already have it in memory
        if contract_name in self._deployed_contracts and not force_redeploy:
            LOG.info(f"Using cached {contract_name} contract")
            return self._deployed_contracts[contract_name]
        
        # Check registry
        if not force_redeploy:
            registry_info = self.registry.get_contract(contract_name)
            if registry_info:
                # Verify contract still exists on chain
                if await self._verify_contract_exists(registry_info['address']):
                    LOG.info(f"Using registered {contract_name} contract at {registry_info['address']}")
                    self._deployed_contracts[contract_name] = registry_info
                    return registry_info
                else:
                    LOG.warning(f"Contract {contract_name} no longer exists on chain, will redeploy")
                    self.registry.unregister_contract(contract_name)
        
        # Deploy new contract
        LOG.info(f"Deploying new {contract_name} contract")
        contract_info = await deploy_func()
        
        # Verify deployment
        if not await self._verify_contract_exists(contract_info['address']):
            raise RuntimeError(f"Contract deployment verification failed for {contract_name}")
        
        # Register contract
        self.registry.register_contract(contract_name, contract_info)
        self._deployed_contracts[contract_name] = contract_info
        
        return contract_info
    
    async def _verify_contract_exists(self, address: str) -> bool:
        """Verify contract exists on chain"""
        try:
            code = await self.client.get_code(address)
            return code and code != "0x" and len(code) > 2
        except Exception as e:
            LOG.warning(f"Failed to verify contract at {address}: {e}")
            return False
    
    async def get_contract_caller(self, contract_name: str, abi: Dict = None) -> ContractCaller:
        """Get ContractCaller for a registered contract
        
        Args:
            contract_name: Contract name/key
            abi: Contract ABI (optional, will try to load if not provided)
            
        Returns:
            ContractCaller instance
        """
        contract_info = await self.get_or_deploy(contract_name, None)  # Just get existing
        
        if abi is None and 'abi' in contract_info:
            abi = contract_info['abi']
        
        return ContractCaller(self.client, contract_info['address'], abi)
    
    def clear_cache(self):
        """Clear in-memory contract cache"""
        self._deployed_contracts.clear()
        LOG.info("Cleared in-memory contract cache")
    
    async def cleanup_invalid_contracts(self):
        """Remove contracts that no longer exist on chain from registry"""
        for contract_name in self.registry.list_contracts():
            contract_info = self.registry.get_contract(contract_name)
            if contract_info and not await self._verify_contract_exists(contract_info['address']):
                LOG.warning(f"Removing invalid contract {contract_name} from registry")
                self.registry.unregister_contract(contract_name)


class ContractDeployer:
    """Utility class for deploying contracts with standardized patterns"""
    
    def __init__(self, run_helper):
        """Initialize contract deployer
        
        Args:
            run_helper: RunHelper instance
        """
        self.run_helper = run_helper
        self.client = run_helper.client
        LOG.debug("ContractDeployer initialized")
    
    async def deploy_simple_contract(self, contract_name: str, deployer_account: Dict = None,
                                    gas_limit: int = 200000, value: int = 0) -> Dict:
        """Deploy a simple contract (no constructor args)
        
        Args:
            contract_name: Name of contract (must have JSON file)
            deployer_account: Account to deploy from (will create if None)
            gas_limit: Gas limit for deployment
            value: ETH value to send with deployment
            
        Returns:
            Contract information dict
        """
        # Load contract data
        contract_data = ContractUtils.load_contract_data(contract_name)
        bytecode = contract_data['bytecode']
        abi = contract_data.get('abi', {})
        
        # Create deployer account if not provided
        if deployer_account is None:
            deployer_account = await self.run_helper.create_test_account(
                f"{contract_name}_deployer", 
                fund_wei=max(1 * 10**18, value + 1000000 * 200000)  # Enough for gas
            )
        
        return await self._deploy_from_bytecode(
            bytecode, abi, deployer_account, gas_limit, value, contract_name
        )
    
    async def deploy_contract_with_args(self, contract_name: str, constructor_args: List[Any],
                                       deployer_account: Dict = None, gas_limit: int = 300000,
                                       value: int = 0) -> Dict:
        """Deploy contract with constructor arguments
        
        Args:
            contract_name: Name of contract
            constructor_args: Constructor arguments
            deployer_account: Account to deploy from
            gas_limit: Gas limit for deployment
            value: ETH value to send
            
        Returns:
            Contract information dict
        """
        # For now, this is a simplified version
        # Full implementation would need to encode constructor arguments
        return await self.deploy_simple_contract(contract_name, deployer_account, gas_limit, value)
    
    async def _deploy_from_bytecode(self, bytecode: str, abi: Dict, deployer_account: Dict,
                                  gas_limit: int, value: int, contract_name: str) -> Dict:
        """Deploy contract from bytecode"""
        # Get nonce and gas price
        nonce = await self.client.get_transaction_count(deployer_account['address'])
        gas_price = await self.client.get_gas_price()
        
        # Build deployment transaction
        deploy_tx = {
            "data": bytecode,
            "gas": hex(gas_limit),
            "gasPrice": hex(gas_price),
            "nonce": hex(nonce),
            "chainId": hex(await self.client.get_chain_id()),
            "value": hex(value) if value else "0x0"
        }
        
        # Sign transaction
        private_key = deployer_account['private_key']
        if private_key.startswith('0x'):
            private_key = private_key[2:]
        
        signed_tx = Account.sign_transaction(deploy_tx, private_key)
        
        LOG.info(f"Deploying {contract_name} from {deployer_account['address']}")
        
        # Send transaction
        tx_hash = await self.client.send_raw_transaction(signed_tx.raw_transaction)
        
        # Wait for confirmation
        receipt = await self.client.wait_for_transaction_receipt(tx_hash, timeout=60)
        
        if receipt.get("status") != "0x1":
            raise RuntimeError(f"{contract_name} deployment failed: {receipt}")
        
        contract_address = receipt.get("contractAddress")
        if not contract_address:
            raise RuntimeError(f"No contract address in deployment receipt for {contract_name}")
        
        gas_used = int(receipt.get("gasUsed", "0x0"), 16)
        
        contract_info = {
            "name": contract_name,
            "address": contract_address,
            "abi": abi,
            "bytecode": bytecode,
            "deploy_tx": tx_hash,
            "deployer": deployer_account['address'],
            "gas_used": gas_used,
            "gas_limit": gas_limit,
            "deployed_at": datetime.now().isoformat()
        }
        
        LOG.info(f"{contract_name} deployed successfully at {contract_address}")
        LOG.info(f"Deployment gas used: {gas_used}")
        
        return contract_info


# Common deployment functions for standard contracts

async def deploy_simple_storage(run_helper, shared_contracts: SharedContracts = None) -> Dict:
    """Deploy SimpleStorage contract"""
    deployer = ContractDeployer(run_helper)
    
    async def deploy_func():
        return await deployer.deploy_simple_contract("SimpleStorage")
    
    if shared_contracts:
        return await shared_contracts.get_or_deploy("SimpleStorage", deploy_func)
    else:
        return await deploy_func()


async def deploy_simple_token(run_helper, token_name: str = "TestToken", 
                             shared_contracts: SharedContracts = None) -> Dict:
    """Deploy SimpleToken contract"""
    deployer = ContractDeployer(run_helper)
    
    async def deploy_func():
        return await deployer.deploy_simple_contract(token_name)
    
    contract_key = f"ERC20_{token_name}"
    
    if shared_contracts:
        return await shared_contracts.get_or_deploy(contract_key, deploy_func)
    else:
        return await deploy_func()


# Utility functions for creating standard contract callers

async def get_erc20_caller(run_helper, token_name: str = "SimpleToken") -> ContractCaller:
    """Get ERC20 contract caller (deploy if needed)"""
    shared_contracts = SharedContracts(run_helper)
    
    # Get or deploy contract
    contract_info = await deploy_simple_token(run_helper, token_name, shared_contracts)
    
    # Create caller
    return ContractCaller(run_helper.client, contract_info['address'], contract_info.get('abi'))


async def get_storage_caller(run_helper, contract_name: str = "SimpleStorage") -> ContractCaller:
    """Get storage contract caller (deploy if needed)"""
    shared_contracts = SharedContracts(run_helper)
    
    # Get or deploy contract
    contract_info = await deploy_simple_storage(run_helper, shared_contracts)
    
    # Create caller
    return ContractCaller(run_helper.client, contract_info['address'], contract_info.get('abi'))