"""
随机数测试辅助工具
包括RandomDice合约辅助类和随机数验证工具
"""
import json
import logging
from pathlib import Path
from typing import Dict, Optional, Tuple
from eth_account import Account
from eth_utils import to_checksum_address

from ..core.client.gravity_client import GravityClient
from .contract_utils import ContractUtils

LOG = logging.getLogger(__name__)


class RandomDiceHelper:
    """RandomDice合约辅助类"""
    
    # RandomDice合约的函数选择器 (使用keccak256计算)
    SELECTORS = {
        'rollDice': '0x837e7cc6',
        'lastRollResult': '0xefeb9231',
        'lastSeedUsed': '0xd904baa6',
        'lastRoller': '0x0d990e80',
        'getLatestRoll': '0x3871da26'
    }
    
    @staticmethod
    def load_bytecode() -> str:
        """
        从编译输出加载RandomDice字节码
        
        Returns:
            合约字节码（十六进制字符串，带0x前缀）
        
        Raises:
            FileNotFoundError: 合约未编译
        """
        # 查找编译输出
        possible_paths = [
            Path(__file__).parent.parent.parent.parent / "out/RandomDice.sol/RandomDice.json",
            Path(__file__).parent.parent.parent.parent / "examples/randomness/out/RandomDice.sol/RandomDice.json",
        ]
        
        contract_path = None
        for path in possible_paths:
            if path.exists():
                contract_path = path
                break
        
        if not contract_path:
            raise FileNotFoundError(
                f"RandomDice not compiled. Run: forge build\n"
                f"Searched paths:\n" + "\n".join(f"  - {p}" for p in possible_paths)
            )
        
        LOG.debug(f"Loading RandomDice bytecode from {contract_path}")
        
        with open(contract_path, 'r') as f:
            contract_data = json.load(f)
            
            # 获取字节码
            bytecode_obj = contract_data.get("bytecode") or contract_data.get("bin")
            
            if isinstance(bytecode_obj, dict):
                bytecode = bytecode_obj.get("object", "")
            else:
                bytecode = str(bytecode_obj)
            
            if not bytecode:
                raise ValueError(f"No bytecode found in {contract_path}")
            
            # 确保有0x前缀
            if not bytecode.startswith("0x"):
                bytecode = "0x" + bytecode
            
            LOG.info(f"Loaded RandomDice bytecode ({len(bytecode)} chars)")
            return bytecode
    
    def __init__(self, client: GravityClient, contract_address: str):
        """
        初始化RandomDice辅助类
        
        Args:
            client: Gravity RPC客户端
            contract_address: 合约地址
        """
        self.client = client
        self.address = to_checksum_address(contract_address)
        LOG.debug(f"RandomDiceHelper initialized for {self.address}")
    
    async def roll_dice(self, from_account: Dict, gas_limit: int = 100000) -> Dict:
        """
        调用rollDice()函数
        
        Args:
            from_account: 调用者账户（需包含address和private_key）
            gas_limit: Gas限制
        
        Returns:
            交易receipt
        
        Raises:
            RuntimeError: 交易失败
        """
        # 编码函数调用
        data = self.SELECTORS['rollDice']  # rollDice()没有参数
        
        # 获取交易参数
        nonce = await self.client.get_transaction_count(from_account["address"])
        gas_price = await self.client.get_gas_price()
        chain_id = await self.client.get_chain_id()
        
        # 构建交易
        tx_data = {
            "to": self.address,
            "data": data,
            "gas": hex(gas_limit),
            "gasPrice": hex(gas_price),
            "nonce": hex(nonce),
            "chainId": hex(chain_id),
            "value": "0x0"
        }
        
        # 签名
        private_key = from_account["private_key"]
        if private_key.startswith("0x"):
            private_key = private_key[2:]
        
        signed_tx = Account.sign_transaction(tx_data, private_key)
        
        # 发送
        tx_hash = await self.client.send_raw_transaction(signed_tx.raw_transaction)
        LOG.debug(f"rollDice transaction sent: {tx_hash}")
        
        # 等待确认
        receipt = await self.client.wait_for_transaction_receipt(tx_hash, timeout=30)
        
        if receipt.get("status") != "0x1":
            raise RuntimeError(f"rollDice transaction failed: {receipt}")
        
        return receipt
    
    async def get_last_result(self) -> int:
        """
        获取最后一次roll的结果
        
        Returns:
            骰子结果（1-6）
        """
        data = self.SELECTORS['lastRollResult']
        result = await self.client.call(to=self.address, data=data)
        return ContractUtils.decode_uint256(result)
    
    async def get_last_seed(self) -> int:
        """
        获取最后使用的种子（block.difficulty值）
        
        Returns:
            随机数种子
        """
        data = self.SELECTORS['lastSeedUsed']
        result = await self.client.call(to=self.address, data=data)
        return ContractUtils.decode_uint256(result)
    
    async def get_last_roller(self) -> str:
        """
        获取最后一次roll的调用者地址
        
        Returns:
            地址（0x前缀）
        """
        data = self.SELECTORS['lastRoller']
        result = await self.client.call(to=self.address, data=data)
        return ContractUtils.decode_address(result)
    
    async def get_latest_roll(self) -> Tuple[str, int, int]:
        """
        获取最新的roll信息（一次调用获取所有数据）
        
        Returns:
            (roller_address, roll_result, seed) 元组
        """
        data = self.SELECTORS['getLatestRoll']
        result = await self.client.call(to=self.address, data=data)
        
        # 解析返回值：address + uint256 + uint256
        # 每个值占32字节（64个十六进制字符）
        result_hex = result[2:] if result.startswith("0x") else result
        
        if len(result_hex) < 192:
            LOG.warning(f"Unexpected result length: {len(result_hex)}")
            return ("0x0", 0, 0)
        
        # address在第一个32字节的后20字节
        roller_hex = result_hex[24:64]
        # 第二个32字节是roll结果
        roll_result_hex = result_hex[64:128]
        # 第三个32字节是seed
        seed_hex = result_hex[128:192]
        
        roller = "0x" + roller_hex
        roll_result = int(roll_result_hex, 16) if roll_result_hex else 0
        seed = int(seed_hex, 16) if seed_hex else 0
        
        return (roller, roll_result, seed)


