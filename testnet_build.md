# Gravity 测试网部署指南

本文档介绍如何从零部署 Gravity 测试网集群（3 个 Genesis 验证器节点）。

## 网络架构

| 角色 | 节点 ID | 服务器 | 内网 IP |
|------|---------|--------|---------|
| 验证器1 | node1 | gravity-testnet-node-oregon-0 | 34.83.28.182 |
| 验证器2 | node2 | gravity-testnet-node-oregon-1 | 34.83.9.159 |
| 验证器3 | node3 | gravity-testnet-node-losangeles | 34.94.164.9 |

| 角色 | 地址 | 初始余额 | 质押金额 |
|------|------|----------|----------|
| V1 | `0xAEd2a948892475F800A337427B3275D190EA3e94` | 1,000,000 G | 1,000,000 G |
| V2 | `0x7b254Bd44F6CE45e00a912b2460D47F3Be56fAD7` | 1,000,000 G | 1,000,000 G |
| V3 | `0x9B2C25E77a97d3e84DC0Cb7F83fb676ddC4F24b9` | 1,000,000 G | 1,000,000 G |
| Faucet | `0x18c23753385ce7A60B15d171302E48b6AFf0BDC5` | 10,000 亿 G | - |

**管理员配置**: 7,000 G

EVM 账户密钥：

| 角色 | Private Key | Address |
|------|-------------|---------|
| V1 | `047a5466f6f9e08c8bcc56213d6530d517c1ef126eefbbdf85ffe8d893ed0e9f` | `0xAEd2a948892475F800A337427B3275D190EA3e94` |
| V2 | `8e52c723ea6bd1f66d8c4935c316d9560836381a49318158fcbb05f8533be16e` | `0x7b254Bd44F6CE45e00a912b2460D47F3Be56fAD7` |
| V3 | `5c173b12be434289682782ac6f7e7bf73a6fa5a20d507e318a4bdb039b1a5f6e` | `0x9B2C25E77a97d3e84DC0Cb7F83fb676ddC4F24b9` |
| Faucet | `b4034281871a4e42d7b99d8cbe61bb0d2fe137fb95871558643235321a8afafc` | `0x18c23753385ce7A60B15d171302E48b6AFf0BDC5` |

<details>
<summary>完整 Public Keys</summary>

- **V1**: `9b0c87243a51b9329d96c05e2b91178b06b666c60b851c2d66f33f9dbe2fd9dfcd0719659303a3e0a735e114a541bc5e21e13104f5646eba63655024d994b62c`
- **V2**: `6fdb915ca82f037c636066bd46753d6c6305ec5dda863ee8483eb18ea38c9bb7e2ce12b62047c735fc419195458584d62c3d41270f9445930d53145fb7f7ae1d`
- **V3**: `6aca8c28525c25c92e32cd0f874ce04fbd0579fda67d774a2e618c52cf2a07bdc7520ee9a1342b883b54be5afea95c2f6cda7cbcf84b79be474d279ae6d3bd05`
- **Faucet**: `ff9cd31d43fa1bc83256bbec4a6dc1e6ea29852076a3fa23c767405351c1a10afb03e747e0a78c43845122963e681e854a1fdd30f6ddd46bb49cf3ed3d3e08ad`

</details>

---

## 环境准备

系统依赖（Ubuntu/Debian）：

```bash
apt-get install -y --no-install-recommends \
    clang llvm build-essential pkg-config libssl-dev libudev-dev \
    procps git jq curl python3 python3-pip python3-venv \
    nodejs npm protobuf-compiler bc gettext-base
```

工具链：
- **Rust**: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **Foundry**: `curl -L https://foundry.paradigm.xyz | bash && foundryup`

---

## 部署步骤

### 1. 克隆代码并编译

```bash
git clone https://github.com/Galxe/gravity-sdk.git
cd gravity-sdk

# 编译（需要 tokio_unstable feature flag）
RUSTFLAGS="--cfg tokio_unstable" cargo build --profile quick-release -p gravity_node -p gravity_cli
```

编译产物位于 `target/quick-release/gravity_node` 和 `target/quick-release/gravity_cli`。

### 2. 准备配置文件

```bash
cd cluster
cp genesis.toml.example genesis.toml
cp cluster.toml.example cluster.toml
```

### 3. 配置 genesis.toml

> [!IMPORTANT]
> `genesis.toml` 中 validators 的 `host` 和 `p2p_port`/`vfn_port` 会被写入链上，部署后不可更改（除非合约支持 updateNetworkAddresses）。请确保端口与 `cluster.toml` 一致。

