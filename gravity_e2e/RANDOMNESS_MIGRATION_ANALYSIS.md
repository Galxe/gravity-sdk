# Randomness测试迁移综合分析报告

## 一、测试框架深度解读

### 1.1 Gravity E2E测试框架架构分析

#### 核心设计理念
gravity_e2e框架采用**轻量级、异步优先、远程节点**的设计理念，专注于对已部署节点进行端到端测试。

#### 架构分层

```
┌─────────────────────────────────────────────────┐
│           Test Cases Layer                      │
│  (test_basic_transfer, test_contract_deploy)    │
└────────────────┬────────────────────────────────┘
                 │
┌────────────────▼────────────────────────────────┐
│         Test Helpers Layer                      │
│  RunHelper: 测试执行辅助                        │
│  TestResult: 结果收集                           │
│  @test_case: 装饰器自动化                       │
└────────────────┬────────────────────────────────┘
                 │
┌────────────────▼────────────────────────────────┐
│         Client Layer                            │
│  GravityClient: JSON-RPC客户端                  │
│  NodeConnector: 多节点连接管理                  │
└────────────────┬────────────────────────────────┘
                 │
┌────────────────▼────────────────────────────────┐
│         Utils Layer                             │
│  contract_utils: 合约编码/解码                  │
│  contract_caller: 统一合约交互                  │
│  account_manager: 账户管理                      │
└─────────────────────────────────────────────────┘
```

### 1.2 关键组件详解

#### 1.2.1 GravityClient - JSON-RPC客户端

**设计亮点:**
- 完整的异步支持（aiohttp）
- 自动错误处理和超时机制
- 支持上下文管理器（async with）
- 请求ID自增管理

**核心方法:**
```python
# 基础查询
get_chain_id() -> int
get_block_number() -> int
get_balance(address) -> int
get_transaction_count(address) -> int

# 交易操作
send_raw_transaction(raw_tx) -> str
wait_for_transaction_receipt(tx_hash, timeout) -> Dict

# 合约交互
call(to, data, from_, block) -> str
get_code(address, block) -> str
estimate_gas(tx, block) -> int

# 日志和事件
get_logs(from_block, to_block, address, topics) -> List[Dict]
```

**局限性:**
- 仅支持EVM兼容的JSON-RPC接口
- 无法直接访问Move链上资源
- 无法执行Move脚本和函数

#### 1.2.2 RunHelper - 测试执行助手

**核心职责:**
1. 测试账户生命周期管理
2. 账户自动充值（从faucet）
3. 提供统一的client访问

**关键特性:**
```python
async def create_test_account(name, fund_wei=None) -> Dict:
    # 1. 生成新账户
    # 2. 自动从faucet充值（如果fund_wei>0）
    # 3. 等待交易确认
    # 4. 返回账户信息（address, private_key, account）
```

**充值机制:**
- 自动获取nonce和gas price
- 构建转账交易
- 签名并发送
- 等待确认（支持多确认）

#### 1.2.3 ContractUtils & ContractCaller

**ContractUtils - 合约工具类:**

功能分类:
1. **函数编码**: 将函数名和参数编码为calldata
2. **结果解码**: 解析合约返回值
3. **类型支持**: uint256, address, string, bytes, arrays, structs

**关键方法:**
```python
# 函数选择器（硬编码常见函数）
ERC20_SELECTORS = {
    'name': '0x06fdde03',
    'symbol': '0x95d89b41',
    'decimals': '0x313ce567',
    'totalSupply': '0x18160ddd',
    'balanceOf': '0x70a08231',
    'transfer': '0xa9059cbb',
    # ...
}

# 参数编码
encode_uint256(value: int) -> str  # 填充到32字节
encode_address(address: str) -> str  # 填充到32字节
encode_args(func_name, args, abi) -> str  # 组合编码

# 结果解码
decode_uint256(hex_str) -> int
decode_address(hex_str) -> str
decode_result(func_name, result, abi) -> Any
```

**ContractCaller - 统一合约交互:**

提供更高级的抽象:
```python
caller = ContractCaller(client, contract_address, abi)

# 只读调用
result = await caller.call("balanceOf", owner_address)

# 写入交易
tx_hash = await caller.send_transaction(
    "transfer", 
    recipient_address, 
    amount,
    from_account=sender
)

# 发送并等待
receipt = await caller.send_and_wait(
    "transfer",
    recipient_address,
    amount,
    from_account=sender
)
```

**高级功能:**
- 自动gas估算
- 自动nonce管理
- ABI驱动的类型编码/解码
- 支持复杂类型（struct, array）

#### 1.2.4 测试装饰器机制

**@test_case装饰器:**

自动化功能:
1. **时间统计**: 记录start_time和end_time
2. **异常处理**: 自动捕获并记录错误
3. **结果管理**: 自动创建TestResult对象
4. **参数注入**: 自动注入test_result参数

```python
@test_case
async def test_my_feature(run_helper: RunHelper, test_result: TestResult):
    # test_result自动注入
    # 异常自动捕获
    # 时间自动统计
    
    # 测试逻辑
    account = await run_helper.create_test_account("test", fund_wei=10**18)
    
    # 标记成功（可选）
    test_result.mark_success(
        account_address=account["address"],
        custom_metric=42
    )
    # 如果不手动mark_success，装饰器会在无异常时自动标记
```

### 1.3 框架能力矩阵

| 能力类别 | 功能 | 支持程度 | 实现方式 |
|---------|------|---------|---------|
| **节点管理** | 连接节点 | ✅ 完全支持 | NodeConnector |
| | 启动/停止节点 | ❌ 不支持 | - |
| | 重启节点 | ❌ 不支持 | - |
| | 修改配置 | ❌ 不支持 | - |
| | 健康检查 | ✅ 完全支持 | health_check() |
| **EVM交互** | 账户管理 | ✅ 完全支持 | eth-account |
| | 余额查询 | ✅ 完全支持 | get_balance() |
| | 转账交易 | ✅ 完全支持 | send_raw_transaction() |
| | 合约部署 | ✅ 完全支持 | 部署字节码 |
| | 合约调用 | ✅ 完全支持 | call() + 编码工具 |
| | 事件监听 | ✅ 完全支持 | get_logs() |
| **Move交互** | 资源查询 | ❌ 不支持 | 需要REST API |
| | 模块发布 | ❌ 不支持 | 需要实现 |
| | 函数调用 | ❌ 不支持 | 需要实现 |
| | 脚本执行 | ❌ 不支持 | 需要实现 |
| **测试工具** | 测试装饰器 | ✅ 完全支持 | @test_case |
| | 结果收集 | ✅ 完全支持 | TestResult |
| | 并发测试 | ✅ 完全支持 | asyncio |
| | 报告生成 | ✅ 基础支持 | JSON输出 |

