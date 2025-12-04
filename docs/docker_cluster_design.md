# Gravity Docker 集群部署方案设计文档

## 1. 概述

### 1.1 目标

- 实现每个节点运行在独立的 Docker 容器中
- 支持多节点集群部署（以 4 validators 为例）
- 在 `gravity_e2e` 中实现 Docker 生命周期管理

### 1.2 参考实现

| 参考项目 | 文件路径 | 功能说明 |
|---------|---------|---------|
| gravity-aptos | `docker/compose/validator-testnet/` | Docker Compose 多服务编排 |
| gravity-aptos | `testsuite/forge/src/backend/local/swarm.rs` | LocalSwarm 集群管理 |
| gravity-aptos | `testsuite/forge/src/backend/local/node.rs` | LocalNode 单节点管理 |
| gravity-aptos | `testsuite/smoke-test/src/smoke_test_environment.rs` | SwarmBuilder 测试环境构建 |

---

## 2. 现有代码分析

### 2.1 gravity-aptos Docker 结构

```
gravity-aptos/docker/
├── builder/
│   ├── validator.Dockerfile      # validator 镜像定义
│   ├── debian-base.Dockerfile    # 基础镜像
│   └── ...
├── compose/
│   ├── aptos-node/
│   │   ├── docker-compose.yaml   # 生产环境单节点
│   │   ├── haproxy.cfg           # 负载均衡配置
│   │   └── validator.yaml        # 节点配置
│   └── validator-testnet/
│       ├── docker-compose.yaml   # 测试网配置
│       └── validator_node_template.yaml
```

#### 关键配置示例 (validator-testnet/docker-compose.yaml)

```yaml
version: "3.8"
services:
  validator:
    image: "${VALIDATOR_IMAGE_REPO:-aptoslabs/validator}:${IMAGE_TAG:-devnet}"
    networks:
      shared:
        ipv4_address: 172.16.1.10    # 固定 IP 地址
    volumes:
      - type: volume
        source: aptos-shared
        target: /opt/aptos/var
    command: ["/usr/local/bin/aptos-node", "--test", "--test-dir", "/opt/aptos/var/"]
    ports:
      - "8080:8080"  # REST API
      - "50051:50051"  # Indexer GRPC

  faucet:
    image: "${FAUCET_IMAGE_REPO:-aptoslabs/faucet}:${IMAGE_TAG:-devnet}"
    depends_on:
      - validator
    networks:
      shared:
        ipv4_address: 172.16.1.11

networks:
  shared:
    name: "aptos-docker-compose-shared"
    ipam:
      config:
        - subnet: 172.16.1.0/24

volumes:
  aptos-shared:
    name: aptos-shared
```

### 2.2 LocalSwarm 集群管理 (swarm.rs)

#### 核心数据结构

```rust
pub struct LocalSwarm {
    node_name_counter: usize,
    genesis: Transaction,
    genesis_waypoint: Waypoint,
    versions: Arc<HashMap<Version, LocalVersion>>,
    validators: HashMap<PeerId, LocalNode>,        // validator 节点集合
    fullnodes: HashMap<PeerId, LocalNode>,         // fullnode 节点集合
    public_networks: HashMap<PeerId, NetworkConfig>,
    dir: SwarmDirectory,
    root_account: Arc<LocalAccount>,
    chain_id: ChainId,
    root_key: ConfigKey<Ed25519PrivateKey>,
    launched: bool,
    guard: ActiveNodesGuard,
}
```

#### 核心方法

| 方法 | 功能 |
|------|------|
| `build()` | 构建 Swarm，生成 genesis、创建节点实例 |
| `launch()` | 启动所有节点 |
| `wait_all_alive()` | 等待所有节点就绪 |
| `wait_for_startup()` | 等待启动完成 |
| `wait_for_connectivity()` | 等待网络连通 |
| `add_validator_fullnode()` | 动态添加 validator fullnode |
| `add_fullnode()` | 动态添加 fullnode |

#### 启动流程

```rust
pub async fn launch(&mut self) -> Result<()> {
    if self.launched {
        return Err(anyhow!("Swarm already launched"));
    }
    self.launched = true;

    // 1. 启动所有 validators
    for validator in self.validators.values_mut() {
        validator.start()?;
    }

    // 2. 等待所有节点存活
    self.wait_all_alive(Duration::from_secs(60)).await?;
    
    info!("Swarm launched successfully.");
    Ok(())
}

pub async fn wait_all_alive(&mut self, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    self.wait_for_startup().await?;
    self.wait_for_connectivity(deadline).await?;
    self.liveness_check(deadline).await?;
    Ok(())
}
```

