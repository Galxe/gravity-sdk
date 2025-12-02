# Randomness 测试用例迁移方案

## 一、迁移策略概述

基于测试用例的复杂度和新框架的能力，采用**分阶段、分优先级**的迁移策略。

### 1.1 迁移优先级

**Phase 1: 基础能力建设** (1-2 周)
- 实现 Move 模块支持
- 实现 REST API 客户端
- 实现基础验证能力

**Phase 2: 简单用例迁移** (2-3 周)
- e2e_basic_consumption
- enable_feature_0/1/2 (简化版)
- disable_feature_0/1 (简化版)

**Phase 3: 中等用例迁移** (3-4 周)
- e2e_correctness (核心验证逻辑)
- entry_func_attrs

**Phase 4: 复杂用例评估** (待定)
- 评估是否需要保留旧框架
- 或实现节点管理接口

## 二、详细迁移方案

### 2.1 Phase 1: 基础能力建设

#### 2.1.1 Move 模块支持

**目标：** 实现 Move 模块的发布和函数调用

**实现方案：**

1. **添加 REST API 客户端**
   ```python
   # gravity_e2e/gravity_e2e/core/client/aptos_rest_client.py
   class AptosRestClient:
       async def get_account_resource_bcs(self, address, resource_type, version=None)
       async def submit_transaction(self, signed_txn)
       async def wait_for_transaction(self, tx_hash)
       async def get_ledger_information(self)
   ```

2. **实现 Move 模块发布**
   ```python
   # gravity_e2e/gravity_e2e/utils/move_utils.py
   async def publish_move_module(
       client: AptosRestClient,
       account: Dict,
       module_bytecode: bytes,
       module_name: str
   ) -> str:
       # 构建发布交易
       # 提交交易
       # 等待确认
   ```

3. **实现 Move 函数调用**
   ```python
   async def call_move_function(
       client: AptosRestClient,
       account: Dict,
       module_address: str,
       module_name: str,
       function_name: str,
       type_args: List[str],
       args: List[Any]
   ) -> TransactionSummary:
       # 构建函数调用交易
       # 提交交易
       # 返回结果
   ```

**依赖：**
- aptos-sdk-python (如果可用)
- 或自行实现 REST API 封装

#### 2.1.2 REST API 客户端

**目标：** 实现链上资源访问

**实现方案：**

```python
# gravity_e2e/gravity_e2e/core/client/aptos_rest_client.py
class AptosRestClient:
    def __init__(self, base_url: str):
        self.base_url = base_url
        self.session = aiohttp.ClientSession()
    
    async def get_account_resource_bcs(
        self,
        address: str,
        resource_type: str,
        version: Optional[int] = None
    ) -> bytes:
        """获取账户资源的 BCS 编码数据"""
        url = f"{self.base_url}/accounts/{address}/resource/{resource_type}"
        if version:
            url += f"?version={version}"
        # 请求并解析 BCS 数据
    
    async def get_on_chain_resource<T>(
        self,
        address: str,
        resource_type: str,
        version: Optional[int] = None
    ) -> T:
        """获取并反序列化链上资源"""
        bcs_data = await self.get_account_resource_bcs(...)
        return bcs_deserialize(bcs_data, T)
```

**需要支持的资源类型：**
- `0x1::dkg::DKGState`
- `0x1::randomness::PerBlockRandomness`
- `0x1::consensus_config::ConsensusConfig`
- `0x1::randomness_config::RandomnessConfig`

#### 2.1.3 Governance 操作封装

**目标：** 封装常见的 governance 操作

**实现方案：**

```python
# gravity_e2e/gravity_e2e/utils/governance_utils.py
async def enable_randomness(
    client: AptosRestClient,
    account: Dict
) -> TransactionSummary:
    """启用随机数主逻辑"""
    script = script_to_enable_main_logic()
    return await execute_governance_script(client, account, script)

async def disable_randomness(
    client: AptosRestClient,
    account: Dict
) -> TransactionSummary:
    """禁用随机数"""
    script = script_to_disable_main_logic()
    return await execute_governance_script(client, account, script)

async def update_consensus_config(
    client: AptosRestClient,
    account: Dict,
    config: OnChainConsensusConfig
) -> TransactionSummary:
    """更新共识配置"""
    script = script_to_update_consensus_config(config)
    return await execute_governance_script(client, account, script)
```