```toml
# Gravity 测试网 Genesis 配置

[dependencies.genesis_contracts]
repo = "https://github.com/Galxe/gravity_chain_core_contracts.git"
ref = "dev-0208-chainid"

[genesis]
chain_id = 7771625
epoch_interval_micros = 7200000000  # 2 小时
major_version = 1
# The consensus_config bytes are BCS-serialized from the following Rust struct:
# V4 { alg: JolteonV2 { main: ConsensusConfigV1 { decoupled_execution: true,
# back_pressure_limit: 10, exclude_round: 40, proposer_election_type:
# RotatingProposer(1), max_failed_authors_to_store: 10 }, quorum_store_enabled:
# true, order_vote_enabled: false }, vtxn: V1 { per_block_limit_txn_count: 2,
# per_block_limit_total_bytes: 2097152 }, window_size: None }
consensus_config = "0x0301010a00000000000000280000000000000001010000000a000000000000000100010200000000000000000020000000000000"
execution_config = "0x00"

[genesis.validator_config]
minimum_bond = "1000000000000000000000000"      # 1,000,000 G
maximum_bond = "10000000000000000000000000000"  # 10亿 G
unbonding_delay_micros = 604800000000          # 7 天
allow_validator_set_change = true
voting_power_increase_limit_pct = 50
max_validator_set_size = "100"

[genesis.staking_config]
minimum_stake = "1000000000000000000000000"     # 1,000,000 G
lockup_duration_micros = 86400000000           # 1 天
unbonding_delay_micros = 86400000000
minimum_proposal_stake = "1000000000000000000000000" # 1,000,000 G

[genesis.governance_config]
min_voting_threshold = "1000000000000000000"
required_proposer_stake = "10000000000000000000"
voting_duration_micros = 604800000000

[genesis.randomness_config]
variant = 1
secrecy_threshold = 9223372036854775808
reconstruction_threshold = 12297829382473033728
fast_path_secrecy_threshold = 12297829382473033728

[genesis.oracle_config]
source_types = [1]
callbacks = ["0x00000000000000000000000000000001625F4001"]

[genesis.oracle_config.bridge_config]
deploy = true
trusted_bridge = "0x79226649b3A20231e6b468a9E1AbBD23d3DFbbC6"

[[genesis.oracle_config.tasks]]
source_type = 0
source_id = 11155111
task_name = "sepolia"
config = "gravity://0/11155111/events?contract=0x60fD4D8fB846D95CcDB1B0b81c5fed1e8b183375&eventSignature=0x5646e682c7d994bf11f5a2c8addb60d03c83cda3b65025a826346589df43406e&fromBlock=10231540"

# JWK config - Google OIDC provider
[genesis.jwk_config]
issuers = ["0x68747470733a2f2f6163636f756e74732e676f6f676c652e636f6d"]

[[genesis.jwk_config.jwks]]
kid = "f5f4c0ae6e6090a65ab0a694d6ba6f19d5d0b4e6"
kty = "RSA"
alg = "RS256"
e = "AQAB"
n = "2K7epoJWl_aBoYGpXmDBBiEnwQ0QdVRU1gsbGXNrEbrZEQdY5KjH5P5gZMq3d3KvT1j5KsD2tF_9jFMDLqV4VWDNJRLgSNJxhJuO_oLO2BXUSL9a7fLHxnZCUfJvT2K-O8AXjT3_ZM8UuL8d4jBn_fZLzdEI4MHrZLVSaHDvvKqL_mExQo6cFD-qyLZ-T6aHv2x8R7L_3X7E1nGMjKVVZMveQ_HMeXvnGxKf5yfEP0hIQlC_kFm4L_1kV1S0UPmMptZL2qI4VnXqmqI6TZJyE-3VXHgNn1Z1O_9QZlPC0fF0spLHf2S3nNqI0v3k2E7q3DkqxVf5xvn7q_X-gPqzVE9Jw"

# ===== Faucet 配置 (第4个地址) =====
[genesis.faucet]
address = "0x18c23753385ce7A60B15d171302E48b6AFf0BDC5"
balance = "10000000000000000000000000000000"    # 10,000 亿 G

# ===== 验证器配置 (3 个节点，每个 1000 G 质押) =====

[[genesis_validators]]
id = "node1"
address = "0xAEd2a948892475F800A337427B3275D190EA3e94"
host = "34.83.28.182"                            # gravity-testnet-node-oregon-0
p2p_port = 6180
vfn_port = 6190
stake_amount = "1000000000000000000000000"       # 1,000,000 G
voting_power = "1000000000000000000000000"

[[genesis_validators]]
id = "node2"
address = "0x7b254Bd44F6CE45e00a912b2460D47F3Be56fAD7"
host = "34.83.9.159"                            # gravity-testnet-node-oregon-1
p2p_port = 6180
vfn_port = 6190
stake_amount = "1000000000000000000000000"       # 1,000,000 G
voting_power = "1000000000000000000000000"

[[genesis_validators]]
id = "node3"
address = "0x9B2C25E77a97d3e84DC0Cb7F83fb676ddC4F24b9"
host = "34.94.164.9"                              # gravity-testnet-node-losangeles
p2p_port = 6180
vfn_port = 6190
stake_amount = "1000000000000000000000000"       # 1,000,000 G
voting_power = "1000000000000000000000000"
```