### 2.3 LocalNode 单节点管理 (node.rs)

#### 核心数据结构

```rust
pub struct LocalNode {
    version: LocalVersion,
    process: std::sync::Mutex<Option<Process>>,  // 进程句柄
    name: String,
    index: usize,
    account_private_key: Option<ConfigKey<Ed25519PrivateKey>>,
    peer_id: AccountAddress,
    directory: PathBuf,
    config: NodeConfig,
}
```

#### 核心方法

| 方法 | 功能 |
|------|------|
| `new()` | 创建节点实例，加载配置 |
| `start()` | 启动节点进程 |
| `stop()` | 停止节点进程 |
| `upgrade()` | 升级节点版本 |
| `health_check()` | 健康检查 |
| `clear_storage()` | 清理存储数据 |

#### 启动实现

```rust
pub fn start(&self) -> Result<()> {
    let mut process_locker = self.process.lock().unwrap();
    ensure!(process_locker.is_none(), "node {} already running", self.name);

    // 1. 创建日志文件
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(self.log_path())?;

    // 2. 构建启动命令
    let mut node_command = Command::new(self.version.bin());
    node_command
        .current_dir(&self.directory)
        .arg("-f")
        .arg(self.config_path());
    
    if env::var("RUST_LOG").is_err() {
        node_command.env("RUST_LOG", "debug");
    }
    
    node_command.stdout(log_file.try_clone()?).stderr(log_file);

    // 3. 启动进程
    let process = node_command.spawn()?;
    
    info!("Started node {} (PID: {})", self.name, process.id());
    
    *process_locker = Some(Process(process));
    Ok(())
}
```

#### 健康检查实现

```rust
pub async fn health_check(&self) -> Result<(), HealthCheckError> {
    // 1. 检查进程是否在运行
    {
        let mut process_locker = self.process.lock().unwrap();
        let process = process_locker.as_mut();
        if let Some(p) = process {
            match p.0.try_wait() {
                Ok(Some(status)) => {
                    return Err(HealthCheckError::NotRunning(...));
                },
                Ok(None) => {}, // 正常运行
                Err(e) => {
                    return Err(HealthCheckError::Unknown(e.into()));
                },
            }
        } else {
            return Err(HealthCheckError::NotRunning(...));
        }
    }

    // 2. 检查 inspection API
    self.inspection_client()
        .get_forge_metrics()
        .await
        .map_err(HealthCheckError::Failure)?;

    // 3. 检查 REST API
    self.rest_client()
        .get_ledger_information()
        .await
        .map_err(|err| HealthCheckError::Failure(err.into()))
}
```

### 2.4 gravity-sdk 现有配置

#### 现有 Docker 配置 (docker/gravity_node/)

```yaml
# docker-compose.yaml
version: '3.8'
services:
  gravity_node:
    build:
      context: ../..
      dockerfile: docker/gravity_node/validator.Dockerfile
    container_name: gravity_node
    volumes:
      - /tmp/gravity_node/data:/gravity_node/data
      - /tmp/gravity_node/logs:/gravity_node/logs
    ports:
      - "8545:8545"    # reth http.port
      - "8551:8551"    # reth authrpc.port
      - "9001:9001"    # reth metrics
      - "12024:12024"  # reth port
      - "2024:2024"    # gravity-sdk network.port
      - "10000:10000"  # gravity-sdk inspection_service.port
    command: sh -c "/gravity_node/script/start.sh && tail -f /dev/null"
```

#### gravity_e2e NodeConnector

现有 `NodeConnector` 只负责**连接**已部署节点，**不负责启动/停止**：

```python
class NodeConnector:
    """Node connector - connects to deployed nodes"""
    
    def __init__(self, nodes_config_path: str = "configs/nodes.json"):
        self.nodes: Dict[str, NodeInfo] = {}
        self.clients: Dict[str, GravityClient] = {}
        self.clusters: Dict[str, List[str]] = {}
    
    async def connect_all(self, target_nodes: List[str] = None) -> Dict[str, bool]:
        """连接到指定节点"""
        ...
    
    async def health_check(self, detailed: bool = False) -> Dict[str, Dict]:
        """检查所有节点健康状态"""
        ...
```