---

## 二、原有Randomness测试用例深度分析

### 2.1 测试用例技术拆解

#### 2.1.1 e2e_basic_consumption.rs

**测试目标**: 验证智能合约能够成功消费链上随机数

**关键技术点:**

1. **Move模块编译与发布**
```rust
cli.init_package("OnChainDice".to_string(), ...)
cli.add_file_in_package("sources/dice.move", content);
cli.publish_package(0, None, named_addresses, None)
```

2. **Move函数调用**
```rust
let roll_func_id = MemberId::from_str(&format!("{}::dice::roll", account))?;
cli.run_function(0, Some(gas_options), roll_func_id, vec![], vec![])
```

3. **链上资源读取**
```rust
rest_client.get_account_resource_bcs::<DiceRollHistory>(
    root_address,
    format!("{}::dice::DiceRollHistory", account).as_str()
)
```

**迁移关键点:**
- 需要实现Move模块的编译和发布流程
- 需要实现Move函数调用的交易构建
- 需要实现BCS格式的资源反序列化

**复杂度**: ⭐⭐⭐ (中等)

#### 2.1.2 e2e_correctness.rs

**测试目标**: 验证DKG transcript和随机数种子的密码学正确性

**核心验证流程:**

```rust
// 1. DKG Transcript验证
verify_dkg_transcript(dkg_session, decrypt_key_map) {
    // 1.1 创建公共参数
    let pub_params = DefaultDKG::new_public_params(&metadata);
    
    // 1.2 反序列化transcript
    let transcript = bcs::from_bytes(dkg_session.transcript)?;
    
    // 1.3 验证transcript有效性
    DefaultDKG::verify_transcript(&pub_params, &transcript)?;
    
    // 1.4 从shares重构secret
    let secret_from_shares = dealt_secret_from_shares(...);
    
    // 1.5 从inputs计算secret
    let secret_from_inputs = dealt_secret_from_input(...);
    
    // 1.6 验证两者相等
    assert_eq!(secret_from_shares, secret_from_inputs);
}

// 2. 随机数种子验证
verify_randomness(decrypt_key_map, rest_client, version) {
    // 2.1 获取DKG状态和随机数
    let dkg_state = get_on_chain_resource::<DKGState>(...);
    let randomness = get_on_chain_resource::<PerBlockRandomness>(...);
    
    // 2.2 重构shared secret
    let dealt_secret = dealt_secret_from_shares(...);
    
    // 2.3 使用WVUF评估
    let rand_metadata = RandMetadata { epoch, round };
    let output = WVUF::eval(&dealt_secret, &bcs::to_bytes(&rand_metadata));
    
    // 2.4 计算期望的种子
    let expected_seed = Sha3_256::digest(&bcs::to_bytes(&output));
    
    // 2.5 验证
    assert_eq!(expected_seed, randomness.seed);
}
```

**密钥管理:**
```rust
fn decrypt_key_map(swarm: &LocalSwarm) -> HashMap<AccountAddress, DecryptKey> {
    swarm.validators().map(|validator| {
        let dk = validator.config()
            .consensus.safety_rules.initial_safety_rules_config
            .identity_blob()
            .unwrap()
            .try_into_dkg_new_validator_decrypt_key()
            .unwrap();
        (validator.peer_id(), dk)
    }).collect()
}
```

**迁移关键点:**
- 需要完整的DKG验证库（可能需要FFI调用Rust）
- 需要BLS12-381加密库
- 需要访问验证器的解密密钥
- 需要BCS序列化/反序列化
- 需要实现WVUF评估

**复杂度**: ⭐⭐⭐⭐⭐ (非常困难)

#### 2.1.3 dkg_with_validator_down.rs

**测试目标**: 验证有验证器下线时DKG仍能完成

**测试流程:**
```rust
// 1. 等待DKG完成
let dkg_session_1 = wait_for_dkg_finish(&client, None, time_limit).await;

// 2. 停止一个验证器
swarm.validators_mut().take(1).for_each(|v| {
    v.stop();
});

// 3. 等待下一个epoch的DKG
let dkg_session_2 = wait_for_dkg_finish(&client, Some(epoch+1), time_limit).await;

// 4. 验证DKG transcript正确
verify_dkg_transcript(&dkg_session_2, &decrypt_key_map)
```

**核心依赖:**
- 节点控制: `v.stop()`
- DKG状态监控: `wait_for_dkg_finish()`
- DKG验证: `verify_dkg_transcript()`

**迁移关键点:**
- 需要节点生命周期管理API
- 需要DKG状态查询能力
- 需要DKG验证能力

**复杂度**: ⭐⭐⭐⭐ (困难)

#### 2.1.4 randomness_stall_recovery.rs

**测试目标**: 验证随机数停滞后通过配置热更新恢复

**测试场景:**

1. **制造停滞**
```rust
// 将所有验证器设为sync_only模式
for validator in swarm.validators_mut() {
    enable_sync_only_mode(4, validator).await;
}

// 验证链已停止
assert!(swarm.liveness_check(timeout).await.is_err());
```

2. **热修复**
```rust
for validator in swarm.validators_mut() {
    // 停止节点
    validator.stop();
    
    // 修改配置
    let mut config = OverrideNodeConfig::load_config(path)?;
    config.override_config_mut().randomness_override_seq_num = 1;
    config.override_config_mut().consensus.sync_only = false;
    config.save_config(path)?;
    
    // 重启节点
    validator.start()?;
}
```

3. **链上配置更新**
```rust
let script = r#"
script {
    use aptos_framework::randomness_config_seqnum;
    fun main(core_resources: &signer) {
        randomness_config_seqnum::set_for_next_epoch(&framework_signer, 2);
        aptos_governance::force_end_epoch(&framework_signer);
    }
}
"#;
cli.run_script_with_gas_options(root_idx, script, gas_options).await
```

