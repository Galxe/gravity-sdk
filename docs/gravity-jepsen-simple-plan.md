# Gravity SDK 简版 Jepsen-style 测试方案

日期：2026-06-24

## 1. 目标

先做一个最小可用的 Jepsen-style / chaos 测试闭环，不接完整 Clojure Jepsen，不做复杂 Byzantine 场景。

第一阶段验证三件事：

1. 节点重启、停摆、长稳期间不会出现同高度不同 block hash。
2. 故障恢复后节点能追上并收敛到同一 canonical chain。
3. 已经确认的交易最终不会丢。

当前已经基本完成前两项；第三项的 P0 也已补上：可以用内置 EVM 转账 workload 持续发哨兵交易，在场景结束后复查已确认 receipt 是否仍在 canonical chain，并把 timeout/error/missing terminal event 这类 tx history 异常纳入场景判定。

## 2. 当前实现

当前实现落在：

```text
cluster/chaos/
  chaos.sh
  scenarios.sh
  oracle.sh
  snapshot.sh
  report.sh
  loop.sh
  lib/
    cluster.py
    net.sh
    tx_workload.py
    receipt_checker.py

docker/gravity_node/
  docker-compose.cluster-bridge.yaml
  render-cluster-bridge-config.sh
  cluster.bridge.toml
```

设计原则：

- 不 deploy、不 init、不 clean 数据目录。
- 节点启停只调用 `cluster/start.sh` 和 `cluster/stop.sh`。
- `majority-failure` 按 stake 计算，不按节点数计算。
- 单点分区和 split 拓扑选择也按 stake 计算：只有隔离后剩余 stake 仍有 >2/3 quorum 的 validator 才会被选为 single victim；no-quorum split 会验证两边都没有 quorum。
- 失败时可自动 snapshot，保留现场。
- 网络规则只进入 `GRAVITY_CHAOS` chain，`heal` 不全局 flush iptables。
- 支持 `CHAOS_BACKEND=local|docker`；macOS 本地建议优先用 Docker backend 做网络隔离。
- 当前附带的 Docker bridge topology 是 4 validator + 1 VFN 的本地示例；更大集群可提供自己的 compose/config，复用同一套 stake-aware 场景逻辑。
- Docker 分区测试的 RPC/oracle 默认走容器内网 `gravity-chaos`，不依赖 macOS host port 转发。

## 3. 已具备能力

### 3.1 基础操作

```bash
cluster/chaos/chaos.sh --config cluster/cluster.toml validators
cluster/chaos/chaos.sh --config cluster/cluster.toml majority-victims
cluster/chaos/chaos.sh --config cluster/cluster.toml snapshot --validators
cluster/chaos/chaos.sh --config cluster/cluster.toml kill node2
cluster/chaos/chaos.sh --config cluster/cluster.toml start node2
cluster/chaos/chaos.sh --config cluster/cluster.toml restart node2 30
```

### 3.2 Oracle

`oracle.sh` 当前检查：

- 进程是否存活。
- RPC 是否可达。
- 节点高度差是否在阈值内。
- validator 是否有超过 2/3 stake 在推进。
- 共同高度 block hash 是否一致。
- 共同高度 state root 是否一致。
- 日志中是否出现新的 panic 事件头或进程异常证据，例如 `thread ... panicked`、`panicked at`、`panic occurred`、`fatal`、`abort`、`segmentation fault`。

注意：`std::panic::catch_unwind` / `std::panicking::catch_unwind` 出现在 Rust stacktrace 里，本身不等同于 panic。Aptos/anyhow 的普通错误 backtrace 也可能包含这些 runtime frame；当前 oracle 不把这种单独的 stack frame 当作 panic fail。

示例：

```bash
cluster/chaos/oracle.sh --config cluster/cluster.toml --validators
```

Docker backend 下有两个额外处理：

- 自动 `--skip-process`，因为容器内进程 PID 不在宿主机 pid file 下。
- 默认 `CHAOS_DOCKER_RPC_NETWORK=gravity-chaos`，通过一次性 `curlimages/curl` 容器访问 `node1:8545`、`node2:8546` 等内部 RPC。
- panic/fatal/abort 扫描会进入节点容器读取 `/gravity/data/execution_logs/dev/reth.log` 和 `/gravity/data/consensus_log/validator.log`。默认只扫每个日志文件最后 `CHAOS_DOCKER_LOG_TAIL_LINES=2000` 行，避免旧日志误伤当前测试。

### 3.3 场景

当前支持：