---

## 3. 方案设计

### 3.1 整体架构

```
gravity-sdk/
├── docker/
│   ├── gravity_node/                    # 现有：单节点配置
│   │   ├── validator.Dockerfile
│   │   └── docker-compose.yaml
│   │
│   └── cluster/                         # [新增] 集群配置
│       ├── docker-compose.cluster.yaml  # 多节点编排
│       ├── Makefile                     # 快捷命令
│       └── configs/                     # 各节点配置
│           ├── node_0/
│           ├── node_1/
│           ├── node_2/
│           └── node_3/
│
├── gravity_e2e/
│   └── gravity_e2e/
│       └── core/
│           ├── node_connector.py        # 现有：RPC 连接管理
│           ├── docker_manager.py        # [新增] Docker 生命周期管理
│           └── cluster_manager.py       # [新增] 集群管理器
│
└── docs/
    └── docker_cluster_design.md         # 本文档
```

### 3.2 Docker Compose 多节点配置

**文件：** `docker/cluster/docker-compose.cluster.yaml`

```yaml
version: '3.8'

x-gravity-node: &gravity-node-common
  build:
    context: ../..
    dockerfile: docker/gravity_node/validator.Dockerfile
  restart: unless-stopped
  environment:
    - RUST_BACKTRACE=1
    - RUST_LOG=info

services:
  gravity_node_0:
    <<: *gravity-node-common
    container_name: gravity_node_0
    hostname: gravity_node_0
    networks:
      gravity-cluster:
        ipv4_address: 172.20.0.10
    volumes:
      - gravity_data_0:/gravity_node/data
      - gravity_logs_0:/gravity_node/logs
      - ./configs/node_0:/gravity_node/config:ro
    ports:
      - "8545:8545"     # RPC
      - "9001:9001"     # Metrics
      - "2024:2024"     # P2P
      - "10000:10000"   # Inspection
    environment:
      - NODE_INDEX=0

  gravity_node_1:
    <<: *gravity-node-common
    container_name: gravity_node_1
    hostname: gravity_node_1
    networks:
      gravity-cluster:
        ipv4_address: 172.20.0.11
    volumes:
      - gravity_data_1:/gravity_node/data
      - gravity_logs_1:/gravity_node/logs
      - ./configs/node_1:/gravity_node/config:ro
    ports:
      - "8546:8545"
      - "9002:9001"
      - "2025:2024"
      - "10001:10000"
    environment:
      - NODE_INDEX=1

  gravity_node_2:
    <<: *gravity-node-common
    container_name: gravity_node_2
    hostname: gravity_node_2
    networks:
      gravity-cluster:
        ipv4_address: 172.20.0.12
    volumes:
      - gravity_data_2:/gravity_node/data
      - gravity_logs_2:/gravity_node/logs
      - ./configs/node_2:/gravity_node/config:ro
    ports:
      - "8547:8545"
      - "9003:9001"
      - "2026:2024"
      - "10002:10000"
    environment:
      - NODE_INDEX=2

  gravity_node_3:
    <<: *gravity-node-common
    container_name: gravity_node_3
    hostname: gravity_node_3
    networks:
      gravity-cluster:
        ipv4_address: 172.20.0.13
    volumes:
      - gravity_data_3:/gravity_node/data
      - gravity_logs_3:/gravity_node/logs
      - ./configs/node_3:/gravity_node/config:ro
    ports:
      - "8548:8545"
      - "9004:9001"
      - "2027:2024"
      - "10003:10000"
    environment:
      - NODE_INDEX=3

networks:
  gravity-cluster:
    name: gravity-cluster
    driver: bridge
    ipam:
      driver: default
      config:
        - subnet: 172.20.0.0/24
          gateway: 172.20.0.1

volumes:
  gravity_data_0:
  gravity_data_1:
  gravity_data_2:
  gravity_data_3:
  gravity_logs_0:
  gravity_logs_1:
  gravity_logs_2:
  gravity_logs_3:
```

### 3.3 Docker Node Manager

**文件：** `gravity_e2e/gravity_e2e/core/docker_manager.py`

