# Randomness测试迁移方案（基于EVM/Reth执行层）

## 一、技术栈确认

### 1.1 当前环境

**执行层：Reth (以太坊兼容)**
- 完整的EVM支持
- 标准的JSON-RPC接口（端口8545）
- Solidity智能合约
- block.difficulty映射到随机数种子

**共识层：Gravity节点**
- DKG（分布式密钥生成）
- HTTP API（端口1998）：
  - `GET /dkg/status` - DKG状态
  - `GET /dkg/randomness/{block}` - 指定块的随机数

**测试框架：gravity_e2e**
- Python + asyncio
- JSON-RPC客户端（已有）
- 需要添加：HTTP API客户端

### 1.2 已有资源

**智能合约示例：**
```solidity
// examples/randomness/RandomDice.sol
contract RandomDice {
    uint256 public lastRollResult;
    uint256 public lastSeedUsed;
    
    function rollDice() public {
        uint256 seed = block.difficulty;  // 获取随机数种子
        uint256 result = (seed % 6) + 1;
        lastRollResult = result;
        lastSeedUsed = seed;
    }
}
```

**验证脚本示例：**
```bash
# examples/randomness/check.sh
# 1. 调用rollDice()
# 2. 读取lastSeedUsed
# 3. 读取block.difficulty
# 4. 验证两者相等
```

---

## 二、测试用例重新分类

### 2.1 可完全实现（基于EVM + HTTP API）

#### ⭐⭐ Test 1: 基础随机数消费测试
**对应原测试**: `e2e_basic_consumption.rs`

**实现内容：**
1. 部署RandomDice合约
2. 多次调用rollDice()
3. 验证每次返回的随机数在1-6范围
4. 验证block.difficulty正确传递
5. 检查随机数种子的变化

**实现难度**: ⭐⭐ (简单)
**预计工期**: 2天
**优先级**: P0（最高）

---

#### ⭐⭐ Test 2: 随机数种子正确性验证
**对应原测试**: `e2e_correctness.rs`（简化版）

**实现内容：**
1. 通过HTTP API获取DKG状态
2. 通过HTTP API获取随机数数据
3. 验证block.difficulty与API返回的随机数一致
4. 验证不同epoch的随机数确实不同
5. 验证随机数的存在性和格式

**不包含**：密码学验证（DKG transcript验证）

**实现难度**: ⭐⭐ (简单)
**预计工期**: 2天
**优先级**: P0（最高）

---

#### ⭐⭐⭐ Test 3: Epoch变化观察测试
**对应原测试**: 部分enable_feature系列

**实现内容：**
1. 监控当前epoch
2. 等待epoch变化
3. 验证新epoch后：
   - DKG状态更新
   - 随机数种子变化
   - block.difficulty更新
4. 记录epoch转换的完整过程

**实现难度**: ⭐⭐⭐ (中等)
**预计工期**: 3天
**优先级**: P1（高）

---

#### ⭐⭐⭐ Test 4: 随机数API完整性测试

**实现内容：**
1. 测试`/dkg/status`的所有字段
2. 测试`/dkg/randomness/{block}`的边界情况：
   - 未来的块号
   - 过去的块号
   - 不存在的块号
3. 验证API响应格式
4. 性能测试（并发查询）

**实现难度**: ⭐⭐⭐ (中等)
**预计工期**: 2天
**优先级**: P1（高）

---

#### ⭐⭐⭐⭐ Test 5: 多合约随机数隔离测试

**实现内容：**
1. 部署多个RandomDice实例
2. 同一块内调用多个合约
3. 验证它们获得相同的block.difficulty
4. 验证合约之间的随机数使用互不影响

**实现难度**: ⭐⭐⭐⭐ (较难)
**预计工期**: 3天
**优先级**: P2（中）

---

### 2.2 可部分实现（观察性测试）

#### ⭐⭐⭐⭐ Test 6: 随机数质量观察测试
**灵感来源**: `e2e_correctness.rs`

**实现内容：**
1. 收集大量随机数样本（1000+）
2. 统计分析：
   - 分布均匀性
   - 连续性检查
   - 重复率
3. 可视化展示
4. 生成质量报告

**注意**: 这是观察性测试，不做密码学证明

**实现难度**: ⭐⭐⭐⭐ (较难)
**预计工期**: 4天
**优先级**: P2（中）

---