```bash
cluster/chaos/scenarios.sh --config cluster/cluster.toml rolling-restart 30
cluster/chaos/scenarios.sh --config cluster/cluster.toml flap node2 10 10
cluster/chaos/scenarios.sh --config cluster/cluster.toml kill-under-load node2
cluster/chaos/scenarios.sh --config cluster/cluster.toml majority-failure
cluster/chaos/scenarios.sh --config cluster/cluster.toml partition-kill node2 node3
cluster/chaos/scenarios.sh --config cluster/cluster.toml partition-random 3 180 60
cluster/chaos/scenarios.sh --config cluster/cluster.toml partition-asym 2 180 60 random random
cluster/chaos/scenarios.sh --config cluster/cluster.toml partition-no-quorum-split 180
cluster/chaos/scenarios.sh --config cluster/cluster.toml partition-majority-minority 180 random
cluster/chaos/scenarios.sh --config cluster/cluster.toml partition-load random 180 60
cluster/chaos/scenarios.sh --config cluster/cluster.toml delay-load-spike node2 300 200ms 50ms 60
cluster/chaos/scenarios.sh --config cluster/cluster.toml partition-mixed 1800 180 60
cluster/chaos/scenarios.sh --config cluster/cluster.toml soak 21600 60
```

说明：

- `rolling-restart`: 逐个重启 validator，每次恢复后跑 oracle。
- `flap`: 对单节点反复 stop/start。
- `kill-under-load`: 支持复用已有 `gravity_bench`，也支持 `CHAOS_LOAD_CMD`、托管 `gravity_bench` 和内置 tx workload 同时运行。
- `majority-failure`: 停掉超过 1/3 stake，观察无 quorum 停摆，然后恢复。
- `partition-kill`: 隔离一个节点，再重启另一个节点，最后 heal 并检查恢复。
- `partition-random`: 随机隔离 1 个 quorum-safe validator，检查剩余 stake 继续推进，被隔离节点不应明显推进，然后 heal。
- `partition-asym`: 对单个 quorum-safe validator 做只断入站或只断出站的方向性分区，然后 heal 并检查恢复。
- `partition-no-quorum-split`: 把全部 validators 拆成两个都没有 quorum 的网络岛，检查两边都不能长期出块，然后 heal。
- `partition-majority-minority`: 把 validators 拆成有 quorum 的 majority side 和无 quorum 的 minority side，检查 majority 继续推进，minority 不应明显推进。
- `partition-2-2` / `partition-3-1`: 兼容旧命令名，分别映射到 no-quorum split 和 majority/minority split；4 validator 等权集群下行为仍近似 2-2 / 3-1。
- `partition-load`: 分区期间启动托管负载，支持 `single`、`no-quorum`、`majority-minority` 以及兼容模式 `2-2`、`3-1`，恢复后检查 oracle 和 tx receipt。
- `delay-load-spike`: 网络延迟期间启动托管负载，周期性跑 oracle，恢复后检查 receipt。
- `partition-mixed`: 在指定总时长内随机混合 `partition-random`、`partition-asym`、`partition-no-quorum-split` 和 `partition-majority-minority`。
- `soak`: 长稳测试，不注入故障，只周期性跑 oracle。

默认时间已经按“比 smoke 长一点”调整：

- `partition-random`: 默认 3 轮，每轮分区 180s，恢复等待 60s。
- `partition-no-quorum-split`: 默认分区 180s。
- `partition-majority-minority`: 默认分区 180s。
- `partition-asym`: 默认 2 轮，每轮分区 180s，恢复等待 60s。
- `partition-load`: 默认分区 180s，恢复等待 60s。
- `delay-load-spike`: 默认持续 300s，延迟 200ms，jitter 50ms，每 60s 跑一次 oracle。
- `partition-mixed`: 默认总时长 1800s，单次分区 180s，恢复等待 60s，混合 single / asym / no-quorum / majority-minority。

`partition-load` 如果启用了 `CHAOS_TX_ENABLE=1` 且没有显式设置 `CHAOS_TX_RECEIPT_TIMEOUT`，会自动把 receipt timeout 设为 `hold_s + recover_s + 60`，避免 no-quorum 分区期间提交的交易在 heal 前被过早判定为 timeout。

### 3.4 长稳测试

1 小时：

```bash
CHAOS_REPORT_FILE=/tmp/gravity-soak-1h.jsonl \
cluster/chaos/scenarios.sh --config cluster/cluster.toml soak 3600 60
```

6 小时：

```bash
CHAOS_REPORT_FILE=/tmp/gravity-soak-6h.jsonl \
cluster/chaos/scenarios.sh --config cluster/cluster.toml soak 21600 60
```

带自定义负载：

```bash
CHAOS_LOAD_CMD='gravity_bench --config /path/to/bench_config.toml' \
CHAOS_REPORT_FILE=/tmp/gravity-soak-6h.jsonl \
cluster/chaos/scenarios.sh --config cluster/cluster.toml soak 21600 60
```

托管 `gravity_bench` 背景压力：