```python
"""
Docker 节点管理器 - 参考 LocalNode 实现
"""
import asyncio
import docker
import logging
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Any
from pathlib import Path
from enum import Enum

LOG = logging.getLogger(__name__)


class NodeStatus(Enum):
    """节点状态"""
    STOPPED = "stopped"
    STARTING = "starting"
    RUNNING = "running"
    UNHEALTHY = "unhealthy"
    ERROR = "error"


@dataclass
class DockerNodeConfig:
    """Docker 节点配置"""
    node_id: str
    container_name: str
    image: str
    host: str = "127.0.0.1"
    rpc_port: int = 8545
    metrics_port: int = 9001
    p2p_port: int = 2024
    inspection_port: int = 10000
    network_ip: str = ""
    config_path: Optional[Path] = None
    data_path: Optional[Path] = None
    
    @property
    def rpc_url(self) -> str:
        return f"http://{self.host}:{self.rpc_port}"
    
    @property
    def metrics_url(self) -> str:
        return f"http://{self.host}:{self.metrics_port}/metrics"


class DockerNodeManager:
    """
    单个 Docker 节点管理器
    
    类似 gravity-aptos 的 LocalNode，但使用 Docker 容器而非本地进程
    """
    
    def __init__(self, config: DockerNodeConfig):
        self.config = config
        self.client = docker.from_env()
        self._container = None
        self._status = NodeStatus.STOPPED
    
    @property
    def name(self) -> str:
        return self.config.node_id
    
    @property
    def status(self) -> NodeStatus:
        return self._status
    
    async def start(self) -> bool:
        """
        启动容器
        
        Returns:
            bool: 是否成功启动
        """
        if self._container is not None:
            LOG.warning(f"Node {self.name} already has a container")
            return False
        
        try:
            self._status = NodeStatus.STARTING
            
            # 检查是否存在已停止的容器
            try:
                existing = self.client.containers.get(self.config.container_name)
                if existing.status == "exited":
                    LOG.info(f"Starting existing container {self.config.container_name}")
                    existing.start()
                    self._container = existing
                else:
                    LOG.info(f"Container {self.config.container_name} status: {existing.status}")
                    self._container = existing
            except docker.errors.NotFound:
                # 创建新容器
                LOG.info(f"Creating new container {self.config.container_name}")
                
                volumes = {}
                if self.config.config_path:
                    volumes[str(self.config.config_path)] = {
                        'bind': '/gravity_node/config', 
                        'mode': 'ro'
                    }
                if self.config.data_path:
                    volumes[str(self.config.data_path)] = {
                        'bind': '/gravity_node/data', 
                        'mode': 'rw'
                    }
                
                self._container = self.client.containers.run(
                    image=self.config.image,
                    name=self.config.container_name,
                    hostname=self.config.container_name,
                    detach=True,
                    volumes=volumes,
                    ports={
                        '8545/tcp': self.config.rpc_port,
                        '9001/tcp': self.config.metrics_port,
                        '2024/tcp': self.config.p2p_port,
                        '10000/tcp': self.config.inspection_port,
                    },
                    environment={
                        'RUST_BACKTRACE': '1',
                        'RUST_LOG': 'info',
                    },
                )
            
            self._status = NodeStatus.RUNNING
            LOG.info(f"Started container {self.config.container_name}")
            return True
            
        except Exception as e:
            self._status = NodeStatus.ERROR
            LOG.error(f"Failed to start {self.config.container_name}: {e}")
            return False
    
    async def stop(self, timeout: int = 10) -> bool:
        """
        停止容器
        
        Args:
            timeout: 停止超时时间（秒）
            
        Returns:
            bool: 是否成功停止
        """
        if self._container is None:
            LOG.warning(f"Node {self.name} has no container")
            return True
        
        try:
            self._container.stop(timeout=timeout)
            LOG.info(f"Stopped container {self.config.container_name}")
            self._status = NodeStatus.STOPPED
            return True
        except Exception as e:
            LOG.error(f"Failed to stop {self.config.container_name}: {e}")
            return False
    
    async def restart(self, timeout: int = 10) -> bool:
        """
        重启容器
        
        Args:
            timeout: 停止超时时间（秒）
            
        Returns:
            bool: 是否成功重启
        """
        await self.stop(timeout)
        await asyncio.sleep(1)
        return await self.start()
    
    async def remove(self, force: bool = False) -> bool:
        """
        删除容器
        
        Args:
            force: 是否强制删除运行中的容器
            
        Returns:
            bool: 是否成功删除
        """
        if self._container is None:
            return True
        
        try:
            self._container.remove(force=force)
            self._container = None
            self._status = NodeStatus.STOPPED
            LOG.info(f"Removed container {self.config.container_name}")
            return True
        except Exception as e:
            LOG.error(f"Failed to remove {self.config.container_name}: {e}")
            return False
    
    async def health_check(self) -> Dict[str, Any]:
        """
        健康检查
        
        Returns:
            健康状态信息
        """
        result = {
            "node_id": self.name,
            "status": "unknown",
            "container_status": None,
            "error": None,
        }
        
        if self._container is None:
            result["status"] = "stopped"
            self._status = NodeStatus.STOPPED
            return result
        
        try:
            self._container.reload()
            container_status = self._container.status
            result["container_status"] = container_status
            
            if container_status == "running":
                # TODO: 可以进一步调用 RPC 检查
                result["status"] = "healthy"
                self._status = NodeStatus.RUNNING
            elif container_status == "exited":
                result["status"] = "stopped"
                self._status = NodeStatus.STOPPED
            else:
                result["status"] = "unhealthy"
                self._status = NodeStatus.UNHEALTHY
                
        except Exception as e:
            result["status"] = "error"
            result["error"] = str(e)
            self._status = NodeStatus.ERROR
        
        return result
    
    def get_logs(self, tail: int = 100, since: Optional[int] = None) -> str:
        """
        获取容器日志
        
        Args:
            tail: 获取最后多少行
            since: 从多少秒前开始获取
            
        Returns:
            日志内容
        """
        if self._container is None:
            return ""
        
        try:
            kwargs = {"tail": tail}
            if since:
                kwargs["since"] = since
            return self._container.logs(**kwargs).decode('utf-8')
        except Exception as e:
            LOG.error(f"Failed to get logs for {self.config.container_name}: {e}")
            return ""
    
    def exec_command(self, cmd: str) -> tuple[int, str]:
        """
        在容器内执行命令
        
        Args:
            cmd: 要执行的命令
            
        Returns:
            (exit_code, output)
        """
        if self._container is None:
            return (-1, "Container not running")
        
        try:
            exit_code, output = self._container.exec_run(cmd)
            return (exit_code, output.decode('utf-8'))
        except Exception as e:
            return (-1, str(e))
```