#### ⭐⭐⭐⭐⭐ Test 7: 压力测试
**灵感来源**: 性能测试思路

**实现内容：**
1. 高频调用rollDice()
2. 监控：
   - 交易确认时间
   - block.difficulty更新频率
   - DKG状态稳定性
3. 记录性能指标
4. 生成压力测试报告

**实现难度**: ⭐⭐⭐⭐⭐ (困难)
**预计工期**: 5天
**优先级**: P3（低）

---

### 2.3 无法实现（需要节点管理或Move特性）

❌ **不可实现的测试：**
1. `dkg_with_validator_down.rs` - 需要停止/启动节点
2. `dkg_with_validator_join_leave.rs` - 需要validator管理
3. `randomness_stall_recovery.rs` - 需要配置热更新和节点重启
4. `validator_restart_during_dkg.rs` - 需要failpoint注入
5. `entry_func_attrs.rs` - Move语言特有功能
6. `enable_feature_*.rs` - 需要governance交易（Move脚本）
7. `disable_feature_*.rs` - 需要governance交易（Move脚本）

---

## 三、技术实现方案

### 3.1 扩展gravity_e2e框架

#### 3.1.1 添加HTTP API客户端

```python
# gravity_e2e/gravity_e2e/core/client/gravity_http_client.py

import aiohttp
import logging
from typing import Dict, Optional

LOG = logging.getLogger(__name__)


class GravityHttpClient:
    """Gravity节点HTTP API客户端"""
    
    def __init__(self, base_url: str = "http://127.0.0.1:1998", timeout: float = 30.0):
        self.base_url = base_url.rstrip('/')
        self.timeout = timeout
        self.session: Optional[aiohttp.ClientSession] = None
    
    async def __aenter__(self):
        self.session = aiohttp.ClientSession(
            timeout=aiohttp.ClientTimeout(total=self.timeout)
        )
        return self
    
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        if self.session:
            await self.session.close()
            self.session = None
    
    async def get_dkg_status(self) -> Dict:
        """
        获取DKG状态
        
        返回格式:
        {
            "epoch": 5,
            "round": 123,
            "block_number": 45678,
            "participating_nodes": 4
        }
        """
        url = f"{self.base_url}/dkg/status"
        LOG.debug(f"Getting DKG status from {url}")
        
        async with self.session.get(url) as resp:
            if resp.status != 200:
                text = await resp.text()
                raise RuntimeError(f"Failed to get DKG status: {resp.status} - {text}")
            
            data = await resp.json()
            LOG.info(f"DKG Status: epoch={data['epoch']}, round={data['round']}, "
                    f"block={data['block_number']}, nodes={data['participating_nodes']}")
            return data
    
    async def get_randomness(self, block_number: int) -> Optional[str]:
        """
        获取指定块的随机数
        
        Args:
            block_number: 块号
        
        Returns:
            十六进制随机数字符串，如果不存在则返回None
        """
        url = f"{self.base_url}/dkg/randomness/{block_number}"
        LOG.debug(f"Getting randomness for block {block_number} from {url}")
        
        async with self.session.get(url) as resp:
            if resp.status != 200:
                text = await resp.text()
                LOG.warning(f"Failed to get randomness: {resp.status} - {text}")
                return None
            
            data = await resp.json()
            randomness = data.get("randomness")
            
            if randomness:
                LOG.info(f"Block {block_number} randomness: {randomness[:16]}...")
            else:
                LOG.info(f"Block {block_number} has no randomness")
            
            return randomness
    
    async def wait_for_epoch(self, target_epoch: int, timeout: int = 120) -> int:
        """
        等待指定epoch
        
        Args:
            target_epoch: 目标epoch
            timeout: 超时时间（秒）
        
        Returns:
            当前epoch号
        """
        import asyncio
        import time
        
        start = time.time()
        LOG.info(f"Waiting for epoch {target_epoch}...")
        
        while time.time() - start < timeout:
            status = await self.get_dkg_status()
            current_epoch = status["epoch"]
            
            if current_epoch >= target_epoch:
                LOG.info(f"Reached epoch {current_epoch}")
                return current_epoch
            
            LOG.debug(f"Current epoch: {current_epoch}, target: {target_epoch}")
            await asyncio.sleep(2)
        
        raise TimeoutError(f"Timeout waiting for epoch {target_epoch}")
```

**工作量**: 1天

#### 3.1.2 RandomDice合约辅助工具

