import asyncio
import json
import logging
from pathlib import Path
from typing import Dict

from ...helpers.test_helpers import RunHelper, TestResult, test_case

LOG = logging.getLogger(__name__)

# Load contract info
CONTRACTS_DIR = Path(__file__).parent.parent.parent.parent / "contracts_data"
SIMPLE_TOKEN_PATH = CONTRACTS_DIR / "SimpleToken.json"

# 加载合约数据
if SIMPLE_TOKEN_PATH.exists():
    with open(SIMPLE_TOKEN_PATH, 'r') as f:
        contract_data = json.load(f)
        SIMPLE_TOKEN_BYTECODE = contract_data["bytecode"]
        SIMPLE_TOKEN_ABI = contract_data["abi"]
else:
    # 如果文件不存在，使用默认值
    SIMPLE_TOKEN_BYTECODE = ""
    SIMPLE_TOKEN_ABI = []

# 加载构造函数编码数据
DEPLOYMENT_DATA_PATH = CONTRACTS_DIR / "deployment_data.json"
CONSTRUCTOR_DATA = ""
if DEPLOYMENT_DATA_PATH.exists():
    with open(DEPLOYMENT_DATA_PATH, 'r') as f:
        deployment_data = json.load(f)
        CONSTRUCTOR_DATA = deployment_data.get("constructor_data", "")


def get_function_selector(function_signature: str) -> str:
    """Get function selector from signature"""
    import hashlib
    return "0x" + hashlib.sha256(function_signature.encode()).hexdigest()[:8]


def encode_uint256(value: int) -> str:
    """Encode uint256 to 32 bytes hex string"""
    return format(value, '064x')


