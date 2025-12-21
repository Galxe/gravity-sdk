"""
Delegation and claim test
委托和取回测试流程
"""
import asyncio
import json
import logging
import subprocess
import time
from typing import Optional, Tuple, Dict

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


def extract_transaction_hash(output: str) -> str:
    """
    从 cast send 的输出中提取 transactionHash
    
    Args:
        output: cast send 命令的输出（可能是 JSON 或纯文本）
    
    Returns:
        交易哈希
    """
    output = output.strip()
    
    # 尝试解析 JSON
    try:
        data = json.loads(output)
        if isinstance(data, dict) and "transactionHash" in data:
            return data["transactionHash"]
        elif isinstance(data, str):
            # 如果整个输出就是一个哈希
            if output.startswith("0x") and len(output) == 66:
                return output
    except json.JSONDecodeError:
        pass
    
    # 如果不是 JSON，尝试从文本中提取
    # cast send 可能输出类似: "blockHash: 0x... transactionHash: 0x..."
    if "transactionHash" in output:
        parts = output.split("transactionHash")
        if len(parts) > 1:
            hash_part = parts[1].strip()
            # 提取 0x 开头的哈希
            if hash_part.startswith("0x"):
                hash_value = hash_part.split()[0] if " " in hash_part else hash_part
                if len(hash_value) == 66:  # 0x + 64 字符
                    return hash_value
    
    # 如果输出本身就是一个哈希
    if output.startswith("0x") and len(output) == 66:
        return output
    
    raise ValueError(f"无法从输出中提取交易哈希: {output}")


def parse_transaction_gas_info(tx_output: str) -> Dict[str, int]:
    """
    从 cast tx 的输出中解析 gas 信息
    
    Args:
        tx_output: cast tx 命令的输出（JSON 格式）
    
    Returns:
        包含 effectiveGasPrice 和 gasUsed 的字典
    """
    try:
        tx_data = json.loads(tx_output)
        
        # 尝试从 receipt 中获取
        receipt = tx_data.get("receipt", {})
        if not receipt:
            # 如果没有 receipt，尝试从根级别获取
            receipt = tx_data
        
        # 获取 gasUsed（可能是十六进制字符串或数字）
        gas_used_hex = receipt.get("gasUsed") or receipt.get("gas_used")
        if gas_used_hex is None:
            raise ValueError("未找到 gasUsed 字段")
        
        if isinstance(gas_used_hex, str):
            gas_used = int(gas_used_hex, 16) if gas_used_hex.startswith("0x") else int(gas_used_hex)
        else:
            gas_used = int(gas_used_hex)
        
        # 获取 effectiveGasPrice
        effective_gas_price_hex = receipt.get("effectiveGasPrice") or receipt.get("effective_gas_price")
        if effective_gas_price_hex is None:
            # 如果没有 effectiveGasPrice，尝试使用 gasPrice
            effective_gas_price_hex = receipt.get("gasPrice") or receipt.get("gas_price")
        
        if effective_gas_price_hex is None:
            raise ValueError("未找到 effectiveGasPrice 或 gasPrice 字段")
        
        if isinstance(effective_gas_price_hex, str):
            effective_gas_price = int(effective_gas_price_hex, 16) if effective_gas_price_hex.startswith("0x") else int(effective_gas_price_hex)
        else:
            effective_gas_price = int(effective_gas_price_hex)
        
        return {
            "gasUsed": gas_used,
            "effectiveGasPrice": effective_gas_price,
            "totalFee": gas_used * effective_gas_price
        }
    except (json.JSONDecodeError, KeyError, ValueError) as e:
        raise ValueError(f"无法解析交易信息: {e}, 输出: {tx_output}")