**迁移关键点:**
- 需要完整的节点控制（停止、配置修改、启动）
- 需要failpoint或sync_only模式控制
- 需要配置文件读写能力
- 需要governance脚本执行

**复杂度**: ⭐⭐⭐⭐⭐ (极度困难)

#### 2.1.5 entry_func_attrs.rs

**测试目标**: 验证不同randomness函数属性的行为

**测试矩阵:**
```rust
// 链配置维度
chain_generates_randomness_seed: bool  // 是否生成随机数种子
chain_requires_deposit_for_randtxn: bool  // 是否需要押金
chain_allows_custom_max_gas_for_randtxn: bool  // 是否允许自定义max_gas

// 函数属性维度
enum RollFunc {
    NoAttr,  // 无randomness属性
    AttrOnly,  // #[randomness]
    AttrWithMaxGasProp,  // #[randomness(max_gas=56789)]
}

// 交易参数维度
max_gas: u64  // 10000, 45678, 56789

// 预期结果
enum TxnResult {
    CommittedSucceeded,
    CommittedWithBiasableAbort,  // 可偏置错误
    CommittedWithNoSeedAbort,  // 无种子错误
    DiscardedWithMaxGasCheck,  // gas检查失败
}
```

**配置API脚本:**
```rust
let script = format!(r#"
script {{
    use aptos_framework::randomness_api_v0_config;
    fun main(core_resources: &signer) {{
        let framework_signer = aptos_governance::get_signer_testnet_only(...);
        let required_gas = if ({}) {{ option::some(10000) }} else {{ option::none() }};
        randomness_api_v0_config::set_for_next_epoch(&framework_signer, required_gas);
        randomness_api_v0_config::set_allow_max_gas_flag_for_next_epoch(...);
        aptos_governance::reconfigure(&framework_signer);
    }}
}}
"#, chain_requires_deposit, chain_allows_custom_max_gas);
```

**迁移关键点:**
- 需要Move模块发布（OnChainDice）
- 需要governance脚本执行
- 需要解析不同类型的交易错误
- 需要精确控制gas参数

**复杂度**: ⭐⭐⭐⭐ (困难)

### 2.2 测试用例依赖关系图

```
                    ┌─────────────────────────┐
                    │   SwarmBuilder (Rust)   │
                    │   - 启动本地节点集群    │
                    │   - 配置genesis         │
                    └────────────┬────────────┘
                                 │
                ┌────────────────┴────────────────┐
                │                                  │
        ┌───────▼────────┐              ┌────────▼────────┐
        │  Node Control  │              │  REST Client    │
        │  - stop()      │              │  - 资源查询     │
        │  - start()     │              │  - 交易提交     │
        │  - config()    │              │                 │
        └────────┬───────┘              └────────┬────────┘
                 │                               │
                 │         ┌─────────────────────┴──────┐
                 │         │                            │
        ┌────────▼─────────▼───┐           ┌───────────▼──────────┐
        │  DKG & Randomness    │           │   Move Support       │
        │  - verify_dkg()      │           │   - publish_module() │
        │  - verify_random()   │           │   - call_function()  │
        │  - decrypt_keys      │           │   - run_script()     │
        └──────────────────────┘           └──────────────────────┘
```

### 2.3 测试用例分类矩阵

| 测试用例 | 节点控制 | DKG验证 | Move支持 | Governance | 难度 | 优先级 |
|---------|---------|---------|---------|-----------|------|-------|
| e2e_basic_consumption | ❌ | ❌ | ✅ | ❌ | ⭐⭐⭐ | P0 |
| e2e_correctness | ❌ | ✅ | ✅ | ❌ | ⭐⭐⭐⭐⭐ | P1 |
| enable_feature_0 | ❌ | ✅ | ✅ | ✅ | ⭐⭐⭐⭐ | P2 |
| enable_feature_1 | ❌ | ✅ | ✅ | ✅ | ⭐⭐⭐⭐ | P2 |
| enable_feature_2 | ❌ | ✅ | ✅ | ✅ | ⭐⭐⭐⭐ | P2 |
| disable_feature_0 | ❌ | ✅ | ✅ | ✅ | ⭐⭐⭐⭐ | P2 |
| disable_feature_1 | ❌ | ✅ | ✅ | ✅ | ⭐⭐⭐⭐ | P2 |
| entry_func_attrs | ❌ | ❌ | ✅ | ✅ | ⭐⭐⭐⭐ | P1 |
| dkg_with_validator_down | ✅ | ✅ | ✅ | ❌ | ⭐⭐⭐⭐⭐ | P3 |
| dkg_with_validator_join_leave | ✅ | ✅ | ✅ | ✅ | ⭐⭐⭐⭐⭐ | P3 |
| randomness_stall_recovery | ✅ | ✅ | ✅ | ✅ | ⭐⭐⭐⭐⭐ | P3 |
| validator_restart_during_dkg | ✅ | ✅ | ✅ | ❌ | ⭐⭐⭐⭐⭐ | P3 |

---

## 三、迁移方案设计

### 3.1 总体策略

采用**分层递进、能力优先**的迁移策略：

```
Phase 0: 基础设施 (2周)
  ├── REST API客户端
  ├── Move模块支持
  └── BCS序列化工具

Phase 1: 简单用例 (3周)
  ├── e2e_basic_consumption
  └── 基础验证测试

Phase 2: 中等用例 (4周)
  ├── entry_func_attrs
  ├── enable_feature (简化版)
  └── Governance支持

Phase 3: 核心验证 (6周)
  ├── e2e_correctness
  ├── DKG验证库集成
  └── 密钥管理

Phase 4: 复杂场景评估 (待定)
  ├── 节点管理方案
  └── 混合框架方案
```

### 3.2 Phase 0: 基础设施建设

#### 3.2.1 REST API客户端

**目标**: 支持Move链上资源访问

**实现方案:**