@test_case
async def test_erc20_deploy_and_transfer(run_helper: RunHelper, test_result: TestResult):
    """测试 ERC20 代币部署和转账"""
    LOG.info("Starting ERC20 token deployment and transfer test")
    
    if not SIMPLE_TOKEN_BYTECODE:
        raise RuntimeError("SimpleToken contract not compiled. Please run forge build first.")
    
    # 1. 创建测试账户
    deployer = await run_helper.create_test_account("deployer", fund_wei=10 * 10**18)  # 10 ETH
    recipient = await run_helper.create_test_account("recipient")
    
    LOG.info(f"Deployer: {deployer['address']}")
    LOG.info(f"Recipient: {recipient['address']}")
    
    # 2. 部署 SimpleToken 合约
    # 使用预生成的构造函数数据
    if not CONSTRUCTOR_DATA:
        raise RuntimeError("Constructor data not found. Please run encode_deploy.py first.")
    
    # 完整的部署字节码
    full_bytecode = SIMPLE_TOKEN_BYTECODE + CONSTRUCTOR_DATA
    
    # 获取部署者 nonce
    nonce = await run_helper.client.get_transaction_count(deployer["address"])
    
    # 获取 gas 价格
    gas_price = await run_helper.client.get_gas_price()
    gas_limit = 3000000  # ERC20 合约需要更多 gas
    
    # 构建部署交易
    deploy_tx_data = {
        "data": full_bytecode if not full_bytecode.startswith("0x") else full_bytecode[2:],
        "gas": hex(gas_limit),
        "gasPrice": hex(gas_price),
        "nonce": hex(nonce),
        "chainId": hex(await run_helper.client.get_chain_id())
    }
    
    # 签名部署交易
    from eth_account import Account
    # 确保私钥格式正确（不带 0x 前缀）
    private_key = deployer["private_key"]
    if private_key.startswith("0x"):
        private_key = private_key[2:]
    
    signed_deploy_tx = Account.sign_transaction(
        deploy_tx_data,
        private_key
    )
    
    # 发送部署交易
    deploy_tx_hash = await run_helper.client.send_raw_transaction(signed_deploy_tx.raw_transaction)
    LOG.info(f"ERC20 deployment transaction sent: {deploy_tx_hash}")
    
    # 等待部署确认
    deploy_receipt = await run_helper.client.wait_for_transaction_receipt(deploy_tx_hash, timeout=120)
    
    if deploy_receipt["status"] != "0x1":
        raise RuntimeError(f"ERC20 deployment failed: {deploy_receipt}")
    
    # 获取合约地址
    contract_address = deploy_receipt.get("contractAddress")
    if not contract_address:
        raise RuntimeError("No contract address in deployment receipt")
    
    LOG.info(f"ERC20 contract deployed at: {contract_address}")
    LOG.info(f"Deployment gas used: {int(deploy_receipt.get('gasUsed', '0x0'), 16)}")
    
    # 3. 测试 ERC20 功能
    
    # 获取函数选择器
    # name() -> 0x06fdde03
    # symbol() -> 0x95d89b41  
    # decimals() -> 0x313ce567
    # totalSupply() -> 0x18160ddd
    # balanceOf(address) -> 0x70a08231
    # transfer(address,uint256) -> 0xa9059cbb
    
    # 测试 name()
    name_result = await run_helper.client.call(
        to=contract_address,
        data="0x06fdde03"
    )
    LOG.info(f"Token name result: {name_result}")
    
    # 测试 symbol()
    symbol_result = await run_helper.client.call(
        to=contract_address,
        data="0x95d89b41"
    )
    LOG.info(f"Token symbol result: {symbol_result}")
    
    # 测试 decimals()
    decimals_result = await run_helper.client.call(
        to=contract_address,
        data="0x313ce567"
    )
    decimals_int = int(decimals_result, 16)
    LOG.info(f"Token decimals: {decimals_int}")
    
    # 测试 totalSupply()
    total_supply_result = await run_helper.client.call(
        to=contract_address,
        data="0x18160ddd"
    )
    total_supply_int = int(total_supply_result, 16)
    total_supply_tokens = total_supply_int / 10**18
    LOG.info(f"Total supply: {total_supply_tokens:.2f} tokens")
    
    # 测试 balanceOf(deployer)
    balance_of_deployer_data = "0x70a08231" + deployer["address"][2:].rjust(64, '0')
    deployer_balance_result = await run_helper.client.call(
        to=contract_address,
        data=balance_of_deployer_data
    )
    deployer_balance_int = int(deployer_balance_result, 16)
    deployer_balance_tokens = deployer_balance_int / 10**18
    LOG.info(f"Deployer balance: {deployer_balance_tokens:.2f} tokens")
    
    # 验证初始余额等于总供应量
    if deployer_balance_int != total_supply_int:
        raise RuntimeError(
            f"Initial balance mismatch: expected {total_supply_int}, got {deployer_balance_int}"
        )
    
    # 4. 测试转账功能
    transfer_amount = 100 * 10**18  # 100 tokens
    
    # 构建 transfer 调用数据
    # transfer(address to, uint256 amount)
    recipient_address_padded = recipient["address"][2:].rjust(64, '0')
    amount_padded = encode_uint256(transfer_amount)
    transfer_data = "0xa9059cbb" + recipient_address_padded + amount_padded
    
    # 获取新的 nonce
    nonce = await run_helper.client.get_transaction_count(deployer["address"])
    
    # 构建转账交易
    transfer_tx_data = {
        "to": contract_address,
        "data": transfer_data,
        "gas": hex(200000),
        "gasPrice": hex(gas_price),
        "nonce": hex(nonce),
        "chainId": hex(await run_helper.client.get_chain_id())
    }
    
    # 签名转账交易
    # 确保私钥格式正确（不带 0x 前缀）
    private_key = deployer["private_key"]
    if private_key.startswith("0x"):
        private_key = private_key[2:]
    
    signed_transfer_tx = Account.sign_transaction(
        transfer_tx_data,
        private_key
    )
    
    # 发送转账交易
    transfer_tx_hash = await run_helper.client.send_raw_transaction(signed_transfer_tx.raw_transaction)
    LOG.info(f"Transfer transaction sent: {transfer_tx_hash}")
    
    # 等待转账确认
    transfer_receipt = await run_helper.client.wait_for_transaction_receipt(transfer_tx_hash, timeout=60)
    
    if transfer_receipt["status"] != "0x1":
        raise RuntimeError(f"Transfer transaction failed: {transfer_receipt}")
    
    LOG.info("Transfer successful!")
    
    # 5. 验证转账后的余额
    recipient_balance_of_data = "0x70a08231" + recipient["address"][2:].rjust(64, '0')
    recipient_balance_result = await run_helper.client.call(
        to=contract_address,
        data=recipient_balance_of_data
    )
    recipient_balance_int = int(recipient_balance_result, 16)
    recipient_balance_tokens = recipient_balance_int / 10**18
    LOG.info(f"Recipient balance after transfer: {recipient_balance_tokens:.2f} tokens")
    
    # 检查收款人余额
    if recipient_balance_int != transfer_amount:
        raise RuntimeError(
            f"Recipient balance mismatch: expected {transfer_amount}, got {recipient_balance_int}"
        )
    
    # 检查部署者余额
    deployer_balance_result = await run_helper.client.call(
        to=contract_address,
        data=balance_of_deployer_data
    )
    new_deployer_balance_int = int(deployer_balance_result, 16)
    new_deployer_balance_tokens = new_deployer_balance_int / 10**18
    LOG.info(f"Deployer balance after transfer: {new_deployer_balance_tokens:.2f} tokens")
    
    expected_deployer_balance = deployer_balance_int - transfer_amount
    if new_deployer_balance_int != expected_deployer_balance:
        raise RuntimeError(
            f"Deployer balance mismatch: expected {expected_deployer_balance}, got {new_deployer_balance_int}"
        )
    
    # 4. 测试转账功能（暂时跳过，只验证部署成功）
    transfer_amount = 100 * 10**18  # 100 tokens
    
    # 记录测试结果
    test_result.mark_success(
        contract_address=contract_address,
        deployment_tx_hash=deploy_tx_hash,
        deployment_gas_used=int(deploy_receipt.get("gasUsed", "0x0"), 16),
        token_name="TestToken",
        token_symbol="TEST",
        token_decimals=decimals_int,
        total_supply=total_supply_int,
        deployer_balance=deployer_balance_int
    )
    
    LOG.info("ERC20 deployment test completed successfully!")
    LOG.info(f"✅ Contract deployed at: {contract_address}")
    LOG.info(f"✅ Token: {decimals_int} decimal places")
    LOG.info(f"✅ Total Supply: {total_supply_int / 10**18} TEST")
    LOG.info(f"✅ Deployer balance: {deployer_balance_int / 10**18} TEST")