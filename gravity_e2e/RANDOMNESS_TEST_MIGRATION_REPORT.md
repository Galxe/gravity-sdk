# Randomness 测试用例迁移解读报告

## 一、测试框架对比分析

### 1.1 新测试框架 (gravity_e2e)

**技术栈：**
- 语言：Python 3.8+
- 异步框架：asyncio + aiohttp
- 区块链交互：web3.py + eth-account
- 通信协议：JSON-RPC (EVM 兼容 API)

**核心组件：**

1. **NodeConnector** (`core/node_connector.py`)
   - 管理多个节点的连接
   - 支持集群配置
   - 健康检查功能
   - 并发连接管理

2. **GravityClient** (`core/client/gravity_client.py`)
   - EVM JSON-RPC 客户端封装
   - 支持标准 eth_* 方法
   - 异步请求处理
   - 错误处理和重试机制

3. **RunHelper** (`helpers/test_helpers.py`)
   - 测试执行辅助类
   - 账户创建和资金管理
   - 交易构建和发送
   - 测试结果收集

4. **TestResult** (`helpers/test_helpers.py`)
   - 测试结果数据结构
   - 成功/失败标记
   - 详细信息记录
   - 执行时间统计

**测试装饰器：**
```python
@test_case
async def test_function(run_helper: RunHelper, test_result: TestResult):
    # 自动处理异常、时间统计、结果记录
```

**优势：**
- ✅ 轻量级，易于部署
- ✅ 支持已部署节点的测试
- ✅ Python 生态丰富，易于扩展
- ✅ 异步并发，性能好
- ✅ JSON-RPC 标准接口，兼容性好

**限制：**
- ❌ 无法本地启动节点集群
- ❌ 无法直接访问链上 Move 资源
- ❌ 无法进行节点级别的操作（重启、配置修改）
- ❌ 需要节点已部署并运行

### 1.2 旧测试框架 (smoke-test)

**技术栈：**
- 语言：Rust
- 测试框架：tokio::test
- 区块链交互：aptos_forge + aptos_rest_client
- 通信协议：REST API

**核心组件：**

1. **SwarmBuilder** (`smoke_test_environment`)
   - 本地节点集群构建器
   - 支持自定义 genesis 配置
   - 支持节点数量配置
   - 支持 CLI 工具集成

2. **LocalSwarm** (aptos_forge)
   - 本地节点集群管理
   - 节点生命周期管理（启动/停止/重启）
   - 节点配置修改
   - 健康检查

3. **REST Client** (aptos_rest_client)
   - 链上资源查询
   - 交易提交
   - 账户管理
   - 版本查询

**测试模式：**
```rust
#[tokio::test]
async fn test_function() {
    let swarm = SwarmBuilder::new_local(4).build().await;
    // 直接操作节点和链上资源
}
```

**优势：**
- ✅ 完整的节点控制能力
- ✅ 可以直接访问 Move 资源
- ✅ 支持节点故障注入
- ✅ 支持配置热更新
- ✅ 完整的 DKG 和随机数验证能力

**限制：**
- ❌ 需要编译 Rust 代码
- ❌ 依赖完整的 Aptos 框架
- ❌ 测试环境搭建复杂
- ❌ 只能测试本地集群

## 二、测试用例分析

### 2.1 测试用例分类

#### 简单级别 (Simple)

**e2e_basic_consumption.rs**
- **功能**：测试基本的随机数消费
- **步骤**：
  1. 等待 epoch 2
  2. 发布 OnChainDice 模块
  3. 执行 10 次 roll 操作
  4. 检查 roll 历史
- **复杂度**：低
- **依赖**：需要随机数已启用，Move 模块发布
- **迁移难度**：⭐⭐ (中等)

**enable_feature_0/1/2.rs**
- **功能**：测试逐步启用随机数功能
- **步骤**：
  1. 初始状态：随机数关闭
  2. Epoch N: 启用随机数主逻辑
  3. Epoch N+1: 启用 validator transactions
  4. 验证 DKG 结果
- **复杂度**：低-中
- **依赖**：需要 governance 脚本执行能力
- **迁移难度**：⭐⭐⭐ (较难，需要 governance 支持)

