# Gravity SDK 安全审计与修复总结

**日期：** 2026-02-23
**审计范围：** `crates/api/`、`bin/gravity_node/`、`bin/gravity_cli/`、`bin/sentinel/`、`crates/block-buffer-manager/`、`aptos-core/consensus/` 修改部分
**仓库：** https://github.com/Galxe/gravity-sdk
**版本：** gravity-testnet-v1.0.0

---

## 审计结果总览

| 严重性 | 发现数量 | 状态 |
|--------|----------|------|
| HIGH（高） | 2 | 全部已修复 |
| MEDIUM（中） | 3 | 全部已修复 |
| **总计** | **5** | **全部已修复** |

### 修复提交

| 提交 | 内容 | 涉及文件数 |
|------|------|------------|
| [`a0bf499`](https://github.com/Richard1048576/gravity-sdk/commit/a0bf499) | GSDK-001/002/003/004：管理路由、明文 HTTP、Sentinel SSRF | 4 |
| [`21e6898`](https://github.com/Richard1048576/gravity-sdk/commit/21e6898) | GSDK-005：修复探针 URL 校验的 DNS 绕过 | 1 |

---

## HIGH 严重性

### GSDK-001：未认证的 `/set_failpoint` 端点

**涉及文件：** `crates/api/src/https/mod.rs:113`

**问题描述：**
`/set_failpoint` 端点注册在明文 HTTP 路由（`http_routes`）上，无任何认证中间件。该端点直接调用 `fail::cfg()`，允许远程注入 failpoint 以控制共识行为。共识层中存在 14 个以上的 failpoint（如 `consensus::send::proposal`、`consensus::process_proposal` 等），攻击者只要能访问验证者的 HTTP 端口（默认 8080），即可停止出块，导致验证者停机。

**修复方案：**
使用 `#[cfg(debug_assertions)]` 编译门控，确保该路由在 release 构建中被完全移除。同时在启动时若 `failpoints` 特性在非 debug 构建中被启用，则输出运行时警告。

> **备选方案（已否决）：** 将端点移到 HTTPS 路由并加 bearer token 认证。否决原因——failpoints 本身就不应在生产环境中存在，即使有认证，凭证泄露的风险也不足以支撑保留该端点。

---

### GSDK-002：未认证的 `/mem_prof` 端点

**涉及文件：** `crates/api/src/https/mod.rs:114`、`crates/api/src/https/heap_profiler.rs`

**问题描述：**
`/mem_prof` 端点同样位于未认证的明文 HTTP 路由上。该端点通过 `mallctl("prof.active")` 和 `mallctl("prof.dump")` 触发 jemalloc 堆内存分析。堆转储（heap dump）可能捕获内存中的**私钥、DKG 转录材料、会话令牌**等敏感数据，且转储文件可能以默认权限（所有人可读）写入磁盘。

**修复方案：**
与 GSDK-001 相同，使用 `#[cfg(debug_assertions)]` 编译门控。此外在 debug 构建中，堆转储文件使用 `fs::set_permissions()` 强制设置为 `0600` 权限（仅 owner 可读写）。

---

## MEDIUM 严重性

### GSDK-003：共识/DKG 端点使用明文 HTTP 传输

**涉及文件：** `crates/api/src/https/mod.rs:105-112`、`bin/gravity_cli/src/dkg.rs`

**问题描述：**
以下 7 个路由在明文 HTTP 上暴露敏感的共识状态，没有 TLS 加密：

| 路由 | 暴露数据 |
|------|----------|
| `/dkg/randomness/:block_number` | 每区块 DKG 随机数种子 |
| `/consensus/latest_ledger_info` | 当前纪元状态 |
| `/consensus/ledger_info/:epoch` | 纪元转换数据（含验证者集合） |
| `/consensus/block/:epoch/:round` | 完整共识区块数据 |
| `/consensus/qc/:epoch/:round` | 法定人数证书（含聚合 BLS 签名） |
| `/consensus/validator_count/:epoch` | 纪元验证者数量（公开元数据） |
| `/dkg/status` | 当前 DKG 会话状态 |

同一网络段的被动中间人（MITM）可收集 DKG 随机数、QC 签名和完整共识区块数据。

**修复方案：**
将敏感路由迁移至 `https_routes`，通过 `ensure_https` 中间件强制 TLS。仅保留 `/consensus/validator_count/:epoch`（纯公开元数据）在 HTTP 上。同步更新 `gravity_cli` DKG 子命令使用 HTTPS URL 访问这些端点。

---

### GSDK-004：Sentinel 探针 URL 的 SSRF 漏洞

**涉及文件：** `bin/sentinel/src/config.rs:14`、`bin/sentinel/src/probe.rs:34`

**问题描述：**
`sentinel.toml` 中的 `ProbeConfig.url` 字段直接传给 `reqwest::get()`，未校验 scheme、host 或 IP 范围。攻击者若取得配置文件的写权限（如通过 CI/CD 被入侵、文件权限过宽或共享配置管理），可将探针 URL 设为云元数据端点（`http://169.254.169.254/latest/meta-data/iam/security-credentials/`），借助 sentinel 的告警 webhook 窃取 IAM 角色凭证。

**修复方案：**
在 `Config::load()` 中加入 URL 校验逻辑：

1. 使用 `url::Url::parse()` 解析 URL
2. **Scheme 白名单：** 仅允许 `http` 和 `https`
3. **IP 范围黑名单：** 拒绝回环地址（`127.0.0.0/8`）、链路本地地址（`169.254.0.0/16`）和 RFC 1918 私有地址段（`10.0.0.0/8`、`172.16.0.0/12`、`192.168.0.0/16`）
4. 校验失败则拒绝启动（hard error）

---

### GSDK-005：GSDK-004 修复的 DNS 绕过漏洞

**涉及文件：** `bin/sentinel/src/config.rs:72`

**问题描述：**
GSDK-004 的修复仅检查了 IP 字面量。当 host 为 DNS 主机名时（例如 `metadata.google.internal` 或 `169.254.169.254.nip.io`），`host.parse::<IpAddr>()` 会失败，导致所有 IP 检查被绕过。此外 userinfo 前缀（如 `http://x@169.254.169.254/`）未被剥离，也可进一步绕过校验。

**修复方案：**
- 使用 `ToSocketAddrs` 对 DNS 主机名进行解析，对**所有**解析出的 IP 地址逐一进行黑名单检查
- 对无法解析的主机名采用 **fail-closed** 策略（直接拒绝）
- 在提取 host 之前先剥离 userinfo 部分

---

## 相关文档

- [安全修复设计方案](../plans/2026-02-23-gsdk-fixes-design.md)
- [安全审计报告（英文原版）](./2026-02-23-security-audit-report.md)
