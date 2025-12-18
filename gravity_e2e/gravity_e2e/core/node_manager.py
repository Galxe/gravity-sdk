"""
Node Manager - 管理节点的部署、启动和停止
支持通过 deploy_utils 脚本部署节点，通过 gravity_cli 启停节点
"""
import asyncio
import logging
import subprocess
import shutil
from pathlib import Path
from typing import Dict, List, Optional, Tuple

LOG = logging.getLogger(__name__)


class NodeManager:
    """节点管理器 - 负责节点的部署、启动和停止"""
    
    def __init__(self, workspace_root: Optional[Path] = None):
        """初始化节点管理器
        
        Args:
            workspace_root: 工作区根目录路径，如果为 None 则自动检测
        """
        if workspace_root is None:
            # 自动检测工作区根目录（gravity-sdk 目录）
            # gravity_e2e/gravity_e2e/core/node_manager.py -> gravity-sdk/
            # core -> gravity_e2e -> gravity_e2e -> gravity-sdk
            current_file = Path(__file__).resolve()
            workspace_root = current_file.parent.parent.parent.parent
        
        self.workspace_root = Path(workspace_root).resolve()
        self.deploy_utils_dir = self.workspace_root / "deploy_utils"
        self.gravity_cli_path = self._find_gravity_cli()
        
        LOG.info(f"NodeManager initialized with workspace: {self.workspace_root}")
        LOG.info(f"Deploy utils directory: {self.deploy_utils_dir}")
        LOG.info(f"Gravity CLI path: {self.gravity_cli_path}")
    
    def _find_gravity_cli(self) -> Path:
        """查找 gravity_cli 二进制文件路径"""
        # 尝试多个可能的位置
        possible_paths = [
            self.workspace_root / "target" / "debug" / "gravity_cli",
            self.workspace_root / "target" / "release" / "gravity_cli",
            self.workspace_root / "target" / "quick-release" / "gravity_cli",
        ]
        
        for path in possible_paths:
            if path.exists() and path.is_file():
                return path
        
        # 如果找不到，返回默认路径（可能不存在，会在使用时报错）
        default_path = possible_paths[0]
        LOG.warning(f"Gravity CLI not found in standard locations, using default: {default_path}")
        return default_path
    
    def _run_command(self, cmd: List[str], cwd: Optional[Path] = None, timeout: int = 60, shell: bool = False) -> Tuple[bool, str, str]:
        """运行 shell 命令
        
        Args:
            cmd: 命令列表
            cwd: 工作目录
            timeout: 超时时间（秒）
        
        Returns:
            (success, stdout, stderr) 元组
        """
        try:
            result = subprocess.run(
                cmd,
                cwd=cwd,
                capture_output=True,
                text=True,
                timeout=timeout,
                check=False
            )
            return result.returncode == 0, result.stdout, result.stderr
        except subprocess.TimeoutExpired:
            LOG.error(f"Command timeout: {' '.join(cmd)}")
            return False, "", "Command timeout"
        except Exception as e:
            LOG.error(f"Command failed: {' '.join(cmd)}, error: {e}")
            return False, "", str(e)
    
    def deploy_node(
        self,
        node_name: str,
        mode: str = "cluster",
        install_dir: str = "/tmp",
        bin_version: str = "debug",
        recover: bool = False
    ) -> bool:
        """部署单个节点
        
        Args:
            node_name: 节点名称
            mode: 部署模式 ("cluster" 或 "single")
            install_dir: 安装目录
            bin_version: 二进制版本 ("debug", "release", "quick-release")
            recover: 是否恢复模式
        
        Returns:
            是否成功
        """
        deploy_script = self.deploy_utils_dir / "deploy.sh"
        if not deploy_script.exists():
            LOG.error(f"Deploy script not found: {deploy_script}")
            return False
        
        # 使用 bash 运行脚本，因为脚本使用了 bash 特性（关联数组）
        # 尝试使用 homebrew 的 bash（支持关联数组），如果不存在则使用系统 bash
        bash_paths = ["/opt/homebrew/bin/bash", "/usr/local/bin/bash", "/bin/bash"]
        bash_path = None
        for path in bash_paths:
            if Path(path).exists():
                bash_path = path
                break
        
        if bash_path is None:
            bash_path = "/bin/bash"  # 最后的后备选项
        
        cmd = [
            bash_path,
            str(deploy_script),
            "-n", node_name,
            "-m", mode,
            "-i", install_dir,
            "-v", bin_version
        ]
        
        if recover:
            cmd.append("-r")
        
        LOG.info(f"Deploying node {node_name} with command: {' '.join(cmd)}")
        success, stdout, stderr = self._run_command(cmd, cwd=self.deploy_utils_dir, timeout=300)
        
        if success:
            LOG.info(f"Node {node_name} deployed successfully")
            if stdout:
                LOG.debug(f"Deploy stdout: {stdout}")
        else:
            LOG.error(f"Failed to deploy node {node_name}")
            if stderr:
                LOG.error(f"Deploy stderr: {stderr}")
            if stdout:
                LOG.error(f"Deploy stdout: {stdout}")
        
        return success
    
    def deploy_nodes(
        self,
        node_names: List[str],
        mode: str = "cluster",
        install_dir: str = "/tmp",
        bin_version: str = "debug",
        recover: bool = False
    ) -> Dict[str, bool]:
        """部署多个节点
        
        Args:
            node_names: 节点名称列表
            mode: 部署模式
            install_dir: 安装目录
            bin_version: 二进制版本
            recover: 是否恢复模式
        
        Returns:
            节点名称到成功状态的映射
        """
        results = {}
        for node_name in node_names:
            results[node_name] = self.deploy_node(
                node_name=node_name,
                mode=mode,
                install_dir=install_dir,
                bin_version=bin_version,
                recover=recover
            )
        return results
    
    def start_node(self, deploy_path: str) -> bool:
        """启动节点
        
        Args:
            deploy_path: 节点部署路径
        
        Returns:
            是否成功
        """
        deploy_path_obj = Path(deploy_path)
        start_script = deploy_path_obj / "script" / "start.sh"
        
        if not start_script.exists():
            LOG.error(f"Start script not found: {start_script}")
            return False
        
        # 直接调用 start.sh 脚本，避免通过 gravity_cli 可能出现的阻塞问题
        bash_paths = ["/opt/homebrew/bin/bash", "/usr/local/bin/bash", "/bin/bash"]
        bash_path = None
        for path in bash_paths:
            if Path(path).exists():
                bash_path = path
                break
        
        if bash_path is None:
            bash_path = "/bin/bash"
        
        cmd = [bash_path, str(start_script)]
        
        LOG.info(f"Starting node at {deploy_path}")
        # start.sh 在后台启动节点，应该立即返回
        # 不判断节点是否真正启动，后续通过 HTTP 端口访问来验证
        # 使用 Popen 而不是 run，避免等待进程结束
        try:
            process = subprocess.Popen(
                cmd,
                cwd=deploy_path_obj,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                bufsize=1  # 行缓冲
            )
            # 不等待进程结束，立即返回
            # start.sh 会在后台启动节点并立即退出
            LOG.info(f"Node start command launched at {deploy_path} (PID: {process.pid})")
            return True
        except Exception as e:
            LOG.error(f"Failed to launch node start command at {deploy_path}: {e}")
            return False
    
    def start_nodes(self, deploy_paths: List[str]) -> Dict[str, bool]:
        """启动多个节点
        
        Args:
            deploy_paths: 节点部署路径列表
        
        Returns:
            部署路径到成功状态的映射
        """
        results = {}
        for deploy_path in deploy_paths:
            results[deploy_path] = self.start_node(deploy_path)
        return results
    
    def stop_node(self, deploy_path: str, cleanup: bool = False) -> bool:
        """停止节点
        
        Args:
            deploy_path: 节点部署路径
            cleanup: 是否在停止后清理部署目录（默认 True）
        
        Returns:
            是否成功
        """
        if not self.gravity_cli_path.exists():
            LOG.error(f"Gravity CLI not found: {self.gravity_cli_path}")
            return False
        
        cmd = [str(self.gravity_cli_path), "node", "stop", "--deploy-path", deploy_path]
        
        LOG.info(f"Stopping node at {deploy_path}")
        success, stdout, stderr = self._run_command(cmd)
        
        if success:
            LOG.info(f"Node stopped successfully at {deploy_path}")
            if stdout:
                LOG.debug(f"Stop stdout: {stdout}")
            
            # 停止成功后，清理部署目录
            if cleanup:
                deploy_path_obj = Path(deploy_path)
                if deploy_path_obj.exists():
                    try:
                        import shutil
                        LOG.info(f"Cleaning up deployment directory: {deploy_path}")
                        shutil.rmtree(deploy_path_obj)
                        LOG.info(f"✅ Deployment directory removed: {deploy_path}")
                    except Exception as e:
                        LOG.warning(f"Failed to remove deployment directory {deploy_path}: {e}")
        else:
            LOG.error(f"Failed to stop node at {deploy_path}")
            if stderr:
                LOG.error(f"Stop stderr: {stderr}")
        
        return success
    
    def stop_nodes(self, deploy_paths: List[str], cleanup: bool = False) -> Dict[str, bool]:
        """停止多个节点
        
        Args:
            deploy_paths: 节点部署路径列表
            cleanup: 是否在停止后清理部署目录（默认 True）
        
        Returns:
            部署路径到成功状态的映射
        """
        results = {}
        for deploy_path in deploy_paths:
            results[deploy_path] = self.stop_node(deploy_path, cleanup=cleanup)
        return results
    
    def get_node_deploy_path(self, node_name: str, install_dir: str) -> str:
        """获取节点的完整部署路径
        
        Args:
            node_name: 节点名称
            install_dir: 安装目录
        
        Returns:
            完整的部署路径
        """
        return str(Path(install_dir) / node_name)