```bash
CHAOS_BENCH_ENABLE=1 \
CHAOS_BENCH_CONFIG=/path/to/bench_config.toml \
CHAOS_REPORT_FILE=/tmp/gravity-soak-6h.jsonl \
cluster/chaos/scenarios.sh --config cluster/cluster.toml soak 21600 60
```

注意：`bench_config.toml` 中的 `performance.duration_secs` 应设为 `0` 或大于 chaos scenario 总时长；如果 `gravity_bench` 提前正常退出，scenario 会按背景压力异常退出处理。

推荐的正确性组合是：`gravity_bench` 负责背景压力，内置 `tx_workload` 负责低频可追踪哨兵交易，最后由 `receipt_checker` 做跨节点 receipt/canonicality 校验。

```bash
CHAOS_BENCH_ENABLE=1 \
CHAOS_BENCH_CONFIG=/path/to/bench_config.toml \
CHAOS_TX_ENABLE=1 \
CHAOS_TX_HISTORY_FILE=/tmp/gravity-chaos-history.jsonl \
CHAOS_REPORT_FILE=/tmp/gravity-soak-6h.jsonl \
cluster/chaos/scenarios.sh --config cluster/cluster.toml soak 21600 60
```

### 3.5 网络原语

当前有基础版：

```bash
cluster/chaos/chaos.sh --config cluster/cluster.toml partition node2
cluster/chaos/chaos.sh --config cluster/cluster.toml partition-asym node2 in
cluster/chaos/chaos.sh --config cluster/cluster.toml partition-asym node2 out
cluster/chaos/chaos.sh --config cluster/cluster.toml delay 200ms 50ms
cluster/chaos/chaos.sh --config cluster/cluster.toml loss 5%
cluster/chaos/chaos.sh --config cluster/cluster.toml throttle 1mbit
cluster/chaos/chaos.sh --config cluster/cluster.toml net-status
cluster/chaos/chaos.sh --config cluster/cluster.toml heal
```

注意：

- `delay/loss/throttle` 依赖 Linux `tc`。
- 本机 localhost 多节点下，`partition <node>` 和 `partition-asym <node> in|out` 主要按目标节点 listener port 做近似隔离。
- 更强隔离应放到 Docker network namespace 或多机 SSH 环境。
- `partition-split` 当前只支持 Docker backend，用于构造 no-quorum / majority-minority 这类真实网络岛。
- Docker `partition-asym` 通过容器内 iptables 实现，需要镜像内有 `iptables` 或 `iptables-legacy`，并且容器具备 `NET_ADMIN` 等权限。
- Docker `delay/loss/throttle` 和 `delay-load-spike` 依赖容器内 `tc`。`docker/gravity_node` 当前基础镜像未内置 `iptables/tc` 时，loop 默认不会随机选择 `partition-asym` / `delay-load-spike`；有这些工具的镜像可以设置 `CHAOS_DOCKER_ENABLE_NET_TOOLS_SCENARIOS=1` 或自定义 `LOOP_SCENARIO_WEIGHTS`。

Docker split 示例：

```bash
CHAOS_BACKEND=docker \
cluster/chaos/chaos.sh --config docker/gravity_node/cluster.bridge.toml \
  partition-split node1,node2 node3,node4

CHAOS_BACKEND=docker \
cluster/chaos/chaos.sh --config docker/gravity_node/cluster.bridge.toml \
  partition-asym node2 in
```

### 3.6 Docker backend

当前已支持 Docker backend。普通容器操作：

```bash
CHAOS_BACKEND=docker cluster/chaos/chaos.sh --config cluster/cluster.toml restart node2
CHAOS_BACKEND=docker cluster/chaos/chaos.sh --config cluster/cluster.toml partition node2
CHAOS_BACKEND=docker cluster/chaos/chaos.sh --config cluster/cluster.toml heal node2
CHAOS_BACKEND=docker cluster/chaos/chaos.sh --config cluster/cluster.toml net-status node2
CHAOS_BACKEND=docker cluster/chaos/scenarios.sh --config cluster/cluster.toml rolling-restart 30
```

Docker backend 映射：

- `kill/start/restart/hard-kill` -> `docker stop/start/restart/kill`
- `partition` -> `docker network disconnect`
- `partition-asym` -> `docker exec --user 0 <container> iptables ...`
- `heal` -> `docker network connect`
- `delay/loss/throttle` -> `docker exec --user 0 <container> tc ...`

容器名解析顺序：

1. node id，例如 `node2`
2. `CHAOS_DOCKER_PREFIX + node id`
3. compose 风格模糊匹配，例如 `gravity-node2-1`

注意：

- Docker backend 不负责创建容器。
- host network 容器不能用 `docker network disconnect` 做分区，脚本会直接报错，避免假分区。
- `delay/loss/throttle` 需要容器里有 `tc`，且容器具备 `NET_ADMIN` 等权限；否则先使用 `partition/heal`。

macOS 本地建议使用专门的 bridge topology：

