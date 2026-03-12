# gravity-sdk 第三阶段安全审计 — 全部发现项修复设计

日期：2026-03-05
范围：GSDK-017 至 GSDK-027（不含 GSDK-025，该项无效）

---

## 概览

| 编号 | 严重程度 | 标题 | 涉及文件 |
|------|---------|------|---------|
| GSDK-017 | HIGH | BlockBufferManager 嵌套 Mutex 持锁 | `block_buffer_manager.rs` |
| GSDK-018 | HIGH | Epoch 切换时在途区块浪费 — 缺少取消机制 | `block_buffer_manager.rs`, `reth_cli.rs` |
| GSDK-019 | MEDIUM | Epoch 不匹配时静默丢弃区块 | `block_buffer_manager.rs` |
| GSDK-020 | MEDIUM | Epoch 切换时丢弃在途区块 | `block_buffer_manager.rs` |
| GSDK-021 | `consume_epoch_change` 中的 TOCTOU 竞态 | MEDIUM | `block_buffer_manager.rs` |
| GSDK-022 | MEDIUM | 执行失败导致 Commit Vote 循环终止 | `reth_cli.rs` |
| GSDK-023 | LOW | `buffer_state` 原子变量在锁外检查 — TOCTOU | `block_buffer_manager.rs` |
| GSDK-024 | LOW | `pop_txns` Gas 计算偏差一位 | `block_buffer_manager.rs` |
| GSDK-026 | LOW | Coinbase 地址硬编码为零地址（部分修复） | `reth_cli.rs` |
| GSDK-027 | LOW | 验证者可能临时观察到不同 Epoch | `block_buffer_manager.rs`（仅文档） |

---

## GSDK-017：BlockBufferManager 嵌套 Mutex 持锁（HIGH）

**问题：** `BlockBufferManager` 有两个 `tokio::sync::Mutex` 字段 — `block_state_machine` 和 `latest_epoch_change_block_number`，在多处以嵌套方式获取。例如 `get_executed_res`（第 579+626 行）、`release_inflight_blocks`（第 941+942 行）以及 `set_compute_res`/`calculate_new_epoch_state`（第 697+674 行）。由于 `tokio::sync::Mutex` 不可重入，不同任务以不同顺序嵌套获取这两个锁可能导致**死锁**。

**修复方案：** 将 `latest_epoch_change_block_number` 合并为 `BlockStateMachine` 的普通字段，彻底消除第二个 Mutex。由于所有对 `latest_epoch_change_block_number` 的访问已经在持有 `block_state_machine` 锁时发生（或可重构为如此），该字段天然属于同一临界区。

**参考代码：**
```rust
// 修改前 (crates/block-buffer-manager/src/block_buffer_manager.rs:164-199)
pub struct BlockStateMachine {
    sender: tokio::sync::broadcast::Sender<()>,
    blocks: HashMap<BlockKey, BlockState>,
    profile: HashMap<BlockKey, BlockProfile>,
    latest_commit_block_number: u64,
    latest_finalized_block_number: u64,
    block_number_to_block_id: HashMap<u64, BlockId>,
    current_epoch: u64,
    next_epoch: Option<u64>,
}

pub struct BlockBufferManager {
    txn_buffer: TxnBuffer,
    block_state_machine: Mutex<BlockStateMachine>,
    buffer_state: AtomicU8,
    config: BlockBufferManagerConfig,
    latest_epoch_change_block_number: Mutex<u64>,
    ready_notifier: Arc<Notify>,
}

// 修改后
pub struct BlockStateMachine {
    sender: tokio::sync::broadcast::Sender<()>,
    blocks: HashMap<BlockKey, BlockState>,
    profile: HashMap<BlockKey, BlockProfile>,
    latest_commit_block_number: u64,
    latest_finalized_block_number: u64,
    block_number_to_block_id: HashMap<u64, BlockId>,
    current_epoch: u64,
    next_epoch: Option<u64>,
    /// 从独立 Mutex 移入，消除嵌套锁 (GSDK-017)。
    latest_epoch_change_block_number: u64,
}

pub struct BlockBufferManager {
    txn_buffer: TxnBuffer,
    block_state_machine: Mutex<BlockStateMachine>,
    buffer_state: AtomicU8,
    config: BlockBufferManagerConfig,
    // 已移除: latest_epoch_change_block_number: Mutex<u64>,
    ready_notifier: Arc<Notify>,
}
```