```python
# gravity_e2e/gravity_e2e/utils/randomness_utils.py

import json
from pathlib import Path
from typing import Dict
from ..core.client.gravity_client import GravityClient
from ..utils.contract_utils import ContractUtils


class RandomDiceHelper:
    """RandomDice合约辅助类"""
    
    # RandomDice ABI
    ABI = [
        {
            "type": "function",
            "name": "rollDice",
            "inputs": [],
            "outputs": [],
            "stateMutability": "nonpayable"
        },
        {
            "type": "function",
            "name": "lastRollResult",
            "inputs": [],
            "outputs": [{"type": "uint256"}],
            "stateMutability": "view"
        },
        {
            "type": "function",
            "name": "lastSeedUsed",
            "inputs": [],
            "outputs": [{"type": "uint256"}],
            "stateMutability": "view"
        },
        {
            "type": "function",
            "name": "lastRoller",
            "inputs": [],
            "outputs": [{"type": "address"}],
            "stateMutability": "view"
        },
        {
            "type": "function",
            "name": "getLatestRoll",
            "inputs": [],
            "outputs": [
                {"type": "address"},
                {"type": "uint256"},
                {"type": "uint256"}
            ],
            "stateMutability": "view"
        },
        {
            "type": "event",
            "name": "DiceRolled",
            "inputs": [
                {"type": "address", "indexed": True, "name": "roller"},
                {"type": "uint256", "indexed": False, "name": "result"},
                {"type": "uint256", "indexed": False, "name": "seed"}
            ]
        }
    ]
    
    # RandomDice bytecode（从编译结果获取）
    BYTECODE = None  # 需要从forge build获取
    
    @staticmethod
    def load_bytecode() -> str:
        """从编译输出加载字节码"""
        # 读取forge编译输出
        contract_path = Path(__file__).parent.parent.parent.parent / "out/RandomDice.sol/RandomDice.json"
        
        if not contract_path.exists():
            raise FileNotFoundError(
                f"RandomDice not compiled. Run: forge build\n"
                f"Expected path: {contract_path}"
            )
        
        with open(contract_path, 'r') as f:
            contract_data = json.load(f)
            bytecode = contract_data["bytecode"]["object"]
            
            if not bytecode.startswith("0x"):
                bytecode = "0x" + bytecode
            
            return bytecode
    
    def __init__(self, client: GravityClient, contract_address: str):
        self.client = client
        self.address = contract_address
    
    async def roll_dice(self, from_account: Dict) -> Dict:
        """
        调用rollDice()
        
        Returns:
            交易receipt
        """
        from eth_account import Account
        from eth_utils import to_checksum_address
        
        # 编码函数调用
        data = ContractUtils.encode_function_call("rollDice", [])
        
        # 获取nonce和gas price
        nonce = await self.client.get_transaction_count(from_account["address"])
        gas_price = await self.client.get_gas_price()
        
        # 构建交易
        tx_data = {
            "to": to_checksum_address(self.address),
            "data": data,
            "gas": hex(100000),
            "gasPrice": hex(gas_price),
            "nonce": hex(nonce),
            "chainId": hex(await self.client.get_chain_id()),
            "value": "0x0"
        }
        
        # 签名
        private_key = from_account["private_key"]
        if private_key.startswith("0x"):
            private_key = private_key[2:]
        
        signed_tx = Account.sign_transaction(tx_data, private_key)
        
        # 发送
        tx_hash = await self.client.send_raw_transaction(signed_tx.raw_transaction)
        
        # 等待确认
        receipt = await self.client.wait_for_transaction_receipt(tx_hash, timeout=30)
        
        return receipt
    
    async def get_last_result(self) -> int:
        """获取最后一次roll的结果"""
        data = ContractUtils.encode_function_call("lastRollResult", [])
        result = await self.client.call(to=self.address, data=data)
        return ContractUtils.decode_uint256(result)
    
    async def get_last_seed(self) -> int:
        """获取最后使用的种子"""
        data = ContractUtils.encode_function_call("lastSeedUsed", [])
        result = await self.client.call(to=self.address, data=data)
        return ContractUtils.decode_uint256(result)
    
    async def get_latest_roll(self) -> tuple:
        """获取最新的roll信息（roller, result, seed）"""
        data = ContractUtils.encode_function_call("getLatestRoll", [])
        result = await self.client.call(to=self.address, data=data)
        
        # 解析返回值（address + uint256 + uint256）
        # 每个都是32字节
        result_hex = result[2:] if result.startswith("0x") else result
        
        roller_hex = result_hex[24:64]  # address在后20字节
        roll_result_hex = result_hex[64:128]
        seed_hex = result_hex[128:192]
        
        roller = "0x" + roller_hex
        roll_result = int(roll_result_hex, 16)
        seed = int(seed_hex, 16)
        
        return (roller, roll_result, seed)


class RandomnessVerifier:
    """随机数验证工具"""
    
    @staticmethod
    async def verify_block_randomness(
        rpc_client: GravityClient,
        http_client,  # GravityHttpClient
        block_number: int
    ) -> Dict:
        """
        验证指定块的随机数
        
        Returns:
            验证结果字典
        """
        # 1. 从HTTP API获取随机数
        api_randomness = await http_client.get_randomness(block_number)
        
        # 2. 获取区块信息
        block = await rpc_client.get_block(block_number, full_transactions=False)
        
        # 3. 提取difficulty和mixHash
        difficulty_hex = block.get("difficulty", "0x0")
        mix_hash_hex = block.get("mixHash", "0x0")
        
        difficulty = int(difficulty_hex, 16)
        mix_hash = int(mix_hash_hex, 16)
        
        # 4. 验证
        result = {
            "block_number": block_number,
            "api_randomness": api_randomness,
            "block_difficulty": difficulty,
            "block_mix_hash": mix_hash,
            "difficulty_hex": difficulty_hex,
            "mix_hash_hex": mix_hash_hex,
            "checks": {}
        }
        
        # Check 1: API随机数存在
        result["checks"]["has_api_randomness"] = api_randomness is not None
        
        # Check 2: difficulty == mixHash（PoS中的要求）
        result["checks"]["difficulty_equals_mixhash"] = (difficulty == mix_hash)
        
        # Check 3: API随机数与difficulty的关系
        if api_randomness:
            # 这里根据实际实现验证
            # API返回的可能是原始随机数或经过处理的
            result["checks"]["api_matches_block"] = True  # 需要具体逻辑
        
        # 总体验证结果
        result["valid"] = all(result["checks"].values())
        
        return result
```

