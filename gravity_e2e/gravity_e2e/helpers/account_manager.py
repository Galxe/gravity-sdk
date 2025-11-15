import asyncio
import json
import logging
import os
from typing import Dict, Optional
from datetime import datetime
from eth_account import Account

from ..utils.common import format_timestamp

LOG = logging.getLogger(__name__)


class TestAccountManager:
    """Test account manager"""
    
    def __init__(self, accounts_config_path: str = "configs/test_accounts.json"):
        self.accounts_config_path = accounts_config_path
        self.accounts: Dict[str, Dict] = {}
        self.faucet: Optional[Dict] = None
        self._lock = asyncio.Lock()
        self.load_accounts()
        
    def load_accounts(self):
        """Load test account configuration"""
        try:
            with open(self.accounts_config_path, 'r') as f:
                config = json.load(f)
                
            # Load faucet account only
            if "faucet" in config:
                self.faucet = config["faucet"]
                LOG.info(f"Loaded faucet account: {self.faucet['address']}")
            else:
                LOG.warning("No faucet account configured")
                
        except FileNotFoundError:
            LOG.warning(f"Accounts config file not found: {self.accounts_config_path}")
        except json.JSONDecodeError as e:
            LOG.error(f"Invalid JSON in accounts config: {e}")
            
    def get_account(self, name: str) -> Optional[Dict]:
        """Get test account"""
        return self.accounts.get(name)
        
    def get_faucet(self) -> Optional[Dict]:
        """Get faucet account"""
        return self.faucet
        
    async def create_test_account(self, name: str, save_immediately: bool = False) -> Dict:
        """Create new test account at runtime
        
        Args:
            name: Account name
            save_immediately: Whether to save to file immediately
            
        Returns:
            Account information dictionary
        """
        async with self._lock:
            if name in self.accounts:
                LOG.warning(f"Account '{name}' already exists, returning existing account")
                return self.accounts[name]
                
            account = Account.create()
            account_info = {
                "name": name,
                "address": account.address,
                "private_key": account.key.hex(),
                "account": account,
                "created_at": format_timestamp()
            }
            
            self.accounts[name] = account_info
            
            # Log account information
            LOG.info(f"Created test account '{name}':")
            LOG.info(f"  Address: {account.address}")
            LOG.info(f"  Private Key: {account.key.hex()}")
            
            if save_immediately:
                await self._save_accounts_async()
                
            return account_info
            
    def save_test_accounts(self, file_path: str = None):
        """Save all generated test accounts to file (synchronous version)"""
        if file_path is None:
            file_path = self.accounts_config_path.replace('.json', '_generated.json')
            
        accounts_to_save = {
            "generated_at": format_timestamp(),
            "faucet": self.faucet,
            "test_accounts": [
                {
                    "name": name,
                    "address": info["address"],
                    "private_key": info["private_key"],
                    "created_at": info.get("created_at")
                }
                for name, info in self.accounts.items()
            ]
        }
        
        try:
            # Create directory if it doesn't exist
            os.makedirs(os.path.dirname(file_path), exist_ok=True)
            
            with open(file_path, 'w') as f:
                json.dump(accounts_to_save, f, indent=2)
            LOG.info(f"Saved test accounts to: {file_path}")
        except Exception as e:
            LOG.error(f"Failed to save test accounts: {e}")
            
    async def _save_accounts_async(self, file_path: str = None):
        """Asynchronously save accounts"""
        loop = asyncio.get_event_loop()
        await loop.run_in_executor(None, self.save_test_accounts, file_path)
        
    def get_or_create_account(self, name: str) -> Dict:
        """Get existing account or create new one"""
        if name in self.accounts:
            return self.accounts[name]
        return asyncio.create_task(self.create_test_account(name))