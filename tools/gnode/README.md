# gnode —— Gravity 集群端到端攻击/PoC 工具

薄包装 CLI，站在 `gravity-sdk/cluster`（集群编排脚本）+ `gravity_e2e`（web3 工具）之上，
让人 / AI 审计 agent 能「拉起真集群 → 部署合约、发交易、打攻击、抓日志」，把已审出的漏洞
变成一键复现的 `gnode attack`。供 Coacker 审计 agent 在沙箱里 shell 调用。

## 依赖 / 环境
- 已构建 `gravity_node` / `gravity_cli`（`target/quick-release/`）；缺失时 `gnode up` 会自动 cargo build。
- `just`（`~/.cargo/bin`）、`forge`（`~/.foundry/bin`）—— gnode 启动器自动补 PATH。
- Python：`web3>=7`、`eth-account>=0.13.6`（7702）。
- **本地 RPC 必须绕过 http_proxy** —— gnode 自动设 `NO_PROXY=127.0.0.1,localhost`。

## 集群档位（preset）
gnode 自持配置在 `presets/<name>/`，base_dir / 端口独立，避免与他人共享 `/tmp` 冲突：

| preset  | RPC 端口 | base_dir            | EIP-7702 |
|---------|---------|---------------------|----------|
| `1node` | 8645    | `/tmp/gnode-1node`  | 否       |
| `prague`| 8647    | `/tmp/gnode-prague` | 是（7702-halt 必需）|

### 多实例并发隔离（instance）
并发审计时,多个 agent 各自起集群会抢同一个 base_dir/端口。用 `--instance N`(或环境变量
`GNODE_INSTANCE`)隔离:instance N 的所有端口 = 基准 + N*100、base_dir 带 `-N` 后缀。**所有 instance
共享同一份 genesis+identity**(单验证者节点不校验对端口,实测可行),所以只有 instance 首次不存在时
才 deploy+start,genesis 只生成一次 → 每个 instance 都很快。

- `gnode up --preset prague`(不带 --instance 且未设 GNODE_INSTANCE)→ **自动分配**空闲实例,打印 `inst=N`。
- 之后对该实例的所有命令带 `--instance N`(或让 Coacker 给每个 agent 预设 `GNODE_INSTANCE`)。
- `gnode status`(不带参数)列出所有 preset 下**已存在 base_dir 的实例**及其存活/区块。
- 攻击某个 instance 只打停它自己,不影响其它 instance(已验证)。

## 命令
```bash
gnode up   --preset 1node|prague [--fresh]   # 拉起集群（--fresh 重新 genesis）
gnode down --preset prague                    # 停止
gnode status --preset prague                  # 存活 / RPC / 区块高度
gnode logs --preset prague --which reth|consensus|debug [-f] [-n N]
gnode state <addr> --preset prague            # balance/nonce/code + halt 探测
gnode deploy <artifact.json> [--args '[..]'] --preset prague  # 部署合约（{abi,bytecode} 或纯 bytecode；--args 传构造参数，需 abi）
gnode send <tx.json> [--no-wait] --preset prague              # 发交易；tx.json 见 `gnode send --help`
                                                              # 支持 type 0/1/2/4（含 7702 自动签名授权）、raw、gasPrice、accessList
gnode attack <scenario> --preset prague       # 跑攻击场景
gnode scenarios                               # 列出内置场景
```

`status` 不带 `--preset` 时列出**所有**档位状态（避免默认档位让人误以为集群没起）。

判定：`attack` 自动区分 **halt**（出块停摆）/ **panic**（进程崩）/ **revert**（交易回滚但链活）/
**alive**（无恙）/ **inconclusive**（注入未同块按序，竞争未成形，应重试）/ **error**（环境未就绪等）。

**退出码约定**（agent/脚本可据此判定）：
| 退出码 | 含义 |
|-------|------|
| 3 | 命中：链停摆（halt / panic）|
| 0 | 链存活（alive / revert）/ 一般成功 |
| 2 | 结论不成立（attack inconclusive）/ 命令用法错误（argparse）/ state 集群不可达 |
| 1 | 出错（error verdict、非法输入、未捕获异常等）|

任何子命令遇到异常都会打印一行 `[gnode] error: ...` 到 stderr 并退出码 1，不会甩 Python traceback。

## 内置攻击场景
- **7702-halt**（`--preset prague`）：EOA 委托到含 CREATE 的合约，跨发送者同块 nonce 竞争
  （faucet→A 触发委托码 CREATE 抬升 A.nonce，令 A 自己 nonce=N 的在途交易在执行时 NonceTooLow），
  打停执行 pipe。已确认在 gravity-reth rev e75679c 上真链停
  （panic @ `pipe-exec-layer-ext-v2/execute/src/lib.rs:1328`，`NonceTooLow{tx,state}`）。

新增场景：在 `gnodelib/scenarios/` 加模块并在 `registry.py` 注册。

## Coacker 集成
- 沙箱 allowlist 默认含 `gnode`（`packages/backend/src/sandbox.ts`）。
- `gnode` 已软链到 `~/.local/bin/gnode`（在 PATH 上），沙箱 `shell:false` 可直接 spawn。
- attacker（红队）prompt 里已告知 agent 可用 gnode 做 execution/consensus 层端到端复现
  （`packages/brain/src/audit/prompts.ts` 的 `ATTACKER_GNODE`）。