### 4. 配置 cluster.toml

> [!WARNING]
> `p2p_port` 和 `vfn_port` 必须与 `genesis.toml` 中的一致，否则节点之间无法通信。

```toml
# Gravity 测试网集群配置

[cluster]
name = "gravity-testnet"
base_dir = "/home/gravity/gravity-testnet"

[build]
binary_path = "../target/quick-release/gravity_node"

[genesis_source]
genesis_path = "./output/genesis.json"
waypoint_path = "./output/waypoint.txt"

# ===== 验证器节点 (3 个) =====

[[nodes]]
id = "node1"
role = "genesis"
host = "34.83.28.182"                            # gravity-testnet-node-oregon-0
p2p_port = 6180
vfn_port = 6190
rpc_port = 8545
metrics_port = 9001
inspection_port = 10000
https_port = 1024
authrpc_port = 8551
reth_p2p_port = 12024

[[nodes]]
id = "node2"
role = "genesis"
host = "34.83.9.159"                            # gravity-testnet-node-oregon-1
p2p_port = 6180
vfn_port = 6190
rpc_port = 8545
metrics_port = 9001
inspection_port = 10000
https_port = 1024
authrpc_port = 8551
reth_p2p_port = 12024

[[nodes]]
id = "node3"
role = "genesis"
host = "34.94.164.9"                              # gravity-testnet-node-losangeles
p2p_port = 6180
vfn_port = 6190
rpc_port = 8545
metrics_port = 9001
inspection_port = 10000
https_port = 1024
authrpc_port = 8551
reth_p2p_port = 12024

[[nodes]]
id = "node4"
role = "vfn"                     
host = "35.199.186.180"                           # gravity-testnet-node-oregon-2
p2p_port = 6180
vfn_port = 6190
rpc_port = 8545
metrics_port = 9001
inspection_port = 10000
https_port = 1024
authrpc_port = 8551
reth_p2p_port = 12024
```

### 5. 生成 Genesis

```bash
# 在协调节点（任意一台有完整代码的机器）执行
make init      # 生成节点密钥（含 VFN 的 identity）
make genesis   # 生成 genesis.json 和 waypoint.txt
```

生成的文件：
- `./output/genesis.json`
- `./output/waypoint.txt`
- `./output/node{1,2,3,4}/config/identity.yaml`

当前节点身份密钥：

**node1** (gravity-testnet-node-oregon-0):
```yaml
account_address: c174a49d543efd4b50f69410b3d251596d541c800be037b4de8bb87ff2bf959d
account_private_key: 1b380f928a30d11a4db8358591d4f6fd28343b45c3a47e706ead31d5e2f7aff3
consensus_private_key: 3950cbfe973ba74b69b4a4ddfbd81e4c5f5821b2e35028f8e25999fcabaf8f6d
network_private_key: 48e922c7911580eeacf2c8a9ff288c5641b5c71f47b6c30354c8d3610bc02a45
consensus_public_key: 9817effccc0cc74fe4dae4998fd3d68e1b8c4e0eca077da8f555328951bd37dd148d71c496d67b3422a257b430040a80
network_public_key: 28fb882f3067c7964793251d643b2c164a66e476c4898d9444a22c1840197a6a
```

**node2** (gravity-testnet-node-oregon-1):
```yaml
account_address: 0067748a121897900f7e2da01ada6bee35c7d17aaa5951a034c7100136c99f94
account_private_key: 06e9e33bfd5802a81a8b0be5388d9525ec4cfe580d6c649e67bd4ad66ea076b0
consensus_private_key: 541ac959332823a9853b07f6fac5b3da6c9b0b4c18cfdb8aede89c8b76d29bba
network_private_key: 289219ebdc8537dccac01db1333d459ed667fbb846c3454b03cbd3c9fd3afe44
consensus_public_key: 8f26e5d7132b98bdfda0b48e278ab50f2f894bead2133af776e0e7a94a12df6daf9d13a109b6fc22e4c18029b57078ae
network_public_key: 3f51ea9c12a1e5241a054a7b0624753a7467dbcd46f6bbb4e144344dac748d28
```

