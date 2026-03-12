# Gravity 质押 (Staking) 流程分析与测试计划

## 1. 当前质押架构与流程

根据对 `@gravity_chain_core_contracts/src/staking` 的分析，质押系统是围绕 **工厂-池 (Factory-Pool)** 模型设计的，并带有 **验证者管理 (Validator Management)** 层。

### 1.1 核心组件

*   **`Staking.sol` (工厂合约)**
    *   **角色**: 创建新质押池 (Stake Pool) 的入口点。
    *   **注册表**: 维护所有有效 `StakePool` 合约的注册表。
    *   **锁仓续期**: `ValidatorManagement` 的授权入口，用于为活跃验证者续期锁仓。

*   **`StakePool.sol` (独立质押池)**
    *   **部署**: 通过 `CREATE2` 部署，具有确定性地址。
    *   **角色**:
        *   `Owner` (所有者): 行政控制（更改运营者、投票者）。
        *   `Staker` (质押者): 资金管理（质押、解除质押、提取）。
        *   `Operator` (运营者): 验证者操作（加入/退出集合、轮换密钥）。
        *   `Voter` (投票者): 参与治理投票。
    *   **锁仓机制**:
        *   `lockedUntil` 时间戳。
        *   **自动延长**: `addStake` 会将锁仓延长至 `max(current, now + minLockup)`。
        *   **提取**: 解除质押 (Unstake) 会将资金移动到 `_pendingBuckets`。资金仅在 `lockedUntil + unbondingDelay` 后可提取。
    *   **投票权**: 计算方式为 `activeStake`（如果已锁定）+ `effectivePending`（有效待处理金额）。使用二分查找 (O(log n)) 来确定任何给定时间的有效待处理质押。

*   **`ValidatorManagement.sol` (验证者管理)**
    *   **角色**: 管理活跃验证者集合 (Active Validator Set)。
    *   **生命周期**:
        *   `INACTIVE` (非活跃) → `joinValidatorSet` → `PENDING_ACTIVE` (待激活)
        *   `PENDING_ACTIVE` → `onNewEpoch` → `ACTIVE` (活跃)
        *   `ACTIVE` → `leaveValidatorSet` → `PENDING_INACTIVE` (待停用)
        *   `PENDING_INACTIVE` → `onNewEpoch` → `INACTIVE`
    *   **安全性**: 强制执行最低保证金 (Minimum Bond)，防止最后一个验证者退出，并自动为活跃验证者续期锁仓。

### 1.2 质押生命周期

1.  **创建**: 用户调用 `Staking.createPool()` 并存入初始质押金。
2.  **质押**: 质押者调用 `StakePool.addStake()`。
    *   锁仓时间确保至少延续到未来的 `minLockup`。
3.  **验证者注册 (可选)**: 运营者调用 `ValidatorManagement.registerValidator()`。
4.  **激活**: 运营者调用 `ValidatorManagement.joinValidatorSet()`。
    *   状态变为 `PENDING_ACTIVE`。
    *   在下一个 Epoch (纪元)，`onNewEpoch` 将其移动到 `ACTIVE`。
5.  **维护**:
    *   当处于 `ACTIVE` 状态时，`ValidatorManagement` 每个 Epoch 都会调用 `Staking.renewPoolLockup()` 以确保投票权保持有效。
6.  **解除质押 (Unstaking)**:
    *   **作为活跃验证者**: 质押金额不能低于 `minimumBond`。会检查 `ValidatorManagement` 中的状态。
    *   **普通情况**: `unstake(amount)` 将代币移动到具有*当前* `lockedUntil` 的 "Pending Bucket" (待处理桶) 中。
7.  **提取 (Withdrawal)**:
    *   等待直到 `now > bucket.lockedUntil + unbondingDelay`。
    *   调用 `withdrawAvailable()` 或 `unstakeAndWithdraw()`。

---

## 2. 当前 E2E 测试覆盖率分析

根据 `@gravity-sdk/gravity_e2e`，目前的测试套件主要集中在 **网络拓扑** 和 **共识一致性** 上。