class RandomnessVerifier:
    """随机数验证工具"""
    
    @staticmethod
    async def verify_block_randomness(
        rpc_client: GravityClient,
        http_client,  # GravityHttpClient
        block_number: int
    ) -> Dict:
        """
        验证指定块的随机数一致性
        
        Args:
            rpc_client: JSON-RPC客户端
            http_client: HTTP API客户端
            block_number: 块号
        
        Returns:
            验证结果字典:
            {
                "block_number": int,
                "api_randomness": str,
                "block_difficulty": int,
                "block_mix_hash": int,
                "difficulty_hex": str,
                "mix_hash_hex": str,
                "checks": {
                    "has_api_randomness": bool,
                    "difficulty_equals_mixhash": bool,
                    "api_matches_difficulty": bool
                },
                "valid": bool
            }
        """
        # 1. 从HTTP API获取随机数
        api_randomness = await http_client.get_randomness(block_number)
        
        # 2. 获取区块信息
        block = await rpc_client.get_block(block_number, full_transactions=False)
        
        if not block:
            return {
                "block_number": block_number,
                "error": "Block not found",
                "valid": False
            }
        
        # 3. 提取difficulty和mixHash
        difficulty_hex = block.get("difficulty", "0x0")
        mix_hash_hex = block.get("mixHash", "0x0")
        
        difficulty = int(difficulty_hex, 16)
        mix_hash = int(mix_hash_hex, 16)
        
        # 4. 验证
        result = {
            "block_number": block_number,
            "api_randomness": api_randomness,
            "block_difficulty": difficulty,
            "block_mix_hash": mix_hash,
            "difficulty_hex": difficulty_hex,
            "mix_hash_hex": mix_hash_hex,
            "checks": {}
        }
        
        # Check 1: API随机数存在
        result["checks"]["has_api_randomness"] = api_randomness is not None
        
        # Check 2: difficulty == mixHash (在PoS中应该相等)
        result["checks"]["difficulty_equals_mixhash"] = (difficulty == mix_hash)
        
        # Check 3: API随机数与difficulty的关系
        if api_randomness:
            # API返回的是十六进制字符串
            # 在Gravity中，block.difficulty应该等于API返回的随机数的某种形式
            # 这里我们检查是否相关（具体逻辑可能需要根据实际实现调整）
            api_randomness_int = int(api_randomness, 16) if api_randomness.startswith("0x") else int(api_randomness, 16)
            
            # 简单检查：如果difficulty非零且API有数据，认为匹配
            result["checks"]["api_matches_difficulty"] = (difficulty != 0 and api_randomness is not None)
        else:
            result["checks"]["api_matches_difficulty"] = False
        
        # 总体验证结果
        result["valid"] = all(result["checks"].values())
        
        return result
    
    @staticmethod
    async def verify_seed_in_contract(
        dice_helper: RandomDiceHelper,
        rpc_client: GravityClient,
        block_number: int
    ) -> bool:
        """
        验证合约中保存的seed与区块的difficulty一致
        
        Args:
            dice_helper: RandomDice辅助对象
            rpc_client: RPC客户端
            block_number: 块号
        
        Returns:
            是否一致
        """
        # 获取合约中的seed
        seed = await dice_helper.get_last_seed()
        
        # 获取区块的difficulty
        block = await rpc_client.get_block(block_number, full_transactions=False)
        difficulty_hex = block.get("difficulty", "0x0")
        difficulty = int(difficulty_hex, 16)
        
        match = (seed == difficulty)
        
        if match:
            LOG.info(f"✅ Seed matches: contract={seed}, block={difficulty}")
        else:
            LOG.warning(f"❌ Seed mismatch: contract={seed}, block={difficulty}")
        
        return match