随后更新所有访问点。`release_inflight_blocks` 示例：
```rust
// 修改前 (第 940-966 行)
pub async fn release_inflight_blocks(&self) {
    let mut block_state_machine = self.block_state_machine.lock().await;
    let latest_epoch_change_block_number = *self.latest_epoch_change_block_number.lock().await;
    // ...
}

// 修改后
pub async fn release_inflight_blocks(&self) {
    let mut block_state_machine = self.block_state_machine.lock().await;
    let latest_epoch_change_block_number =
        block_state_machine.latest_epoch_change_block_number;
    // ...
}
```

`calculate_new_epoch_state` 示例：
```rust
// 修改前 (第 674 行)
*self.latest_epoch_change_block_number.lock().await = block_num;

// 修改后 — 调用者已持有 block_state_machine 锁
block_state_machine.latest_epoch_change_block_number = block_num;
```

`get_executed_res` 示例：
```rust
// 修改前 (第 630-631 行)
let latest_epoch_change_block_number =
    *self.latest_epoch_change_block_number.lock().await;

// 修改后 — 已持有 block_state_machine 锁
let latest_epoch_change_block_number =
    block_state_machine.latest_epoch_change_block_number;
```

`init` 示例：
```rust
// 修改前 (第 279 行)
*self.latest_epoch_change_block_number.lock().await = latest_commit_block_number;

// 修改后
block_state_machine.latest_epoch_change_block_number = latest_commit_block_number;
```

`latest_epoch_change_block_number` 访问器示例：
```rust
// 修改前 (第 335-337 行)
pub async fn latest_epoch_change_block_number(&self) -> u64 {
    *self.latest_epoch_change_block_number.lock().await
}

// 修改后
pub async fn latest_epoch_change_block_number(&self) -> u64 {
    let bsm = self.block_state_machine.lock().await;
    bsm.latest_epoch_change_block_number
}
```

**涉及文件：**
- `crates/block-buffer-manager/src/block_buffer_manager.rs`

**Review Comments** reviewer: Lightman; state: Accepted;

---

## GSDK-018：Epoch 切换时在途区块浪费 — 缺少取消机制（HIGH）

**问题：** Epoch 切换时，`release_inflight_blocks` 仅保留 `latest_epoch_change_block_number` 及以下的区块。但当前正在 reth 执行层执行的区块会继续执行直到超时（`max_wait_timeout` 最长 2 秒），**浪费 CPU 并延迟新 Epoch 第一个区块的处理**。

**修复方案：** 引入 `CancellationToken`（来自 `tokio_util`），每个 Epoch 一个。当 Epoch 切换发生时，取消旧 Epoch 的令牌。执行层在开始和执行过程中检查该令牌，以便立即终止过期工作。

**参考代码：**
```rust
// 修改前 — BlockBufferManager 字段
pub struct BlockBufferManager {
    txn_buffer: TxnBuffer,
    block_state_machine: Mutex<BlockStateMachine>,
    buffer_state: AtomicU8,
    config: BlockBufferManagerConfig,
    ready_notifier: Arc<Notify>,
}

// 修改后 — 新增 epoch 取消令牌
use tokio_util::sync::CancellationToken;

pub struct BlockBufferManager {
    txn_buffer: TxnBuffer,
    block_state_machine: Mutex<BlockStateMachine>,
    buffer_state: AtomicU8,
    config: BlockBufferManagerConfig,
    ready_notifier: Arc<Notify>,
    /// 当前 Epoch 的取消令牌。Epoch 切换时取消，
    /// 以终止旧 Epoch 的在途执行工作 (GSDK-018)。
    epoch_cancel_token: Mutex<CancellationToken>,
}
```