```bash
cd docker/gravity_node
./render-cluster-bridge-config.sh
GRAVITY_IMAGE=gravity_node IMAGE_TAG=<your-tag> \
  docker compose -f docker-compose.cluster-bridge.yaml up -d
cd ../..
```

这个拓扑包含：

- `node1..node4`: 4 个 validator。
- `vfn1`: 1 个 VFN。
- Docker 网络：`gravity-chaos`。
- 对外发布 RPC 端口：`18545`、`18546`、`18547`、`18548`、`18550`。
- chaos/oracle 配置：`docker/gravity_node/cluster.bridge.toml`。

注意：分区测试过程中会 `docker network disconnect/connect`，Docker Desktop 可能让部分 macOS host-published RPC port 进入 reset 状态；这不代表容器内 RPC 异常。因此自动化检查统一走 Docker 内网 DNS：

```bash
CHAOS_BACKEND=docker \
cluster/chaos/oracle.sh --config docker/gravity_node/cluster.bridge.toml
```

Docker 分区场景：

```bash
# 随机隔离 1 个 validator；默认 3 轮、180s hold、60s recover。
CHAOS_BACKEND=docker \
cluster/chaos/scenarios.sh --config docker/gravity_node/cluster.bridge.toml \
  partition-random 3 180 60

# 非对称分区；随机选择 in/out 和 validator，默认 2 轮、180s hold、60s recover。
CHAOS_BACKEND=docker \
cluster/chaos/scenarios.sh --config docker/gravity_node/cluster.bridge.toml \
  partition-asym 2 180 60 random random

# no-quorum validator split；默认按 stake 自动选择两边都无 quorum 的分组，180s hold。
CHAOS_BACKEND=docker \
cluster/chaos/scenarios.sh --config docker/gravity_node/cluster.bridge.toml \
  partition-no-quorum-split 180

# majority/minority validator split；默认随机选择一个 quorum-safe minority side，180s hold。
CHAOS_BACKEND=docker \
cluster/chaos/scenarios.sh --config docker/gravity_node/cluster.bridge.toml \
  partition-majority-minority 180 random

# 分区期间叠加内置 tx 哨兵负载，恢复后检查 receipt。
CHAOS_BACKEND=docker \
CHAOS_TX_ENABLE=1 \
cluster/chaos/scenarios.sh --config docker/gravity_node/cluster.bridge.toml \
  partition-load majority-minority 180 60

# 网络延迟期间叠加负载，周期性跑 oracle。
CHAOS_BACKEND=docker \
CHAOS_TX_ENABLE=1 \
cluster/chaos/scenarios.sh --config docker/gravity_node/cluster.bridge.toml \
  delay-load-spike node2 300 200ms 50ms 60

# 混合长测；默认 1800s 总时长、180s hold、60s recover。
CHAOS_BACKEND=docker \
cluster/chaos/scenarios.sh --config docker/gravity_node/cluster.bridge.toml \
  partition-mixed 1800 180 60
```

### 3.7 Snapshot 和 Report

失败现场：

```bash
cluster/chaos/snapshot.sh --config cluster/cluster.toml --out /tmp/gravity-chaos-snapshot
```

报告汇总：

```bash
cluster/chaos/report.sh /tmp/gravity-chaos.jsonl
```

### 3.8 内置 tx workload 和 receipt checker

P0 基础版已实现：

```text
cluster/chaos/lib/tx_workload.py
cluster/chaos/lib/receipt_checker.py
```

`tx_workload.py` 能做的事：

- 从 `CHAOS_TX_PRIVATE_KEYS`、`CHAOS_TX_ACCOUNTS_FILE`、`GRAVITY_ARTIFACTS_DIR/accounts.csv`、`[faucet_init]` 或 `[genesis.faucet]` 读取 funded EVM private keys。
- 轮询 RPC 节点提交普通 EVM transfer。
- 记录 `tx_submit`、`tx_receipt`、`tx_timeout`、`tx_error` 等 JSONL history。
- workload stop 时写入 `status=pass/fail`；默认 `tx_error` 或 receipt timeout 会让 workload 进入 fail 状态。
- Docker backend 下复用 `CHAOS_DOCKER_RPC_NETWORK=gravity-chaos` 内网 RPC。

`receipt_checker.py` 能做的事：

- 读取 history 中所有 `tx_receipt`。
- 至少在两个节点上重新查询 receipt。
- 确认 receipt 仍存在。
- 确认 receipt 的 `blockHash` 没变。
- 确认该 block number 上的 canonical block hash 仍等于 receipt 的 `blockHash`。
- 审计 tx history：`tx_timeout`、`tx_error`、缺少 terminal event、缺少 `tx_workload_stop`、workload stop status 为 `fail` 默认都算硬失败。
- `tx_interrupted` 默认记为 `inconclusive`，用于区分测试结束时主动停止 workload 和真正的 receipt 丢失；如需严格失败可设置 `CHAOS_TX_FAIL_INTERRUPTED=1` 或 `CHAOS_TX_FAIL_ON_INCONCLUSIVE=1`。