```python
# gravity_e2e/gravity_e2e/core/client/aptos_rest_client.py

class AptosRestClient:
    """Aptos REST API 客户端，用于访问Move链上资源"""
    
    def __init__(self, base_url: str, timeout: float = 30.0):
        self.base_url = base_url
        self.session = aiohttp.ClientSession(
            timeout=aiohttp.ClientTimeout(total=timeout)
        )
    
    async def get_ledger_information(self) -> Dict:
        """获取账本信息（版本、epoch等）"""
        async with self.session.get(f"{self.base_url}/") as resp:
            return await resp.json()
    
    async def get_account_resource_bcs(
        self,
        address: str,
        resource_type: str,
        version: Optional[int] = None
    ) -> bytes:
        """获取账户资源的BCS编码数据"""
        url = f"{self.base_url}/accounts/{address}/resource/{resource_type}"
        if version:
            url += f"?ledger_version={version}"
        
        headers = {"Accept": "application/x-bcs"}
        async with self.session.get(url, headers=headers) as resp:
            if resp.status != 200:
                raise APIError(f"Failed to get resource: {await resp.text()}")
            return await resp.read()
    
    async def get_account_resource_json(
        self,
        address: str,
        resource_type: str,
        version: Optional[int] = None
    ) -> Dict:
        """获取账户资源的JSON格式"""
        url = f"{self.base_url}/accounts/{address}/resource/{resource_type}"
        if version:
            url += f"?ledger_version={version}"
        
        async with self.session.get(url) as resp:
            if resp.status == 404:
                return None
            if resp.status != 200:
                raise APIError(f"Failed to get resource: {await resp.text()}")
            return await resp.json()
    
    async def submit_transaction(self, signed_txn: Dict) -> str:
        """提交已签名的交易"""
        async with self.session.post(
            f"{self.base_url}/transactions",
            json=signed_txn
        ) as resp:
            if resp.status not in [200, 202]:
                raise APIError(f"Transaction submission failed: {await resp.text()}")
            result = await resp.json()
            return result["hash"]
    
    async def wait_for_transaction(
        self,
        tx_hash: str,
        timeout: int = 60
    ) -> Dict:
        """等待交易确认"""
        start = time.time()
        while time.time() - start < timeout:
            try:
                async with self.session.get(
                    f"{self.base_url}/transactions/by_hash/{tx_hash}"
                ) as resp:
                    if resp.status == 200:
                        return await resp.json()
            except:
                pass
            await asyncio.sleep(1)
        raise TimeoutError(f"Transaction {tx_hash} not confirmed in {timeout}s")
```

**工作量**: 3天

#### 3.2.2 Move模块支持

**目标**: 实现Move模块的编译、发布和调用

**实现方案:**

```python
# gravity_e2e/gravity_e2e/utils/move_utils.py

class MoveModuleManager:
    """Move模块管理器"""
    
    def __init__(self, rest_client: AptosRestClient):
        self.client = rest_client
    
    async def compile_module(
        self,
        source_path: str,
        named_addresses: Dict[str, str]
    ) -> bytes:
        """
        编译Move模块
        
        选项1: 调用aptos CLI
        选项2: 使用move-cli库
        选项3: 调用远程编译服务
        """
        # 使用aptos CLI编译
        import subprocess
        
        cmd = [
            "aptos", "move", "compile",
            "--save-metadata",
            "--package-dir", source_path,
        ]
        
        for name, addr in named_addresses.items():
            cmd.extend(["--named-addresses", f"{name}={addr}"])
        
        result = subprocess.run(cmd, capture_output=True)
        if result.returncode != 0:
            raise CompilationError(result.stderr.decode())
        
        # 读取编译后的字节码
        return self._read_compiled_bytecode(source_path)
    
    async def publish_module(
        self,
        account: Dict,
        module_bytecode: bytes,
        max_gas: int = 2000000
    ) -> str:
        """
        发布Move模块
        
        构建Module Publish交易并提交
        """
        # 1. 获取序列号
        account_info = await self.client.get_account(account["address"])
        sequence_number = int(account_info["sequence_number"])
        
        # 2. 构建交易载荷
        payload = {
            "type": "module_bundle_payload",
            "modules": [
                {"bytecode": f"0x{module_bytecode.hex()}"}
            ]
        }
        
        # 3. 构建交易
        txn = {
            "sender": account["address"],
            "sequence_number": str(sequence_number),
            "max_gas_amount": str(max_gas),
            "gas_unit_price": "100",
            "expiration_timestamp_secs": str(int(time.time()) + 600),
            "payload": payload
        }
        
        # 4. 签名
        signed_txn = await self._sign_transaction(txn, account["private_key"])
        
        # 5. 提交
        tx_hash = await self.client.submit_transaction(signed_txn)
        
        # 6. 等待确认
        await self.client.wait_for_transaction(tx_hash)
        
        return tx_hash
    
    async def call_function(
        self,
        account: Dict,
        module_address: str,
        module_name: str,
        function_name: str,
        type_args: List[str],
        args: List[Any],
        max_gas: int = 10000
    ) -> Dict:
        """
        调用Move函数
        
        构建Entry Function交易并提交
        """
        # 1. 获取序列号
        account_info = await self.client.get_account(account["address"])
        sequence_number = int(account_info["sequence_number"])
        
        # 2. 构建函数ID
        function_id = f"{module_address}::{module_name}::{function_name}"
        
        # 3. 编码参数（需要BCS编码）
        encoded_args = [self._encode_argument(arg) for arg in args]
        
        # 4. 构建交易载荷
        payload = {
            "type": "entry_function_payload",
            "function": function_id,
            "type_arguments": type_args,
            "arguments": encoded_args
        }
        
        # 5. 构建交易
        txn = {
            "sender": account["address"],
            "sequence_number": str(sequence_number),
            "max_gas_amount": str(max_gas),
            "gas_unit_price": "100",
            "expiration_timestamp_secs": str(int(time.time()) + 600),
            "payload": payload
        }
        
        # 6. 签名
        signed_txn = await self._sign_transaction(txn, account["private_key"])
        
        # 7. 提交
        tx_hash = await self.client.submit_transaction(signed_txn)
        
        # 8. 等待并返回结果
        return await self.client.wait_for_transaction(tx_hash)
    
    def _encode_argument(self, arg: Any) -> str:
        """参数编码（简化版，完整版需要BCS库）"""
        if isinstance(arg, int):
            return str(arg)
        elif isinstance(arg, str):
            if arg.startswith("0x"):
                return arg
            return f'"{arg}"'
        elif isinstance(arg, bool):
            return "true" if arg else "false"
        elif isinstance(arg, list):
            return json.dumps(arg)
        else:
            raise ValueError(f"Unsupported argument type: {type(arg)}")
```

