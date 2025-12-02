"""
Gravity Node HTTP API Client
For accessing DKG status and randomness data
"""
import aiohttp
import asyncio
import logging
import time
from typing import Dict, Optional

LOG = logging.getLogger(__name__)


class GravityHttpClient:
    """Gravity Node HTTP API Client"""
    
    def __init__(self, base_url: str = "http://127.0.0.1:1998", timeout: float = 30.0):
        """
        Initialize HTTP client
        
        Args:
            base_url: Gravity Node HTTP API address
            timeout: Request timeout (seconds)
        """
        self.base_url = base_url.rstrip('/')
        self.timeout = timeout
        self.session: Optional[aiohttp.ClientSession] = None
    
    async def __aenter__(self):
        """Async context manager entry"""
        self.session = aiohttp.ClientSession(
            timeout=aiohttp.ClientTimeout(total=self.timeout),
            connector=aiohttp.TCPConnector(ssl=False)  # Disable SSL verification (local testing)
        )
        return self
    
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Async context manager exit"""
        if self.session:
            await self.session.close()
            self.session = None
    
    async def get_dkg_status(self) -> Dict:
        """
        Get DKG status
        
        Returns:
            DKG status dictionary:
            {
                "epoch": int,
                "round": int,
                "block_number": int,
                "participating_nodes": int
            }
        
        Raises:
            RuntimeError: Request failed
        """
        url = f"{self.base_url}/dkg/status"
        LOG.debug(f"Getting DKG status from {url}")
        
        if not self.session:
            raise RuntimeError("Client not initialized. Use 'async with' statement.")
        
        try:
            async with self.session.get(url) as resp:
                if resp.status != 200:
                    text = await resp.text()
                    raise RuntimeError(f"Failed to get DKG status: {resp.status} - {text}")
                
                data = await resp.json()
                LOG.info(
                    f"DKG Status: epoch={data['epoch']}, round={data['round']}, "
                    f"block={data['block_number']}, nodes={data['participating_nodes']}"
                )
                return data
        except aiohttp.ClientError as e:
            raise RuntimeError(f"HTTP request failed: {e}")
    
    async def get_randomness(self, block_number: int) -> Optional[str]:
        """
        Get randomness for a specified block
        
        Args:
            block_number: Block number
        
        Returns:
            Hex randomness string (with 0x prefix), or None if doesn't exist
        """
        url = f"{self.base_url}/dkg/randomness/{block_number}"
        LOG.debug(f"Getting randomness for block {block_number} from {url}")
        
        if not self.session:
            raise RuntimeError("Client not initialized. Use 'async with' statement.")
        
        try:
            async with self.session.get(url) as resp:
                if resp.status == 200:
                    data = await resp.json()
                    randomness = data.get("randomness")
                    
                    if randomness:
                        LOG.info(f"Block {block_number} randomness: {randomness[:16]}...")
                        # Ensure 0x prefix
                        if not randomness.startswith("0x"):
                            randomness = "0x" + randomness
                        return randomness
                    else:
                        LOG.info(f"Block {block_number} has no randomness")
                        return None
                else:
                    text = await resp.text()
                    LOG.warning(f"Failed to get randomness for block {block_number}: {resp.status} - {text}")
                    return None
        except aiohttp.ClientError as e:
            LOG.warning(f"HTTP request failed for block {block_number}: {e}")
            return None
    
    async def wait_for_epoch(self, target_epoch: int, timeout: int = 120) -> int:
        """
        Wait for a specified epoch
        
        Args:
            target_epoch: Target epoch
            timeout: Timeout (seconds)
        
        Returns:
            Current epoch number
        
        Raises:
            TimeoutError: Timeout
        """
        start = time.time()
        LOG.info(f"Waiting for epoch {target_epoch}...")
        
        last_epoch = None
        while time.time() - start < timeout:
            try:
                status = await self.get_dkg_status()
                current_epoch = status["epoch"]
                
                if current_epoch != last_epoch:
                    LOG.debug(f"Current epoch: {current_epoch}, target: {target_epoch}")
                    last_epoch = current_epoch
                
                if current_epoch >= target_epoch:
                    LOG.info(f"Reached epoch {current_epoch}")
                    return current_epoch
                
            except Exception as e:
                LOG.warning(f"Error checking epoch: {e}")
            
            await asyncio.sleep(2)
        
        raise TimeoutError(
            f"Timeout waiting for epoch {target_epoch} "
            f"(current: {last_epoch}, timeout: {timeout}s)"
        )
    
    async def get_current_epoch(self) -> int:
        """
        Get current epoch
        
        Returns:
            Current epoch number
        """
        status = await self.get_dkg_status()
        return status["epoch"]
    
    async def get_current_block(self) -> int:
        """
        Get current block number
        
        Returns:
            Current block number
        """
        status = await self.get_dkg_status()
        return status["block_number"]

