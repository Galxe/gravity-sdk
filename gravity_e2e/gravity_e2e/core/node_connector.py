"""
Gravity Node Connector - Multi-node connection management

This module provides connection management for multiple Gravity nodes in a test
environment, supporting cluster configurations and health monitoring.

Design Notes:
- Manages connections to multiple nodes simultaneously
- Supports validator and VFN (validator full node) configurations
- Provides automatic health checking and reconnection
- Async context manager support for resource cleanup
- Type-safe node information with dataclasses
- Cluster-based node organization

Usage:
    async with NodeConnector("configs/nodes.json") as connector:
        # Connect to all nodes
        results = await connector.connect_all()

        # Get client for specific node
        client = connector.get_client("validator1")

        # Perform health check
        status = await connector.health_check()
"""

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
    """
    Complete information about a Gravity node.

    This dataclass encapsulates all necessary information about a node in the
    Gravity network, including connection details, role, and capabilities.

    Attributes:
        node_id: Unique identifier for the node
        type: Node type - "validator" or "vfn" (validator full node)
        role: Node role - "primary", "secondary", or "read_only"
        host: Hostname or IP address of the node
        rpc_port: JSON-RPC API port
        metrics_port: Prometheus metrics port
        ws_port: WebSocket port (optional)
        p2p_port: P2P networking port (optional)
        description: Human-readable description of the node
        connected_to: For VFNs, the validator they're connected to
        capabilities: List of supported capabilities/features
        rpc_url: Auto-generated RPC URL (http://host:port)
        metrics_url: Auto-generated metrics URL (http://host:port/metrics)
    """
    node_id: str
    type: str  # validator, vfn
    role: str  # primary, secondary, read_only
    host: str
    rpc_port: int
    metrics_port: int
    ws_port: Optional[int] = None
    p2p_port: Optional[int] = None
    description: Optional[str] = None
    connected_to: Optional[str] = None  # VFN connected validator
    capabilities: List[str] = field(default_factory=list)  # Node capabilities list
    rpc_url: Optional[str] = field(init=False)  # Auto calculated
    metrics_url: Optional[str] = field(init=False)  # Auto calculated

    def __post_init__(self):
        """Auto-generate URLs from host and port information."""
        # Auto generate RPC URL
        self.rpc_url = f"http://{self.host}:{self.rpc_port}"
        self.metrics_url = f"http://{self.host}:{self.metrics_port}/metrics"