### 3.4 Cluster Manager

**文件：** `gravity_e2e/gravity_e2e/core/cluster_manager.py`

```python
"""
集群管理器 - 参考 LocalSwarm 实现
"""
import asyncio
import json
import logging
from dataclasses import dataclass
from typing import Dict, List, Optional, Any
from pathlib import Path

from .docker_manager import DockerNodeManager, DockerNodeConfig, NodeStatus
from .node_connector import NodeConnector, NodeInfo

LOG = logging.getLogger(__name__)


@dataclass
class ClusterConfig:
    """集群配置"""
    cluster_name: str
    network_name: str
    subnet: str
    chain_id: int
    image: str
    compose_file: Optional[str] = None


class GravityClusterManager:
    """
    Gravity 集群管理器
    
    类似 gravity-aptos 的 LocalSwarm，管理多个 Docker 节点
    """
    
    def __init__(self, cluster_config_path: str):
        """
        初始化集群管理器
        
        Args:
            cluster_config_path: 集群配置文件路径
        """
        self.config_path = Path(cluster_config_path)
        self.cluster_config: Optional[ClusterConfig] = None
        self.nodes: Dict[str, DockerNodeManager] = {}
        self.node_connector: Optional[NodeConnector] = None
        self.launched = False
        
        self._load_config()
    
    def _load_config(self):
        """加载集群配置"""
        with open(self.config_path, 'r') as f:
            config = json.load(f)
        
        # 解析集群配置
        self.cluster_config = ClusterConfig(
            cluster_name=config.get("cluster_name", "gravity-cluster"),
            network_name=config.get("network", {}).get("name", "gravity-cluster"),
            subnet=config.get("network", {}).get("subnet", "172.20.0.0/24"),
            chain_id=config.get("network", {}).get("chain_id", 1337),
            image=config.get("docker", {}).get("image", "gravity_node:latest"),
            compose_file=config.get("docker", {}).get("compose_file"),
        )
        
        # 解析节点配置
        for node_id, node_config in config.get("nodes", {}).items():
            docker_config = DockerNodeConfig(
                node_id=node_id,
                container_name=node_config.get("container_name", f"gravity_{node_id}"),
                image=self.cluster_config.image,
                host="127.0.0.1",
                rpc_port=node_config.get("rpc_port", 8545),
                metrics_port=node_config.get("metrics_port", 9001),
                p2p_port=node_config.get("p2p_port", 2024),
                inspection_port=node_config.get("inspection_port", 10000),
                network_ip=node_config.get("network_ip", ""),
                config_path=Path(node_config["config_dir"]) if "config_dir" in node_config else None,
            )
            self.nodes[node_id] = DockerNodeManager(docker_config)
        
        LOG.info(f"Loaded cluster config: {self.cluster_config.cluster_name} with {len(self.nodes)} nodes")
    
    async def launch(self, timeout: int = 120) -> bool:
        """
        启动整个集群
        
        类似 LocalSwarm.launch()
        
        Args:
            timeout: 等待所有节点就绪的超时时间（秒）
            
        Returns:
            bool: 是否成功启动
        """
        if self.launched:
            raise RuntimeError("Cluster already launched")
        
        LOG.info(f"Launching cluster {self.cluster_config.cluster_name}...")
        
        # 1. 启动所有节点
        start_tasks = []
        for node_id, node_manager in self.nodes.items():
            LOG.info(f"Starting node {node_id}...")
            start_tasks.append(node_manager.start())
        
        results = await asyncio.gather(*start_tasks, return_exceptions=True)
        
        # 检查启动结果
        for node_id, result in zip(self.nodes.keys(), results):
            if isinstance(result, Exception):
                LOG.error(f"Failed to start {node_id}: {result}")
                await self.shutdown()
                return False
            if not result:
                LOG.error(f"Failed to start {node_id}")
                await self.shutdown()
                return False
        
        # 2. 等待所有节点就绪
        success = await self._wait_all_alive(timeout)
        if not success:
            LOG.error("Cluster failed to become ready")
            await self.shutdown()
            return False
        
        # 3. 初始化节点连接器
        await self._init_node_connector()
        
        self.launched = True
        LOG.info(f"Cluster {self.cluster_config.cluster_name} launched successfully")
        return True
    
    async def _wait_all_alive(self, timeout: int) -> bool:
        """
        等待所有节点存活
        
        类似 LocalSwarm.wait_all_alive()
        
        Args:
            timeout: 超时时间（秒）
            
        Returns:
            bool: 所有节点是否就绪
        """
        LOG.info(f"Waiting for all nodes to be ready (timeout: {timeout}s)...")
        
        start_time = asyncio.get_event_loop().time()
        attempt = 0
        
        while asyncio.get_event_loop().time() - start_time < timeout:
            attempt += 1
            all_healthy = True
            
            # 检查所有节点健康状态
            for node_id, node_manager in self.nodes.items():
                health = await node_manager.health_check()
                if health["status"] != "healthy":
                    all_healthy = False
                    LOG.debug(f"Node {node_id} not ready: {health}")
                    break
            
            if all_healthy:
                # 额外检查 RPC 连接
                if await self._check_rpc_connectivity():
                    LOG.info(f"All nodes ready after {attempt} attempts")
                    return True
            
            await asyncio.sleep(2)
        
        LOG.error(f"Timeout waiting for nodes to be ready")
        return False
    
    async def _check_rpc_connectivity(self) -> bool:
        """检查 RPC 连接"""
        import aiohttp
        
        for node_id, node_manager in self.nodes.items():
            url = node_manager.config.rpc_url
            try:
                async with aiohttp.ClientSession() as session:
                    async with session.post(
                        url,
                        json={"jsonrpc": "2.0", "method": "eth_blockNumber", "params": [], "id": 1},
                        timeout=aiohttp.ClientTimeout(total=5)
                    ) as resp:
                        if resp.status != 200:
                            LOG.debug(f"Node {node_id} RPC not ready: HTTP {resp.status}")
                            return False
            except Exception as e:
                LOG.debug(f"Node {node_id} RPC not ready: {e}")
                return False
        
        return True
    
    async def _init_node_connector(self):
        """初始化节点连接器"""
        # 生成临时节点配置
        nodes_config = {
            "network": {
                "name": self.cluster_config.cluster_name,
                "chain_id": self.cluster_config.chain_id,
            },
            "clusters": {
                "all": {
                    "description": "All nodes",
                    "nodes": list(self.nodes.keys()),
                }
            },
            "nodes": {}
        }
        
        for node_id, node_manager in self.nodes.items():
            nodes_config["nodes"][node_id] = {
                "type": "validator",
                "role": "primary",
                "host": node_manager.config.host,
                "rpc_port": node_manager.config.rpc_port,
                "metrics_port": node_manager.config.metrics_port,
            }
        
        # 保存临时配置文件
        tmp_config = Path("/tmp/gravity_cluster_nodes.json")
        with open(tmp_config, 'w') as f:
            json.dump(nodes_config, f, indent=2)
        
        self.node_connector = NodeConnector(str(tmp_config))
        await self.node_connector.connect_all()
    
    async def shutdown(self):
        """
        关闭整个集群
        
        类似 LocalSwarm 的 Drop 实现
        """
        LOG.info(f"Shutting down cluster {self.cluster_config.cluster_name}...")
        
        # 关闭节点连接器
        if self.node_connector:
            await self.node_connector.close_all()
            self.node_connector = None
        
        # 停止所有节点
        stop_tasks = []
        for node_id, node_manager in self.nodes.items():
            LOG.info(f"Stopping node {node_id}...")
            stop_tasks.append(node_manager.stop())
        
        await asyncio.gather(*stop_tasks, return_exceptions=True)
        
        self.launched = False
        LOG.info("Cluster shutdown complete")
    
    async def restart_node(self, node_id: str, timeout: int = 60) -> bool:
        """
        重启单个节点
        
        Args:
            node_id: 节点 ID
            timeout: 等待节点就绪的超时时间
            
        Returns:
            bool: 是否成功重启
        """
        if node_id not in self.nodes:
            raise ValueError(f"Unknown node: {node_id}")
        
        LOG.info(f"Restarting node {node_id}...")
        
        node_manager = self.nodes[node_id]
        success = await node_manager.restart()
        
        if success:
            # 等待节点就绪
            start_time = asyncio.get_event_loop().time()
            while asyncio.get_event_loop().time() - start_time < timeout:
                health = await node_manager.health_check()
                if health["status"] == "healthy":
                    LOG.info(f"Node {node_id} restarted successfully")
                    return True
                await asyncio.sleep(2)
            
            LOG.error(f"Node {node_id} failed to become ready after restart")
            return False
        
        return False
    
    async def stop_node(self, node_id: str) -> bool:
        """停止单个节点"""
        if node_id not in self.nodes:
            raise ValueError(f"Unknown node: {node_id}")
        
        LOG.info(f"Stopping node {node_id}...")
        return await self.nodes[node_id].stop()
    
    async def start_node(self, node_id: str) -> bool:
        """启动单个节点"""
        if node_id not in self.nodes:
            raise ValueError(f"Unknown node: {node_id}")
        
        LOG.info(f"Starting node {node_id}...")
        return await self.nodes[node_id].start()
    
    async def health_check(self) -> Dict[str, Dict]:
        """
        检查所有节点健康状态
        
        Returns:
            {node_id: health_info}
        """
        results = {}
        for node_id, node_manager in self.nodes.items():
            results[node_id] = await node_manager.health_check()
        return results
    
    def get_node_logs(self, node_id: str, tail: int = 100) -> str:
        """获取节点日志"""
        if node_id not in self.nodes:
            raise ValueError(f"Unknown node: {node_id}")
        return self.nodes[node_id].get_logs(tail=tail)
    
    def get_client(self, node_id: str):
        """获取节点 RPC 客户端"""
        if not self.node_connector:
            raise RuntimeError("Cluster not launched")
        return self.node_connector.get_client(node_id)
    
    # 上下文管理器支持
    async def __aenter__(self):
        await self.launch()
        return self
    
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        await self.shutdown()
```