*   **`test_validator_add_remove.py`**:
    *   **流程**: 部署节点 -> 加入 (通过 CLI) -> 启动节点 -> 验证数量 -> 退出 (通过 CLI) -> 验证数量。
    *   **覆盖**: 覆盖了 `ValidatorManagement` 生命周期 (`join`, `leave`, `onNewEpoch` 隐式覆盖)。
    *   **局限**: 依赖 `gravity_cli`。它测试了验证者加入/退出的 "Happy Path" (正常路径)，但将 `StakePool` 视为黑盒。

*   **`test_epoch_consistency.py`**:
    *   **流程**: 运行链 N 个 Epoch。
    *   **覆盖**: 验证 Epoch 之间的 LedgerInfo 和 QC 连续性。
    *   **局限**: 不直接与质押机制交互。

*   **缺失的覆盖**:
    *   **直接 StakePool 交互**: 没有通过合约调用测试 `addStake`, `unstake`, `withdraw`。
    *   **锁仓机制**: 没有验证 `lockedUntil` 逻辑、自动续期验证或具体的解绑延迟 (unbonding delay)。
    *   **边界情况**:
        *   解除所有资金质押（剩余活跃资金为 0）。
        *   部分解除质押，产生多个待处理桶 (pending buckets)。
        *   尝试在 `lockedUntil` 之前提取。
        *   锁仓时间的溢出检查。

---

## 3. 测试建议

为了完全确保质押协议的安全性，我们建议添加一个 `tests/contracts/test_staking_flow.py` 测试套件，使用 `web3.py` 直接与部署的合约进行交互。

### 3.1 建议的测试用例

#### A. 基础质押池生命周期 (用户流程)
1.  **创建池**: 验证 `Staking.createPool` 触发事件并部署合约。
2.  **增加质押**: 调用 `addStake` 并验证 `activeStake` 增加且 `lockedUntil` 更新。
3.  **解除质押**: 调用 `unstake` 并验证 `activeStake` 减少且 `getTotalPending` 增加。
4.  **提取失败**: 立即尝试 `withdrawAvailable` (应返回 0)。
5.  **时间旅行**: 将时间快进超过 `lockedUntil + unbondingDelay`。
6.  **提取成功**: 调用 `withdrawAvailable` 并验证 ETH 余额增加。

#### B. 验证者限制
1.  **最低保证金**: 当池是 `ACTIVE` 验证者时，尝试解除质押导致 `activeStake < minimumBond`。预期 revert (回滚)。
2.  **活跃时提取**: 尝试在 `ACTIVE` 状态下提取待处理资金。（如果待处理桶足够旧，应该允许，但新的解除质押受限）。

#### C. 锁仓计算与桶机制
1.  **多个桶**: 在不同时间执行多次 `unstake` 操作（如果在其间调用了 `renewLockUntil`），以创建多个待处理桶。
2.  **投票权**: 验证 `getVotingPower(now)` 匹配预期的曲线（活跃 + 有效待处理）。

### 3.2 实施策略

不要仅依赖 `gravity_cli`。使用 `GravityClient` 的 `web3` 实例或原始调用与 ABI 交互。

1.  **加载 ABI**: 你需要 `Staking.sol` 和 `StakePool.sol` 的 ABI。
2.  **合约 Helper**: 在 `gravity_e2e/utils` 中为这些合约创建 Python 包装器。

---


## 4. 具体测试用例 (Specific Test Cases)

我们重点测试 **资金安全 (Safety)** 和 **协议约束 (Constraints)**。

### Case 1: 完整的质押与提取生命周期 (Staking Lifecycle)
*   **测试内容**: 验证普通用户从“进”到“出”的完整流程，确保存入的钱能取出来，且锁仓期内取不出来。
*   **具体步骤**:
    1.  用户调用 `Staking.createPool()` 创建池。
    2.  调用 `StakePool.addStake()` 存入 100 ETH。
    3.  **立即尝试** 调用 `StakePool.withdrawAvailable()`。
    4.  **等待** (或模拟时间流逝) 直到 `lockedUntil + unbondingDelay` 之后。
    5.  再次调用 `StakePool.withdrawAvailable()`。
*   **验证点**:
    *   步骤 3 必须**失败**（或提取 0），验证锁仓机制有效。
    *   步骤 5 必须**成功**，验证资金最终可赎回。

### Case 2: 提前解除质押的限制 (Early Unstake Restrictions)
*   **测试内容**: 验证 `unstake` 操作是否正确触发“解绑期”，而不是立即释放资金。
*   **具体步骤**:
    1.  在已有质押的池中，调用 `StakePool.unstake(20 ETH)`。
    2.  立即尝试提取这 20 ETH。