### 2.2 Phase 2: 简单用例迁移

#### 2.2.1 e2e_basic_consumption

**迁移步骤：**

1. **创建测试文件**
   ```python
   # gravity_e2e/gravity_e2e/tests/test_cases/test_randomness_basic_consumption.py
   ```

2. **实现核心逻辑**
   ```python
   @test_case
   async def test_randomness_basic_consumption(
       run_helper: RunHelper,
       test_result: TestResult
   ):
       # 1. 等待 epoch 2
       await wait_for_epoch(run_helper.client, target_epoch=2)
       
       # 2. 发布 OnChainDice 模块
       rest_client = AptosRestClient(run_helper.client.base_url)
       dice_module_address = await publish_on_chain_dice_module(
           rest_client,
           run_helper.faucet_account
       )
       
       # 3. 执行 10 次 roll
       roll_results = []
       for i in range(10):
           result = await call_move_function(
               rest_client,
               run_helper.faucet_account,
               dice_module_address,
               "dice",
               "roll",
               [],
               []
           )
           roll_results.append(result)
       
       # 4. 读取 roll 历史
       history = await get_account_resource_bcs(
           rest_client,
           dice_module_address,
           f"{dice_module_address}::dice::DiceRollHistory"
       )
       
       test_result.mark_success(
           roll_count=len(roll_results),
           history_length=len(history.rolls)
       )
   ```

3. **需要实现的辅助函数**
   - `wait_for_epoch()`: 等待指定 epoch
   - `publish_on_chain_dice_module()`: 发布 Dice 模块
   - `call_move_function()`: 调用 Move 函数

**难点：**
- Move 模块的编译和发布
- Move 函数调用的参数编码
- 资源数据的 BCS 反序列化

**预计工作量：** 3-5 天

#### 2.2.2 enable_feature_0/1/2

**迁移策略：** 简化版本，只测试核心功能

**实现方案：**

```python
@test_case
async def test_enable_randomness_feature(
    run_helper: RunHelper,
    test_result: TestResult
):
    rest_client = AptosRestClient(run_helper.client.base_url)
    
    # 1. 等待初始 epoch
    await wait_for_epoch(rest_client, target_epoch=3)
    
    # 2. 启用随机数主逻辑
    await enable_randomness(rest_client, run_helper.faucet_account)
    
    # 3. 等待下一个 epoch
    await wait_for_epoch(rest_client, target_epoch=4)
    
    # 4. 启用 validator transactions
    config = await get_current_consensus_config(rest_client)
    config.enable_validator_txns()
    await update_consensus_config(rest_client, run_helper.faucet_account, config)
    
    # 5. 等待 DKG 完成
    await wait_for_epoch(rest_client, target_epoch=6)
    
    # 6. 验证 DKG 结果存在
    dkg_state = await get_on_chain_resource<DKGState>(
        rest_client,
        CORE_CODE_ADDRESS,
        "0x1::dkg::DKGState"
    )
    assert dkg_state.last_completed is not None
    
    test_result.mark_success(
        final_epoch=6,
        dkg_completed=True
    )
```

**预计工作量：** 2-3 天

### 2.3 Phase 3: 中等用例迁移

#### 2.3.1 e2e_correctness

**这是最核心的测试用例，需要完整的 DKG 验证能力**

**实现方案：**

1. **实现 DKG 验证逻辑**
   ```python
   # gravity_e2e/gravity_e2e/utils/dkg_utils.py
   def verify_dkg_transcript(
       dkg_session: DKGSessionState,
       decrypt_key_map: Dict[AccountAddress, DecryptKey]
   ) -> bool:
       """验证 DKG transcript 的正确性"""
       # 1. 创建公共参数
       pub_params = DefaultDKG::new_public_params(dkg_session.metadata)
       
       # 2. 反序列化 transcript
       transcript = bcs_deserialize(dkg_session.transcript, Transcript)
       
       # 3. 验证 transcript
       DefaultDKG::verify_transcript(pub_params, transcript)
       
       # 4. 从 shares 重构 secret
       dealt_secret_from_shares = reconstruct_from_shares(...)
       
       # 5. 从 inputs 计算 secret
       dealt_secret_from_inputs = compute_from_inputs(...)
       
       # 6. 比较两个 secret
       return dealt_secret_from_shares == dealt_secret_from_inputs
   ```