**依赖:**
- aptos CLI (用于编译)
- 或者: move-cli Python binding
- BCS序列化库 (python-bcs或自实现)

**工作量**: 5天

#### 3.2.3 BCS序列化工具

**目标**: 实现BCS序列化/反序列化

**实现方案:**

```python
# gravity_e2e/gravity_e2e/utils/bcs_utils.py

import struct
from typing import Any, List, Dict, Type

class BCSSerializer:
    """BCS序列化器"""
    
    @staticmethod
    def serialize_u64(value: int) -> bytes:
        """序列化u64（小端序）"""
        return struct.pack('<Q', value)
    
    @staticmethod
    def serialize_u8(value: int) -> bytes:
        return struct.pack('<B', value)
    
    @staticmethod
    def serialize_bool(value: bool) -> bytes:
        return b'\x01' if value else b'\x00'
    
    @staticmethod
    def serialize_bytes(value: bytes) -> bytes:
        """序列化bytes（长度+内容）"""
        length = BCSSerializer.serialize_uleb128(len(value))
        return length + value
    
    @staticmethod
    def serialize_string(value: str) -> bytes:
        """序列化string"""
        return BCSSerializer.serialize_bytes(value.encode('utf-8'))
    
    @staticmethod
    def serialize_vector(values: List[Any], item_serializer) -> bytes:
        """序列化vector"""
        result = BCSSerializer.serialize_uleb128(len(values))
        for value in values:
            result += item_serializer(value)
        return result
    
    @staticmethod
    def serialize_uleb128(value: int) -> bytes:
        """ULEB128编码"""
        result = bytearray()
        while value >= 0x80:
            result.append((value & 0x7f) | 0x80)
            value >>= 7
        result.append(value & 0x7f)
        return bytes(result)

class BCSDeserializer:
    """BCS反序列化器"""
    
    def __init__(self, data: bytes):
        self.data = data
        self.offset = 0
    
    def read_u64(self) -> int:
        value = struct.unpack_from('<Q', self.data, self.offset)[0]
        self.offset += 8
        return value
    
    def read_u8(self) -> int:
        value = self.data[self.offset]
        self.offset += 1
        return value
    
    def read_bool(self) -> bool:
        return self.read_u8() != 0
    
    def read_bytes(self) -> bytes:
        length = self.read_uleb128()
        value = self.data[self.offset:self.offset + length]
        self.offset += length
        return value
    
    def read_string(self) -> str:
        return self.read_bytes().decode('utf-8')
    
    def read_vector(self, item_deserializer) -> List[Any]:
        length = self.read_uleb128()
        return [item_deserializer(self) for _ in range(length)]
    
    def read_uleb128(self) -> int:
        value = 0
        shift = 0
        while True:
            byte = self.read_u8()
            value |= (byte & 0x7f) << shift
            if byte & 0x80 == 0:
                break
            shift += 7
        return value

# 资源类型定义
class DKGState:
    """DKG状态资源"""
    
    @staticmethod
    def deserialize(data: bytes) -> 'DKGState':
        des = BCSDeserializer(data)
        # 根据实际DKGState结构反序列化
        # 这里需要参考Rust定义
        ...

class PerBlockRandomness:
    """每块随机数资源"""
    
    @staticmethod
    def deserialize(data: bytes) -> 'PerBlockRandomness':
        des = BCSDeserializer(data)
        ...
```

**工作量**: 4天

### 3.3 Phase 1: 简单用例迁移

#### 3.3.1 e2e_basic_consumption迁移

**完整实现:**

