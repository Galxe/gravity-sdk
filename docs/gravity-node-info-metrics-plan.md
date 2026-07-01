# gravity_node 二进制信息 Metrics 方案

## 背景

网络升级前，运维侧需要一个 Dashboard 来确认全网节点是否运行同一份
`gravity_node` 二进制。第一版目标先只把 `gravity_node` 二进制与构建信息暴露到
metrics，用于升级前核对全网节点是否已经切到同一份 binary。

这些指标用于升级前一致性核对，不是高频运行时遥测。Genesis
`chain_spec.genesis.config.extra_fields` 暂不纳入第一版。

## 目标

- 让每个节点暴露足够的信息，方便 Grafana 对比全网节点的二进制版本和哈希。
- 实现范围尽量收敛在 `gravity_node` 启动路径。
- 本地验证只做代码检查，不启动、重启或部署任何测试集群。

## 非目标

- 不新增 RPC endpoint。
- 不在第一版上报 `chain_spec.genesis.config.extra_fields`。
- 不把所有 genesis 字段都拆成独立 metrics。
- 不把这些静态信息做成动态运行时指标。
- 不为这个需求修改 `greth` 或 `gaptos` 依赖 revision。

## 指标形态

采用 Prometheus 常见的 info gauge 模式：指标值恒为 `1`，具体信息放在 label
里。这样 Grafana 可以直接按 label 聚合，检查全网是否出现多个不同值。

指标：

```text
gravity_node_build_info{
  package_version="0.1.0",
  commit_hash="...",
  branch="...",
  tag="...",
  build_time="...",
  build_os="...",
  rust_version="...",
  profile="release",
  clean_checkout="true",
  binary_sha256="..."
} 1
```

信息来源：

- 复用已有的 `build_info::build_information!()`。
- 运行时通过 `std::env::current_exe()` 找到当前执行的 `gravity_node` 文件，并计算
  sha256。
- 如果读取当前可执行文件失败，则设置 `binary_sha256="unknown"`，并打一条 warning
  log。

这里不只上报 git commit，还上报二进制 sha256，是因为两个节点即使 commit 相同，也
可能因为 profile、features 或构建环境不同而产出不同二进制。

## 实现方案

1. 新增 `bin/gravity_node/src/node_metrics.rs`。
2. 使用 `gaptos::aptos_metrics_core::register_int_gauge_vec` 定义一个
   `Lazy<IntGaugeVec>`。
3. 新增一个公开给 crate 内使用的 helper：

   ```rust
   pub(crate) fn register_binary_info_metrics()
   ```

4. helper 内部逻辑：

   - 从 `build_information!()` 读取构建信息。
   - 计算当前可执行文件 sha256。
   - 设置 `gravity_node_build_info` 为 `1`。

5. 在 `bin/gravity_node/src/main.rs` 的完整 node 启动路径中调用：

   ```rust
   node_metrics::register_binary_info_metrics();
   ```

6. 在 `main.rs` 增加 `mod node_metrics;`。
7. 在 `bin/gravity_node/Cargo.toml` 为 sha256 计算增加 `sha2` 依赖。

## Dashboard 使用方式

示例检查：

```promql
count by (commit_hash, binary_sha256) (gravity_node_build_info)
```

用于查看全网有几种不同的二进制身份。

升级前 Dashboard 可以包含这些面板：

- 不同 `gravity_node` commit hash 数量。
- 不同 binary sha256 数量。
- 节点明细表：instance、commit hash、branch、tag、build time、profile、binary sha256。

## 验证方式

只跑本地代码检查：

```bash
cargo fmt --check
cargo check -p gravity_node
```

如果 `cargo check -p gravity_node` 太慢，或者被无关依赖问题阻塞，需要记录具体失败
原因；至少确认改动能通过依赖图允许范围内的编译检查。

除非明确要求，不执行任何 cluster start、restart、deploy、stop 命令。

## 风险与注意事项

- label 必须保持有界。这里使用的是构建信息，属于静态小集合，风险可控。
- `binary_sha256` 需要读取当前可执行文件。如果读取失败，指标仍会上报，但
  `binary_sha256` 会是 `unknown`，同时日志里会出现 warning。
- 如果后续还需要做 genesis hardfork 一致性核对，可以再新增
  `gravity_node_genesis_extra_field_info`，与本次二进制信息指标保持同样的 info gauge
  模式。