2. **实现随机数验证**
   ```python
   async def verify_randomness(
       decrypt_key_map: Dict[AccountAddress, DecryptKey],
       rest_client: AptosRestClient,
       version: int
   ) -> bool:
       """验证随机数种子的正确性"""
       # 1. 获取 DKG state 和 PerBlockRandomness
       dkg_state = await get_on_chain_resource_at_version<DKGState>(...)
       randomness = await get_on_chain_resource_at_version<PerBlockRandomness>(...)
       
       # 2. 从 DKG 重构 shared secret
       dealt_secret = reconstruct_dealt_secret(dkg_state, decrypt_key_map)
       
       # 3. 计算期望的随机数种子
       rand_metadata = RandMetadata(epoch=randomness.epoch, round=randomness.round)
       input_bytes = bcs_serialize(rand_metadata)
       output = WVUF::eval(dealt_secret, input_bytes)
       expected_seed = sha3_256(bcs_serialize(output))
       
       # 4. 比较
       return expected_seed == randomness.seed
   ```

3. **实现密钥管理**
   ```python
   # gravity_e2e/gravity_e2e/utils/key_manager.py
   def load_decrypt_keys_from_config(config_path: str) -> Dict[AccountAddress, DecryptKey]:
       """从配置文件加载验证器解密密钥"""
       # 需要从节点配置或 API 获取
   ```

**关键依赖：**
- DKG 库的 Python 绑定（可能需要 FFI）
- BCS 序列化/反序列化库
- 加密算法库（BLS12-381）

**预计工作量：** 1-2 周

#### 2.3.2 entry_func_attrs

**实现方案：**

```python
@test_case
async def test_randomness_entry_func_attrs(
    run_helper: RunHelper,
    test_result: TestResult
):
    rest_client = AptosRestClient(run_helper.client.base_url)
    
    # 1. 发布 OnChainDice 模块
    dice_address = await publish_on_chain_dice_module(...)
    
    # 2. 配置随机数 API
    await configure_randomness_api(
        rest_client,
        run_helper.faucet_account,
        require_deposit=False,
        allow_custom_max_gas=False
    )
    
    # 3. 测试不同函数和参数组合
    test_cases = [
        (RollFunc.NoAttr, 10000, TxnResult.CommittedWithBiasableAbort),
        (RollFunc.AttrOnly, 10000, TxnResult.CommittedWithNoSeedAbort),
        # ...
    ]
    
    for func, max_gas, expected_result in test_cases:
        result = await call_roll_function(
            rest_client,
            dice_address,
            func,
            max_gas
        )
        assert TxnResult.from(result) == expected_result
    
    test_result.mark_success(test_cases_passed=len(test_cases))
```

**预计工作量：** 3-5 天

### 2.4 Phase 4: 复杂用例评估

#### 2.4.1 节点管理接口

**方案 A: 外部 API 控制**

```python
# gravity_e2e/gravity_e2e/core/node_manager.py
class NodeManager:
    """通过外部 API 控制节点"""
    
    async def stop_node(self, node_id: str):
        """停止节点"""
        # 调用节点管理 API
        await self.api_client.post(f"/nodes/{node_id}/stop")
    
    async def start_node(self, node_id: str):
        """启动节点"""
        await self.api_client.post(f"/nodes/{node_id}/start")
    
    async def restart_node(self, node_id: str):
        """重启节点"""
        await self.stop_node(node_id)
        await asyncio.sleep(2)
        await self.start_node(node_id)
    
    async def update_node_config(
        self,
        node_id: str,
        config_updates: Dict
    ):
        """更新节点配置"""
        await self.api_client.post(
            f"/nodes/{node_id}/config",
            json=config_updates
        )
```

**方案 B: SSH/容器控制**

```python
class NodeManager:
    """通过 SSH 或容器 API 控制节点"""
    
    async def stop_node(self, node_id: str):
        """通过 SSH 执行停止命令"""
        await self.ssh_client.execute(
            node_id,
            "systemctl stop gravity-node"
        )
```