在 `release_inflight_blocks` 中取消旧令牌并创建新令牌：
```rust
pub async fn release_inflight_blocks(&self) {
    let mut block_state_machine = self.block_state_machine.lock().await;
    // ...现有 epoch 更新逻辑...

    // GSDK-018: 取消旧 Epoch 的在途执行
    {
        let mut token = self.epoch_cancel_token.lock().await;
        token.cancel();
        *token = CancellationToken::new();
    }

    block_state_machine
        .blocks
        .retain(|key, _| key.block_number <= latest_epoch_change_block_number);
    self.buffer_state.store(BufferState::EpochChange as u8, Ordering::SeqCst);
    // ...
}
```

暴露令牌供执行层检查：
```rust
/// 返回当前 Epoch 取消令牌的克隆。
/// 执行层应在区块执行期间检查此令牌，以便在 Epoch 变更时提前终止。
pub async fn current_epoch_cancel_token(&self) -> CancellationToken {
    self.epoch_cancel_token.lock().await.clone()
}
```

**涉及文件：**
- `crates/block-buffer-manager/src/block_buffer_manager.rs`
- `bin/gravity_node/src/reth_cli.rs`（在执行循环中检查令牌）

**Review Comments** reviewer: Lightman; state: Reject; comments: epoch change 的间隔比较大，浪费一部分CPU没所谓，引入token 增加了复杂度。

---

## GSDK-019：Epoch 不匹配时静默丢弃区块（MEDIUM）

**问题：** `set_ordered_blocks` 中，当区块的 Epoch 不匹配 `current_epoch` 时，以 `return Ok(())` 静默丢弃（第 392-405 行）。这会隐藏合法问题 — 未来 Epoch 的区块可能表示共识层 bug，仅靠 `warn` 级别日志在生产环境中容易被忽视。且没有发出指标，监控系统无法捕获。

**修复方案：** 为 Epoch 不匹配导致的区块丢弃添加 metrics 计数器；对于未来 Epoch 的区块（在正确共识下不应出现），返回错误而非静默忽略。

**参考代码：**
```rust
// 修改前 (第 390-405 行)
if block.block_meta.epoch < current_epoch {
    warn!("...");
    return Ok(());
}
if block.block_meta.epoch > current_epoch {
    warn!("...");
    return Ok(());
}

// 修改后
if block.block_meta.epoch < current_epoch {
    warn!("set_ordered_blocks: ignoring block {} with old epoch {} (current epoch: {})",
        block.block_meta.block_number, block.block_meta.epoch, current_epoch);
    metrics::counter!(
        "block_buffer_manager_dropped_blocks",
        &[("reason", "old_epoch")]
    ).increment(1);
    return Ok(());
}

if block.block_meta.epoch > current_epoch {
    // 在正确共识下不应收到未来 Epoch 的区块，作为错误上报。
    let msg = format!(
        "set_ordered_blocks: block {} has future epoch {} (current epoch: {})",
        block.block_meta.block_number, block.block_meta.epoch, current_epoch
    );
    warn!("{}", msg);
    metrics::counter!(
        "block_buffer_manager_dropped_blocks",
        &[("reason", "future_epoch")]
    ).increment(1);
    return Err(anyhow::anyhow!("{msg}"));
}
```

**涉及文件：**
- `crates/block-buffer-manager/src/block_buffer_manager.rs`

**Review Comments** reviewer: Lightman; state: Accepted;

---

## GSDK-020：Epoch 切换时丢弃在途区块（MEDIUM）

**问题：** `release_inflight_blocks` 使用 `retain()` 移除所有 `block_number > latest_epoch_change_block_number` 的区块，**不区分其状态**（Ordered、Computed、Committed）。处于 `Computed` 状态的区块已完成执行 — 丢弃它们浪费了执行工作，且如果执行层期望得到这些区块的响应，可能导致不一致状态。

**修复方案：** 按区块状态添加丢弃计数的 metrics，并在丢弃 `Computed` 或 `Committed` 状态的区块时记录 `warn` 日志。执行层已通过 `get_executed_res` 中的超时处理缺失区块的情况，因此无需功能性变更 — 但可观测性非常重要。