直接运行：

```bash
CHAOS_BACKEND=docker CHAOS_DOCKER_RPC_NETWORK=gravity-chaos \
python3 cluster/chaos/lib/tx_workload.py \
  --config docker/gravity_node/cluster.bridge.toml \
  --history-file /tmp/gravity-chaos-history.jsonl \
  --max-txs 10

CHAOS_BACKEND=docker CHAOS_DOCKER_RPC_NETWORK=gravity-chaos \
python3 cluster/chaos/lib/receipt_checker.py \
  --config docker/gravity_node/cluster.bridge.toml \
  --history-file /tmp/gravity-chaos-history.jsonl \
  --require-txs
```

接入场景：

```bash
CHAOS_BACKEND=docker \
CHAOS_TX_ENABLE=1 \
CHAOS_TX_HISTORY_FILE=/tmp/gravity-chaos-history.jsonl \
cluster/chaos/scenarios.sh --config docker/gravity_node/cluster.bridge.toml \
  soak 3600 60

CHAOS_BACKEND=docker \
CHAOS_TX_ENABLE=1 \
CHAOS_TX_HISTORY_FILE=/tmp/gravity-chaos-history.jsonl \
cluster/chaos/scenarios.sh --config docker/gravity_node/cluster.bridge.toml \
  kill-under-load node2
```

托管 `gravity_bench` 和哨兵 tx 可以同时启用：

```bash
CHAOS_BACKEND=docker \
CHAOS_BENCH_ENABLE=1 \
CHAOS_BENCH_CONFIG=/path/to/bench_config.toml \
CHAOS_TX_ENABLE=1 \
CHAOS_TX_HISTORY_FILE=/tmp/gravity-chaos-history.jsonl \
cluster/chaos/scenarios.sh --config docker/gravity_node/cluster.bridge.toml \
  soak 21600 60
```

这里不建议让 `gravity_bench` 直接替代 `tx_workload`：`gravity_bench` 的优势是高 TPS 背景压力，而 `tx_workload` 的目标是生成可完整追踪的 Jepsen-style history。后续如果给 `gravity_bench` 增加逐 tx history export，可以复用同一个 `receipt_checker`，但 checker/oracle 层仍应保留。

## 4. 已验证情况

已做过基础验证：

```bash
bash -n cluster/chaos/chaos.sh
bash -n cluster/chaos/oracle.sh
bash -n cluster/chaos/scenarios.sh
bash -n cluster/chaos/snapshot.sh
bash -n cluster/chaos/loop.sh
bash -n cluster/chaos/report.sh
bash -n cluster/chaos/lib/net.sh
python3 -m py_compile \
  cluster/chaos/lib/cluster.py \
  cluster/chaos/lib/tx_workload.py \
  cluster/chaos/lib/receipt_checker.py
```

P0 收口后新增的非破坏验证：

```text
synthetic tx_timeout history:
status=fail pass=false
history_failures=["1 tx_timeout events", "tx workload stop status is fail"]

synthetic tx_interrupted-only history with --require-txs:
status=inconclusive pass=true
history_warnings=["1 tx_interrupted events"]

report.sh:
inconclusive 会单独展示，不混入硬失败列表。
```

在 `/tmp/gravity-cluster` 本地集群上，`oracle.sh --validators` 已通过：

- 5 个 validator 进程存活。
- RPC 可达。
- 高度持续推进。
- 高度差在阈值内。
- 共同高度 block hash 一致。
- 共同高度 state root 一致。

也已启动过 1 小时 `soak` 后台任务，第一轮 oracle 通过。

Docker bridge topology 上已验证：

```bash
python3 -m py_compile cluster/chaos/lib/cluster.py
bash -n cluster/chaos/scenarios.sh cluster/chaos/oracle.sh cluster/chaos/chaos.sh cluster/chaos/lib/docker.sh
```

收尾 oracle 通过：

- RPC 可达。
- 高度差在阈值内。
- 4 个 validator 都在推进。
- 共同高度 block hash 一致。
- 共同高度 state root 一致。
- panic/fatal/abort 扫描通过，实际扫描到 9 个容器日志文件。

已跑过两个 Docker 分区场景：

```text
partition-2-2 180: pass
report: /tmp/gravity-chaos-partition-2-2-20260623-154449.jsonl

partition-random 1 180 30: pass
victim: node2
partitioned height delta: 1
report: /tmp/gravity-chaos-partition-random-20260623-155013.jsonl
```

2-2 分区期间观测到：

```text
node1 delta=8
node2 delta=6
node3 delta=5
node4 delta=3
```