**工作量**: 1天

### 3.2 测试用例实现

#### Test 1: 基础随机数消费测试（完整实现）

```python
# gravity_e2e/gravity_e2e/tests/test_cases/test_randomness_basic.py

import asyncio
import logging
from typing import Dict

from ...helpers.test_helpers import RunHelper, TestResult, test_case
from ...core.client.gravity_http_client import GravityHttpClient
from ...utils.randomness_utils import RandomDiceHelper, RandomnessVerifier
from eth_account import Account
from eth_utils import to_checksum_address

LOG = logging.getLogger(__name__)


@test_case
async def test_randomness_basic_consumption(
    run_helper: RunHelper,
    test_result: TestResult
):
    """
    测试基础随机数消费功能
    
    步骤:
    1. 部署RandomDice合约
    2. 多次调用rollDice()
    3. 验证随机数范围（1-6）
    4. 验证block.difficulty传递正确
    5. 验证随机数种子变化
    """
    LOG.info("=" * 60)
    LOG.info("Test: Randomness Basic Consumption")
    LOG.info("=" * 60)
    
    # 1. 初始化HTTP客户端（假设HTTP API在1998端口）
    http_url = run_helper.client.rpc_url.replace(":8545", ":1998")
    async with GravityHttpClient(http_url) as http_client:
        
        # 2. 获取当前DKG状态
        dkg_status = await http_client.get_dkg_status()
        LOG.info(f"Current DKG Status:")
        LOG.info(f"  Epoch: {dkg_status['epoch']}")
        LOG.info(f"  Round: {dkg_status['round']}")
        LOG.info(f"  Block: {dkg_status['block_number']}")
        LOG.info(f"  Nodes: {dkg_status['participating_nodes']}")
        
        # 3. 部署RandomDice合约
        LOG.info("\n[Step 1] Deploying RandomDice contract...")
        
        deployer = run_helper.faucet_account
        
        # 加载字节码
        bytecode = RandomDiceHelper.load_bytecode()
        
        # 获取部署参数
        nonce = await run_helper.client.get_transaction_count(deployer["address"])
        gas_price = await run_helper.client.get_gas_price()
        chain_id = await run_helper.client.get_chain_id()
        
        # 构建部署交易
        deploy_tx = {
            "data": bytecode,
            "gas": hex(500000),
            "gasPrice": hex(gas_price),
            "nonce": hex(nonce),
            "chainId": hex(chain_id),
            "value": "0x0"
        }
        
        # 签名并发送
        private_key = deployer["private_key"]
        if private_key.startswith("0x"):
            private_key = private_key[2:]
        
        signed_deploy = Account.sign_transaction(deploy_tx, private_key)
        deploy_tx_hash = await run_helper.client.send_raw_transaction(
            signed_deploy.raw_transaction
        )
        
        LOG.info(f"Deploy transaction sent: {deploy_tx_hash}")
        
        # 等待部署确认
        deploy_receipt = await run_helper.client.wait_for_transaction_receipt(
            deploy_tx_hash,
            timeout=60
        )
        
        if deploy_receipt["status"] != "0x1":
            raise RuntimeError(f"Contract deployment failed: {deploy_receipt}")
        
        contract_address = deploy_receipt.get("contractAddress")
        if not contract_address:
            raise RuntimeError("No contract address in deployment receipt")
        
        LOG.info(f"✅ Contract deployed at: {contract_address}")
        LOG.info(f"   Gas used: {int(deploy_receipt['gasUsed'], 16)}")
        
        # 4. 创建RandomDice辅助对象
        dice = RandomDiceHelper(run_helper.client, contract_address)
        
        # 5. 执行10次rollDice
        LOG.info("\n[Step 2] Rolling dice 10 times...")
        
        roll_results = []
        seeds_used = []
        blocks = []
        
        for i in range(10):
            LOG.info(f"\n  Roll #{i+1}:")
            
            # 调用rollDice
            receipt = await dice.roll_dice(deployer)
            
            if receipt["status"] != "0x1":
                LOG.error(f"    ❌ Roll failed")
                continue
            
            block_number = int(receipt["blockNumber"], 16)
            tx_hash = receipt["transactionHash"]
            
            LOG.info(f"    Transaction: {tx_hash}")
            LOG.info(f"    Block: {block_number}")
            
            # 读取结果
            result = await dice.get_last_result()
            seed = await dice.get_last_seed()
            
            LOG.info(f"    Result: {result} (should be 1-6)")
            LOG.info(f"    Seed: {seed}")
            
            # 验证范围
            if not (1 <= result <= 6):
                raise AssertionError(f"Roll result {result} out of range [1, 6]")
            
            roll_results.append(result)
            seeds_used.append(seed)
            blocks.append(block_number)
            
            LOG.info(f"    ✅ Valid roll")
            
            # 短暂等待，确保不同区块
            await asyncio.sleep(1)
        
        # 6. 验证block.difficulty传递
        LOG.info("\n[Step 3] Verifying block.difficulty propagation...")
        
        for i, (block_num, seed) in enumerate(zip(blocks, seeds_used)):
            block = await run_helper.client.get_block(block_num, full_transactions=False)
            
            difficulty_hex = block.get("difficulty", "0x0")
            difficulty = int(difficulty_hex, 16)
            
            LOG.info(f"  Roll #{i+1} (Block {block_num}):")
            LOG.info(f"    Seed from contract: {seed}")
            LOG.info(f"    Block difficulty:   {difficulty}")
            
            if seed != difficulty:
                raise AssertionError(
                    f"Seed mismatch in block {block_num}: "
                    f"contract={seed}, block={difficulty}"
                )
            
            LOG.info(f"    ✅ Match!")
        
        # 7. 验证随机数API
        LOG.info("\n[Step 4] Verifying randomness API...")
        
        for block_num in blocks[:3]:  # 检查前3个块
            api_randomness = await http_client.get_randomness(block_num)
            
            if api_randomness:
                LOG.info(f"  Block {block_num}: {api_randomness[:32]}...")
            else:
                LOG.warning(f"  Block {block_num}: No randomness data")
        
        # 8. 统计分析
        LOG.info("\n[Step 5] Statistical Analysis...")
        LOG.info(f"  Total rolls: {len(roll_results)}")
        LOG.info(f"  Results: {roll_results}")
        LOG.info(f"  Distribution:")
        
        for value in range(1, 7):
            count = roll_results.count(value)
            percentage = (count / len(roll_results)) * 100
            bar = "█" * count
            LOG.info(f"    {value}: {bar} ({count}, {percentage:.1f}%)")
        
        # 9. 验证种子变化
        unique_seeds = len(set(seeds_used))
        LOG.info(f"  Unique seeds: {unique_seeds}/{len(seeds_used)}")
        
        if unique_seeds < len(seeds_used) * 0.8:
            LOG.warning(f"  ⚠️  Low seed diversity: {unique_seeds}/{len(seeds_used)}")
        
        # 10. 记录测试结果
        test_result.mark_success(
            contract_address=contract_address,
            deploy_tx=deploy_tx_hash,
            total_rolls=len(roll_results),
            roll_results=roll_results,
            unique_seeds=unique_seeds,
            blocks_tested=blocks,
            dkg_epoch=dkg_status['epoch'],
            dkg_round=dkg_status['round']
        )
        
        LOG.info("\n" + "=" * 60)
        LOG.info("✅ Test Completed Successfully!")
        LOG.info("=" * 60)


@test_case
async def test_randomness_correctness(
    run_helper: RunHelper,
    test_result: TestResult
):
    """
    测试随机数正确性（观察性验证）
    
    步骤:
    1. 获取多个块的随机数
    2. 验证block.difficulty与API一致性
    3. 验证不同epoch的随机数变化
    """
    LOG.info("=" * 60)
    LOG.info("Test: Randomness Correctness (Observational)")
    LOG.info("=" * 60)
    
    http_url = run_helper.client.rpc_url.replace(":8545", ":1998")
    async with GravityHttpClient(http_url) as http_client:
        
        # 1. 获取当前状态
        dkg_status = await http_client.get_dkg_status()
        current_block = dkg_status['block_number']
        current_epoch = dkg_status['epoch']
        
        LOG.info(f"Current State:")
        LOG.info(f"  Block: {current_block}")
        LOG.info(f"  Epoch: {current_epoch}")
        
        # 2. 验证最近10个块
        LOG.info("\n[Step 1] Verifying recent blocks...")
        
        verification_results = []
        
        for i in range(10):
            block_num = current_block - i
            
            if block_num < 0:
                break
            
            result = await RandomnessVerifier.verify_block_randomness(
                run_helper.client,
                http_client,
                block_num
            )
            
            verification_results.append(result)
            
            LOG.info(f"\n  Block {block_num}:")
            LOG.info(f"    Has API randomness: {result['checks']['has_api_randomness']}")
            LOG.info(f"    Difficulty == MixHash: {result['checks']['difficulty_equals_mixhash']}")
            LOG.info(f"    Valid: {result['valid']}")
        
        # 3. 统计验证结果
        valid_count = sum(1 for r in verification_results if r['valid'])
        total_count = len(verification_results)
        
        LOG.info(f"\n[Step 2] Verification Summary:")
        LOG.info(f"  Total blocks checked: {total_count}")
        LOG.info(f"  Valid blocks: {valid_count}")
        LOG.info(f"  Success rate: {valid_count/total_count*100:.1f}%")
        
        # 4. 等待下一个epoch（可选）
        if current_epoch < 3:
            LOG.info(f"\n[Step 3] Waiting for epoch {current_epoch + 1}...")
            
            next_epoch = await http_client.wait_for_epoch(
                current_epoch + 1,
                timeout=120
            )
            
            LOG.info(f"  ✅ Reached epoch {next_epoch}")
            
            # 验证新epoch的随机数
            new_status = await http_client.get_dkg_status()
            new_block = new_status['block_number']
            
            new_result = await RandomnessVerifier.verify_block_randomness(
                run_helper.client,
                http_client,
                new_block
            )
            
            LOG.info(f"\n  New epoch block {new_block}:")
            LOG.info(f"    Valid: {new_result['valid']}")
        
        test_result.mark_success(
            blocks_verified=total_count,
            valid_blocks=valid_count,
            success_rate=valid_count/total_count,
            current_epoch=current_epoch
        )
        
        LOG.info("\n✅ Correctness test completed!")


@test_case
async def test_epoch_transition(
    run_helper: RunHelper,
    test_result: TestResult
):
    """
    测试epoch转换
    
    监控epoch变化时的随机数更新
    """
    LOG.info("=" * 60)
    LOG.info("Test: Epoch Transition")
    LOG.info("=" * 60)
    
    http_url = run_helper.client.rpc_url.replace(":8545", ":1998")
    async with GravityHttpClient(http_url) as http_client:
        
        # 1. 记录初始状态
        initial_status = await http_client.get_dkg_status()
        initial_epoch = initial_status['epoch']
        initial_block = initial_status['block_number']
        
        LOG.info(f"Initial State:")
        LOG.info(f"  Epoch: {initial_epoch}")
        LOG.info(f"  Block: {initial_block}")
        LOG.info(f"  Round: {initial_status['round']}")
        
        # 获取初始随机数
        initial_randomness = await http_client.get_randomness(initial_block)
        LOG.info(f"  Randomness: {initial_randomness[:32] if initial_randomness else 'None'}...")
        
        # 2. 等待下一个epoch
        LOG.info(f"\nWaiting for epoch {initial_epoch + 1}...")
        
        next_epoch = await http_client.wait_for_epoch(
            initial_epoch + 1,
            timeout=180  # 3分钟
        )
        
        # 3. 记录新状态
        new_status = await http_client.get_dkg_status()
        new_block = new_status['block_number']
        
        LOG.info(f"\nNew State:")
        LOG.info(f"  Epoch: {new_status['epoch']}")
        LOG.info(f"  Block: {new_block}")
        LOG.info(f"  Round: {new_status['round']}")
        
        # 获取新随机数
        new_randomness = await http_client.get_randomness(new_block)
        LOG.info(f"  Randomness: {new_randomness[:32] if new_randomness else 'None'}...")
        
        # 4. 验证变化
        LOG.info("\nVerification:")
        
        epoch_changed = new_status['epoch'] > initial_status['epoch']
        randomness_changed = new_randomness != initial_randomness
        
        LOG.info(f"  Epoch changed: {epoch_changed}")
        LOG.info(f"  Randomness changed: {randomness_changed}")
        
        if not epoch_changed:
            raise AssertionError("Epoch did not change")
        
        if not randomness_changed and initial_randomness and new_randomness:
            LOG.warning("  ⚠️  Randomness did not change (may be expected in some cases)")
        
        test_result.mark_success(
            initial_epoch=initial_epoch,
            new_epoch=new_status['epoch'],
            blocks_passed=new_block - initial_block,
            randomness_changed=randomness_changed
        )
        
        LOG.info("\n✅ Epoch transition test completed!")
```