**参考代码：**
```rust
// 修改前 (第 958-964 行)
block_state_machine
    .blocks
    .retain(|key, _| key.block_number <= latest_epoch_change_block_number);

// 修改后
let mut discarded_ordered = 0u64;
let mut discarded_computed = 0u64;
let mut discarded_committed = 0u64;

block_state_machine.blocks.retain(|key, state| {
    if key.block_number <= latest_epoch_change_block_number {
        return true;
    }
    match state {
        BlockState::Ordered { .. } => discarded_ordered += 1,
        BlockState::Computed { id, .. } => {
            warn!("release_inflight_blocks: discarding Computed block {:?} num {}",
                id, key.block_number);
            discarded_computed += 1;
        }
        BlockState::Committed { id, .. } => {
            warn!("release_inflight_blocks: discarding Committed block {:?} num {}",
                id, key.block_number);
            discarded_committed += 1;
        }
        BlockState::Historical { .. } => {}
    }
    false
});

if discarded_ordered + discarded_computed + discarded_committed > 0 {
    info!("release_inflight_blocks: discarded {} ordered, {} computed, {} committed blocks \
         above epoch change block {}",
        discarded_ordered, discarded_computed, discarded_committed,
        latest_epoch_change_block_number);
    metrics::counter!("block_buffer_manager_epoch_discarded_blocks",
        &[("state", "ordered")]).increment(discarded_ordered);
    metrics::counter!("block_buffer_manager_epoch_discarded_blocks",
        &[("state", "computed")]).increment(discarded_computed);
    metrics::counter!("block_buffer_manager_epoch_discarded_blocks",
        &[("state", "committed")]).increment(discarded_committed);
}
```

**涉及文件：**
- `crates/block-buffer-manager/src/block_buffer_manager.rs`

**Review Comments** reviewer: Lightman; state: Reject; comments: epoch change 之后丢弃上一个epoch的区块属于正常行为

---

## GSDK-021：`consume_epoch_change` 中的 TOCTOU 竞态（MEDIUM）

**问题：** `consume_epoch_change`（第 329-333 行）中，`buffer_state` 通过原子 store 被设为 `Ready`，但这发生在获取 `block_state_machine` 锁**之前**。在原子写入和锁获取之间，其他任务调用 `is_epoch_change()` 或 `get_ordered_blocks()` 时可能看到 `Ready` 状态，并在 Epoch 切换尚未完全完成时尝试处理区块。

**修复方案：** 调整操作顺序：先获取 `block_state_machine` 锁，读取 Epoch 值，然后再将 `buffer_state` 设为 `Ready`。

**参考代码：**
```rust
// 修改前 (第 329-333 行)
pub async fn consume_epoch_change(&self) -> u64 {
    self.buffer_state.store(BufferState::Ready as u8, Ordering::SeqCst);
    let block_state_machine = self.block_state_machine.lock().await;
    block_state_machine.current_epoch
}

// 修改后
pub async fn consume_epoch_change(&self) -> u64 {
    // GSDK-021: 先获取锁，防止 TOCTOU 竞态。
    let block_state_machine = self.block_state_machine.lock().await;
    let epoch = block_state_machine.current_epoch;
    // 此时才通知 Epoch 切换已完成
    self.buffer_state.store(BufferState::Ready as u8, Ordering::SeqCst);
    epoch
}
```

**涉及文件：**
- `crates/block-buffer-manager/src/block_buffer_manager.rs`

**Review Comments** reviewer: Lightman; state: Accepted;

---

## GSDK-022：执行失败导致 Commit Vote 循环终止（MEDIUM）

**问题：** `start_commit_vote`（第 327-368 行）中，当 `recv_compute_res()` 返回 `Err` 时，循环直接 `break`，**永久终止** Commit Vote 管道。这意味着不会再有执行结果被转发到共识层，导致整个节点停滞。该错误可能只是暂时性的通道问题（例如发送端在 Epoch 切换期间临时断开）。

**修复方案：** 区分致命错误（通道永久关闭）和暂时性错误。对暂时性错误，记录日志并继续循环；对致命错误（发送端已丢弃），触发优雅关闭而非静默退出。