均小于默认 `MAX_PARTITION_ADVANCE=10`，heal 后 oracle 通过。测试结束后临时 `gravity-chaos-partition-*` 网络已清理，只保留 `gravity-chaos`。

P0 tx workload / receipt checker 已做过 Docker bridge smoke：

```text
tx_workload --max-txs 2:
sent=2 confirmed=2 failed=0 timeout=0

receipt_checker:
checked=2 failed=0 min_nodes=2 pass=true

CHAOS_TX_ENABLE=1 soak 6 2:
tx_submit=4 tx_receipt=4
receipt_checker checked=4 failed=0 pass=true
```

## 5. 还缺什么

### 5.1 内置 tx workload 与背景压力

P0 已经实现。当前 `kill-under-load` 和 `soak` 可以通过 `CHAOS_TX_ENABLE=1` 自动启动内置 tx worker；也支持通过 `CHAOS_BENCH_ENABLE=1` 托管 `gravity_bench`，或通过 `CHAOS_LOAD_CMD` 调任意外部负载。

已支持：

- 从 private keys / accounts.csv / faucet 配置加载账户。
- 每账户 nonce cache，失败后刷新 pending nonce。
- 轮询 RPC 节点提交普通 EVM 转账。
- 记录 `tx_hash`、提交节点、receipt、block number、block hash。
- `tx_error`、`tx_timeout` 默认让 workload stop status 变为 `fail`。
- 通过 `CHAOS_TX_INTERVAL` 控制发送间隔。
- `gravity_bench` 和内置 tx workload 可以同时运行：前者负责压力，后者负责可追踪正确性样本。
- scenario 会检测 `CHAOS_LOAD_CMD`、托管 `gravity_bench`、内置 tx workload 是否提前退出。

后续还需要增强：

- 并发 worker 数和更高 TPS。
- 自动生成并预充值多账户，而不只读取已有 funded accounts。
- 更完整的 latency 分位数统计。
- 自动从 cluster config 生成最小 `gravity_bench` 配置，降低 Docker bridge 场景的手工配置成本。

### 5.2 Receipt persistence checker

P0 已经实现。场景结束后可以对所有已确认 receipt 做 canonical chain 复查，也会审计 tx history 的失败语义。

已支持：

- 对所有已经成功 receipt 的交易，测试结束后重新查询 receipt。
- 至少从两个节点查询。
- receipt 必须存在。
- receipt 所属 block 必须仍在 canonical chain。
- receipt 的 `blockHash` 不能被替换。
- `tx_timeout`、`tx_error`、缺少 terminal event、缺少 `tx_workload_stop`、workload stop status 为 `fail` 都是硬失败。
- `tx_interrupted` 默认标记为 `inconclusive`，可通过 `CHAOS_TX_FAIL_INTERRUPTED=1` 或 `CHAOS_TX_FAIL_ON_INCONCLUSIVE=1` 提升为失败。

后续还需要增强：

- 支持按场景阶段分组统计 receipt latency。
- 与统一 history/report 更紧密集成。
- 如果接入 `gravity_bench` 逐 tx history export，则让同一个 checker 消费 bench history。

### 5.3 History 统一格式

当前 report 是 scenario-level JSONL，还不是 Jepsen-style operation history。

需要补统一 history：

```json
{"ts": 1710000000.1, "type": "head", "node": "node1", "height": 100, "hash": "0x..."}
{"ts": 1710000001.2, "type": "tx_ok", "node": "node2", "tx_hash": "0x...", "block_number": 101}
{"ts": 1710000030.0, "type": "nemesis", "action": "restart", "node": "node4"}
{"ts": 1710000060.0, "type": "oracle", "result": "pass"}
```

建议新增：

```text
cluster/chaos/history/
```

或者统一写入：

```text
CHAOS_HISTORY_FILE=/tmp/gravity-chaos-history.jsonl
```

### 5.4 Panic/log checker 增强

当前 oracle 已经会扫描：

- `debug.log`
- `consensus_log/*.log`
- `execution_logs/**/reth.log`

并识别：

- `panic`
- `thread ... panicked`
- `fatal`
- `abort`
- `segmentation fault`

场景脚本会用测试开始时间作为 `LOG_SINCE`，避免旧日志误伤。

Docker backend 下已经补充容器内日志扫描：

- `/gravity/data/execution_logs/dev/reth.log`
- `/gravity/data/consensus_log/validator.log`
- 最近 `CHAOS_DOCKER_LOG_TAIL_LINES=2000` 行

后续可增强：

- 支持更精确的日志时间解析，而不只依赖文件 mtime。
- 增加 allowlist，避免已知无害 fatal 字样误报。
- 把 panic match 附近上下文也写入 snapshot。

### 5.5 更强网络隔离

当前已经具备 Docker bridge 网络隔离，能够覆盖随机单节点分区和 stake-aware no-quorum split。local backend 仍然只是 localhost 端口级近似隔离。