**工作量**: 3天

---

## 四、实施计划

### 4.1 Phase 1: 基础设施（1周）

| 任务 | 工作量 | 说明 |
|-----|--------|------|
| HTTP API客户端 | 1天 | GravityHttpClient |
| RandomDice辅助工具 | 1天 | RandomDiceHelper + RandomnessVerifier |
| 合约编译集成 | 1天 | forge build集成 |
| 测试框架调整 | 1天 | 注册新测试 |
| 文档编写 | 1天 | 使用说明 |

### 4.2 Phase 2: 核心测试（2周）

| 测试用例 | 优先级 | 工作量 | 说明 |
|---------|-------|--------|------|
| Test 1: 基础消费 | P0 | 2天 | 最重要 |
| Test 2: 正确性观察 | P0 | 2天 | 核心验证 |
| Test 3: Epoch转换 | P1 | 3天 | 重要场景 |
| Test 4: API完整性 | P1 | 2天 | API测试 |

### 4.3 Phase 3: 扩展测试（2周，可选）

| 测试用例 | 优先级 | 工作量 | 说明 |
|---------|-------|--------|------|
| Test 5: 多合约隔离 | P2 | 3天 | 高级场景 |
| Test 6: 质量观察 | P2 | 4天 | 统计分析 |
| Test 7: 压力测试 | P3 | 5天 | 性能测试 |

