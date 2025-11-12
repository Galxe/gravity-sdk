import asyncio
import json
import logging
import time
from datetime import datetime
from typing import Dict, List, Optional
from dataclasses import dataclass, field

from .client.gravity_client import GravityClient
from ..utils.exceptions import ConnectionError, NodeError
from ..utils.common import format_timestamp

LOG = logging.getLogger(__name__)


@dataclass
class NodeInfo:
    """节点信息"""
    node_id: str
    type: str  # validator, vfn
    role: str  # primary, secondary, read_only
    host: str
    rpc_port: int
    metrics_port: int
    ws_port: Optional[int] = None
    p2p_port: Optional[int] = None
    description: Optional[str] = None
    connected_to: Optional[str] = None  # VFN连接的validator
    capabilities: List[str] = field(default_factory=list)  # 节点能力列表
    rpc_url: Optional[str] = field(init=False)  # 自动计算
    metrics_url: Optional[str] = field(init=False)  # 自动计算
    
    def __post_init__(self):
        # 自动生成 RPC URL
        self.rpc_url = f"http://{self.host}:{self.rpc_port}"
        self.metrics_url = f"http://{self.host}:{self.metrics_port}/metrics"


class NodeConnector:
    """节点连接器 - 连接到已部署的节点"""
    
    def __init__(self, nodes_config_path: str = "configs/nodes.json"):
        self.nodes_config_path = nodes_config_path
        self.nodes: Dict[str, NodeInfo] = {}
        self.clients: Dict[str, GravityClient] = {}
        self.clusters: Dict[str, List[str]] = {}
        self.network_config: Dict = {}
        self._lock = asyncio.Lock()
        self.load_nodes_config()
        
    def load_nodes_config(self):
        """加载节点配置"""
        try:
            with open(self.nodes_config_path, 'r') as f:
                config = json.load(f)
                
            # 加载网络配置
            self.network_config = config.get("network", {})
            LOG.info(f"Loaded network config: {self.network_config.get('name')}")
            
            # 加载集群配置
            self.clusters = {
                name: cluster_info["nodes"]
                for name, cluster_info in config.get("clusters", {}).items()
            }
            LOG.info(f"Loaded {len(self.clusters)} cluster configurations")
            
            # 加载节点配置
            for node_id, node_data in config.get("nodes", {}).items():
                node = NodeInfo(node_id=node_id, **node_data)
                self.nodes[node_id] = node
                LOG.info(f"Loaded node config: {node_id} ({node.type}) - {node.rpc_url}")
                
        except FileNotFoundError:
            raise NodeError(f"Nodes config file not found: {self.nodes_config_path}")
        except json.JSONDecodeError as e:
            raise NodeError(f"Invalid JSON in nodes config: {e}")
            
    async def connect_all(self, target_nodes: List[str] = None, max_retries: int = 3, retry_delay: float = 2.0) -> Dict[str, bool]:
        """连接到指定节点
        
        Args:
            target_nodes: 要连接的节点列表，None表示所有节点
            max_retries: 最大重试次数
            retry_delay: 重试间隔（秒）
            
        Returns:
            连接结果字典 {node_id: success}
        """
        if target_nodes is None:
            target_nodes = list(self.nodes.keys())
            
        results = {}
        
        # 并发连接指定节点
        async def connect_node(node_id: str, node: NodeInfo) -> bool:
            for attempt in range(max_retries):
                try:
                    async with GravityClient(node.rpc_url, node_id) as client:
                        # 测试连接
                        await client.get_block_number()
                        async with self._lock:
                            # Create a new client for persistent use
                            self.clients[node_id] = GravityClient(node.rpc_url, node_id)
                            # Initialize the session for the persistent client
                            await self.clients[node_id].__aenter__()
                    LOG.info(f"Connected to node {node_id}")
                    return True
                except Exception as e:
                    if attempt == max_retries - 1:
                        LOG.error(f"Failed to connect to node {node_id} after {max_retries} attempts: {e}")
                    else:
                        LOG.warning(f"Attempt {attempt + 1} failed for node {node_id}: {e}")
                        await asyncio.sleep(retry_delay)
            return False
            
        # 并发执行所有连接
        tasks = [
            connect_node(node_id, self.nodes[node_id]) 
            for node_id in target_nodes
            if node_id in self.nodes
        ]
        
        connection_results = await asyncio.gather(*tasks)
        
        for node_id, success in zip(target_nodes, connection_results):
            results[node_id] = success
            
        return results
        
    def get_client(self, node_id: str) -> Optional[GravityClient]:
        """获取节点的 RPC 客户端"""
        return self.clients.get(node_id)
        
    def get_node(self, node_id: str) -> Optional[NodeInfo]:
        """获取节点信息"""
        return self.nodes.get(node_id)
        
    def list_nodes(self) -> List[str]:
        """列出所有节点ID"""
        return list(self.nodes.keys())
        
    def get_cluster_nodes(self, cluster_name: str) -> List[str]:
        """获取集群中的节点列表"""
        if cluster_name not in self.clusters:
            raise NodeError(f"Cluster '{cluster_name}' not found")
        return self.clusters[cluster_name]
        
    def list_clusters(self) -> List[str]:
        """列出所有集群名称"""
        return list(self.clusters.keys())
        
    def get_nodes_by_type(self, node_type: str) -> List[str]:
        """根据类型获取节点列表"""
        return [
            node_id for node_id, node in self.nodes.items()
            if node.type == node_type
        ]
        
    def get_nodes_by_capability(self, capability: str) -> List[str]:
        """根据能力获取节点列表"""
        return [
            node_id for node_id, node in self.nodes.items()
            if capability in node.capabilities
        ]
        
    async def health_check(self, detailed: bool = False) -> Dict[str, Dict]:
        """检查所有节点健康状态
        
        Args:
            detailed: 是否获取详细信息（会增加响应时间）
            
        Returns:
            健康状态字典
        """
        health_status = {}
        
        async def check_node(node_id: str, client: GravityClient):
            try:
                # 基础检查：获取区块号
                block_number = await asyncio.wait_for(
                    client.get_block_number(), 
                    timeout=5.0
                )
                
                health_data = {
                    "status": "healthy",
                    "block_number": block_number,
                    "last_check": format_timestamp()
                }
                
                # 详细检查
                if detailed:
                    # 获取链ID
                    chain_id = await client.get_chain_id()
                    # 获取 gas 价格
                    gas_price = await client.get_gas_price()
                    
                    health_data.update({
                        "chain_id": chain_id,
                        "gas_price": gas_price,
                        "node_info": self.nodes[node_id].__dict__
                    })
                
                return node_id, health_data
                
            except asyncio.TimeoutError:
                return node_id, {
                    "status": "timeout",
                    "error": "Request timeout",
                    "last_check": format_timestamp()
                }
            except Exception as e:
                return node_id, {
                    "status": "unhealthy",
                    "error": str(e),
                    "last_check": format_timestamp()
                }
        
        # 并发检查所有节点
        tasks = [
            check_node(node_id, client) 
            for node_id, client in self.clients.items()
        ]
        
        results = await asyncio.gather(*tasks)
        
        for node_id, health_data in results:
            health_status[node_id] = health_data
            
        return health_status
        
    async def wait_for_ready(self, node_id: str, timeout: int = 60, check_interval: float = 2.0):
        """等待特定节点就绪
        
        Args:
            node_id: 节点ID
            timeout: 超时时间（秒）
            check_interval: 检查间隔（秒）
            
        Returns:
            bool: 节点是否就绪
            
        Raises:
            NodeError: 节点不存在
            ConnectionError: 等待超时
        """
        if node_id not in self.nodes:
            raise NodeError(f"Node {node_id} not found in config")
            
        start_time = time.time()
        
        while time.time() - start_time < timeout:
            try:
                async with GravityClient(self.nodes[node_id].rpc_url, node_id) as client:
                    # 尝试获取区块号
                    block_number = await asyncio.wait_for(
                        client.get_block_number(),
                        timeout=5.0
                    )
                    LOG.info(f"Node {node_id} is ready at block {block_number}")
                    return True
            except Exception as e:
                LOG.debug(f"Node {node_id} not ready yet: {e}")
                await asyncio.sleep(check_interval)
                
        raise ConnectionError(
            f"Node {node_id} not ready within {timeout}s. "
            f"Last error: {str(e)}"
        )
        
    async def close_all(self):
        """关闭所有客户端连接"""
        async with self._lock:
            for node_id, client in self.clients.items():
                try:
                    await client.__aexit__(None, None, None)
                    LOG.info(f"Closed connection to node {node_id}")
                except Exception as e:
                    LOG.warning(f"Error closing connection to node {node_id}: {e}")
            self.clients.clear()
            
    async def __aenter__(self):
        return self
        
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        await self.close_all()