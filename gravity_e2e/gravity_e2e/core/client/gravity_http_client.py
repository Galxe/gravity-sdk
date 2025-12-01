"""
Gravity节点HTTP API客户端
用于访问DKG状态和随机数数据
"""
import aiohttp
import asyncio
import logging
import time
from typing import Dict, Optional

LOG = logging.getLogger(__name__)


class GravityHttpClient:
    """Gravity节点HTTP API客户端"""
    
    def __init__(self, base_url: str = "http://127.0.0.1:1998", timeout: float = 30.0):
        """
        初始化HTTP客户端
        
        Args:
            base_url: Gravity节点HTTP API地址
            timeout: 请求超时时间（秒）
        """
        self.base_url = base_url.rstrip('/')
        self.timeout = timeout
        self.session: Optional[aiohttp.ClientSession] = None
    
    async def __aenter__(self):
        """异步上下文管理器入口"""
        self.session = aiohttp.ClientSession(
            timeout=aiohttp.ClientTimeout(total=self.timeout),
            connector=aiohttp.TCPConnector(ssl=False)  # 禁用SSL验证（本地测试）
        )
        return self
    
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """异步上下文管理器退出"""
        if self.session:
            await self.session.close()
            self.session = None
    
    async def get_dkg_status(self) -> Dict:
        """
        获取DKG状态
        
        Returns:
            DKG状态字典:
            {
                "epoch": int,
                "round": int,
                "block_number": int,
                "participating_nodes": int
            }
        
        Raises:
            RuntimeError: 请求失败
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
        获取指定块的随机数
        
        Args:
            block_number: 块号
        
        Returns:
            十六进制随机数字符串（带0x前缀），如果不存在则返回None
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
                        # 确保有0x前缀
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
        等待指定epoch
        
        Args:
            target_epoch: 目标epoch
            timeout: 超时时间（秒）
        
        Returns:
            当前epoch号
        
        Raises:
            TimeoutError: 超时
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
        获取当前epoch
        
        Returns:
            当前epoch号
        """
        status = await self.get_dkg_status()
        return status["epoch"]
    
    async def get_current_block(self) -> int:
        """
        获取当前块号
        
        Returns:
            当前块号
        """
        status = await self.get_dkg_status()
        return status["block_number"]

