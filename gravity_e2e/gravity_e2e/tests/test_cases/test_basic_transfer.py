import asyncio
import logging
from typing import Dict

from ...helpers.test_helpers import RunHelper, TestResult, test_case

LOG = logging.getLogger(__name__)


@test_case
async def test_eth_transfer(run_helper: RunHelper, test_result: TestResult):
    """测试基础 ETH 转账功能"""
    LOG.info("Starting ETH transfer test")
    
    # 1. 创建测试账户
    sender = await run_helper.create_test_account("sender", fund_wei=10**18)  # 1 ETH
    receiver = await run_helper.create_test_account("receiver")
    
    LOG.info(f"Sender: {sender['address']}")
    LOG.info(f"Receiver: {receiver['address']}")
    
    # 2. 获取转账前余额
    sender_balance_before = await run_helper.client.get_balance(sender["address"])
    receiver_balance_before = await run_helper.client.get_balance(receiver["address"])
    
    LOG.info(f"Sender balance before: {run_helper.client.web3.from_wei(sender_balance_before, 'ether')} ETH")
    LOG.info(f"Receiver balance before: {run_helper.client.web3.from_wei(receiver_balance_before, 'ether')} ETH")
    
    # 3. 执行转账
    transfer_amount = 10**17  # 0.1 ETH
    
    # 获取当前 gas 价格
    gas_price = await run_helper.client.get_gas_price()
    gas_limit = 21000
    
    # 计算总费用
    total_cost = transfer_amount + (gas_price * gas_limit)
    
    # 获取 sender nonce
    nonce = await run_helper.client.get_transaction_count(sender["address"])
    
    # 构建交易
    tx_data = {
        "to": receiver["address"],
        "value": hex(transfer_amount),
        "gas": hex(gas_limit),
        "gasPrice": hex(gas_price),
        "nonce": hex(nonce),
        "chainId": hex(await run_helper.client.get_chain_id())
    }
    
    # 签名交易
    signed_tx = run_helper.client.web3.eth.account.sign_transaction(
        tx_data, 
        sender["private_key"]
    )
    
    # 发送交易
    tx_hash = await run_helper.client.send_raw_transaction(signed_tx.raw_transaction)
    LOG.info(f"Transfer transaction sent: {tx_hash}")
    
    # 等待交易确认
    receipt = await run_helper.client.wait_for_transaction_receipt(tx_hash, timeout=60)
    
    if receipt["status"] != "0x1":
        raise RuntimeError(f"Transfer transaction failed: {receipt}")
    
    # 4. 验证转账后余额
    sender_balance_after = await run_helper.client.get_balance(sender["address"])
    receiver_balance_after = await run_helper.client.get_balance(receiver["address"])
    
    LOG.info(f"Sender balance after: {run_helper.client.web3.from_wei(sender_balance_after, 'ether')} ETH")
    LOG.info(f"Receiver balance after: {run_helper.client.web3.from_wei(receiver_balance_after, 'ether')} ETH")
    
    # 验证余额变化
    expected_receiver_balance = receiver_balance_before + transfer_amount
    expected_sender_balance = sender_balance_before - total_cost
    
    # 允许一定的误差（由于可能的手续费变化）
    balance_tolerance = 10**15  # 0.001 ETH
    
    if abs(receiver_balance_after - expected_receiver_balance) > balance_tolerance:
        raise RuntimeError(
            f"Receiver balance mismatch: expected {expected_receiver_balance}, "
            f"got {receiver_balance_after}"
        )
    
    if abs(sender_balance_after - expected_sender_balance) > balance_tolerance:
        raise RuntimeError(
            f"Sender balance mismatch: expected {expected_sender_balance}, "
            f"got {sender_balance_after}"
        )
    
    # 记录测试结果
    test_result.mark_success(
        tx_hash=tx_hash,
        transfer_amount=transfer_amount,
        gas_used=int(receipt.get("gasUsed", "0x0"), 16),
        sender_final_balance=sender_balance_after,
        receiver_final_balance=receiver_balance_after
    )
    
    LOG.info("ETH transfer test completed successfully")