@test_case
async def test_wait_balance_100(
    run_helper: RunHelper,
    test_result: TestResult
):
    """
    完整的委托和取回测试流程（包含转账和总额比对）
    
    测试步骤:
    1. 启动并校验委托者是否是 100 ETH
    2. 启动并校验验证者是否是 40000 ETH
    3. 从验证者转账 50 ether 给委托者，并获取 transactionHash
    4. 校验委托者是否是 150 ETH
    5. 校验验证者是否小于 39950 ETH
    6. 查询 transactionHash，获取 gas 费 (effectiveGasPrice * GasUsed)
    7. 计算总余额 + gas 费是否接近 40100 ETH
    8. 委托 50 ether 给验证者，并等待 10 s
    9. 获取验证者的 StakeCredit 地址
    10. 获取委托者的 PooledG 余额
    11. 将 PooledG 转换为 ether，并校验是否等于 50
    12. 校验委托者余额小于 100 ETH
    13. 取消委托，并等待 2 mins
    14. 取回 ClaimableAmount
    15. 校验委托者余额是否大于 149 ETH
    16. 提取未记录的 token
    17. 校验验证者余额是否大于 39950 ETH
    """
    LOG.info("=" * 70)
    LOG.info("Test: Delegation and Claim Flow")
    LOG.info("=" * 70)
    
    # 配置参数
    delegator_address = "0x6954476eAe13Bd072D9f19406A6B9543514f765C"
    validator_address = "0xAEd2a948892475F800A337427B3275D190EA3e94"
    private_key = "0xf1e579ef20d8131c9735afc1f32af9fb7d03c921f260b9b10c0cedf7ba574576"
    validator_private_key = "0x047a5466f6f9e08c8bcc56213d6530d517c1ef126eefbbdf85ffe8d893ed0e9f"  # 用于提取未记录 token 的私钥
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
    
    # 步骤 2: 校验验证者初始余额是否为 40000 ETH
    LOG.info("\n[步骤 2] 校验验证者初始余额是否为 40000 ETH")
    LOG.info("-" * 70)
    
    validator_check_interval = 10  # 检查间隔：10 秒
    validator_max_wait_time = 120  # 最长等待时间：2 分钟（120 秒）
    
    validator_start_time = time.time()
    validator_last_balance = None
    
    while True:
        elapsed_time = time.time() - validator_start_time
        if elapsed_time >= validator_max_wait_time:
            raise TimeoutError(
                f"等待超时！在 {validator_max_wait_time} 秒内验证者余额未达到 40000 ETH。"
                f"当前余额: {validator_last_balance} ETH"
            )
        
        cmd = ["cast", "balance", validator_address, "--ether", "--rpc-url", rpc_url]
        success, stdout, stderr = run_cast_command(cmd)
        
        if not success:
            LOG.warning(f"cast balance 命令执行失败: {stderr}")
            await asyncio.sleep(validator_check_interval)
            continue
        
        try:
            current_balance = float(stdout)
        except ValueError:
            LOG.warning(f"无法解析余额: {stdout}")
            await asyncio.sleep(validator_check_interval)
            continue
        
        validator_last_balance = current_balance
        elapsed = int(elapsed_time)
        
        LOG.info(f"[{elapsed}s] 验证者当前余额: {current_balance} ETH (目标: 40000 ETH)")
        
        if current_balance >= 40000.0:
            LOG.info(f"✅ 步骤 2 完成：验证者余额已达到 40000 ETH")
            break
        
        if current_balance == 0:
            LOG.info(f"余额为 0，等待 {validator_check_interval} 秒后重试...")
        else:
            LOG.info(f"余额未达到目标，等待 {validator_check_interval} 秒后重试...")
        
        await asyncio.sleep(validator_check_interval)
    
    # 步骤 3: 从验证者转账 50 ether 给委托者，并获取 transactionHash
    LOG.info("\n[步骤 3] 从验证者转账 50 ether 给委托者")
    LOG.info("-" * 70)
    
    cmd = [
        "cast", "send",
        delegator_address,
        "--value", "50ether",
        "--priority-gas-price", "1.5gwei",
        "--gas-price", "50gwei",
        "--private-key", validator_private_key,
        "--rpc-url", rpc_url
    ]
    
    success, stdout, stderr = run_cast_command(cmd, timeout=60)
    if not success:
        raise RuntimeError(f"转账交易失败: {stderr}")
    
    # 提取交易哈希
    try:
        transaction_hash = extract_transaction_hash(stdout)
        LOG.info(f"✅ 转账交易已发送，交易哈希: {transaction_hash}")
    except ValueError as e:
        raise RuntimeError(f"无法提取交易哈希: {e}, 输出: {stdout}")
    
    LOG.info("等待交易确认...")
    await asyncio.sleep(10)
    
    # 步骤 4: 校验委托者是否是 150 ETH
    LOG.info("\n[步骤 4] 校验委托者是否是 150 ETH")
    LOG.info("-" * 70)
    
    check_interval = 10
    max_wait_time = 120
    start_time = time.time()
    
    while True:
        elapsed_time = time.time() - start_time
        if elapsed_time >= max_wait_time:
            raise TimeoutError(f"等待超时！在 {max_wait_time} 秒内委托者余额未达到 150 ETH")
        
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
        
        elapsed = int(elapsed_time)
        LOG.info(f"[{elapsed}s] 委托者当前余额: {current_balance} ETH (目标: 150 ETH)")
        
        if current_balance >= 150.0:
            LOG.info(f"✅ 步骤 4 完成：委托者余额已达到 150 ETH")
            break
        
        await asyncio.sleep(check_interval)
    
    # 步骤 5: 校验验证者是否小于 39950 ETH
    LOG.info("\n[步骤 5] 校验验证者是否小于 39950 ETH")
    LOG.info("-" * 70)
    
    cmd = ["cast", "balance", validator_address, "--ether", "--rpc-url", rpc_url]
    success, stdout, stderr = run_cast_command(cmd)
    
    if not success:
        raise RuntimeError(f"获取验证者余额失败: {stderr}")
    
    validator_balance_after_transfer = float(stdout)
    LOG.info(f"验证者余额: {validator_balance_after_transfer} ETH")
    
    if validator_balance_after_transfer >= 39950.0:
        raise RuntimeError(
            f"验证者余额应该小于 39950 ETH，但实际为: {validator_balance_after_transfer} ETH"
        )
    
    LOG.info(f"✅ 验证者余额验证通过: {validator_balance_after_transfer} ETH < 39950 ETH")
    
    # 步骤 6: 查询 transactionHash，获取 gas 费
    LOG.info("\n[步骤 6] 查询交易信息，获取 gas 费")
    LOG.info("-" * 70)
    
    cmd = ["cast", "tx", transaction_hash, "--rpc-url", rpc_url]
    success, stdout, stderr = run_cast_command(cmd)
    
    if not success:
        raise RuntimeError(f"查询交易信息失败: {stderr}")
    
    try:
        gas_info = parse_transaction_gas_info(stdout)
        total_fee_wei = gas_info["totalFee"]
        total_fee_ether = total_fee_wei / 10**18
        
        LOG.info(f"Gas Used: {gas_info['gasUsed']}")
        LOG.info(f"Effective Gas Price: {gas_info['effectiveGasPrice']} wei")
        LOG.info(f"Total Fee: {total_fee_wei} wei ({total_fee_ether} ETH)")
    except ValueError as e:
        raise RuntimeError(f"解析 gas 信息失败: {e}")
    
    # 步骤 7: 计算总余额 + gas 费是否接近 40100 ETH
    LOG.info("\n[步骤 7] 计算总余额 + gas 费是否接近 40100 ETH")
    LOG.info("-" * 70)
    
    delegator_balance_wei = int(current_balance * 10**18)
    validator_balance_wei = int(validator_balance_after_transfer * 10**18)
    total_balance_wei = delegator_balance_wei + validator_balance_wei + total_fee_wei
    total_balance_ether = total_balance_wei / 10**18
    
    expected_total = 40100.0
    tolerance = 0.1  # 允许 0.1 ETH 的误差
    
    LOG.info(f"委托者余额: {current_balance} ETH")
    LOG.info(f"验证者余额: {validator_balance_after_transfer} ETH")
    LOG.info(f"Gas 费: {total_fee_ether} ETH")
    LOG.info(f"总余额 + Gas 费: {total_balance_ether} ETH")
    LOG.info(f"期望总额: {expected_total} ETH")
    
    if abs(total_balance_ether - expected_total) > tolerance:
        raise RuntimeError(
            f"总余额不匹配！期望: {expected_total} ETH (±{tolerance} ETH), "
            f"实际: {total_balance_ether} ETH"
        )
    
    LOG.info(f"✅ 总余额验证通过: {total_balance_ether} ETH ≈ {expected_total} ETH")
    
    # 步骤 8: 委托 50 ether 给验证者
    LOG.info("\n[步骤 8] 委托 50 ether 给验证者")
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
    
    # 步骤 9: 获取验证者的 StakeCredit 地址
    LOG.info("\n[步骤 9] 获取验证者的 StakeCredit 地址")
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
    
    # 步骤 10: 获取委托者的 PooledG 余额
    LOG.info("\n[步骤 10] 获取委托者的 PooledG 余额")
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
    
    # 步骤 11: 将 PooledG 转换为 ether，并校验是否等于 50
    LOG.info("\n[步骤 11] 将 PooledG 转换为 ether，并校验是否等于 50")
    LOG.info("-" * 70)
    
    pooled_g_ether = parse_hex_to_ether(pooled_g_hex)
    LOG.info(f"PooledG: {pooled_g_ether} ETH")
    
    # 允许小的误差（0.01 ETH）
    if abs(pooled_g_ether - 50.0) > 0.01:
        raise RuntimeError(
            f"PooledG 余额不匹配！期望: 50 ETH, 实际: {pooled_g_ether} ETH"
        )
    
    LOG.info(f"✅ PooledG 余额正确: {pooled_g_ether} ETH")
    
    # 步骤 12: 校验委托者余额小于 100 ETH
    LOG.info("\n[步骤 12] 校验委托者余额小于 100 ETH")
    LOG.info("-" * 70)
    
    cmd = ["cast", "balance", delegator_address, "--ether", "--rpc-url", rpc_url]
    success, stdout, stderr = run_cast_command(cmd)
    
    if not success:
        raise RuntimeError(f"获取余额失败: {stderr}")
    
    current_balance = float(stdout)
    LOG.info(f"当前余额: {current_balance} ETH")
    
    if current_balance >= 100.0:
        raise RuntimeError(
            f"余额应该小于 100 ETH，但实际为: {current_balance} ETH"
        )
    
    LOG.info(f"✅ 余额验证通过: {current_balance} ETH < 100 ETH")
    
    # 步骤 13: 取消委托，并等待 2 mins
    LOG.info("\n[步骤 13] 取消委托，并等待 2 mins")
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
    
    # 步骤 14: 取回 ClaimableAmount
    LOG.info("\n[步骤 14] 取回 ClaimableAmount")
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
    
    # 步骤 15: 校验委托者余额是否大于 149 ETH
    LOG.info("\n[步骤 15] 校验委托者余额是否大于 149 ETH")
    LOG.info("-" * 70)
    
    cmd = ["cast", "balance", delegator_address, "--ether", "--rpc-url", rpc_url]
    success, stdout, stderr = run_cast_command(cmd)
    
    if not success:
        raise RuntimeError(f"获取余额失败: {stderr}")
    
    final_balance = float(stdout)
    LOG.info(f"最终余额: {final_balance} ETH")
    
    if final_balance <= 149.0:
        raise RuntimeError(
            f"余额应该大于 149 ETH，但实际为: {final_balance} ETH"
        )
    
    LOG.info(f"✅ 余额验证通过: {final_balance} ETH > 149 ETH")
    
    # 步骤 16: 提取未记录的 token
    LOG.info("\n[步骤 16] 提取未记录的 token")
    LOG.info("-" * 70)
    
    cmd = [
        "cast", "send",
        stake_credit_address,
        "extractUnrecordedTokens()",
        "--private-key", validator_private_key,
        "--rpc-url", rpc_url
    ]
    
    success, stdout, stderr = run_cast_command(cmd, timeout=60)
    if not success:
        raise RuntimeError(f"提取未记录 token 交易失败: {stderr}")
    
    LOG.info(f"✅ 提取未记录 token 交易已发送: {stdout}")
    LOG.info("等待交易确认...")
    await asyncio.sleep(10)
    
    # 步骤 17: 校验验证者余额是否大于 39950 ETH
    LOG.info("\n[步骤 17] 校验验证者余额是否大于 39950 ETH")
    LOG.info("-" * 70)
    
    cmd = ["cast", "balance", validator_address, "--ether", "--rpc-url", rpc_url]
    success, stdout, stderr = run_cast_command(cmd)
    
    if not success:
        raise RuntimeError(f"获取验证者余额失败: {stderr}")
    
    validator_final_balance = float(stdout)
    LOG.info(f"验证者最终余额: {validator_final_balance} ETH")
    
    if validator_final_balance <= 39950.0:
        raise RuntimeError(
            f"验证者余额应该大于 39950 ETH，但实际为: {validator_final_balance} ETH"
        )
    
    LOG.info(f"✅ 验证者余额验证通过: {validator_final_balance} ETH > 39950 ETH")
    
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
        stake_credit_address=stake_credit_address,
        validator_initial_balance=validator_last_balance,
        validator_final_balance=validator_final_balance,
        transaction_hash=transaction_hash,
        total_fee_ether=total_fee_ether,
        total_balance_ether=total_balance_ether
    )