*   **验证点**:
    *   `unstake` 调用应成功，资金状态从 `activeStake` 变为 `pending`。
    *   **立即提取应失败**。必须验证资金确实被锁定在“待处理桶”中，直到解绑期结束。

### Case 3: 验证者最低保证金约束 (Validator Minimum Bond)
*   **测试内容**: 验证协议是否强制活跃验证者维持最低质押金，防止恶作剧或攻击。
*   **具体步骤**:
    1.  注册一个验证者并使其加入集合 (`ACTIVE` 状态)。
    2.  尝试调用 `unstake`，使得剩余 `activeStake` 低于 `minimumBond` (例如 1000 ETH)。
*   **验证点**:
    *   交易必须 **Revert (回滚)**。
    *   验证者必须先退出集合 (`leaveValidatorSet`) 才能取出这部分底仓。

### Case 4: 锁仓权益计算 (Voting Power Calculation)
*   **测试内容**: 验证投票权计算逻辑。即使解除质押，在锁仓期结束前，投票权应仍被保留。
*   **具体步骤**:
    1.  创建池并质押 100 ETH。
    2.  解除质押 50 ETH（这会生成一个待处理桶）。
    3.  立即调用 `StakePool.getVotingPower()`。
*   **验证点**:
    *   投票权应仍为 100 (50 Active + 50 Pending)。
    *   这是为了防止验证者通过“解除质押”来逃避锁仓惩罚，同时又想保留投票权的攻击行为。

### Case 5: 委托模式 / 角色分离 (Delegation / Separated Roles)
*   **测试内容**: 验证资金方 (Staker) 和运营方 (Operator) 的权限隔离，确保“第三方运营”场景下的资金安全。
*   **具体步骤**:
    1.  创建两个不同的账户: `Alice_Staker` 和 `Bob_Operator`。
    2.  `Alice_Staker` 调用 `createPool`，指定 `_staker = Alice_Staker`, `_operator = Bob_Operator`。
    3.  **权限验证**:
        *   `Alice_Staker` 调用 `addStake` -> **成功** (资金方存钱)。
        *   `Bob_Operator` 尝试调用 `withdrawAvailable` -> **失败/Revert** (运营方不能动钱)。
        *   `Bob_Operator` 尝试调用 `rotateConsensusKey` (或其他运营指令) -> **成功** (运营方管理节点)。
        *   `Alice_Staker` 尝试调用 `rotateConsensusKey` -> **失败/Revert** (资金方不能干扰节点运维)。
*   **验证点**:
    *   资金控制权完全归 `Staker`。
    *   运营控制权完全归 `Operator`。

### Case 6: 运维与管理功能 (Operations & Maintenance)
*   **测试内容**: 覆盖合约的剩余管理功能，确保灵活性和完整性。
*   **具体步骤**:
    1.  **更改角色**:
        *   Calling `StakePool.setOperator(NewOperator)`: 验证旧 Operator 失效，新 Operator 生效。
        *   Calling `StakePool.setVoter(NewVoter)`: 验证投票权代理变更。
    2.  **锁仓管理**:
        *   Calling `StakePool.renewLockUntil(newTime)`: 验证在不增加质押的情况下手动延长锁仓。
    3.  **便捷函数**:
        *   Calling `StakePool.unstakeAndWithdraw()`: 验证这是一个原子操作（解质押+提取），简化用户 UX。
    4.  **收益接收**:
        *   Calling `ValidatorManagement.setFeeRecipient(newRecipient)`: 验证验证者可以更改出块奖励接收地址。
*   **验证点**:
    *   所有 Setter 函数正确触发 Event 并更新状态。
    *   非 Owner/Operator 调用这些函数应 Revert。



## 5. 代码实现示例 (Python)

参考 `test_erc20.py` 的风格，这是一个具体的 Python 测试代码草案。你可以将其保存为 `gravity_e2e/tests/test_cases/test_staking.py`。