class NodeConnector:
    """
    Manages connections to multiple Gravity nodes.

    This class provides a unified interface for connecting to, managing, and
    monitoring multiple Gravity nodes in a test environment. It supports both
    individual node operations and cluster-wide operations.

    Features:
    - Connection pooling and management
    - Automatic health checking
    - Cluster-based node organization
    - Async context manager support
    - Reconnection handling

    Example:
        # Load nodes from configuration
        connector = NodeConnector("configs/nodes.json")

        # Connect to specific nodes
        results = await connector.connect_all(target_nodes=["validator1", "vfn1"])

        # Use a specific client
        client = connector.get_client("validator1")
        block_number = await client.get_block_number()
    """
    
    def __init__(self, nodes_config_path: str = "configs/nodes.json"):
        self.nodes_config_path = nodes_config_path
        self.nodes: Dict[str, NodeInfo] = {}
        self.clients: Dict[str, GravityClient] = {}
        self.clusters: Dict[str, List[str]] = {}
        self.network_config: Dict = {}
        self._lock = asyncio.Lock()
        self.load_nodes_config()
        
    def load_nodes_config(self):
        """Load node configuration"""
        try:
            with open(self.nodes_config_path, 'r') as f:
                config = json.load(f)
                
            # Load network configuration
            self.network_config = config.get("network", {})
            LOG.info(f"Loaded network config: {self.network_config.get('name')}")
            
            # Load cluster configuration
            self.clusters = {
                name: cluster_info["nodes"]
                for name, cluster_info in config.get("clusters", {}).items()
            }
            LOG.info(f"Loaded {len(self.clusters)} cluster configurations")
            
            # Load node configuration
            for node_id, node_data in config.get("nodes", {}).items():
                node = NodeInfo(node_id=node_id, **node_data)
                self.nodes[node_id] = node
                LOG.info(f"Loaded node config: {node_id} ({node.type}) - {node.rpc_url}")
                
        except FileNotFoundError:
            raise NodeError(f"Nodes config file not found: {self.nodes_config_path}")
        except json.JSONDecodeError as e:
            raise NodeError(f"Invalid JSON in nodes config: {e}")
            
    async def connect_all(self, target_nodes: List[str] = None, max_retries: int = 3, retry_delay: float = 2.0) -> Dict[str, bool]:
        """Connect to specified nodes
        
        Args:
            target_nodes: List of nodes to connect, None for all nodes
            max_retries: Maximum retry attempts
            retry_delay: Retry delay in seconds
            
        Returns:
            Connection result dictionary {node_id: success}
        """
        if target_nodes is None:
            target_nodes = list(self.nodes.keys())
            
        results = {}
        
        # Concurrently connect to specified nodes
        async def connect_node(node_id: str, node: NodeInfo) -> bool:
            for attempt in range(max_retries):
                try:
                    async with GravityClient(node.rpc_url, node_id) as client:
                        # Test connection
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
            
        # Concurrently execute all connections
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
        """Get RPC client for node"""
        return self.clients.get(node_id)
        
    def get_node(self, node_id: str) -> Optional[NodeInfo]:
        """Get node information"""
        return self.nodes.get(node_id)
        
    def list_nodes(self) -> List[str]:
        """List all node IDs"""
        return list(self.nodes.keys())
        
    def get_cluster_nodes(self, cluster_name: str) -> List[str]:
        """Get list of nodes in cluster"""
        if cluster_name not in self.clusters:
            raise NodeError(f"Cluster '{cluster_name}' not found")
        return self.clusters[cluster_name]
        
    def list_clusters(self) -> List[str]:
        """List all cluster names"""
        return list(self.clusters.keys())
        
    def get_nodes_by_type(self, node_type: str) -> List[str]:
        """Get nodes list by type"""
        return [
            node_id for node_id, node in self.nodes.items()
            if node.type == node_type
        ]
        
    def get_nodes_by_capability(self, capability: str) -> List[str]:
        """Get nodes list by capability"""
        return [
            node_id for node_id, node in self.nodes.items()
            if capability in node.capabilities
        ]
        
    async def health_check(self, detailed: bool = False) -> Dict[str, Dict]:
        """Check health status of all nodes
        
        Args:
            detailed: Whether to get detailed information (increases response time)
            
        Returns:
            Health status dictionary
        """
        health_status = {}
        
        async def check_node(node_id: str, client: GravityClient):
            try:
                # Basic check: get block number
                block_number = await asyncio.wait_for(
                    client.get_block_number(), 
                    timeout=5.0
                )
                
                health_data = {
                    "status": "healthy",
                    "block_number": block_number,
                    "last_check": format_timestamp()
                }
                
                # Detailed check
                if detailed:
                    # Get chain ID
                    chain_id = await client.get_chain_id()
                    # Get gas price
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
        
        # Concurrently check all nodes
        tasks = [
            check_node(node_id, client) 
            for node_id, client in self.clients.items()
        ]
        
        results = await asyncio.gather(*tasks)
        
        for node_id, health_data in results:
            health_status[node_id] = health_data
            
        return health_status
        
    async def wait_for_ready(self, node_id: str, timeout: int = 60, check_interval: float = 2.0):
        """Wait for specific node to be ready
        
        Args:
            node_id: Node ID
            timeout: Timeout in seconds
            check_interval: Check interval in seconds
            
        Returns:
            bool: Whether node is ready
            
        Raises:
            NodeError: Node not found
            ConnectionError: Wait timeout
        """
        if node_id not in self.nodes:
            raise NodeError(f"Node {node_id} not found in config")
            
        start_time = time.time()
        
        while time.time() - start_time < timeout:
            try:
                async with GravityClient(self.nodes[node_id].rpc_url, node_id) as client:
                    # Try to get block number
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
        """Close all client connections"""
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