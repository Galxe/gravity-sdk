import asyncio
import json
import logging
from typing import Dict

from ...helpers.test_helpers import RunHelper, TestResult, test_case

# 简单的存储合约示例
STORAGE_CONTRACT_BYTECODE = "608060405234801561001057600080fd5b50610150806100206000396000f3fe608060405234801561001057600080fd5b50600436106100365760003560e01c80632e64cec11461003b5780636057361d14610059575b600080fd5b610043610075565b60405161005091906100a1565b60405180910390f35b610073600480360381019061006e91906100ed565b61007e565b005b8060008190555050565b8060008190555050565b60008151905061009881600019816002026002026100b2565b50565b6000819050919050565b6000815190506100b38160008160026002026100c9565b50565b6000602082840312156100c857600080fd5b505191905056fea2646970667358221220c5c4d9f6aa8d4a9d8b2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f64736f6c63430008110033"

STORAGE_CONTRACT_ABI = [
    {
        "inputs": [],
        "name": "retrieve",
        "outputs": [
            {
                "internalType": "uint256",
                "name": "",
                "type": "uint256"
            }
        ],
        "stateMutability": "view",
        "type": "function"
    },
    {
        "inputs": [
            {
                "internalType": "uint256",
                "name": "num",
                "type": "uint256"
            }
        ],
        "name": "store",
        "outputs": [],
        "stateMutability": "nonpayable",
        "type": "function"
    }
]

LOG = logging.getLogger(__name__)


@test_case
async def test_contract_deploy_and_interact(run_helper: RunHelper, test_result: TestResult):
    """测试智能合约部署和交互"""
    LOG.info("Starting contract deploy and interaction test")
    
    # 1. 创建测试账户
    deployer = await run_helper.create_test_account("deployer", fund_wei=5 * 10**18)  # 5 ETH
    
    LOG.info(f"Deployer: {deployer['address']}")
    
    # 2. 部署合约
    # 获取部署者 nonce
    nonce = await run_helper.client.get_transaction_count(deployer["address"])
    
    # 获取 gas 价格
    gas_price = await run_helper.client.get_gas_price()
    gas_limit = 200000  # 估算的 gas 限制
    
    # 构建部署交易
    deploy_tx_data = {
        "data": f"0x{STORAGE_CONTRACT_BYTECODE}",
        "gas": hex(gas_limit),
        "gasPrice": hex(gas_price),
        "nonce": hex(nonce),
        "chainId": hex(await run_helper.client.get_chain_id())
    }
    
    # 签名部署交易
    signed_deploy_tx = run_helper.client.web3.eth.account.sign_transaction(
        deploy_tx_data,
        deployer["private_key"]
    )
    
    # 发送部署交易
    deploy_tx_hash = await run_helper.client.send_raw_transaction(signed_deploy_tx.raw_transaction)
    LOG.info(f"Contract deployment transaction sent: {deploy_tx_hash}")
    
    # 等待部署确认
    deploy_receipt = await run_helper.client.wait_for_transaction_receipt(deploy_tx_hash, timeout=60)
    
    if deploy_receipt["status"] != "0x1":
        raise RuntimeError(f"Contract deployment failed: {deploy_receipt}")
    
    # 获取合约地址
    contract_address = deploy_receipt.get("contractAddress")
    if not contract_address:
        raise RuntimeError("No contract address in deployment receipt")
    
    LOG.info(f"Contract deployed at: {contract_address}")
    
    # 3. 测试合约交互
    
    # 初始值应该为 0
    initial_value = await run_helper.client.call(
        to=contract_address,
        data="0x2e64cec1"  # retrieve() 函数的选择器
    )
    initial_value_int = int(initial_value, 16)
    
    if initial_value_int != 0:
        raise RuntimeError(f"Initial value should be 0, got {initial_value_int}")
    
    LOG.info(f"Initial contract value: {initial_value_int}")
    
    # 调用 store() 函数存储值 42
    store_value = 42
    store_data = "0x6057361d" + format(store_value, '064x')  # store(uint256) 的选择器 + 参数
    
    # 获取新的 nonce
    nonce = await run_helper.client.get_transaction_count(deployer["address"])
    
    # 构建存储交易
    store_tx_data = {
        "to": contract_address,
        "data": store_data,
        "gas": hex(100000),  # 估算的 gas 限制
        "gasPrice": hex(gas_price),
        "nonce": hex(nonce),
        "chainId": hex(await run_helper.client.get_chain_id())
    }
    
    # 签名存储交易
    signed_store_tx = run_helper.client.web3.eth.account.sign_transaction(
        store_tx_data,
        deployer["private_key"]
    )
    
    # 发送存储交易
    store_tx_hash = await run_helper.client.send_raw_transaction(signed_store_tx.raw_transaction)
    LOG.info(f"Store transaction sent: {store_tx_hash}")
    
    # 等待存储确认
    store_receipt = await run_helper.client.wait_for_transaction_receipt(store_tx_hash, timeout=60)
    
    if store_receipt["status"] != "0x1":
        raise RuntimeError(f"Store transaction failed: {store_receipt}")
    
    LOG.info(f"Value stored successfully: {store_value}")
    
    # 再次调用 retrieve() 函数验证值
    stored_value = await run_helper.client.call(
        to=contract_address,
        data="0x2e64cec1"  # retrieve() 函数的选择器
    )
    stored_value_int = int(stored_value, 16)
    
    if stored_value_int != store_value:
        raise RuntimeError(
            f"Stored value mismatch: expected {store_value}, got {stored_value_int}"
        )
    
    LOG.info(f"Retrieved contract value: {stored_value_int}")
    
    # 4. 测试多次存储
    test_values = [100, 200, 300]
    
    for value in test_values:
        # 获取新的 nonce
        nonce = await run_helper.client.get_transaction_count(deployer["address"])
        
        # 构建存储交易
        store_data = "0x6057361d" + format(value, '064x')
        
        store_tx_data = {
            "to": contract_address,
            "data": store_data,
            "gas": hex(100000),
            "gasPrice": hex(gas_price),
            "nonce": hex(nonce),
            "chainId": hex(await run_helper.client.get_chain_id())
        }
        
        # 签名并发送
        signed_tx = run_helper.client.web3.eth.account.sign_transaction(
            store_tx_data,
            deployer["private_key"]
        )
        
        tx_hash = await run_helper.client.send_raw_transaction(signed_tx.raw_transaction)
        receipt = await run_helper.client.wait_for_transaction_receipt(tx_hash, timeout=60)
        
        if receipt["status"] != "0x1":
            raise RuntimeError(f"Store transaction failed for value {value}")
        
        # 验证存储的值
        retrieved = await run_helper.client.call(
            to=contract_address,
            data="0x2e64cec1"
        )
        retrieved_int = int(retrieved, 16)
        
        if retrieved_int != value:
            raise RuntimeError(
                f"Value mismatch after storing {value}: retrieved {retrieved_int}"
            )
    
    LOG.info(f"Successfully stored and retrieved values: {test_values}")
    
    # 记录测试结果
    test_result.mark_success(
        contract_address=contract_address,
        deployment_tx_hash=deploy_tx_hash,
        deployment_gas_used=int(deploy_receipt.get("gasUsed", "0x0"), 16),
        final_stored_value=stored_value_int,
        test_values=test_values
    )
    
    LOG.info("Contract deploy and interaction test completed successfully")