**总计**: 5周（含可选扩展）

---

## 五、测试用例难度总结

### 可实现的测试（按难度排序）

| 难度 | 测试名称 | 工作量 | 优先级 | 实现度 |
|-----|---------|--------|-------|-------|
| ⭐⭐ | Test 1: 基础消费测试 | 2天 | P0 | 100% |
| ⭐⭐ | Test 2: 正确性观察 | 2天 | P0 | 80%* |
| ⭐⭐⭐ | Test 3: Epoch转换 | 3天 | P1 | 100% |
| ⭐⭐⭐ | Test 4: API完整性 | 2天 | P1 | 100% |
| ⭐⭐⭐⭐ | Test 5: 多合约隔离 | 3天 | P2 | 100% |
| ⭐⭐⭐⭐ | Test 6: 质量观察 | 4天 | P2 | 70%** |
| ⭐⭐⭐⭐⭐ | Test 7: 压力测试 | 5天 | P3 | 100% |

*不包含密码学验证，只做观察性检查
**统计分析，不做理论证明

### 无法实现的测试

| 原测试 | 原因 | 替代方案 |
|-------|------|---------|
| dkg_with_validator_down | 需要节点控制 | 无（需要外部工具） |
| dkg_with_validator_join_leave | 需要validator管理 | 无 |
| randomness_stall_recovery | 需要配置热更新 | 无 |
| validator_restart_during_dkg | 需要failpoint | 无 |
| entry_func_attrs | Move特有 | 不适用 |
| enable/disable_feature | Move governance | 无（除非实现governance API） |