```python
# gravity_e2e/gravity_e2e/tests/test_cases/test_randomness_basic_consumption.py

import asyncio
import logging
from pathlib import Path
from typing import Dict

from ...helpers.test_helpers import RunHelper, TestResult, test_case
from ...core.client.aptos_rest_client import AptosRestClient
from ...utils.move_utils import MoveModuleManager
from ...utils.epoch_utils import wait_for_epoch

LOG = logging.getLogger(__name__)

# OnChainDice模块源码
DICE_MODULE_SOURCE = """
module module_owner::dice {
    use std::option;
    use std::signer;
    use std::vector;
    use aptos_framework::randomness;
    use aptos_framework::timestamp;
    
    struct DiceRollHistory has key {
        rolls: vector<u64>,
    }
    
    #[randomness]
    entry fun roll(account: &signer) acquires DiceRollHistory {
        let addr = signer::address_of(account);
        
        if (!exists<DiceRollHistory>(addr)) {
            move_to(account, DiceRollHistory {
                rolls: vector::empty<u64>(),
            });
        };
        
        // 生成1-6的随机数
        let random_value = randomness::u64_range(1, 7);
        
        let history = borrow_global_mut<DiceRollHistory>(addr);
        vector::push_back(&mut history.rolls, random_value);
    }
    
    #[randomness]
    entry fun roll_v0(account: &signer) acquires DiceRollHistory {
        // 没有randomness属性的版本（用于测试）
        roll(account)
    }
    
    #[randomness(max_gas=56789)]
    entry fun roll_v2(account: &signer) acquires DiceRollHistory {
        // 带max_gas属性的版本
        roll(account)
    }
    
    #[view]
    public fun get_rolls(addr: address): vector<u64> acquires DiceRollHistory {
        if (!exists<DiceRollHistory>(addr)) {
            vector::empty<u64>()
        } else {
            *&borrow_global<DiceRollHistory>(addr).rolls
        }
    }
}
"""

@test_case
async def test_randomness_basic_consumption(
    run_helper: RunHelper,
    test_result: TestResult
):
    """测试基本的随机数消费功能"""
    
    LOG.info("Starting randomness basic consumption test")
    
    # 1. 初始化客户端
    # 假设Gravity节点同时提供了REST API（端口8080）和JSON-RPC（端口8545）
    rest_url = run_helper.client.rpc_url.replace(":8545", ":8080")
    rest_client = AptosRestClient(rest_url)
    move_manager = MoveModuleManager(rest_client)
    
    # 2. 等待epoch 2（epoch 1没有随机数）
    LOG.info("Waiting for epoch 2...")
    current_epoch = await wait_for_epoch(rest_client, target_epoch=2, timeout=60)
    LOG.info(f"Current epoch: {current_epoch}")
    
    # 3. 准备发布者账户
    publisher = run_helper.faucet_account
    publisher_address = publisher["address"]
    
    LOG.info(f"Publisher address: {publisher_address}")
    
    # 4. 准备Move模块
    # 将源码写入临时目录
    temp_dir = Path("/tmp/gravity_e2e_dice")
    temp_dir.mkdir(exist_ok=True)
    
    sources_dir = temp_dir / "sources"
    sources_dir.mkdir(exist_ok=True)
    
    dice_source_path = sources_dir / "dice.move"
    dice_source_path.write_text(DICE_MODULE_SOURCE)
    
    # 创建Move.toml
    move_toml = f"""
[package]
name = "OnChainDice"
version = "1.0.0"

[addresses]
module_owner = "{publisher_address}"

[dependencies.AptosFramework]
git = "https://github.com/aptos-labs/aptos-core.git"
rev = "main"
subdir = "aptos-move/framework/aptos-framework"
"""
    (temp_dir / "Move.toml").write_text(move_toml)
    
    # 5. 编译模块
    LOG.info("Compiling OnChainDice module...")
    module_bytecode = await move_manager.compile_module(
        str(temp_dir),
        {"module_owner": publisher_address}
    )
    
    # 6. 发布模块
    LOG.info("Publishing OnChainDice module...")
    publish_tx_hash = await move_manager.publish_module(
        publisher,
        module_bytecode,
        max_gas=2000000
    )
    LOG.info(f"Module published, tx_hash: {publish_tx_hash}")
    
    # 7. 执行10次roll
    LOG.info("Rolling the dice 10 times...")
    roll_results = []
    
    for i in range(10):
        try:
            tx_result = await move_manager.call_function(
                publisher,
                publisher_address,
                "dice",
                "roll",
                [],  # type_args
                [],  # args
                max_gas=10000
            )
            roll_results.append(tx_result)
            LOG.info(f"Roll {i+1} completed, tx_hash: {tx_result['hash']}")
        except Exception as e:
            LOG.error(f"Roll {i+1} failed: {e}")
            raise
    
    # 8. 读取roll历史
    LOG.info("Fetching roll history...")
    resource_type = f"{publisher_address}::dice::DiceRollHistory"
    
    history_resource = await rest_client.get_account_resource_json(
        publisher_address,
        resource_type
    )
    
    if history_resource:
        rolls = history_resource["data"]["rolls"]
        LOG.info(f"Roll history: {rolls}")
        
        # 验证
        assert len(rolls) == 10, f"Expected 10 rolls, got {len(rolls)}"
        for roll_value in rolls:
            roll_num = int(roll_value)
            assert 1 <= roll_num <= 6, f"Roll value {roll_num} out of range [1, 6]"
        
        test_result.mark_success(
            roll_count=len(rolls),
            roll_history=rolls,
            publish_tx=publish_tx_hash,
            current_epoch=current_epoch
        )
        
        LOG.info("Randomness basic consumption test completed successfully!")
    else:
        raise RuntimeError("DiceRollHistory resource not found")
```

**辅助工具:**

```python
# gravity_e2e/gravity_e2e/utils/epoch_utils.py

async def wait_for_epoch(
    rest_client: AptosRestClient,
    target_epoch: int,
    timeout: int = 120
) -> int:
    """等待指定的epoch"""
    start = time.time()
    
    while time.time() - start < timeout:
        ledger_info = await rest_client.get_ledger_information()
        current_epoch = int(ledger_info["epoch"])
        
        if current_epoch >= target_epoch:
            return current_epoch
        
        await asyncio.sleep(2)
    
    raise TimeoutError(f"Timeout waiting for epoch {target_epoch}")

async def get_current_epoch(rest_client: AptosRestClient) -> int:
    """获取当前epoch"""
    ledger_info = await rest_client.get_ledger_information()
    return int(ledger_info["epoch"])
```

**注册测试:**

```python
# gravity_e2e/gravity_e2e/main.py

from .tests.test_cases.test_randomness_basic_consumption import (
    test_randomness_basic_consumption
)

async def run_test_module(module_name: str, test_helper: RunHelper, test_results: list):
    # ... 现有代码 ...
    
    elif module_name == "cases.randomness_basic":
        result = await test_randomness_basic_consumption(run_helper=test_helper)
        test_results.append(result)
```

**工作量**: 5天

### 3.4 Phase 2: Governance支持

#### 3.4.1 Governance脚本执行

