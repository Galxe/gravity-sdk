# Gravity 新验证器节点加入指南（Validator 模式直接部署）

本文档介绍如何将一个新节点以 **validator 角色**直接部署并加入已运行的 Gravity 网络。

> [!NOTE]
> 与 `testnet_vfn.md`（先以 VFN 启动再升级）不同，本文档直接以 validator 身份部署，无需后续修改配置。

## 前置条件

1. **已运行的 Gravity 网络**：至少有 3 个 Genesis 验证器节点正在运行

   | 节点 ID | 服务器 | IP |
   |---------|--------|------|
   | node1 | gravity-testnet-node-oregon-0 | 34.83.28.182 |
   | node2 | gravity-testnet-node-oregon-1 | 34.83.9.159 |
   | node3 | gravity-testnet-node-losangeles | 34.94.164.9 |

2. **新节点服务器**：已安装依赖并编译好二进制（参见 `testnet_build.md` 环境准备）
3. **编译二进制文件**：
   ```bash
   RUSTFLAGS="--cfg tokio_unstable" cargo build --profile quick-release -p gravity_node -p gravity_cli
   ```
4. **网络连通性**：新节点可以访问现有节点的 P2P 端口（默认 6180/6190）
5. **资金账户**：用于质押的 EVM 账户（需要足够余额支付 gas + 质押金额）

---

## 第一部分：部署并启动节点

### 1. 获取 Genesis 文件

todo

### 2. 配置 cluster.toml

在新节点的 `gravity-sdk/cluster/` 目录下创建 `cluster.toml`：

```toml
[cluster]
name = "my-validator"
base_dir = "/home/gravity/gravity-testnet"

[build]
binary_path = "../target/quick-release/gravity_node"

[genesis_source]
genesis_path = "./output/genesis.json"
waypoint_path = "./output/waypoint.txt"

[[nodes]]
id = "my-node"
role = "validator"
host = "<YOUR_IP>"                  # 节点 IP（其他验证器可达的地址）
p2p_port = 6180
vfn_port = 6190
rpc_port = 8545
metrics_port = 9001
inspection_port = 10000
https_port = 1024
authrpc_port = 8551
reth_p2p_port = 12024
```

> [!IMPORTANT]
> `role` 为 `validator`（直接以验证器身份部署）。deploy 会生成 `validator.yaml` 配置，包含 `validator_network` 段。

### 3. 初始化并部署

```bash
cd gravity-sdk/cluster
make init      # 生成 identity（consensus_public_key + network_public_key）
make deploy    # 生成运行配置和启动脚本
```

部署后目录结构：

```
/home/gravity/gravity-testnet/
├── genesis.json
├── gravity_node
├── gravity_cli
└── my-node/
    ├── config/
    │   ├── validator.yaml          # 验证器配置（含 validator_network）
    │   ├── identity.yaml
    │   ├── reth_config.json
    │   ├── relayer_config.json
    │   └── waypoint.txt
    ├── script/
    │   ├── start.sh
    │   └── stop.sh
    ├── data/
    ├── logs/
    ├── execution_logs/
    └── consensus_log/
```

### 4. 启动节点

```bash
cd /home/gravity/gravity-testnet/my-node/script
./start.sh
```

### 5. 确认节点运行

```bash
# 检查进程
ps aux | grep gravity_node

# 查看日志（应能看到同步区块）
tail -f /home/gravity/gravity-testnet/my-node/execution_logs/<chain_id>/reth.log
```

节点启动后会自动从网络同步数据。

---

## 第二部分：加入验证器集群

> [!TIP]
> 发送 join 交易**无需等待同步完成**，可以在节点刚启动后立即执行。

### 1. 获取公钥信息

从 identity 文件中获取两个公钥：

```bash
cat /home/gravity/gravity-testnet/my-node/config/identity.yaml
```

记录以下字段（不需要加 `0x` 前缀）：
- `consensus_public_key`
- `network_public_key`

### 2. 创建 StakePool

```bash
./gravity_cli stake create \
  --rpc-url http://<EXISTING_NODE_IP>:8545 \
  --private-key <YOUR_PRIVATE_KEY> \
  --stake-amount <AMOUNT_IN_ETH>
```

成功后会输出 `Pool address`，记录下来用于下一步。

**查询已有的 StakePool**：