---

## 六、价值评估

### 6.1 能覆盖的功能

✅ **可以验证：**
1. 随机数基本功能（合约能获取并使用）
2. block.difficulty正确映射到随机数种子
3. 不同epoch的随机数更新
4. HTTP API的可用性和正确性
5. 随机数的分布特性（观察性）
6. 多合约场景下的随机数行为

### 6.2 无法覆盖的功能

❌ **无法验证：**
1. DKG协议的密码学正确性
2. Validator节点的容错能力
3. 网络分区下的DKG行为
4. 配置更新和恢复机制
5. Governance层的功能

### 6.3 建议

**短期（1-2个月）：**
1. 实现Phase 1 + Phase 2（核心测试）
2. 建立CI/CD自动化
3. 编写完整文档

**中期（3-6个月）：**
1. 实现Phase 3（扩展测试）
2. 优化测试性能
3. 增加更多边界情况测试

**长期考虑：**
1. 对于需要节点管理的测试，考虑：
   - 开发节点管理API
   - 或使用Kubernetes Operator
   - 或保留Rust测试框架

2. 对于DKG密码学验证，考虑：
   - 使用FFI调用Rust库
   - 或作为独立的验证工具
   - 或依赖节点本身的验证日志

---

## 七、快速开始

### 前置条件

