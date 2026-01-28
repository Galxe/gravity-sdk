# Gravity SDK CI 优化记录

> 创建时间: 2025-01-28
> 分支: fix/cluster-benchmark-improvements
> 状态: 进行中

## 1. 问题背景

原始 CI 在 GitHub Actions 上失败，错误：`No space left on device`

原因：
- GitHub Actions runner 只有约 14GB 磁盘空间
- Rust 编译产生大量中间文件
- 单个 test job 运行所有测试，磁盘空间不足

## 2. 优化方案

### 2.1 测试分割

将单个 test job 分割为 5 个并行 job：

| Job | 测试包 | 说明 |
|-----|--------|------|
| test-core | gravity-primitives, gravity-storage, api, api-types | 核心 crate |
| test-consensus | block-buffer-manager, txn_metrics | 共识相关 |
| test-aptos | aptos-consensus, aptos-executor | Aptos crate |
| test-dependencies | gaptos | 外部依赖 |
| test-binaries | gravity_node, gravity_cli | 二进制 |

### 2.2 容器化

使用 Docker 容器运行重量级 job：
- 镜像: `rust:1.88.0-bookworm`
- 运行时安装依赖: clang, llvm, libudev-dev, libssl-dev, pkg-config

### 2.3 磁盘空间管理

- 在大型测试之间运行 `cargo clean -p <package>`
- 显示磁盘空间: `df -h`
- 串行运行内存密集型测试: `--test-threads=1`

### 2.4 自定义 Docker 镜像 (待实现)

已创建但尚未启用：
- `/.github/docker/rust-ci.Dockerfile`
- `/.github/workflows/build-ci-image.yml`

需要先合并到 main 分支才能构建镜像。

## 3. 修复记录

### 3.1 包名修复
```yaml
# 错误
cargo test -p txn-metrics
# 正确
cargo test -p txn_metrics
```

### 3.2 Aptos 包歧义修复
```yaml
# 错误 - 有多个同名包
cargo test -p aptos-consensus
# 正确 - 使用 manifest-path
cargo test --manifest-path aptos-core/consensus/Cargo.toml
```

### 3.3 Docker 镜像认证
```yaml
# 公共镜像不需要 credentials
container:
  image: rust:1.88.0-bookworm
  # 不要添加 credentials 块
```

## 4. 当前 CI 配置

文件: `.github/workflows/rust-ci.yml`

```yaml
jobs:
  fmt:          # 轻量级，直接在 runner 上运行
  clippy:       # 容器中运行
  build:        # 容器中运行
  test-core:    # 容器中运行，需要 CICD:run-tests 标签
  test-consensus:
  test-aptos:
  test-dependencies:
  test-binaries:
  test-results:  # 聚合测试结果
  dependency-check:  # 检查 greth 分支依赖
```

## 5. 已知问题

### 5.1 大部分测试包没有测试

| 包 | 测试数 |
|----|--------|
| gravity-primitives | 0 |
| gravity-storage | 0 |
| api | 1 (可能失败) |
| api-types | 0 |
| block-buffer-manager | 0 |
| txn_metrics | 0 |
| gaptos | 0 |
| gravity_node | 0 |
| gravity_cli | 0 |
| aptos-consensus | 213 |

### 5.2 API 测试可能失败

`crates/api/src/https/mod.rs` 中的 `https::test::work` 是集成测试，需要：
- 启动 HTTPS 服务器
- fail-points 功能

在 CI 环境中可能不稳定。

### 5.3 aptos-consensus 测试未验证

213 个测试尚未在 CI 中完整验证，可能存在：
- 环境依赖问题
- 超时问题
- 资源竞争问题

## 6. 后续优化

### 短期 (测试网后)
- [ ] 构建并发布自定义 Docker 镜像到 GHCR
- [ ] 切换 CI 使用自定义镜像 (预装依赖，更快)
- [ ] 修复或禁用不稳定的测试

### 中期
- [ ] 添加 cargo 缓存优化
- [ ] 考虑使用 sccache 加速编译
- [ ] 评估是否需要更大的 runner

### 长期
- [ ] 迁移更多 gaptos 测试到本地
- [ ] 建立测试覆盖率报告
- [ ] 添加性能基准测试

## 7. 相关 Commits

```
d0d73de fix(ci): correct package name txn_metrics
8fd8190 fix(ci): use manifest-path to disambiguate aptos packages
7c28c2f fix(ci): use public rust image for all jobs, add missing deps install
86b4c21 ci: revert to public rust image for initial validation
...
```

## 8. 触发 CI 的方式

1. **Push 到 main/branch-v** 分支** - 自动触发
2. **PR 带 `CICD:run-tests` 标签** - 触发测试 jobs
3. **手动触发 (workflow_dispatch)** - 在 Actions 页面点击 "Run workflow"

## 9. 参考文档

- `todo/architecture-sync-plan.md` - 架构同步计划
- `todo/test-migration-analysis.md` - 测试迁移分析
- `CLAUDE.md` - 项目构建指南