### 3.5 集群配置文件

**文件：** `gravity_e2e/configs/cluster.json`

```json
{
  "cluster_name": "gravity-local-cluster",
  "network": {
    "name": "gravity-cluster",
    "subnet": "172.20.0.0/24",
    "chain_id": 1337
  },
  "docker": {
    "image": "gravity_node:latest",
    "compose_file": "docker/cluster/docker-compose.cluster.yaml"
  },
  "nodes": {
    "node_0": {
      "container_name": "gravity_node_0",
      "role": "validator",
      "network_ip": "172.20.0.10",
      "rpc_port": 8545,
      "metrics_port": 9001,
      "p2p_port": 2024,
      "inspection_port": 10000,
      "config_dir": "docker/cluster/configs/node_0"
    },
    "node_1": {
      "container_name": "gravity_node_1",
      "role": "validator",
      "network_ip": "172.20.0.11",
      "rpc_port": 8546,
      "metrics_port": 9002,
      "p2p_port": 2025,
      "inspection_port": 10001,
      "config_dir": "docker/cluster/configs/node_1"
    },
    "node_2": {
      "container_name": "gravity_node_2",
      "role": "validator",
      "network_ip": "172.20.0.12",
      "rpc_port": 8547,
      "metrics_port": 9003,
      "p2p_port": 2026,
      "inspection_port": 10002,
      "config_dir": "docker/cluster/configs/node_2"
    },
    "node_3": {
      "container_name": "gravity_node_3",
      "role": "validator",
      "network_ip": "172.20.0.13",
      "rpc_port": 8548,
      "metrics_port": 9004,
      "p2p_port": 2027,
      "inspection_port": 10003,
      "config_dir": "docker/cluster/configs/node_3"
    }
  }
}
```