**方案 C: 保留旧框架**

- 复杂用例继续使用 Rust 测试框架
- 简单用例使用新框架
- 通过 CI/CD 统一运行

#### 2.4.2 randomness_stall_recovery

**如果实现节点管理接口，迁移方案：**

```python
@test_case
async def test_randomness_stall_recovery(
    run_helper: RunHelper,
    test_result: TestResult
):
    node_manager = NodeManager(run_helper.config)
    rest_client = AptosRestClient(run_helper.client.base_url)
    
    # 1. 等待 epoch 2
    await wait_for_epoch(rest_client, 2)
    
    # 2. 将所有验证器设为 sync_only
    for node_id in node_manager.list_validators():
        await node_manager.enable_sync_only_mode(node_id)
    
    # 3. 验证链已停止
    assert not await check_chain_liveness(rest_client, timeout=20)
    
    # 4. 热修复所有节点
    for node_id in node_manager.list_validators():
        await node_manager.stop_node(node_id)
        await node_manager.update_node_config(node_id, {
            "randomness_override_seq_num": 1,
            "consensus.sync_only": False
        })
        await node_manager.start_node(node_id)
        await asyncio.sleep(5)
    
    # 5. 更新链上配置
    await bump_randomness_config_seqnum(rest_client, run_helper.faucet_account, 2)
    
    # 6. 验证随机数恢复
    await wait_for_epoch(rest_client, target_epoch)
    randomness = await get_on_chain_resource<PerBlockRandomness>(...)
    assert randomness.seed is not None
    
    test_result.mark_success(recovery_successful=True)
```

**预计工作量：** 取决于节点管理接口的实现方式

## 三、实施计划

### 3.1 时间线

| 阶段 | 时间 | 任务 | 负责人 |
|------|------|------|--------|
| Phase 1 | Week 1-2 | 基础能力建设 | TBD |
| Phase 2 | Week 3-5 | 简单用例迁移 | TBD |
| Phase 3 | Week 6-9 | 中等用例迁移 | TBD |
| Phase 4 | Week 10+ | 复杂用例评估 | TBD |

### 3.2 资源需求

1. **开发资源**
   - Python 开发人员：1-2 人
   - Rust/DKG 专家：0.5 人（咨询）
   - 测试人员：1 人

2. **技术资源**
   - DKG 库的 Python 绑定
   - BCS 序列化库
   - Move 编译工具链

3. **基础设施**
   - 测试节点集群
   - CI/CD 环境
   - 文档和代码审查

### 3.3 风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| DKG 验证逻辑复杂 | 高 | 考虑使用 FFI 调用 Rust 库 |
| Move 支持不完整 | 中 | 优先支持核心功能，逐步扩展 |
| 节点管理接口缺失 | 高 | 评估外部工具或保留旧框架 |
| 测试环境不稳定 | 中 | 加强错误处理和重试机制 |

## 四、成功标准

### 4.1 Phase 1 成功标准

- ✅ REST API 客户端可以访问所有需要的链上资源
- ✅ Move 模块可以成功发布
- ✅ Move 函数可以成功调用
- ✅ Governance 脚本可以执行

### 4.2 Phase 2 成功标准

- ✅ e2e_basic_consumption 测试通过
- ✅ enable_feature 测试通过（简化版）
- ✅ 所有测试用例有清晰的文档

### 4.3 Phase 3 成功标准

- ✅ e2e_correctness 测试通过
- ✅ DKG 验证逻辑正确
- ✅ 随机数验证逻辑正确
- ✅ entry_func_attrs 测试通过

### 4.4 Phase 4 成功标准

- ✅ 复杂用例有明确的迁移或保留方案
- ✅ 测试覆盖率达到 80% 以上
- ✅ 所有测试用例在 CI/CD 中稳定运行

## 五、后续工作

1. **持续优化**
   - 提高测试执行速度
   - 优化错误信息
   - 增强日志记录

2. **扩展能力**
   - 支持更多 Move 功能
   - 支持更多节点操作
   - 支持分布式测试

3. **文档完善**
   - 测试用例文档
   - API 文档
   - 迁移指南

