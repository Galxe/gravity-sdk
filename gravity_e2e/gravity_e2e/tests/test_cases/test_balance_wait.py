"""
Delegation and claim test
委托和取回测试流程
"""
import asyncio
import logging
import subprocess
import time
from typing import Optional, Tuple

from ...helpers.test_helpers import RunHelper, TestResult, test_case

LOG = logging.getLogger(__name__)


def run_cast_command(cmd: list, timeout: int = 30) -> Tuple[bool, str, str]:
    """
    执行 cast 命令
    
    Returns:
        (success, stdout, stderr)
    """
    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=timeout
        )
        return result.returncode == 0, result.stdout.strip(), result.stderr.strip()
    except subprocess.TimeoutExpired:
        return False, "", "Command timeout"
    except Exception as e:
        return False, "", str(e)


def parse_hex_to_ether(hex_value: str) -> float:
    """将十六进制值转换为 ETH"""
    # 移除 0x 前缀
    if hex_value.startswith("0x"):
        hex_value = hex_value[2:]
    
    # 转换为整数（wei）
    wei_value = int(hex_value, 16)
    # 转换为 ETH
    ether_value = wei_value / 10**18
    return ether_value


def extract_address_from_bytes32(hex_value: str) -> str:
    """
    从 32 字节的返回值中提取 20 字节的以太坊地址
    
    Args:
        hex_value: 32 字节的十六进制值，例如: 0x000000000000000000000000ccac93b6b9abf19e1f0c2ec9c12d909b85b0ca2e
    
    Returns:
        20 字节的以太坊地址，例如: 0xccac93b6b9abf19e1f0c2ec9c12d909b85b0ca2e
    """
    # 移除 0x 前缀
    if hex_value.startswith("0x"):
        hex_value = hex_value[2:]
    
    # 提取最后 40 个字符（20 字节 = 40 个十六进制字符）
    if len(hex_value) < 40:
        raise ValueError(f"十六进制值长度不足: {hex_value}")
    
    address_hex = hex_value[-40:]  # 取最后 40 个字符
    return "0x" + address_hex


