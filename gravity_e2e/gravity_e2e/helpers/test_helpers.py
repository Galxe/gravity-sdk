import asyncio
import logging
from typing import Dict, Optional
from eth_account import Account
from web3 import Web3

from .account_manager import TestAccountManager
from ..core.client.gravity_client import GravityClient

LOG = logging.getLogger(__name__)


class TestResult:
    """Test results"""
    def __init__(self, test_name: str):
        self.test_name = test_name
        self.success = False
        self.error = None
        self.start_time = None
        self.end_time = None
        self.details = {}
        
    def mark_success(self, **details):
        """Mark test as successful"""
        self.success = True
        if details:
            self.details.update(details)
            
    def mark_failure(self, error: str, **details):
        """Mark test as failed"""
        self.success = False
        self.error = error
        if details:
            self.details.update(details)
            
    def set_duration(self, duration: float):
        """Set test duration"""
        self.details["duration"] = duration
    
    def to_dict(self):
        """Convert to dictionary for JSON serialization"""
        return {
            "test_name": self.test_name,
            "success": self.success,
            "error": self.error,
            "start_time": self.start_time,
            "end_time": self.end_time,
            "details": self.details
        }


class RunHelper:
    """Test execution helper"""
    
    def __init__(self, client: GravityClient, working_dir: str, faucet_account: Optional[Dict] = None):
        self.client = client
        self.working_dir = working_dir
        self.faucet_account = faucet_account
        
    async def create_test_account(self, name: str, fund_wei: Optional[int] = None) -> Dict:
        """Create and optionally fund test account
        
        Args:
            name: Account name
            fund_wei: Funding amount in wei, None means no funding
            
        Returns:
            Account information dictionary
        """
        from eth_account import Account
        
        account = Account.create()
        account_info = {
            "name": name,
            "address": account.address,
            "private_key": account.key.hex(),
            "account": account
        }
        
        # Log account information
        LOG.info(f"Created test account '{name}':")
        LOG.info(f"  Address: {account.address}")
        LOG.info(f"  Private Key: {account.key.hex()}")
        
        # Fund account if needed and faucet is configured
        if fund_wei and fund_wei > 0 and self.faucet_account:
            await self._fund_account(account_info, fund_wei)
            
        return account_info
        
    async def _fund_account(self, account: Dict, amount_wei: int, confirmations: int = 1):
        """Fund test account using faucet account
        
        Args:
            account: Account information
            amount_wei: Funding amount in wei
            confirmations: Number of confirmations to wait
            
        Returns:
            Transaction receipt
        """
        try:
            # Get faucet account nonce
            nonce = await self.client.get_transaction_count(self.faucet_account["address"])
            
            # Get current gas price
            gas_price = await self.client.get_gas_price()
            
            # Build transfer transaction
            tx_data = {
                "to": account["address"],
                "value": hex(amount_wei),
                "gas": hex(21000),
                "gasPrice": hex(gas_price),
                "nonce": hex(nonce),
                "chainId": hex(await self.client.get_chain_id())
            }
            
            # Sign and send
            signed_tx = Account.sign_transaction(
                tx_data, 
                self.faucet_account["private_key"]
            )
            
            tx_hash = await self.client.send_raw_transaction(signed_tx.raw_transaction)
            LOG.info(f"Funding transaction sent: {tx_hash}")
            
            # Wait for confirmation
            receipt = await self.client.wait_for_transaction_receipt(tx_hash)
            
            if receipt["status"] != "0x1":
                raise RuntimeError(f"Funding transaction failed for account '{account['name']}'")
            
            # Wait for additional confirmations
            if confirmations > 1:
                current_block = int(receipt["blockNumber"], 16)
                target_block = current_block + confirmations
                
                while int(await self.client.get_block_number()) < target_block:
                    await asyncio.sleep(0.5)
            
            LOG.info(f"Funded account '{account['name']}' with {amount_wei / 10**18:.6f} ETH")
            return receipt
            
        except Exception as e:
            LOG.error(f"Failed to fund account: {e}")
            raise


def test_case(func):
    """Test case decorator"""
    async def wrapper(*args, **kwargs):
        test_name = kwargs.get('test_name') or func.__name__
        result = TestResult(test_name)
        result.start_time = asyncio.get_event_loop().time()
        result.end_time = result.start_time  # Initialize end_time
        
        try:
            await func(*args, **kwargs, test_result=result)
            result.end_time = asyncio.get_event_loop().time()
            result.set_duration(result.end_time - result.start_time)
            result.mark_success()
        except Exception as e:
            result.end_time = asyncio.get_event_loop().time()
            result.set_duration(result.end_time - result.start_time)
            result.mark_failure(str(e))
            raise
            
        return result
    
    return wrapper