```python
# gravity_e2e/gravity_e2e/utils/governance_utils.py

class GovernanceManager:
    """Governance操作管理器"""
    
    def __init__(self, rest_client: AptosRestClient):
        self.client = rest_client
    
    async def execute_script(
        self,
        account: Dict,
        script_source: str,
        type_args: List[str] = None,
        args: List[Any] = None,
        max_gas: int = 2000000
    ) -> Dict:
        """
        执行Move脚本
        
        步骤:
        1. 编译脚本到字节码
        2. 构建脚本交易
        3. 签名并提交
        """
        # 1. 编译脚本
        script_bytecode = await self._compile_script(script_source)
        
        # 2. 获取序列号
        account_info = await self.client.get_account(account["address"])
        sequence_number = int(account_info["sequence_number"])
        
        # 3. 构建交易
        payload = {
            "type": "script_payload",
            "code": {
                "bytecode": f"0x{script_bytecode.hex()}"
            },
            "type_arguments": type_args or [],
            "arguments": args or []
        }
        
        txn = {
            "sender": account["address"],
            "sequence_number": str(sequence_number),
            "max_gas_amount": str(max_gas),
            "gas_unit_price": "100",
            "expiration_timestamp_secs": str(int(time.time()) + 600),
            "payload": payload
        }
        
        # 4. 签名
        signed_txn = await self._sign_transaction(txn, account["private_key"])
        
        # 5. 提交
        tx_hash = await self.client.submit_transaction(signed_txn)
        
        # 6. 等待确认
        return await self.client.wait_for_transaction(tx_hash)
    
    async def enable_randomness(
        self,
        core_resources_account: Dict
    ) -> Dict:
        """启用随机数主逻辑"""
        script = """
script {
    use aptos_framework::aptos_governance;
    use aptos_framework::randomness_config;
    use aptos_std::fixed_point64;

    fun main(core_resources: &signer) {
        let framework_signer = aptos_governance::get_signer_testnet_only(core_resources, @0x1);
        let config = randomness_config::new_v1(
            fixed_point64::create_from_rational(1, 2),
            fixed_point64::create_from_rational(2, 3)
        );
        randomness_config::set_for_next_epoch(&framework_signer, config);
        aptos_governance::reconfigure(&framework_signer);
    }
}
"""
        return await self.execute_script(core_resources_account, script)
    
    async def disable_randomness(
        self,
        core_resources_account: Dict
    ) -> Dict:
        """禁用随机数"""
        script = """
script {
    use aptos_framework::aptos_governance;
    use aptos_framework::randomness_config;
    
    fun main(core_resources: &signer) {
        let framework_signer = aptos_governance::get_signer_testnet_only(core_resources, @0x1);
        let config = randomness_config::new_off();
        randomness_config::set_for_next_epoch(&framework_signer, config);
        aptos_governance::reconfigure(&framework_signer);
    }
}
"""
        return await self.execute_script(core_resources_account, script)
    
    async def update_consensus_config(
        self,
        core_resources_account: Dict,
        enable_validator_txns: bool
    ) -> Dict:
        """更新共识配置"""
        # 先获取当前配置
        current_config = await self._get_consensus_config()
        
        # 修改配置
        if enable_validator_txns:
            # 设置标志
            pass
        
        # 序列化配置
        config_bytes = self._serialize_consensus_config(current_config)
        
        script = f"""
script {{
    use aptos_framework::aptos_governance;
    use aptos_framework::consensus_config;

    fun main(core_resources: &signer) {{
        let framework_signer = aptos_governance::get_signer_testnet_only(core_resources, @0x1);
        let config_bytes = vector{list(config_bytes)};
        consensus_config::set_for_next_epoch(&framework_signer, config_bytes);
        aptos_governance::reconfigure(&framework_signer);
    }}
}}
"""
        return await self.execute_script(core_resources_account, script)
```

**工作量**: 4天

### 3.5 Phase 3: 核心验证能力

#### 3.5.1 DKG验证集成

**策略选择:**

由于DKG验证涉及复杂的密码学操作（BLS12-381、PVSS等），有几种实现方案：

**方案A: FFI调用Rust库（推荐）**

```python
# gravity_e2e/gravity_e2e/utils/dkg_ffi.py

import ctypes
from pathlib import Path

# 加载Rust编译的动态库
lib_path = Path(__file__).parent / "libdkg_verify.so"
dkg_lib = ctypes.CDLL(str(lib_path))

# 定义函数签名
dkg_lib.verify_dkg_transcript.argtypes = [
    ctypes.c_void_p,  # transcript bytes
    ctypes.c_size_t,  # transcript length
    ctypes.c_void_p,  # metadata bytes
    ctypes.c_size_t,  # metadata length
]
dkg_lib.verify_dkg_transcript.restype = ctypes.c_bool

def verify_dkg_transcript(
    transcript_bytes: bytes,
    metadata_bytes: bytes
) -> bool:
    """调用Rust库验证DKG transcript"""
    return dkg_lib.verify_dkg_transcript(
        transcript_bytes,
        len(transcript_bytes),
        metadata_bytes,
        len(metadata_bytes)
    )

# 对应的Rust代码（需要单独编译）
"""
// gravity_e2e/dkg_verify/src/lib.rs

use aptos_types::dkg::{DKGSessionState, DKGTrait, DefaultDKG};

#[no_mangle]
pub extern "C" fn verify_dkg_transcript(
    transcript_ptr: *const u8,
    transcript_len: usize,
    metadata_ptr: *const u8,
    metadata_len: usize,
) -> bool {
    let transcript_bytes = unsafe {
        std::slice::from_raw_parts(transcript_ptr, transcript_len)
    };
    
    let metadata_bytes = unsafe {
        std::slice::from_raw_parts(metadata_ptr, metadata_len)
    };
    
    // 反序列化
    let transcript = match bcs::from_bytes(transcript_bytes) {
        Ok(t) => t,
        Err(_) => return false,
    };
    
    let metadata = match bcs::from_bytes(metadata_bytes) {
        Ok(m) => m,
        Err(_) => return false,
    };
    
    // 验证
    let pub_params = DefaultDKG::new_public_params(&metadata);
    DefaultDKG::verify_transcript(&pub_params, &transcript).is_ok()
}
"""
```

**方案B: 纯Python实现（工作量大）**

使用py_ecc库实现BLS12-381操作，重新实现DKG验证逻辑。

**方案C: 简化验证（阶段性方案）**

只验证DKG状态的存在性和基本属性，不进行完整的密码学验证。

```python
# gravity_e2e/gravity_e2e/utils/dkg_utils.py

class DKGVerifier:
    """DKG验证器（简化版）"""
    
    def __init__(self, rest_client: AptosRestClient):
        self.client = rest_client
    
    async def wait_for_dkg_finish(
        self,
        target_epoch: Optional[int] = None,
        timeout: int = 120
    ) -> Dict:
        """等待DKG完成"""
        start = time.time()
        
        while time.time() - start < timeout:
            dkg_state = await self.get_dkg_state()
            
            # 检查是否完成
            if (dkg_state.get("in_progress") is None and
                dkg_state.get("last_completed") is not None):
                
                last_completed = dkg_state["last_completed"]
                dealer_epoch = last_completed["metadata"]["dealer_epoch"]
                
                if target_epoch is None or dealer_epoch + 1 == target_epoch:
                    return last_completed
            
            await asyncio.sleep(2)
        
        raise TimeoutError(f"DKG not finished within {timeout}s")
    
    async def get_dkg_state(self) -> Dict:
        """获取DKG状态"""
        return await self.client.get_account_resource_json(
            "0x1",  # CORE_CODE_ADDRESS
            "0x1::dkg::DKGState"
        )
    
    async def get_randomness(self, version: Optional[int] = None) -> Dict:
        """获取随机数"""
        return await self.client.get_account_resource_json(
            "0x1",
            "0x1::randomness::PerBlockRandomness",
            version=version
        )
    
    async def verify_randomness_exists(self, epoch: int) -> bool:
        """验证随机数存在性（简化版）"""
        randomness = await self.get_randomness()
        
        if randomness is None:
            return False
        
        seed = randomness["data"].get("seed")
        current_epoch = int(randomness["data"]["epoch"])
        
        return seed is not None and current_epoch == epoch
```

