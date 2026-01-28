# Gravity SDK 架构同步计划

> 创建时间: 2025-01-28
> 状态: 版本冻结中 (测试网准备)
> 优先级: 测试网上线后处理

## 1. 当前架构分析

### 1.1 依赖结构

```
Gravity SDK 依赖结构
├── 本地复制/修改的模块 (需要定制化)
│   ├── aptos-core/consensus/           ← 290 个 Rust 文件
│   ├── aptos-core/consensus/consensus-types/
│   ├── aptos-core/consensus/safety-rules/
│   ├── aptos-core/mempool/
│   ├── dependencies/aptos-executor-types/
│   └── dependencies/aptos-executor/
│
└── 从 gaptos 引用的模块 (无需修改)
    └── gaptos (git: gravity-aptos, rev: 977f5b9388183c8a14c0ddcb4e2ac9f265d45184)
```

### 1.2 Gravity 特有文件

仅 2 个文件是 Gravity 专属:
- `aptos-core/consensus/src/gravity_state_computer.rs` (194 行) - GCEI 集成核心
- `aptos-core/consensus/src/qc_aggregator.rs` (~50 行) - QC 聚合

### 1.3 为什么需要复制

`gravity_state_computer.rs` 实现了 GCEI 协议的共识层集成:
- 包装 `BlockExecutor` 以集成 `block_buffer_manager`
- 在 `commit_ledger()` 中调用 `get_block_buffer_manager().set_commit_blocks()`
- 无法通过简单的 trait 注入实现

## 2. GAPTOS vs 本地差异

### 2.1 GAPTOS 新增功能 (可考虑迁移)

| 功能 | 文件 | 优先级 | 说明 |
|------|------|--------|------|
| PayloadTxnsSize | consensus-types/src/utils.rs | 中 | 带不变量检查的 payload 大小管理 |
| RoundTimeoutReason | consensus-types/src/round_timeout.rs | 低 | 更详细的超时诊断 |
| 模块化 consensus_observer | src/consensus_observer/*/ | 低 | 更好的代码组织 |
| 模块化 payload_manager | src/payload_manager/ | 低 | 更好的代码组织 |

### 2.2 测试基础设施差异

| 方面 | 本地 | GAPTOS |
|------|------|--------|
| prepare_safety_rules() | async | sync |
| State Computer 通知 | Callback tuple | BoxFuture |
| 模块可见性 | pub mod | mod |

## 3. 测试迁移计划

详见 `todo/test-migration-analysis.md`

## 4. 后续行动项

### 4.1 测试网后 - 短期 (1-2 周)

- [ ] 迁移可用的 gaptos 测试到本地
- [ ] 修复 `crates/api` 中的 https 测试
- [ ] 验证所有 CI 测试通过

### 4.2 测试网后 - 中期 (1-2 月)

- [ ] 评估是否同步 PayloadTxnsSize 安全结构体
- [ ] 重构 consensus_observer 为模块化结构
- [ ] 更新本地副本中的关键 bug fix

### 4.3 长期考虑

- [ ] 评估将 GCEI 协议贡献回 gravity-aptos
- [ ] 减少本地复制代码量
- [ ] 建立定期同步机制

## 5. 相关文件

- `CLAUDE.md` - 项目构建指南
- `todo/test-migration-analysis.md` - 测试迁移详细分析
- `todo/ci-optimization.md` - CI 优化记录
