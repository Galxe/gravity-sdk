# Gravity SDK 测试迁移分析

> 创建时间: 2025-01-28
> GAPTOS 版本: rev 977f5b9388183c8a14c0ddcb4e2ac9f265d45184
> 状态: 待测试网上线后执行

## 1. 测试统计概览

| 类别 | 文件数 | 测试数 | 难度 | 优先级 |
|------|--------|--------|------|--------|
| Block Types | 1 | 7 | EASY | P0 |
| Consensus DB | 4 | 6 | EASY | P0 |
| Block Storage | 1 | 10 | MEDIUM | P1 |
| Quorum Store | 8 | 32 | EASY-HARD | P1 |
| Liveness | 7 | 24 | EASY-HARD | P1-P2 |
| Safety Rules | 3 | 19 | HARD | P1 |
| DAG | 8 | 20 | HARD | P2 |
| Round Manager | 1 | 24 | HARD | P2 |
| Pipeline | 4 | 5 | HARD | P2 |
| Network | 1 | 4 | HARD | P2 |
| State Computer | 1 | 2 | HARD | P2 |
| Randomness/DKG | 8 | 27 | HARD | P3 |
| Twins | 1 | 5 | HARD | P3 |
| **总计** | **47** | **157** | **Mixed** | **8 周** |

## 2. 难度分类

### 2.1 EASY (30-40 测试) - 可直接迁移

这些测试只需修改 import 路径：

```
aptos-core/consensus/consensus-types/src/block_test.rs (7 tests)
├── test_genesis
├── test_nil_block
├── test_block_relation
├── test_same_qc_different_authors
├── test_block_metadata_bitvec
├── test_nil_block_metadata_bitvec
└── test_failed_authors_well_formed

aptos-core/consensus/src/consensusdb/ (6 tests)
├── consensusdb_test.rs: test_put_get, test_delete_block_and_qc, test_dag
└── schema/*/test.rs: block, qc, single_entry

aptos-core/consensus/src/liveness/ (5 tests)
├── rotating_proposer_test.rs (3 tests)
├── round_proposer_test.rs (1 test)
└── cached_proposer_election_test.rs (1 test)

aptos-core/consensus/src/quorum_store/tests/ (4 tests)
├── batch_store_test.rs (3 tests)
└── types_test.rs (1 test)
```

### 2.2 MEDIUM (35-45 测试) - 需要 API 适配

```
aptos-core/consensus/src/block_storage/block_store_test.rs
├── test_highest_block_and_quorum_cert
├── test_qc_ancestry
├── test_path_from_root
├── test_highest_qc
└── test_need_fetch_for_qc

aptos-core/consensus/src/quorum_store/tests/
├── batch_generator_test.rs (6 tests)
└── batch_requester_test.rs (3 tests)

aptos-core/consensus/src/dag/tests/
├── types_test.rs (5 tests)
└── order_rule_tests.rs (2 tests)

aptos-core/consensus/src/liveness/
├── round_state_test.rs (3 tests)
└── unequivocal_proposer_election_test.rs (1 test)
```

### 2.3 HARD (82-95 测试) - 需要重构

**Safety Rules (19 tests) - 关键路径**
```
aptos-core/consensus/safety-rules/src/tests/suite.rs
├── test_initialize
├── test_end_to_end
├── test_2chain_rules
├── test_2chain_timeout
├── test_order_votes_*
└── ... (18 tests total)
```

**Round Manager (24 tests) - 依赖 Storage + Safety**
```
aptos-core/consensus/src/round_manager_test.rs
├── Basic consensus flow (6 tests)
├── Timeout handling (6 tests)
├── Execution & commit (7 tests)
└── Network integration (5 tests)
```

**DAG Protocol (20 tests) - 全部或无**
```
aptos-core/consensus/src/dag/tests/
├── dag_test.rs (4 tests)
├── dag_driver_tests.rs (2 tests)
├── dag_network_test.rs (1 test)
├── dag_state_sync_tests.rs (1 test)
├── fetcher_test.rs (1 test)
├── integration_tests.rs (1 test)
└── rb_handler_tests.rs (3 tests)
```

**DKG/Randomness (27 tests) - 密码学相关**
```
aptos-core/consensus/src/rand/dkg/
├── crypto_tests.rs (9 tests)
├── pvss_tests.rs (7 tests)
├── fft_tests.rs (3 tests)
├── accumulator_tests.rs (2 tests)
├── dkg_tests.rs (2 tests)
├── dkg_runtime_tests.rs (2 tests)
├── weighted_vuf_tests.rs (1 test)
└── secret_sharing_config_tests.rs (1 test)
```

## 3. 迁移计划

### Phase 1: Foundation (Week 1-2)
**目标: 验证测试基础设施**

```bash
# 迁移 EASY 测试
- [ ] block_test.rs (7 tests)
- [ ] consensusdb_test.rs (3 tests)
- [ ] schema tests (3 tests)
- [ ] rotating_proposer_test.rs (3 tests)
- [ ] batch_store_test.rs (3 tests)
```

预计工作量: 5-10 小时

### Phase 2: Storage & Quorum Store (Week 2-3)
**目标: 验证存储集成**