**node3** (gravity-testnet-node-losangeles):
```yaml
account_address: 305599a531a9c9a38e8ba93f254f3312caa862066ef02c63f8aefedcae109625
account_private_key: 9eae80dc1c62a135ef77f6bee042c4b9b03b0cf44bddea51f96caff07d5587f3
consensus_private_key: 03856093fc172a887a7684f7fcd3b9835b3b6b0e3c3a39dfaac47cda1cc4e5f1
network_private_key: b048564ed5a430abc9ef139c09b79a06898e96f48e952fbe5e2de3b5f36a0247
consensus_public_key: 8dc31dce6108cc09a98bd0289a4d27d0dc47b078b79d448bac15bc8a127784f72385477d280f189507ceb43cf6828ba4
network_public_key: 1ffd5e81b1fff58cef199a98a55ebcf8cef7e1f7d6e7ce74fe921c8b44087719
```

**node4** (gravity-testnet-node-oregon-2, VFN):
```yaml
account_address: 617f7166b8f4546a964875a4d21ffd53e8d59345f1dc58e7070e273410feebe1
account_private_key: 6057be10d9abda71c742c4bf0aa049a4b651fd1e862f167f5aa66b248864de25
consensus_private_key: 5eb21ea2227ac02ec3e153a9cbf95901ea3289acc24650d54eac8aa62ebfd7f6
network_private_key: a88b57a0eeb6a2f761cf21fc367da8d1d7fe868cdb4445db731965857a2f6177
consensus_public_key: b586473e5326e3cbf2a2cd6ba21cdb99537cadd8c3013cd5fb7e419222ff1114248c8c0adfed80f66eab125b7558935c
network_public_key: 3f862d4a32ab5ef7105c856659e134eac6cd7a06ca5b032f97f6339e40843e53
```

### 6. 部署配置

```bash
make deploy    # 生成节点运行配置和启动脚本
```

> [!NOTE]
> 如果目标目录已有内容，`make deploy` 会提示是否清空。选择 `n` 可以保留已有数据，只覆盖配置文件。

部署后每台节点上的目录结构如下：

```
/home/gravity/gravity-testnet/
├── genesis.json          # 创世配置
├── gravity_node          # 节点二进制
├── gravity_cli           # CLI 工具
└── node1/                # 节点目录（node1/node2/node3 对应不同节点）
    ├── config/
    │   ├── validator.yaml          # 验证器配置
    │   ├── identity.yaml           # 节点身份密钥
    │   ├── reth_config.json        # 执行层配置
    │   ├── relayer_config.json     # Relayer 配置
    │   └── waypoint.txt            # 创世 waypoint
    ├── script/
    │   ├── start.sh                # 启动脚本
    │   ├── stop.sh                 # 停止脚本
    │   └── node.pid                # 进程 PID（运行后生成）
    ├── data/                       # 链数据（运行后生成）
    ├── logs/                       # 日志目录
    ├── execution_logs/             # 执行层日志
    └── consensus_log/              # 共识日志
```

### 7. 分发到各节点

将本地 deploy 生成的目录拷贝过去：

```bash
scp -r /home/gravity/gravity-testnet/node1 node1:/home/gravity/gravity-testnet/
scp /home/gravity/gravity-testnet/genesis.json node1:/home/gravity/gravity-testnet/
scp /home/gravity/gravity-testnet/gravity_node node1:/home/gravity/gravity-testnet/
```

### 8. 启动节点

在每台节点机器上执行：
```bash
cd /home/gravity/gravity-testnet/<node_id>/script
./start.sh
```

### 9. 验证

```bash
# 检查区块高度
curl -s -X POST http://34.83.28.182:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' | jq

# 检查 Faucet 余额
curl -s -X POST http://34.83.28.182:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_getBalance","params":["0x18c23753385ce7A60B15d171302E48b6AFf0BDC5","latest"],"id":1}' | jq

# 列出验证器
./gravity_cli validator list --rpc-url http://34.83.28.182:8545
```

---

## 常用命令

| 命令 | 描述 |
|------|------|
| `make init` | 生成节点密钥（所有节点含 VFN） |
| `make genesis` | 生成 genesis.json 和 waypoint.txt |
| `make deploy` | 生成运行配置和启动/停止脚本 |

---

## 故障排查

| 问题 | 解决方案 |
|------|----------|
| 节点无法连接 | 检查防火墙是否开放 P2P 端口 (6180, 6190) |
| 节点之间握手失败 | 确认 `genesis.toml` 和 `cluster.toml` 的端口一致 |
| 区块高度为 0 | 确认 3 个验证器都已启动，且使用相同的 genesis.json |
| sed 报错（Linux） | deploy.sh 已支持跨平台，确保使用最新版本 |
| deploy 时报 identity 不存在 | 确保先执行 `make init` 生成密钥 |

查看日志：
```bash
tail -f /home/gravity/gravity-testnet/<node_id>/logs/debug.log
tail -f /home/gravity/gravity-testnet/<node_id>/consensus_log/validator.log
tail -f /home/gravity/gravity-testnet/<node_id>/execution_log/7771625/reth.log
```
