# GSDK 安全修复设计（第二轮）

日期：2026-02-28

---

## GSDK-006：共识关键执行循环中的 `unwrap()` 导致 panic（高）

**问题：** `reth_cli.rs` 中的 `start_execution()`、`start_commit_vote()` 和 `start_commit()` 包含 15+ 个对可失败操作的裸 `.unwrap()` 调用（交易解码、签名恢复、DB 读取、向量索引）。单个格式错误的交易或瞬时 DB 错误将导致整个验证器节点崩溃。攻击者如果能向排序层注入恶意交易，可同时 DoS 所有验证器。

**修复：** 将 `.unwrap()` 替换为正确的错误处理：
1. `TransactionSigned::decode_2718().unwrap()` / `recover_signer().unwrap()` → 格式错误时跳过交易并 `warn!()`
2. `senders[idx].unwrap()` / `transactions[idx].unwrap()` → 过滤 `None` 条目并记录跳过的交易
3. `provider.recover_block_number().unwrap()` → 通过 `?` 向上传播错误（函数已返回 `Result`）
4. `exec_blocks.unwrap()` → 合并错误分支，在 unwrap 前处理所有错误
5. `block_ids.last().unwrap()` → 用 `is_empty()` 守卫（已存在但 unwrap 紧随其后）
6. `.set_state().await.unwrap()` / `.wait_for_block_persistence().await.unwrap()` → 通过 `?` 传播

**文件：** `bin/gravity_node/src/reth_cli.rs`



---

## GSDK-007：Relayer `get_last_state` 在缓存结果缺失时 panic（高）

**问题：** `RelayerWrapper::get_last_state()` 在 `should_block_poll` 返回 `true` 但没有缓存结果时 panic。panic 消息说 "No cached result for uri"，但上方注释说 "fall through to poll"。这是逻辑 bug：应该降级到 `poll_and_update_state()`，而非 panic。

**修复：** 将 L280 的 `panic!()` 替换为：
```rust
warn!("No cached result for uri: {uri}, falling through to poll");
// 降级到下面的 poll
```
移除 `return` 路径，让执行继续到 `poll_and_update_state()`。

**文件：** `bin/gravity_node/src/relayer.rs`

---

## GSDK-008：内存池外部交易签名恢复未验证（高）

**问题：** `add_external_txn()` 对外部提交的交易调用 `txn.recover_signer().unwrap()`。一个精心构造的交易如果能通过 `decode_2718` 但在 `recover_signer` 失败，将导致验证器崩溃。

**修复：** 将 `.unwrap()` 替换为错误处理：
```rust
let signer = match txn.recover_signer() {
    Some(s) => s,
    None => {
        tracing::error!("Failed to recover signer for external transaction");
        return false;
    }
};
```

**文件：** `bin/gravity_node/src/mempool.rs`

---

## GSDK-009：HTTP API 响应中的详细内部错误信息（中）

**问题：** `consensus.rs` 和 `dkg.rs` 中的错误响应使用 `format!("{e:?}")`，向 API 消费者泄露了内部类型名、schema 名称和潜在文件路径。

**修复：** 创建统一的错误响应模式：
1. 用 `error!()` 在服务端记录详细错误
2. 向客户端返回通用消息："Internal server error"、"Resource not found"、"Service unavailable"
3. 提取为辅助函数，接收日志级别的错误并返回脱敏后的响应

**文件：** `crates/api/src/https/consensus.rs`、`crates/api/src/https/dkg.rs`

---

## GSDK-010：启动竞态中 `GLOBAL_CONFIG_STORAGE` `.unwrap()` 导致 Relayer 崩溃（中）

**问题：** `get_oracle_source_states()` 中的 `GLOBAL_CONFIG_STORAGE.get().unwrap()` 在存储初始化前被调用时 panic。节点启动期间如果 relayer 在 config storage 就绪前被激活，就会触发此问题。

**修复：** 将 `.get().unwrap()` 替换为：
```rust
let config_storage = match GLOBAL_CONFIG_STORAGE.get() {
    Some(cs) => cs,
    None => {
        warn!("GLOBAL_CONFIG_STORAGE not yet initialized, returning empty oracle states");
        return vec![];
    }
};
```

