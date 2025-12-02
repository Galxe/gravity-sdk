# Randomness 测试使用指南

## 快速开始

### 环境准备

```bash
cd /Users/lightman/repos/gravity-sdk/gravity_e2e
source venv/bin/activate
```

### 编译RandomDice合约

```bash
cd /Users/lightman/repos/gravity-sdk
forge build
```

### 运行测试

```bash
# 运行所有randomness测试 (~102秒)
python -m gravity_e2e.main --test-suite randomness --log-level INFO

# 运行单个测试
python -m gravity_e2e.main --test-suite randomness_smoke --log-level INFO
python -m gravity_e2e.main --test-suite randomness_reconfiguration --log-level INFO
python -m gravity_e2e.main --test-suite randomness_multi_contract --log-level INFO
python -m gravity_e2e.main --test-suite randomness_api_completeness --log-level INFO
```

## 可用的测试套件

| 测试套件名称 | 说明 | 耗时 |
|------------|------|------|
| `randomness` | 运行所有6个核心测试 | ~102s |
| `randomness_smoke` | DKG健康检查 | ~13s |
| `randomness_basic` | 基础随机数消费 | ~21s |
| `randomness_correctness` | 随机性正确性观测 | ~1s |
| `randomness_reconfiguration` | **Epoch转换测试** | ~26s |
| `randomness_multi_contract` | **多合约隔离测试** | ~40s |
| `randomness_api_completeness` | **API完整性测试** | ~1s |
| `randomness_stress` | 压力测试 (50次调用) | ~100s |

## 新增测试详解

### 1. Epoch转换测试

验证DKG状态在时间维度上的演进：

```bash
python -m gravity_e2e.main --test-suite randomness_reconfiguration --log-level INFO
```

**验证内容**:
- Epoch、Round、Block的正确演进
- Block number的单调递增性
- 历史随机性的持续可用性

### 2. 多合约隔离测试

验证多个合约同时使用随机性时的行为：

```bash
python -m gravity_e2e.main --test-suite randomness_multi_contract --log-level INFO
```

**验证内容**:
- 同一区块内的合约获得相同seed
- 不同合约的结果具有独立性
- 并发调用的正确处理

### 3. API完整性测试

全面验证DKG HTTP API的功能：

```bash
python -m gravity_e2e.main --test-suite randomness_api_completeness --log-level INFO
```

**验证内容**:
- `/dkg/status` 端点功能
- `/dkg/randomness/{block}` 端点功能
- 历史数据可用性
- 并发请求性能
- 边界条件处理

## 查看测试结果

测试结果自动保存到:

```bash
cat gravity_e2e/output/test_results.json
```

## 故障排查

### 合约未编译

错误: `RandomDice not compiled`

解决:
```bash
cd /Users/lightman/repos/gravity-sdk
forge build
```

### Gravity节点未运行

错误: `Failed to connect to node`

解决:
```bash
# 检查Gravity节点是否在运行
curl http://127.0.0.1:8545 -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'
```

### DKG API不可用

错误: `DKG status endpoint returned None`

解决:
```bash
# 检查DKG HTTP API是否在运行
curl http://127.0.0.1:1998/dkg/status
```

## 详细文档

- [完整实现报告](./RANDOMNESS_TESTS_FINAL_REPORT.md)
- [技术实现细节](./RANDOMNESS_TESTS_IMPLEMENTATION.md)
- [迁移计划](./RANDOMNESS_EVM_MIGRATION_PLAN.md)
