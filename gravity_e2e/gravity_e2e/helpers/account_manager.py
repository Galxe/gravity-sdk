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
    """测试账户管理器"""
    
    def __init__(self, accounts_config_path: str = "configs/test_accounts.json"):
        self.accounts_config_path = accounts_config_path
        self.accounts: Dict[str, Dict] = {}
        self.faucet: Optional[Dict] = None
        self._lock = asyncio.Lock()
        self.load_accounts()
        
    def load_accounts(self):
        """加载测试账户配置"""
        try:
            with open(self.accounts_config_path, 'r') as f:
                config = json.load(f)
                
            # 只加载水龙头账户（预设的有余额账户）
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
        """获取测试账户"""
        return self.accounts.get(name)
        
    def get_faucet(self) -> Optional[Dict]:
        """获取水龙头账户"""
        return self.faucet
        
    async def create_test_account(self, name: str, save_immediately: bool = False) -> Dict:
        """创建新的测试账户（运行时）
        
        Args:
            name: 账户名称
            save_immediately: 是否立即保存到文件
            
        Returns:
            账户信息字典
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
            
            # 打印账户信息到日志
            LOG.info(f"Created test account '{name}':")
            LOG.info(f"  Address: {account.address}")
            LOG.info(f"  Private Key: {account.key.hex()}")
            
            if save_immediately:
                await self._save_accounts_async()
                
            return account_info
            
    def save_test_accounts(self, file_path: str = None):
        """保存所有生成的测试账户到文件（同步版本）"""
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
            # 创建目录（如果不存在）
            os.makedirs(os.path.dirname(file_path), exist_ok=True)
            
            with open(file_path, 'w') as f:
                json.dump(accounts_to_save, f, indent=2)
            LOG.info(f"Saved test accounts to: {file_path}")
        except Exception as e:
            LOG.error(f"Failed to save test accounts: {e}")
            
    async def _save_accounts_async(self, file_path: str = None):
        """异步保存账户"""
        loop = asyncio.get_event_loop()
        await loop.run_in_executor(None, self.save_test_accounts, file_path)
        
    def get_or_create_account(self, name: str) -> Dict:
        """获取已存在账户或创建新账户"""
        if name in self.accounts:
            return self.accounts[name]
        return asyncio.create_task(self.create_test_account(name))