**参考代码：**
```rust
// 修改前 (bin/gravity_node/src/reth_cli.rs:327-368)
let execution_result = match execution_result {
    Ok(res) => res,
    Err(e) => {
        warn!("recv_compute_res failed: {}. Stopping commit vote loop.", e);
        break;  // ← 永久终止循环
    }
};

// 修改后
let mut consecutive_errors = 0u32;
const MAX_CONSECUTIVE_ERRORS: u32 = 5;

// ... 在循环内 ...
let execution_result = match execution_result {
    Ok(res) => {
        consecutive_errors = 0; // 成功时重置计数
        res
    }
    Err(e) => {
        consecutive_errors += 1;
        if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
            // 连续失败过多 — 通道很可能已永久损坏，触发关闭
            error!("recv_compute_res failed {} consecutive times (last: {}). \
                 Triggering graceful shutdown.", consecutive_errors, e);
            return Err(format!(
                "Commit vote loop terminated after {} consecutive errors: {}",
                consecutive_errors, e
            ));
        }
        warn!("recv_compute_res failed (attempt {}/{}): {}. Retrying...",
            consecutive_errors, MAX_CONSECUTIVE_ERRORS, e);
        // 短暂延迟后重试，避免紧密错误循环
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        continue;
    }
};
```

**涉及文件：**
- `bin/gravity_node/src/reth_cli.rs`

**Review Comments** reviewer: Jian Xie; state: Accepted; comments: The recv_compute_res channel can indeed experience transient errors during epoch transitions. The original break permanently killed the entire pipeline with no recovery path. Introducing a consecutive_errors counter with 100ms backoff retry is a sound resilience strategy, and triggering graceful shutdown after 5 consecutive failures is far better than silently stalling. One suggestion: MAX_CONSECUTIVE_ERRORS should be a config parameter rather than a hardcoded const, to allow tuning across different deployment environments.

---

## GSDK-023：`buffer_state` 原子变量在锁外检查 — TOCTOU（LOW）

**问题：** `buffer_state`（`AtomicU8`）在多处通过 `is_ready()` 和 `is_epoch_change()` 于获取 `block_state_machine` 锁**之前**被检查。状态可能在原子读取和锁获取之间发生变化，导致方法基于过期状态执行。

**修复方案：** 在每个调用点记录可接受的竞态条件说明。这些竞态是**良性的**：
1. `is_ready()` 仅控制 `Notify` 等待，锁内重新检查效果等价
2. `get_ordered_blocks` 中的 `is_epoch_change()` 返回的错误由调用者优雅处理

在关键位置添加注释，必要时在锁内重新检查：
```rust
// 修改后 (get_ordered_blocks)
// GSDK-023: 锁外检查 is_ready() 是有意为之 — 仅控制 Notify 等待。
if !self.is_ready() {
    self.ready_notifier.notified().await;
}

// GSDK-023: is_epoch_change() 是快速退出路径。
if self.is_epoch_change() {
    return Err(anyhow::anyhow!("Buffer is in epoch change"));
}

// 在锁内重新检查以确保正确性
{
    let block_state_machine = self.block_state_machine.lock().await;
    if self.buffer_state.load(Ordering::SeqCst) == BufferState::EpochChange as u8 {
        return Err(anyhow::anyhow!("Buffer is in epoch change (re-checked under lock)"));
    }
}
```

**涉及文件：**
- `crates/block-buffer-manager/src/block_buffer_manager.rs`

**Review Comments** reviewer: Lightman; state: Reject; comments: 不需要再做一次这种检测，因为这些线程不存在多并发，get_ordered_blocks 是由 reth_cli 调用的，如果遇到 is_epoch_change 的状态，只能 reth_cli consume_epoch_change 才能让 block_buffer_manager 变回 Ready。

---

## GSDK-024：`pop_txns` Gas 计算偏差一位（LOW）