```bash
./gravity_cli stake get \
  --rpc-url http://<EXISTING_NODE_IP>:8545 \
  --owner <YOUR_WALLET_ADDRESS>
```

### 3. 发送 Validator Join 交易

```bash
./gravity_cli validator join \
  --rpc-url http://<EXISTING_NODE_IP>:8545 \
  --private-key <YOUR_PRIVATE_KEY> \
  --stake-pool <STAKE_POOL_ADDRESS> \
  --consensus-public-key "<CONSENSUS_PUBLIC_KEY>" \
  --network-public-key "<NETWORK_PUBLIC_KEY>" \
  --validator-network-address "/ip4/<YOUR_IP>/tcp/6180" \
  --fullnode-network-address "/ip4/<YOUR_IP>/tcp/6190" \
  --moniker "<MY_VALIDATOR_NAME>"
```

**参数说明**：

| 参数 | 说明 |
|------|------|
| `--rpc-url` | 任意一个已运行节点的 RPC 地址 |
| `--private-key` | 有余额的 EVM 账户私钥（带 `0x` 前缀） |
| `--stake-pool` | 已创建的 StakePool 地址（通过 `stake create` 创建） |
| `--consensus-public-key` | 来自 identity.yaml，**不加 `0x` 前缀** |
| `--network-public-key` | 来自 identity.yaml，64 位 hex（32 字节） |
| `--validator-network-address` | P2P 地址，格式 `/ip4/{IP}/tcp/{port}`，CLI 自动拼接 noise-ik |
| `--fullnode-network-address` | VFN 地址，格式 `/ip4/{IP}/tcp/{port}`，CLI 自动拼接 noise-ik |
| `--moniker` | 验证器名称（最长 31 字节） |

> [!NOTE]
> CLI 会自动执行两步操作：**注册验证器** → **加入验证器集合**。如果已注册则跳过注册步骤。

> [!CAUTION]
> - network address 中的 IP **必须**填写其他节点可达的地址（跨 VPC 用外网 IP）
> - `--stake-pool` 必须是已创建且有足够质押的 StakePool 地址

### 4. 验证状态

```bash
./gravity_cli validator list --rpc-url http://34.83.28.182:8545
```

成功后你的节点会出现在 `pending_active` 列表中，**下一个 epoch** 切换后自动变为 `ACTIVE`。

切换到 `ACTIVE` 后，可以通过共识层日志确认是否正在出块：

```bash
grep "send the block" consensus_log/validator.log
```

---

## 验证器退出

```bash
./gravity_cli validator leave \
  --rpc-url http://<EXISTING_NODE_IP>:8545 \
  --private-key <YOUR_PRIVATE_KEY> \
  --stake-pool <YOUR_STAKE_POOL_ADDRESS>
```

退出后节点变为 `PENDING_INACTIVE`，下一个 epoch 后变为 `INACTIVE`。

---

## VFN 模式 vs Validator 模式对比

| | VFN 模式 (`testnet_vfn.md`) | Validator 模式（本文档） |
|---|---|---|
| cluster.toml `role` | `vfn` | `validator` |
| 生成的配置文件 | `validator_full_node.yaml` | `validator.yaml` |
| `base.role` | `full_node` | `validator` |
| `validator_network` | ❌ 无 | ✅ 有 |
| join 后需修改配置 | ✅ 需手动添加 `validator_network`，改 role | ❌ 无需修改 |
| 适用场景 | 先同步再加入 | 直接以验证器身份启动 |

---

## 故障排查

| 问题 | 解决方案 |
|------|----------|
| 节点无法同步 | 检查 genesis.json 和 waypoint.txt 是否与网络一致 |
| validator join 卡在 "Registering validator" | 等待 60 秒超时后用 `--stake-pool` 重试 |
| `insufficient funds` 错误 | `--stake-amount` 单位是 ETH 不是 wei，检查数值是否过大 |
| 一直卡在 `pending_active` | `--stake-amount` 超过了 `votingPowerIncreaseLimitPct` 限制。公式：`maxIncrease = totalVotingPower × limitPct%`。例如当前总 3000、限制 20% 时最多只能加 600。需 leave 后以更小金额重新 join |
| 链上显示 `YOUR_PUBLIC_IP` | network address 中用了占位符，目前需要 leave → 重新 register + join |