**文件：** `bin/gravity_node/src/relayer.rs`

---

## GSDK-011：HTTP/HTTPS 端点无速率限制或请求大小限制（中）

**问题：** HTTP/HTTPS 服务器没有速率限制、请求体大小限制或连接数限制。攻击者可以通过洪泛请求或发送超大 payload 来耗尽资源。

**修复：** 向路由器添加中间件层：
1. `axum::extract::DefaultBodyLimit::max(1_048_576)`（最大请求体 1 MB）
2. `tower::limit::RateLimitLayer::new(100, Duration::from_secs(1))`（100 请求/秒）
3. `tower::limit::ConcurrencyLimitLayer::new(256)`（最大并发请求数）

**文件：** `crates/api/src/https/mod.rs`

---

## GSDK-012：Sentinel Webhook URL 未做 SSRF 验证（中）

**问题：** `AlertingConfig` 中的 `feishu_webhook` 和 `slack_webhook` 直接用于 `reqwest::Client::post()`，没有 SSRF 验证。与 GSDK-004 威胁模型相同，但修复未应用到 webhook URL。

**修复：** 在 `Config::load()` 反序列化后，对每个 webhook URL 调用 `validate_probe_url()`：
```rust
if let Some(feishu) = &config.alerting.feishu_webhook {
    if !feishu.is_empty() {
        validate_probe_url(feishu)?;
    }
}
if let Some(slack) = &config.alerting.slack_webhook {
    if !slack.is_empty() {
        validate_probe_url(slack)?;
    }
}
```

**文件：** `bin/sentinel/src/config.rs`

---

## GSDK-013：`ensure_https` 中间件对纯 TCP 连接无效（中）

**问题：** `ensure_https` 检查 `req.uri().scheme_str()`，但 Axum/hyper 不会为传入的 TCP 连接填充 URI scheme。当未配置 TLS（cert/key 为 `None`）时，回退路径通过纯 HTTP 服务所有路由（包括"仅 HTTPS"路由）。中间件可能不会拒绝这些请求，因为 `scheme_str()` 返回 `None`（不是 `"http"`）。

**修复：** 将 HTTP 和 HTTPS 路由分离到不同的监听器：
1. 配置 TLS 时：将 `https_routes` 绑定到 TLS 监听器，`http_routes` 绑定到明文监听器（或仅提供 HTTPS）
2. 未配置 TLS 时：完全不注册敏感的 `https_routes`。记录启动警告，说明共识/DKG 端点在无 TLS 时被禁用

**替代方案：** 如果需要单端口，检查 `ConnectInfo` 或 TLS acceptor 设置的自定义 `Extension` 来检测连接是否加密。

**文件：** `crates/api/src/https/mod.rs`

---

## GSDK-014：无效绑定地址导致地址解析 `unwrap()` 崩溃服务器（低）

**问题：** L127 的 `self.address.parse().unwrap()` 在配置的地址格式错误时 panic，没有诊断信息。

**修复：** 替换为 `.parse().unwrap_or_else(|e| panic!("Invalid bind address '{}': {e}", self.address))` 或返回 `Result`。

**文件：** `crates/api/src/https/mod.rs`

---

## GSDK-015：Sentinel 白名单中用户提供的正则表达式存在 ReDoS 风险（低）

**问题：** 白名单 CSV 接受任意正则表达式。精心构造的恶意正则可通过灾难性回溯冻结 sentinel。

**修复：** 使用 `RegexBuilder::new(pattern_str).size_limit(1 << 20).dfa_size_limit(1 << 20).build()` 限制编译后的正则复杂度。如果编译超过大小限制，记录警告并跳过该规则。

**文件：** `bin/sentinel/src/whitelist.rs`

---

## GSDK-016：Sentinel 文件监控中的 Glob 模式注入（低）

**问题：** `file_patterns` 配置字段直接传递给 `glob()`。类似 `/**/*` 的模式会递归扫描整个文件系统。

**修复：** 在 `Config::load()` 中验证模式：
1. 拒绝包含 `..` 的模式
2. 拒绝以 `/` 开头的模式（要求相对路径）
3. 为 glob 结果添加最大深度限制

**文件：** `bin/sentinel/src/config.rs`、`bin/sentinel/src/watcher.rs`