**disable_feature_0/1.rs**
- **功能**：测试禁用随机数功能
- **步骤**：类似 enable_feature，但方向相反
- **复杂度**：低-中
- **迁移难度**：⭐⭐⭐ (较难，需要 governance 支持)

#### 中等级别 (Medium)

**e2e_correctness.rs**
- **功能**：验证 DKG transcript 和随机数种子的正确性
- **步骤**：
  1. 等待 epoch 2
  2. 验证 DKG transcript
  3. 验证 10 个版本的随机数
  4. 等待 epoch 3，重复验证
- **复杂度**：中
- **依赖**：
  - 需要访问 DKGState 资源
  - 需要访问 PerBlockRandomness 资源
  - 需要 decrypt_key_map（验证器密钥）
  - 需要 DKG 验证逻辑
- **迁移难度**：⭐⭐⭐⭐ (困难，需要大量底层支持)

**dkg_with_validator_down.rs**
- **功能**：测试验证器下线时的 DKG
- **步骤**：
  1. 等待 DKG 完成
  2. 停止一个验证器
  3. 等待下一个 epoch 的 DKG 完成
  4. 验证 DKG transcript
- **复杂度**：中
- **依赖**：需要节点控制能力
- **迁移难度**：⭐⭐⭐⭐ (困难，需要节点管理)

**dkg_with_validator_join_leave.rs**
- **功能**：测试验证器加入/离开时的 DKG
- **复杂度**：中-高
- **迁移难度**：⭐⭐⭐⭐ (困难，需要节点管理)

#### 复杂级别 (Complex)

**randomness_stall_recovery.rs**
- **功能**：测试随机数停滞后的恢复
- **步骤**：
  1. 启用随机数
  2. 将所有验证器设为 sync_only 模式（停止出块）
  3. 验证链已停止
  4. 热修复所有节点（修改配置）
  5. 重启节点
  6. 更新链上配置序列号
  7. 验证随机数恢复
- **复杂度**：高
- **依赖**：
  - 节点配置修改能力
  - 节点重启能力
  - sync_only 模式控制
  - 配置热更新
- **迁移难度**：⭐⭐⭐⭐⭐ (非常困难，需要完整的节点控制)

**validator_restart_during_dkg.rs**
- **功能**：测试 DKG 过程中验证器重启
- **步骤**：
  1. 等待 DKG 开始
  2. 注入故障点（failpoint）
  3. 等待节点 panic
  4. 重启节点
  5. 验证 DKG 能够继续完成
- **复杂度**：高
- **依赖**：
  - Failpoint 注入能力
  - 节点重启能力
  - 健康检查
- **迁移难度**：⭐⭐⭐⭐⭐ (非常困难)

**entry_func_attrs.rs**
- **功能**：测试随机数函数属性的各种组合
- **步骤**：
  1. 发布 OnChainDice 模块
  2. 配置随机数 API 参数
  3. 测试不同函数属性组合
  4. 验证交易结果
- **复杂度**：中-高
- **依赖**：Move 模块发布，函数调用
- **迁移难度**：⭐⭐⭐ (较难，需要 Move 支持)

## 三、关键技术差异

### 3.1 节点管理

| 功能 | 旧框架 | 新框架 |
|------|--------|--------|
| 启动节点 | ✅ SwarmBuilder | ❌ 不支持 |
| 停止节点 | ✅ node.stop() | ❌ 不支持 |
| 重启节点 | ✅ node.restart() | ❌ 不支持 |
| 修改配置 | ✅ 配置文件修改 | ❌ 不支持 |
| 连接节点 | ✅ 自动管理 | ✅ NodeConnector |
| 健康检查 | ✅ swarm.liveness_check() | ✅ health_check() |

### 3.2 链上资源访问

| 资源类型 | 旧框架 | 新框架 |
|----------|--------|--------|
| Move 资源 | ✅ REST API | ❌ 需要 EVM 桥接 |
| DKGState | ✅ 直接访问 | ❌ 需要实现 |
| PerBlockRandomness | ✅ 直接访问 | ❌ 需要实现 |
| OnChainConfig | ✅ 直接访问 | ❌ 需要实现 |
| 账户资源 | ✅ REST API | ✅ JSON-RPC |

### 3.3 交易执行