**推荐方案**: 先实现方案C（简化验证），满足基本测试需求，后续根据需要补充方案A（FFI）进行完整验证。

**工作量**: 
- 方案C: 3天
- 方案A: 10天（包括Rust库开发和Python绑定）

### 3.6 迁移时间表

| Phase | 任务 | 工作量 | 周期 | 负责人 |
|-------|-----|--------|------|-------|
| Phase 0 | REST API客户端 | 3天 | Week 1 | TBD |
|  | Move模块支持 | 5天 | Week 1-2 | TBD |
|  | BCS工具 | 4天 | Week 2 | TBD |
| Phase 1 | e2e_basic_consumption | 5天 | Week 3 | TBD |
|  | 测试验证 | 3天 | Week 3 | TBD |
| Phase 2 | Governance支持 | 4天 | Week 4 | TBD |
|  | entry_func_attrs | 5天 | Week 4-5 | TBD |
|  | enable_feature (简化) | 4天 | Week 5 | TBD |
| Phase 3 | DKG简化验证 | 3天 | Week 6 | TBD |
|  | e2e_correctness (简化) | 5天 | Week 6-7 | TBD |
|  | DKG完整验证 (可选) | 10天 | Week 7-8 | TBD |
| Phase 4 | 复杂用例评估 | 3天 | Week 9 | TBD |
|  | 方案设计 | 2天 | Week 9 | TBD |

**总计**: 9周（含可选的完整DKG验证）

---

## 四、风险评估与应对

### 4.1 技术风险

| 风险 | 概率 | 影响 | 应对措施 |
|-----|------|------|---------|
| Move模块编译复杂 | 高 | 中 | 使用aptos CLI，或调用远程服务 |
| BCS序列化实现困难 | 中 | 中 | 使用现有库或简化实现 |
| DKG验证逻辑复杂 | 高 | 高 | 采用FFI或简化验证 |
| 节点管理接口缺失 | 高 | 高 | 外部工具或保留旧框架 |
| 测试环境不稳定 | 中 | 中 | 增强重试和错误处理 |

### 4.2 进度风险

| 风险 | 应对措施 |
|-----|---------|
| 依赖库缺失 | 提前调研和准备替代方案 |
| 技术难点阻塞 | 分阶段验证，及时调整方案 |
| 人力不足 | 优先级排序，集中资源 |

### 4.3 质量风险

| 风险 | 应对措施 |
|-----|---------|
| 测试覆盖不足 | 逐步迁移，保证每个用例质量 |
| 验证逻辑错误 | 与旧框架对比验证 |
| 文档缺失 | 边开发边写文档 |

---

## 五、成功标准

### 5.1 Phase 0成功标准

- [x] REST API客户端可以访问Move资源
- [x] Move模块可以成功编译和发布
- [x] Move函数可以成功调用
- [x] BCS序列化/反序列化工作正常

### 5.2 Phase 1成功标准

- [x] e2e_basic_consumption测试通过
- [x] 可以读取DiceRollHistory资源
- [x] 随机数生成正确（1-6范围）
- [x] 测试结果可重复

### 5.3 Phase 2成功标准

- [x] Governance脚本可以执行
- [x] entry_func_attrs测试通过
- [x] enable_feature测试通过（简化版）
- [x] 配置更新生效

### 5.4 Phase 3成功标准

- [x] DKG状态可以查询
- [x] e2e_correctness测试通过（简化验证）
- [x] 随机数存在性验证通过
- [ ] DKG完整验证通过（可选）

---

## 六、后续建议

### 6.1 短期（1-3个月）

1. **优先完成Phase 0-2**
   - 实现基础设施
   - 迁移简单和中等用例
   - 建立CI/CD流程

2. **建立测试最佳实践**
   - 编写测试编写指南
   - 建立代码审查流程
   - 统一日志和错误处理

### 6.2 中期（3-6个月）

1. **完善验证能力**
   - 实现完整DKG验证
   - 优化性能
   - 增强错误信息

2. **扩展工具链**
   - 开发调试工具
   - 改进报告生成
   - 支持更多Move功能

### 6.3 长期（6-12个月）

1. **混合框架方案**
   - 评估保留旧框架的必要性
   - 设计统一的测试接口
   - 支持多种后端

2. **节点管理方案**
   - 设计节点管理API
   - 实现节点控制接口
   - 支持复杂测试场景

---

## 七、总结

### 7.1 框架对比总结

| 维度 | 新框架 (gravity_e2e) | 旧框架 (smoke-test) |
|-----|---------------------|-------------------|
| **部署模式** | 远程节点 | 本地集群 |
| **主要技术** | Python + JSON-RPC | Rust + REST API |
| **节点控制** | 不支持 | 完全支持 |
| **Move支持** | 需扩展 | 原生支持 |
| **EVM支持** | 完全支持 | 不支持 |
| **学习曲线** | 低 | 中 |
| **扩展性** | 高 | 中 |
| **适用场景** | 已部署节点E2E测试 | 本地开发和集成测试 |

### 7.2 迁移优先级总结

**高优先级（必须迁移）:**
- e2e_basic_consumption
- entry_func_attrs
- e2e_correctness（简化版）

**中优先级（逐步迁移）:**
- enable_feature系列
- disable_feature系列

**低优先级（评估后决定）:**
- dkg_with_validator_down
- dkg_with_validator_join_leave
- randomness_stall_recovery
- validator_restart_during_dkg

### 7.3 最终建议

1. **采用渐进式迁移策略**，先实现基础能力，再逐步扩展
2. **对于需要节点控制的复杂用例**，建议保留旧框架或开发外部管理工具
3. **重点关注DKG验证能力**，这是核心价值所在
4. **建立完善的文档和示例**，降低后续维护成本
5. **持续优化测试框架**，使其成为可复用的基础设施