后续可增强：

- 多机 SSH 执行 `chaos.sh partition/heal`。
- 明确区分 validator network、VFN network、PFN network。
- `3/1` 多数/少数分区。
- 更长时间的 `partition-mixed` 夜间测试。

### 5.6 Feishu 告警

当前没有 Feishu webhook。

需要补：

- `FEISHU_WEBHOOK_URL`
- 场景失败时发送告警。
- soak 长稳完成时发送 summary。
- snapshot 路径、report 路径进入消息。

### 5.7 结合原飞书设计的补充差距

原飞书文档《Gravity Node 无-K8s 混沌工具集设计》比当前 simple 版更偏“可长跑、可自动化、可定位问题”的完整工具集。对照后，当前还需要补这些能力：

#### P0: 交易闭环

P0 已经落地：`kill-under-load` 和 `soak` 可以通过 `CHAOS_TX_ENABLE=1` 启动内置 tx workload，并在结束时运行 receipt checker；也可以通过 `CHAOS_BENCH_ENABLE=1` 托管 `gravity_bench` 做背景压力。

已具备：

- 内置 tx workload，持续提交普通 EVM 转账。
- 记录提交节点、`tx_hash`、receipt、block number、block hash。
- 故障恢复后重新查询所有已成功 receipt 的交易。
- receipt 必须仍存在，且所属 block 仍在 canonical chain。
- receipt 的 `blockHash` 不能被替换。
- tx history 失败语义：timeout、error、missing terminal、workload fail status 会让 checker 失败；interrupted 可记为 inconclusive。
- managed background pressure：scenario 可同时启动 `gravity_bench` 和 tx workload，并检测后台进程是否提前退出。

后续增强重点是并发 worker、账户自动预充值、latency 分位数、自动生成 bench config，以及让 `gravity_bench` 可选输出逐 tx history。

#### P1: 更完整的网络故障

P1 基础版已经落地。当前 Docker bridge 除了随机单节点隔离和 no-quorum split，还补了 majority/minority split、非对称分区、分区叠加负载、延迟叠加负载。

已具备：

- `partition-asym <node> in|out`: 非对称分区，只断入站或只断出站。
- `partition-no-quorum-split`: 自动按 stake 选择两个都没有 quorum 的分区，旧 `partition-2-2` 是兼容 alias。
- `partition-majority-minority`: 自动按 stake 选择有 quorum 的 majority side 和无 quorum 的 minority side，旧 `partition-3-1` 是兼容 alias。
- `partition-load`: 分区期间持续打托管负载，恢复后检查 receipt 和 canonical chain。
- `delay-load-spike`: 网络延迟期间叠加托管负载，周期性跑 oracle。
- Docker `partition-asym` 使用容器内 iptables；Docker `delay/loss/throttle` 使用容器内 `tc`。
- `partition-mixed` 已升级为混合 single / asym / no-quorum / majority-minority。
- 当前 `docker/gravity_node` 基础镜像没有 `iptables/tc` 时，Docker loop 默认只选择不依赖容器 net tools 的场景；需要方向性分区和 delay 场景时，应换带 net tools + `NET_ADMIN` 的镜像，并设置 `CHAOS_DOCKER_ENABLE_NET_TOOLS_SCENARIOS=1`。

后续增强：

- localhost 端口级 `tc`: 使用 `iptables MARK + tc filter fw`，避免整机 `lo` 被拖慢。
- 明确拆分 validator network / VFN network / PFN network 的故障矩阵。
- 增加 `loss+load-spike`、`throttle+load-spike`、`delay+partition+load` 这类复合场景。
- 自动检查 Docker 镜像是否具备 `iptables`、`tc`、`NET_ADMIN`，并在 report 中输出环境缺口。

#### P1: 长跑 loop 产品化

P1 基础版已经落地。暂时不接 Feishu 告警，先把长跑 loop 的可配置、可复盘、失败冻结能力补齐。

已具备：

- 场景权重可配置，例如 kill / delay / partition / composite 的比例。
- pause-on-failure 时明确冻结现场：保留分区和故障状态，不自动 heal。
- 每轮写更完整的结构化 JSON：scenario、target、fault、duration、assertions、snapshot_path。
- 长稳完成时输出 summary，包括总轮次、pass rate、失败分布、round duration 分位数。
- dry-run 模式：可以验证随机选择和 report 输出，不触碰集群。
- `report.sh` 可以汇总 loop pass rate、duration p50/p95 和 selected scenario 分布。
- scenario 会额外写 `result=phase` 行，记录 `heal`、`recovery-oracle`、`receipt-check` 等关键阶段的耗时和 pass/fail 状态。
- `LOOP_SUMMARY_FILE` 可以输出单独 summary JSON，便于 CI 或后续脚本消费。

示例：