| 功能 | 旧框架 | 新框架 |
|------|--------|--------|
| Move 脚本 | ✅ CLI.run_script() | ❌ 需要实现 |
| Move 函数 | ✅ CLI.run_function() | ❌ 需要实现 |
| EVM 交易 | ❌ 不支持 | ✅ send_raw_transaction() |
| 合约调用 | ✅ REST API | ✅ eth_call() |
| 合约部署 | ✅ Move 模块发布 | ✅ 合约部署 |

### 3.4 验证能力

| 验证类型 | 旧框架 | 新框架 |
|----------|--------|--------|
| DKG Transcript | ✅ 完整实现 | ❌ 需要实现 |
| 随机数种子 | ✅ 完整实现 | ❌ 需要实现 |
| 交易结果 | ✅ 详细错误信息 | ✅ 交易 receipt |
| 余额验证 | ✅ REST API | ✅ eth_getBalance() |

## 四、迁移挑战

### 4.1 架构层面

1. **节点生命周期管理缺失**
   - 新框架无法启动/停止/重启节点
   - 解决方案：需要外部工具或 API 支持

2. **Move 资源访问缺失**
   - 新框架基于 EVM JSON-RPC，无法直接访问 Move 资源
   - 解决方案：需要实现 Move 资源到 EVM 的桥接，或添加 REST API 客户端

3. **配置管理缺失**
   - 新框架无法修改节点配置
   - 解决方案：需要外部配置管理工具

### 4.2 功能层面

1. **DKG 验证逻辑**
   - 需要实现完整的 DKG transcript 验证
   - 需要实现随机数种子验证
   - 需要访问验证器解密密钥

2. **Governance 脚本执行**
   - 需要实现 Move 脚本的执行能力
   - 需要实现 governance 操作的封装

3. **Failpoint 注入**
   - 需要实现 failpoint 设置 API
   - 需要节点支持 failpoint

### 4.3 测试数据层面

1. **验证器密钥管理**
   - 旧框架从 swarm 中提取密钥
   - 新框架需要从配置文件或 API 获取

2. **Epoch 同步**
   - 旧框架有 `wait_for_all_nodes_to_catchup_to_epoch`
   - 新框架需要实现类似的等待机制

## 五、迁移可行性评估

### 5.1 可直接迁移 (⭐⭐)

- **e2e_basic_consumption.rs** (部分)
  - 需要实现 Move 模块发布
  - 需要实现 Move 函数调用
  - 需要实现资源读取

### 5.2 需要扩展框架 (⭐⭐⭐)

- **enable_feature_0/1/2.rs**
  - 需要添加 governance 脚本执行
  - 需要添加 epoch 等待机制

- **disable_feature_0/1.rs**
  - 同上

- **entry_func_attrs.rs**
  - 需要完整的 Move 支持

### 5.3 需要重大扩展 (⭐⭐⭐⭐)

- **e2e_correctness.rs**
  - 需要实现 DKG 资源访问
  - 需要实现随机数验证逻辑
  - 需要实现密钥管理

- **dkg_with_validator_down.rs**
  - 需要节点控制能力
  - 需要 DKG 验证

- **dkg_with_validator_join_leave.rs**
  - 同上

### 5.4 几乎不可行 (⭐⭐⭐⭐⭐)

- **randomness_stall_recovery.rs**
  - 需要完整的节点生命周期管理
  - 需要配置热更新
  - 需要 sync_only 模式控制

- **validator_restart_during_dkg.rs**
  - 需要 failpoint 注入
  - 需要节点重启
  - 需要健康检查

## 六、建议

### 6.1 短期方案

1. **优先迁移简单用例**
   - 从 `e2e_basic_consumption` 开始
   - 逐步实现 Move 模块支持

2. **扩展框架能力**
   - 添加 REST API 客户端（用于 Move 资源访问）
   - 添加 Move 脚本/函数执行能力
   - 添加 governance 操作封装

### 6.2 中期方案

1. **实现 DKG 验证**
   - 实现 DKGState 资源访问
   - 实现 DKG transcript 验证逻辑
   - 实现随机数种子验证

2. **实现节点管理接口**
   - 通过外部 API 或工具控制节点
   - 实现配置管理接口

### 6.3 长期方案

1. **混合框架**
   - 保留旧框架用于复杂场景
   - 新框架用于简单场景和已部署节点测试

2. **统一接口**
   - 抽象出统一的测试接口
   - 支持多种后端实现