@test_case
async def test_wait_balance_100(
    run_helper: RunHelper,
    test_result: TestResult
):
    """
    完整的委托和取回测试流程
    
    测试步骤:
    1. 启动并校验 0x6954476eAe13Bd072D9f19406A6B9543514f765C 是否是 100 ETH
    2. 委托 50 ether 给验证者，并等待 10 s
    3. 获取验证者的 StakeCredit 地址
    4. 获取委托者的 PooledG 余额
    5. 将 PooledG 转换为 ether，并校验是否等于 50
    6. 校验委托者余额小于 50 ETH
    7. 取消委托，并等待 2 mins
    8. 取回 ClaimableAmount
    9. 校验委托者余额是否大于 99 ETH
    """
    LOG.info("=" * 70)
    LOG.info("Test: Delegation and Claim Flow")
    LOG.info("=" * 70)
    
    # 配置参数
    delegator_address = "0x6954476eAe13Bd072D9f19406A6B9543514f765C"
    validator_address = "0x6e2021ee24e2430da0f5bb9c2ae6c586bf3e0a0f"
    private_key = "0xf1e579ef20d8131c9735afc1f32af9fb7d03c921f260b9b10c0cedf7ba574576"
    rpc_url = "http://localhost:8545"
    staking_contract = "0x0000000000000000000000000000000000002012"
    validator_manager = "0x0000000000000000000000000000000000002013"
    
    # 步骤 1: 校验初始余额是否为 100 ETH
    LOG.info("\n[步骤 1] 校验初始余额是否为 100 ETH")
    LOG.info("-" * 70)
    
    check_interval = 10  # 检查间隔：10 秒
    max_wait_time = 120  # 最长等待时间：2 分钟（120 秒）
    
    start_time = time.time()
    last_balance = None
    
    while True:
        elapsed_time = time.time() - start_time
        if elapsed_time >= max_wait_time:
            raise TimeoutError(
                f"等待超时！在 {max_wait_time} 秒内余额未达到 100 ETH。"
                f"当前余额: {last_balance} ETH"
            )
        
        cmd = ["cast", "balance", delegator_address, "--ether", "--rpc-url", rpc_url]
        success, stdout, stderr = run_cast_command(cmd)
        
        if not success:
            LOG.warning(f"cast balance 命令执行失败: {stderr}")
            await asyncio.sleep(check_interval)
            continue
        
        try:
            current_balance = float(stdout)
        except ValueError:
            LOG.warning(f"无法解析余额: {stdout}")
            await asyncio.sleep(check_interval)
            continue
        
        last_balance = current_balance
        elapsed = int(elapsed_time)
        
        LOG.info(f"[{elapsed}s] 当前余额: {current_balance} ETH (目标: 100 ETH)")
        
        if current_balance >= 100.0:
            LOG.info(f"✅ 步骤 1 完成：余额已达到 100 ETH")
            break
        
        if current_balance == 0:
            LOG.info(f"余额为 0，等待 {check_interval} 秒后重试...")
        else:
            LOG.info(f"余额未达到目标，等待 {check_interval} 秒后重试...")
        
        await asyncio.sleep(check_interval)
    
    # 步骤 2: 委托 50 ether 给验证者
    LOG.info("\n[步骤 2] 委托 50 ether 给验证者")
    LOG.info("-" * 70)
    
    delegate_amount = "50ether"
    cmd = [
        "cast", "send",
        staking_contract,
        "delegate(address)",
        validator_address,
        "--value", delegate_amount,
        "--private-key", private_key,
        "--rpc-url", rpc_url
    ]
    
    success, stdout, stderr = run_cast_command(cmd, timeout=60)
    if not success:
        raise RuntimeError(f"委托交易失败: {stderr}")
    
    LOG.info(f"✅ 委托交易已发送: {stdout}")
    LOG.info("等待 10 秒...")
    await asyncio.sleep(10)
    
    # 步骤 3: 获取验证者的 StakeCredit 地址
    LOG.info("\n[步骤 3] 获取验证者的 StakeCredit 地址")
    LOG.info("-" * 70)
    
    cmd = [
        "cast", "call",
        validator_manager,
        "getValidatorStakeCredit(address)",
        validator_address,
        "--rpc-url", rpc_url
    ]
    
    success, stdout, stderr = run_cast_command(cmd)
    if not success:
        raise RuntimeError(f"获取 StakeCredit 地址失败: {stderr}")
    
    # 从 32 字节返回值中提取 20 字节地址
    raw_address = stdout.strip()
    stake_credit_address = extract_address_from_bytes32(raw_address)
    LOG.info(f"原始返回值: {raw_address}")
    LOG.info(f"✅ StakeCredit 地址: {stake_credit_address}")
    
    # 步骤 4: 获取委托者的 PooledG 余额
    LOG.info("\n[步骤 4] 获取委托者的 PooledG 余额")
    LOG.info("-" * 70)
    
    cmd = [
        "cast", "call",
        stake_credit_address,
        "getPooledGByDelegator(address)",
        delegator_address,
        "--rpc-url", rpc_url
    ]
    
    success, stdout, stderr = run_cast_command(cmd)
    if not success:
        raise RuntimeError(f"获取 PooledG 余额失败: {stderr}")
    
    pooled_g_hex = stdout.strip()
    LOG.info(f"✅ PooledG (hex): {pooled_g_hex}")
    
    # 步骤 5: 将 PooledG 转换为 ether，并校验是否等于 50
    LOG.info("\n[步骤 5] 将 PooledG 转换为 ether，并校验是否等于 50")
    LOG.info("-" * 70)
    
    pooled_g_ether = parse_hex_to_ether(pooled_g_hex)
    LOG.info(f"PooledG: {pooled_g_ether} ETH")
    
    # 允许小的误差（0.01 ETH）
    if abs(pooled_g_ether - 50.0) > 0.01:
        raise RuntimeError(
            f"PooledG 余额不匹配！期望: 50 ETH, 实际: {pooled_g_ether} ETH"
        )
    
    LOG.info(f"✅ PooledG 余额正确: {pooled_g_ether} ETH")
    
    # 步骤 6: 校验委托者余额小于 50 ETH
    LOG.info("\n[步骤 6] 校验委托者余额小于 50 ETH")
    LOG.info("-" * 70)
    
    cmd = ["cast", "balance", delegator_address, "--ether", "--rpc-url", rpc_url]
    success, stdout, stderr = run_cast_command(cmd)
    
    if not success:
        raise RuntimeError(f"获取余额失败: {stderr}")
    
    current_balance = float(stdout)
    LOG.info(f"当前余额: {current_balance} ETH")
    
    if current_balance >= 50.0:
        raise RuntimeError(
            f"余额应该小于 50 ETH，但实际为: {current_balance} ETH"
        )
    
    LOG.info(f"✅ 余额验证通过: {current_balance} ETH < 50 ETH")
    
    # 步骤 7: 取消委托，并等待 2 mins
    LOG.info("\n[步骤 7] 取消委托，并等待 2 mins")
    LOG.info("-" * 70)
    
    # 使用步骤 4 获取的 PooledG 值
    undelegate_amount = pooled_g_hex
    cmd = [
        "cast", "send",
        staking_contract,
        "undelegate(address,uint256)",
        validator_address,
        undelegate_amount,
        "--private-key", private_key,
        "--rpc-url", rpc_url
    ]
    
    success, stdout, stderr = run_cast_command(cmd, timeout=60)
    if not success:
        raise RuntimeError(f"取消委托交易失败: {stderr}")
    
    LOG.info(f"✅ 取消委托交易已发送: {stdout}")
    LOG.info("等待 2 分钟...")
    await asyncio.sleep(120)  # 等待 2 分钟
    
    # 步骤 8: 取回 ClaimableAmount
    LOG.info("\n[步骤 8] 取回 ClaimableAmount")
    LOG.info("-" * 70)
    
    cmd = [
        "cast", "send",
        staking_contract,
        "claim(address)",
        validator_address,
        "--private-key", private_key,
        "--rpc-url", rpc_url
    ]
    
    success, stdout, stderr = run_cast_command(cmd, timeout=60)
    if not success:
        raise RuntimeError(f"取回交易失败: {stderr}")
    
    LOG.info(f"✅ 取回交易已发送: {stdout}")
    LOG.info("等待交易确认...")
    await asyncio.sleep(10)
    
    # 步骤 9: 校验委托者余额是否大于 99 ETH
    LOG.info("\n[步骤 9] 校验委托者余额是否大于 99 ETH")
    LOG.info("-" * 70)
    
    cmd = ["cast", "balance", delegator_address, "--ether", "--rpc-url", rpc_url]
    success, stdout, stderr = run_cast_command(cmd)
    
    if not success:
        raise RuntimeError(f"获取余额失败: {stderr}")
    
    final_balance = float(stdout)
    LOG.info(f"最终余额: {final_balance} ETH")
    
    if final_balance <= 99.0:
        raise RuntimeError(
            f"余额应该大于 99 ETH，但实际为: {final_balance} ETH"
        )
    
    LOG.info(f"✅ 余额验证通过: {final_balance} ETH > 99 ETH")
    
    # 测试完成
    LOG.info("\n" + "=" * 70)
    LOG.info("✅ 所有测试步骤完成！")
    LOG.info("=" * 70)
    
    test_result.mark_success(
        delegator_address=delegator_address,
        validator_address=validator_address,
        initial_balance=last_balance,
        final_balance=final_balance,
        pooled_g_ether=pooled_g_ether,
        stake_credit_address=stake_credit_address
    )