```python
import logging
import time
from gravity_e2e.helpers.test_helpers import RunHelper, TestResult, test_case
from gravity_e2e.utils.transaction_builder import TransactionBuilder, TransactionOptions, run_sync
from gravity_e2e.utils.exceptions import TransactionError

LOG = logging.getLogger(__name__)

# 系统合约地址 (需与 genesis 配置一致)
STAKING_PROXY_ADDRESS = "0x0000000000000000000000000000000000001001" 

# 简化的 ABI 定义 (实际使用请从 json 文件加载)
STAKING_ABI = [{"inputs":[{"internalType":"address","name":"_owner","type":"address"},{"internalType":"address","name":"_staker","type":"address"},{"internalType":"address","name":"_operator","type":"address"},{"internalType":"address","name":"_voter","type":"address"},{"internalType":"uint256","name":"_lockedUntil","type":"uint256"}],"name":"createPool","outputs":[{"internalType":"address","name":"pool","type":"address"}],"stateMutability":"payable","type":"function"}, {"anonymous":False,"inputs":[{"indexed":True,"internalType":"address","name":"pool","type":"address"}],"name":"PoolCreated","type":"event"}]
STAKE_POOL_ABI = [{"inputs":[],"name":"activeStake","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"}, {"inputs":[],"name":"withdrawAvailable","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"nonpayable","type":"function"}, {"inputs":[],"name":"getClaimableAmount","outputs":[{"internalType":"uint256","name":"","type":"uint256"}],"stateMutability":"view","type":"function"}]

@test_case
async def test_staking_lifecycle(run_helper: RunHelper, test_result: TestResult):
    """
    Test Case 1: 验证基本的质押生命周期和锁仓强制执行
    """
    LOG.info("Starting Staking Lifecycle Test")

    try:
        w3 = run_helper.client.web3
        
        # 1. 准备账户
        user = await run_helper.create_test_account("staker", fund_wei=200 * 10**18)
        tx_builder = TransactionBuilder(w3, user['account'])
        staking_contract = w3.eth.contract(address=STAKING_PROXY_ADDRESS, abi=STAKING_ABI)

        # 2. 创建质押池 (Create Pool)
        initial_stake = 100 * 10**18
        lock_until = int(time.time()) + 86400 # 锁仓 1 天

        LOG.info(f"Creating pool with {initial_stake} wei, locked until {lock_until}")

        create_result = await tx_builder.build_and_send_tx(
            to=STAKING_PROXY_ADDRESS,
            data=staking_contract.encode_abi('createPool', [
                user['address'], user['address'], user['address'], user['address'], lock_until
            ]),
            value=initial_stake,
            options=TransactionOptions(gas_limit=5000000)
        )

        if not create_result.success:
            raise TransactionError(f"Create pool failed: {create_result.error}")

        # 3. 获取新部署的池地址 (解析 PoolCreated 事件)
        receipt = w3.eth.get_transaction_receipt(create_result.tx_hash)
        logs = staking_contract.events.PoolCreated().process_receipt(receipt)
        if not logs:
            raise Exception("PoolCreated event not found")
        
        pool_address = logs[0]['args']['pool']
        LOG.info(f"Stake Pool deployed at: {pool_address}")
        pool_contract = w3.eth.contract(address=pool_address, abi=STAKE_POOL_ABI)

        # 4. 验证初始状态
        active_stake = await run_sync(pool_contract.functions.activeStake().call)
        if active_stake != initial_stake:
            raise Exception(f"Stake mismatch. Expected {initial_stake}, got {active_stake}")

        # 5. 验证锁仓 (Lock Enforcement)
        # 尝试查询可提取金额，应为 0
        claimable = await run_sync(pool_contract.functions.getClaimableAmount().call)
        if claimable != 0:
            raise Exception(f"Funds should be locked but claimable amount is {claimable}")

        # 尝试提取
        balance_before = w3.eth.get_balance(user['address'])
        await tx_builder.build_and_send_tx(
            to=pool_address,
            data=pool_contract.encode_abi('withdrawAvailable', []),
            options=TransactionOptions(gas_limit=200000)
        )
        balance_after = w3.eth.get_balance(user['address'])
        
        # 余额不应增加 (只扣除 Gas)
        if balance_after > balance_before:
             raise Exception("Withdrawal succeeded despite lockup!")

        LOG.info("Lockup correctly enforced.")
        
        test_result.mark_success(pool_address=pool_address, active_stake=active_stake)

    except Exception as e:
        test_result.mark_failure(error=str(e))
        raise
```