```bash
# 迁移 MEDIUM 测试
- [ ] block_store_test.rs 基础测试 (5 tests)
- [ ] batch_generator_test.rs (6 tests)
- [ ] batch_requester_test.rs (3 tests)
- [ ] quorum_store_db_test.rs (2 tests)
```

预计工作量: 15-25 小时

### Phase 3: Safety Rules (Week 3-4)
**目标: 验证共识安全性**

```bash
# 关键路径 - 必须通过
- [ ] test_initialize
- [ ] test_end_to_end
- [ ] test_2chain_rules
- [ ] test_2chain_timeout
- [ ] test_order_votes_* (4 tests)
```

预计工作量: 20-30 小时

### Phase 4: Consensus Core (Week 5-6)
**目标: 验证共识协议**

```bash
# Round Manager 基础
- [ ] Basic consensus flow (6 tests)
- [ ] Timeout handling (6 tests)

# DAG Types
- [ ] types_test.rs (5 tests)
- [ ] order_rule_tests.rs (2 tests)
```

预计工作量: 30-40 小时

### Phase 5: Integration (Week 7-8)
**目标: 完整集成测试**

```bash
- [ ] Round Manager network tests
- [ ] Pipeline tests
- [ ] Network tests
- [ ] DAG protocol tests (optional)
```

预计工作量: 20-30 小时

## 4. 依赖关系图

```
                    ┌─────────────┐
                    │ Block Types │
                    │   (EASY)    │
                    └──────┬──────┘
                           │
              ┌────────────┼────────────┐
              ▼            ▼            ▼
        ┌──────────┐ ┌──────────┐ ┌──────────┐
        │ Consensus│ │  Block   │ │  Quorum  │
        │    DB    │ │  Store   │ │  Store   │
        │  (EASY)  │ │ (MEDIUM) │ │ (MEDIUM) │
        └────┬─────┘ └────┬─────┘ └────┬─────┘
             │            │            │
             └────────────┼────────────┘
                          ▼
                   ┌─────────────┐
                   │ Safety Rules│
                   │   (HARD)    │
                   └──────┬──────┘
                          │
              ┌───────────┼───────────┐
              ▼           ▼           ▼
        ┌──────────┐ ┌──────────┐ ┌──────────┐
        │  Round   │ │   DAG    │ │ Pipeline │
        │ Manager  │ │ Protocol │ │  Tests   │
        │  (HARD)  │ │  (HARD)  │ │  (HARD)  │
        └──────────┘ └──────────┘ └──────────┘
```

## 5. Mock 组件需求

迁移测试需要以下 mock 组件：

| 组件 | 代码量 | 用途 |
|------|--------|------|
| NetworkPlayground | ~500 LOC | 网络模拟 |
| TreeInserter | ~300 LOC | 区块树构建 |
| MockStorage | ~400 LOC | 持久化状态 |
| MockExecutionClient | ~200 LOC | 执行模拟 |
| MockMempoolClient | ~150 LOC | 交易池模拟 |

## 6. 风险评估

### 高风险
- **密码学库变更**: 影响 Safety Rules, DKG 测试
- **网络协议演进**: 影响 DAG, Round Manager 测试
- **执行器接口差异**: 影响 Pipeline 测试

### 中风险
- **存储 Schema 变更**: 影响所有存储相关测试
- **Mempool 集成**: 影响 Batch Generator, Proposal Generator

### 低风险
- **类型定义变更**: 影响 Block Types 测试
- **提议者选举算法**: 影响 Liveness 测试

## 7. 成功标准

### Phase 1 完成标准
- [ ] 所有 EASY 测试通过 (约 35 tests)
- [ ] CI 能够运行测试

### Phase 2 完成标准
- [ ] 存储相关测试通过 (约 20 tests)
- [ ] Quorum Store 基础测试通过

### Phase 3 完成标准
- [ ] Safety Rules 核心测试通过 (19 tests)
- [ ] 2-chain 规则验证通过

### Phase 4 完成标准
- [ ] Round Manager 基础测试通过 (12 tests)
- [ ] DAG 类型测试通过 (7 tests)

### 最终完成标准
- [ ] 157 个测试中至少 120 个通过 (76%)
- [ ] 所有 Safety Rules 测试通过 (100%)
- [ ] CI 流水线稳定运行

## 8. 下一步行动

测试网上线后：

1. **Week 1**: 开始 Phase 1，迁移 EASY 测试
2. **Week 2**: 修复发现的问题，开始 Phase 2
3. **Week 3-4**: 重点攻克 Safety Rules
4. **Week 5-6**: 评估是否继续 Round Manager 测试
5. **Week 7-8**: 根据实际情况调整计划

## 9. 新增 CI 工作流

已创建 `.github/workflows/migrated-tests.yml`，包含：

| Job | 测试包 | 说明 |
|-----|--------|------|
| test-consensus-types | aptos-consensus-types | 11 个测试，已验证通过 |
| test-safety-rules | aptos-safety-rules | Safety Rules 测试 |
| test-consensus-core | aptos-consensus | 主要共识测试 (部分可能失败) |

## 10. 相关文档

- `todo/architecture-sync-plan.md` - 架构同步计划
- `todo/ci-optimization.md` - CI 优化记录
- `.github/workflows/migrated-tests.yml` - 迁移测试 CI
- `CLAUDE.md` - 项目构建指南