**问题：** `pop_txns`（第 339-370 行）中，迭代器使用 `position()` 闭包在第一项时（`total_gas_limit == 0`）返回 `false` 并跳过 Gas 累加。导致**第一笔交易的 `gas_limit` 从未被计入 `total_gas_limit`**，`count` 也不会为第一项递增。最终效果是 `pop_txns` 总是多包含一项的 Gas 超出请求限额。

**修复方案：** 统一处理所有项的 Gas 累加逻辑，移除对第一项的特殊处理。

**参考代码：**
```rust
// 修改前
let split_point = txn_buffer
    .iter()
    .position(|item| {
        if total_gas_limit == 0 {
            return false;  // ← 第一项跳过检查，不累加 gas
        }
        if total_gas_limit + item.gas_limit > gas_limit || count >= max_size {
            return true;
        }
        total_gas_limit += item.gas_limit;
        count += 1;
        false
    })
    .unwrap_or(txn_buffer.len());

// 修改后 — 统一对每一项进行 Gas 计算
let split_point = txn_buffer
    .iter()
    .position(|item| {
        if total_gas_limit + item.gas_limit > gas_limit || count >= max_size {
            return true;
        }
        total_gas_limit += item.gas_limit;
        count += 1;
        false
    })
    .unwrap_or(txn_buffer.len());
```

**涉及文件：**
- `crates/block-buffer-manager/src/block_buffer_manager.rs`

**Review Comments** reviewer: Jian Xie; state: Accepted; comments: The original total_gas_limit == 0 special case caused the first item's gas_limit to never be accumulated, so all subsequent gas checks were off by one item's worth. Although the EVM execution layer enforces gas limits again, exceeding the limit at the batching stage causes unnecessary block rejections and retry overhead. Removing the special case so all items go through uniform gas accumulation logic is the correct fix.

---

## GSDK-026：Coinbase 地址硬编码为零地址（LOW，部分修复）

**问题：** `get_coinbase_from_proposer_index`（第 136-161 行）中，当 `proposer_index` 为 `None` 或提议者到 reth 地址映射查找失败时，返回 `Address::ZERO`。使用零地址作为 coinbase 意味着出块奖励和交易手续费流入**不可恢复的地址**。其中 `proposer_index == None` 的情况会**静默返回零地址**而无任何日志。

**修复方案：** 为 `proposer_index` 为 `None` 的情况添加警告日志，并添加零地址回退的 metrics 计数器以便监控。

**参考代码：**
```rust
// 修改前
None => return Address::ZERO,  // ← 静默返回

// 修改后
None => {
    warn!("Block has no proposer_index in metadata, using Address::ZERO as coinbase. \
         This may indicate a consensus-layer issue.");
    metrics::counter!(
        "coinbase_zero_address_fallback",
        &[("reason", "no_proposer_index")]
    ).increment(1);
    return Address::ZERO;
}
```

同样为地址长度异常和映射查找失败的分支添加 metrics。

**涉及文件：**
- `bin/gravity_node/src/reth_cli.rs`

**Review Comments** reviewer: Lightman; state: Accepted;

---

## GSDK-027：验证者可能临时观察到不同 Epoch（LOW）

**问题：** Epoch 切换期间，不同验证者可能临时观察到不同的 `current_epoch` 值。这是因为 `release_inflight_blocks`（更新 `current_epoch`）在每个验证者收到共识层的 Epoch 变更事件时异步调用。

**修复方案：** **无需代码变更。** 这是 BFT 共识下的预期行为。添加文档说明此时序偏差是安全的：

1. **共识保证收敛**：AptosBFT 确保所有验证者最终收敛到相同 Epoch
2. **仲裁防护**：区块需要 2/3+ 仲裁才能提交，防止从过期 Epoch 最终确认
3. **已有防御逻辑**：`set_ordered_blocks` 丢弃不匹配区块，`get_ordered_blocks` 返回错误由调用者重试
4. **偏差窗口有限**：受网络传播 + 处理时间限制，通常不超过 1 秒

**涉及文件：**
- `crates/block-buffer-manager/src/block_buffer_manager.rs`（仅添加文档注释）

**Review Comments** reviewer: Lightman; state: Accepted;