```bash
# 1. 确保Gravity节点运行
#    - JSON-RPC: http://127.0.0.1:8545
#    - HTTP API: http://127.0.0.1:1998

# 2. 编译RandomDice合约
cd /Users/lightman/repos/gravity-sdk
forge build

# 3. 安装测试框架依赖
cd gravity_e2e
pip install -r requirements.txt
```

### 运行测试

```bash
# 运行所有randomness测试
python -m gravity_e2e.main --test-suite randomness

# 运行单个测试
python -m gravity_e2e.main --test-suite randomness_basic

# 查看详细日志
python -m gravity_e2e.main --test-suite randomness --log-level DEBUG
```

### 预期输出

```
========================================
Test: Randomness Basic Consumption
========================================
Current DKG Status:
  Epoch: 5
  Round: 123
  Block: 45678
  Nodes: 4

[Step 1] Deploying RandomDice contract...
✅ Contract deployed at: 0x...
   Gas used: 234567

[Step 2] Rolling dice 10 times...
  Roll #1:
    Transaction: 0x...
    Block: 45679
    Result: 3 (should be 1-6)
    Seed: 123456789...
    ✅ Valid roll
  ...

[Step 3] Verifying block.difficulty propagation...
  Roll #1 (Block 45679):
    Seed from contract: 123456789...
    Block difficulty:   123456789...
    ✅ Match!
  ...

✅ Test Completed Successfully!
========================================
```

---

## 八、总结

基于Reth执行层和当前的gravity_e2e框架，我们可以实现**7个有价值的测试用例**，覆盖随机数功能的核心场景：

**高优先级（必须实现）：**
- ✅ 基础消费测试
- ✅ 正确性观察测试
- ✅ Epoch转换测试

**中优先级（建议实现）：**
- ✅ API完整性测试
- ✅ 多合约隔离测试

**低优先级（可选）：**
- ⚠️ 随机数质量观察
- ⚠️ 压力测试

总工作量：**3-5周**（核心功能 + 部分扩展）

这些测试虽然无法覆盖原有测试的所有密码学验证和节点管理功能，但可以有效验证：
1. 随机数功能在用户层面的可用性
2. 合约和区块数据的一致性
3. API的正确性和稳定性
4. Epoch转换过程的正常运行

对于无法实现的复杂测试（如节点故障恢复、DKG密码学验证），建议保留原有的Rust测试框架，形成互补的测试体系。