```bash
LOOP_DRY_RUN=1 LOOP_MAX_ROUNDS=5 LOOP_INTERVAL=0 \
LOOP_SCENARIO_WEIGHTS='rolling-restart=1,partition-asym=1,delay-load-spike=1' \
cluster/chaos/loop.sh --config docker/gravity_node/cluster.bridge.toml

CHAOS_BACKEND=docker \
CHAOS_TX_ENABLE=1 \
LOOP_SCENARIO_WEIGHTS='partition-random=10,partition-no-quorum-split=10,partition-majority-minority=10,partition-load=20' \
LOOP_MAX_ROUNDS=20 \
LOOP_INTERVAL=120 \
cluster/chaos/loop.sh --config docker/gravity_node/cluster.bridge.toml

LOOP_SUMMARY_FILE=/tmp/gravity-loop-summary.json \
LOOP_MAX_ROUNDS=10 \
cluster/chaos/loop.sh --config docker/gravity_node/cluster.bridge.toml
```

后续增强：

- Feishu webhook 告警。
- 更细的故障 hold / load startup / load shutdown 统计。

#### P2: Snapshot 增强

当前 snapshot 已包含 RPC、oracle、net-status、节点元数据、日志 tail、FD 计数。原设计还建议补：

- inspection_port metrics: 当前 round、是否等待 vote/QC、是否卡在 pacemaker timeout。
- RSS、FD、磁盘占用、reth datadir、consensus storage。
- panic/fatal 命中附近上下文，而不仅是单行匹配。
- Docker backend 下采集容器内 `/proc`、日志、网络状态。
- 快照里明确记录当前未 heal 的故障状态，方便复现。

#### P2: 报告和 Jepsen-style history

当前 report 是 scenario-level JSONL 汇总，还不能完整复盘一次 chaos run 的事件序列。

需要补统一 history：

- `head`: 每次高度/hash/stateRoot 采样。
- `tx_submit`: 交易提交事件。
- `tx_receipt`: receipt 确认事件。
- `nemesis`: 故障注入、heal、restart、kill 等事件。
- `oracle`: 每次检查结果。
- `snapshot`: 失败现场路径。

这样后续才方便做更像 Jepsen 的 checker 和离线分析。

## 6. 下一步优先级

建议按这个顺序继续：

1. **P0 收口: tx workload + receipt checker + bench 背景压力**
   已完成基础闭环。下一步是补并发 worker、账户自动预充值、latency 统计和自动 bench config。

2. **P1: 网络故障补全**
   基础版已完成：`partition-asym`、stake-aware no-quorum split、stake-aware majority/minority split、`delay-load-spike`、`partition-load` 已接入。Docker bridge 默认长跑优先使用不依赖容器 net tools 的场景；`partition-asym` / `delay-load-spike` 需要带 `iptables/tc` 的镜像再显式开启。下一步是环境能力预检、loss/throttle 组合和网络矩阵化。

3. **P1: loop 长跑产品化**
   基础版已完成：可配置场景权重、失败冻结、结构化 round JSON、summary 和 dry-run 已接入。Feishu webhook 暂缓，下一步是更细的恢复耗时拆分和单独 summary 导出。

4. **P2: snapshot 增强**
   增加 inspection metrics、RSS/FD/磁盘、panic 上下文和 Docker 容器内资源采集。

5. **P2: history JSONL / 报告增强**
   把 head、tx、nemesis、oracle、snapshot 写成统一 history，支持离线复盘和趋势分析。

6. **P2: 多机 SSH 和网络矩阵**
   Docker bridge 下的本机强隔离已具备；下一步是多机 SSH 和 validator/VFN/PFN 网络矩阵。

## 7. 当前结论

当前 `cluster/chaos` 已经能作为第一阶段 chaos/oracle 工具使用，覆盖：

- 节点重启恢复。
- 多数/少数 stake 停摆恢复。
- 长稳 no-fork / state-root 一致性。
- Docker bridge 随机单节点分区。
- Docker bridge stake-aware no-quorum validator split。
- Docker bridge stake-aware majority/minority validator split。
- 非对称入站/出站分区。
- 分区叠加托管负载。
- 延迟叠加托管负载。
- `gravity_bench` 背景压力 + 内置 tx 哨兵交易 + receipt checker 的 P0 组合。
- 失败 snapshot 和 report。

但如果严格按“简版 Jepsen-style”目标衡量，还需要补：

- tx workload 并发增强、自动预充值、latency 统计和自动 bench config。
- 网络故障环境能力预检、loss/throttle 组合、更大 Docker topology 生成和网络矩阵化。
- loop 长跑 Feishu 告警、更细恢复耗时拆分和 summary 导出。
- snapshot 诊断信息增强。
- 统一 operation history。

补完这些后，simple 版才算从“能注入故障并判断恢复”升级为“能长跑、能告警、能复盘、能验证交易语义”的完整第一阶段工具。