---

## 4. 使用示例

### 4.1 在测试中使用

```python
import asyncio
from gravity_e2e.core.cluster_manager import GravityClusterManager

async def test_with_docker_cluster():
    """使用 Docker 集群进行测试"""
    
    # 方式1: 使用上下文管理器
    async with GravityClusterManager("configs/cluster.json") as cluster:
        # 获取节点客户端
        client = cluster.get_client("node_0")
        
        # 执行测试...
        block_number = await client.get_block_number()
        print(f"Current block: {block_number}")
        
        # 测试节点重启场景
        await cluster.stop_node("node_1")
        await asyncio.sleep(5)
        await cluster.start_node("node_1")
        
        # 等待节点恢复
        await asyncio.sleep(10)
        
        # 验证集群状态
        health = await cluster.health_check()
        print(f"Cluster health: {health}")

    # 方式2: 手动管理
    cluster = GravityClusterManager("configs/cluster.json")
    try:
        await cluster.launch(timeout=120)
        
        # 执行测试...
        
    finally:
        await cluster.shutdown()


if __name__ == "__main__":
    asyncio.run(test_with_docker_cluster())
```

### 4.2 命令行使用

```bash
# 启动集群
docker-compose -f docker/cluster/docker-compose.cluster.yaml up -d

# 查看状态
docker-compose -f docker/cluster/docker-compose.cluster.yaml ps

# 查看日志
docker-compose -f docker/cluster/docker-compose.cluster.yaml logs -f gravity_node_0

# 重启单个节点
docker-compose -f docker/cluster/docker-compose.cluster.yaml restart gravity_node_1

# 停止集群
docker-compose -f docker/cluster/docker-compose.cluster.yaml down

# 清理数据
docker-compose -f docker/cluster/docker-compose.cluster.yaml down -v
```

