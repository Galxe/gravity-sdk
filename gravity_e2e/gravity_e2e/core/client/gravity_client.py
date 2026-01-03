import asyncio
import json
import logging
import aiohttp
import time
from typing import Any, Dict, List, Optional, Union
from web3 import Web3

from ...utils.exceptions import APIError
from ...utils.common import hex_to_int


def to_checksum_address(address: str) -> str:
    """Convert address to EIP-55 checksum format"""
    if not address.startswith("0x"):
        address = "0x" + address
    return Web3.to_checksum_address(address)

LOG = logging.getLogger(__name__)


class GravityClient:
    """Gravity Node EVM API Client"""
    
    def __init__(self, rpc_url: str, node_id: str, timeout: float = 30.0):
        self.rpc_url = rpc_url
        self.node_id = node_id
        self.timeout = timeout
        self._web3 = Web3(Web3.HTTPProvider(rpc_url))
        self.session: Optional[aiohttp.ClientSession] = None
        self._request_id = 0
    
    @property
    def web3(self) -> Web3:
        """Get Web3 instance for synchronous operations (use with caution in async context)"""
        return self._web3
    
    @property
    def w3(self) -> Web3:
        """Alias for web3 property (deprecated, use web3 instead)"""
        return self._web3
        
    async def __aenter__(self):
        self.session = aiohttp.ClientSession(
            timeout=aiohttp.ClientTimeout(total=self.timeout)
        )
        return self
        
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        if self.session:
            await self.session.close()
            self.session = None
            
    async def send_request(self, 
                          method: str, 
                          params: List[Any] = None,
                          timeout: Optional[float] = None) -> Any:
        """Send JSON-RPC request
        
        Args:
            method: RPC method name
            params: Parameter list
            timeout: Timeout (overrides default)
            
        Returns:
            RPC response result
            
        Raises:
            APIError: Request failed or returned error
        """
        if not self.session:
            raise RuntimeError("Client not initialized. Use async with statement.")
            
        self._request_id += 1
        payload = {
            "jsonrpc": "2.0",
            "method": method,
            "params": params or [],
            "id": self._request_id
        }
        
        try:
            # Note: Don't create new ClientTimeout for each request as it can cause
            # issues with pytest-asyncio. Use session's default timeout instead.
            async with self.session.post(
                self.rpc_url,
                json=payload
            ) as response:
                if response.status != 200:
                    text = await response.text()
                    raise APIError(
                        f"HTTP {response.status}: {text}",
                        code=response.status
                    )
                    
                result = await response.json()
                if "error" in result:
                    error = result["error"]
                    raise APIError(
                        f"RPC Error: {error.get('message', str(error))}",
                        code=error.get('code')
                    )
                    
                return result["result"]
                
        except asyncio.TimeoutError:
            raise APIError(f"Request timeout after {timeout}s")
        except aiohttp.ClientError as e:
            raise APIError(f"Connection error: {e}")
        except json.JSONDecodeError as e:
            raise APIError(f"Invalid JSON response: {e}")
    
    async def get_chain_id(self) -> int:
        """Get chain ID"""
        chain_id = await self.send_request("eth_chainId")
        return hex_to_int(chain_id)
    
    async def get_block_number(self) -> int:
        """Get latest block number"""
        block = await self.send_request("eth_blockNumber")
        return hex_to_int(block)
    
    async def get_balance(self, address: str, block: str = "latest") -> int:
        """Get account balance (wei)"""
        address = to_checksum_address(address)
        balance = await self.send_request("eth_getBalance", [address, block])
        return hex_to_int(balance)
    
    async def get_transaction_count(self, address: str, block: str = "latest") -> int:
        """Get account transaction count (nonce)"""
        address = to_checksum_address(address)
        count = await self.send_request("eth_getTransactionCount", [address, block])
        return hex_to_int(count)
    
    async def send_raw_transaction(self, raw_tx) -> str:
        """Send raw transaction"""
        # Convert HexBytes to hex string if needed
        if hasattr(raw_tx, 'hex'):
            raw_tx = raw_tx.hex()
        elif isinstance(raw_tx, bytes):
            raw_tx = "0x" + raw_tx.hex()
        return await self.send_request("eth_sendRawTransaction", [raw_tx])
    
    async def get_transaction_receipt(self, tx_hash: str) -> Optional[Dict]:
        """Get transaction receipt"""
        receipt = await self.send_request("eth_getTransactionReceipt", [tx_hash])
        return receipt if receipt else None
    
    async def wait_for_transaction_receipt(self, 
                                          tx_hash: str, 
                                          timeout: int = 120,
                                          poll_interval: float = 1.0) -> Dict:
        """Wait for transaction confirmation"""
        start_time = time.time()
        
        while time.time() - start_time < timeout:
            receipt = await self.get_transaction_receipt(tx_hash)
            if receipt is not None:
                return receipt
            await asyncio.sleep(poll_interval)
            
        raise APIError(f"Transaction {tx_hash} not confirmed within {timeout}s")
    
    async def get_block(self, block_number: Union[int, str], full_transactions: bool = True) -> Dict:
        """Get block information"""
        if isinstance(block_number, int):
            block_identifier = hex(block_number)
        elif block_number in ("latest", "pending", "earliest"):
            block_identifier = block_number
        else:
            block_identifier = block_number
            
        return await self.send_request(
            "eth_getBlockByNumber",
            [block_identifier, full_transactions]
        )
    
    async def get_code(self, address: str, block: str = "latest") -> str:
        """Get contract code"""
        address = to_checksum_address(address)
        code = await self.send_request("eth_getCode", [address, block])
        return code
    
    async def call(self, 
                   to: str, 
                   data: str = "0x", 
                   from_: str = None,
                   block: str = "latest") -> str:
        """Execute contract call (read-only)"""
        to = to_checksum_address(to)
        params = {
            "to": to,
            "data": data
        }
        if from_:
            params["from"] = to_checksum_address(from_)
            
        return await self.send_request(
            "eth_call",
            [params, block]
        )
    
    async def estimate_gas(self, 
                          tx: Dict,
                          block: str = "latest") -> int:
        """Estimate transaction gas"""
        gas = await self.send_request("eth_estimateGas", [tx, block])
        return hex_to_int(gas)
    
    async def get_logs(self, 
                      from_block: Union[int, str] = "latest",
                      to_block: Union[int, str] = "latest",
                      address: Union[str, List[str]] = None,
                      topics: List[List[str]] = None) -> List[Dict]:
        """Get logs"""
        params = {
            "fromBlock": hex(from_block) if isinstance(from_block, int) else from_block,
            "toBlock": hex(to_block) if isinstance(to_block, int) else to_block,
        }
        
        if address:
            if isinstance(address, str):
                address = to_checksum_address(address)
            params["address"] = address
        if topics:
            params["topics"] = topics
            
        return await self.send_request("eth_getLogs", [params])
    
    async def get_gas_price(self) -> int:
        """Get current gas price"""
        gas_price = await self.send_request("eth_gasPrice")
        return hex_to_int(gas_price)