---

## 5. 实现步骤

| 阶段 | 任务 | 优先级 | 说明 |
|------|------|--------|------|
| 1 | 创建目录结构 | P0 | 创建 `docker/cluster/` 和配置文件 |
| 2 | 编写 docker-compose.cluster.yaml | P0 | 多节点 Docker Compose 配置 |
| 3 | 实现 DockerNodeManager | P0 | 单节点 Docker 生命周期管理 |
| 4 | 实现 GravityClusterManager | P0 | 集群编排管理 |
| 5 | 集成到 gravity_e2e | P1 | 添加命令行参数和测试支持 |
| 6 | 添加 Genesis 自动生成 | P2 | 根据节点数量自动生成配置 |
| 7 | 添加网络分区测试支持 | P2 | 模拟网络故障场景 |

---

## 6. 对比总结

| 功能 | gravity-aptos LocalSwarm | gravity-sdk GravityClusterManager |
|------|--------------------------|-----------------------------------|
| 节点管理单元 | LocalNode (进程) | DockerNodeManager (容器) |
| 启动方式 | Command::spawn() | docker.containers.run() |
| 停止方式 | Process Drop | container.stop() |
| 健康检查 | REST API + Inspection | Container status + RPC |
| 配置管理 | NodeConfig (yaml) | DockerNodeConfig (json) |
| 网络隔离 | 共享本地网络 | Docker bridge network |
| 日志管理 | 文件重定向 | container.logs() |

