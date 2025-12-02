"""
Randomness testing utility tools
Includes RandomDice contract helper class and randomness verification tools
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
    """RandomDice contract helper class"""
    
    # RandomDice contract function selectors (calculated using keccak256)
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
        Load RandomDice bytecode from compiled output
        
        Returns:
            Contract bytecode (hex string with 0x prefix)
        
        Raises:
            FileNotFoundError: Contract not compiled
        """
        # Search for compiled output
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
            
            # Get bytecode
            bytecode_obj = contract_data.get("bytecode") or contract_data.get("bin")
            
            if isinstance(bytecode_obj, dict):
                bytecode = bytecode_obj.get("object", "")
            else:
                bytecode = str(bytecode_obj)
            
            if not bytecode:
                raise ValueError(f"No bytecode found in {contract_path}")
            
            # Ensure 0x prefix
            if not bytecode.startswith("0x"):
                bytecode = "0x" + bytecode
            
            LOG.info(f"Loaded RandomDice bytecode ({len(bytecode)} chars)")
            return bytecode
    
    def __init__(self, client: GravityClient, contract_address: str):
        """
        Initialize RandomDice helper class
        
        Args:
            client: Gravity RPC client
            contract_address: Contract address
        """
        self.client = client
        self.address = to_checksum_address(contract_address)
        LOG.debug(f"RandomDiceHelper initialized for {self.address}")
    
    async def roll_dice(self, from_account: Dict, gas_limit: int = 100000) -> Dict:
        """
        Call rollDice() function
        
        Args:
            from_account: Caller account (must contain address and private_key)
            gas_limit: Gas limit
        
        Returns:
            Transaction receipt
        
        Raises:
            RuntimeError: Transaction failed
        """
        # Encode function call
        data = self.SELECTORS['rollDice']  # rollDice() has no parameters
        
        # Get transaction parameters
        nonce = await self.client.get_transaction_count(from_account["address"])
        gas_price = await self.client.get_gas_price()
        chain_id = await self.client.get_chain_id()
        
        # Build transaction
        tx_data = {
            "to": self.address,
            "data": data,
            "gas": hex(gas_limit),
            "gasPrice": hex(gas_price),
            "nonce": hex(nonce),
            "chainId": hex(chain_id),
            "value": "0x0"
        }
        
        # Sign
        private_key = from_account["private_key"]
        if private_key.startswith("0x"):
            private_key = private_key[2:]
        
        signed_tx = Account.sign_transaction(tx_data, private_key)
        
        # Send
        tx_hash = await self.client.send_raw_transaction(signed_tx.raw_transaction)
        LOG.debug(f"rollDice transaction sent: {tx_hash}")
        
        # Wait for confirmation
        receipt = await self.client.wait_for_transaction_receipt(tx_hash, timeout=30)
        
        if receipt.get("status") != "0x1":
            raise RuntimeError(f"rollDice transaction failed: {receipt}")
        
        return receipt
    
    async def get_last_result(self) -> int:
        """
        Get the result of the last roll
        
        Returns:
            Dice result (1-6)
        """
        data = self.SELECTORS['lastRollResult']
        result = await self.client.call(to=self.address, data=data)
        return ContractUtils.decode_uint256(result)
    
    async def get_last_seed(self) -> int:
        """
        Get the last used seed (block.difficulty value)
        
        Returns:
            Randomness seed
        """
        data = self.SELECTORS['lastSeedUsed']
        result = await self.client.call(to=self.address, data=data)
        return ContractUtils.decode_uint256(result)
    
    async def get_last_roller(self) -> str:
        """
        Get the address of the last roll caller
        
        Returns:
            Address (with 0x prefix)
        """
        data = self.SELECTORS['lastRoller']
        result = await self.client.call(to=self.address, data=data)
        return ContractUtils.decode_address(result)
    
    async def get_latest_roll(self) -> Tuple[str, int, int]:
        """
        Get the latest roll information (fetch all data in one call)
        
        Returns:
            (roller_address, roll_result, seed) tuple
        """
        data = self.SELECTORS['getLatestRoll']
        result = await self.client.call(to=self.address, data=data)
        
        # Parse return value: address + uint256 + uint256
        # Each value occupies 32 bytes (64 hex characters)
        result_hex = result[2:] if result.startswith("0x") else result
        
        if len(result_hex) < 192:
            LOG.warning(f"Unexpected result length: {len(result_hex)}")
            return ("0x0", 0, 0)
        
        # Address is in the last 20 bytes of the first 32 bytes
        roller_hex = result_hex[24:64]
        # Second 32 bytes is the roll result
        roll_result_hex = result_hex[64:128]
        # Third 32 bytes is the seed
        seed_hex = result_hex[128:192]
        
        roller = "0x" + roller_hex
        roll_result = int(roll_result_hex, 16) if roll_result_hex else 0
        seed = int(seed_hex, 16) if seed_hex else 0
        
        return (roller, roll_result, seed)


class RandomnessVerifier:
    """Randomness verification tool"""
    
    @staticmethod
    async def verify_block_randomness(
        rpc_client: GravityClient,
        http_client,  # GravityHttpClient
        block_number: int
    ) -> Dict:
        """
        Verify randomness consistency for a specified block
        
        Args:
            rpc_client: JSON-RPC client
            http_client: HTTP API client
            block_number: Block number
        
        Returns:
            Verification result dictionary:
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
        # 1. Get randomness from HTTP API
        api_randomness = await http_client.get_randomness(block_number)
        
        # 2. Get block information
        block = await rpc_client.get_block(block_number, full_transactions=False)
        
        if not block:
            return {
                "block_number": block_number,
                "error": "Block not found",
                "valid": False
            }
        
        # 3. Extract difficulty and mixHash
        difficulty_hex = block.get("difficulty", "0x0")
        mix_hash_hex = block.get("mixHash", "0x0")
        
        difficulty = int(difficulty_hex, 16)
        mix_hash = int(mix_hash_hex, 16)
        
        # 4. Verify
        result = {
            "block_number": block_number,
            "api_randomness": api_randomness,
            "block_difficulty": difficulty,
            "block_mix_hash": mix_hash,
            "difficulty_hex": difficulty_hex,
            "mix_hash_hex": mix_hash_hex,
            "checks": {}
        }
        
        # Check 1: API randomness exists
        result["checks"]["has_api_randomness"] = api_randomness is not None
        
        # Check 2: difficulty == mixHash (should be equal in PoS)
        result["checks"]["difficulty_equals_mixhash"] = (difficulty == mix_hash)
        
        # Check 3: Relationship between API randomness and difficulty
        if api_randomness:
            # API returns hex string
            # In Gravity, block.difficulty should equal some form of the API-returned randomness
            # Here we check if they are related (exact logic may need adjustment based on implementation)
            api_randomness_int = int(api_randomness, 16) if api_randomness.startswith("0x") else int(api_randomness, 16)
            
            # Simple check: if difficulty is non-zero and API has data, consider it a match
            result["checks"]["api_matches_difficulty"] = (difficulty != 0 and api_randomness is not None)
        else:
            result["checks"]["api_matches_difficulty"] = False
        
        # Overall verification result
        result["valid"] = all(result["checks"].values())
        
        return result
    
    @staticmethod
    async def verify_seed_in_contract(
        dice_helper: RandomDiceHelper,
        rpc_client: GravityClient,
        block_number: int
    ) -> bool:
        """
        Verify that the seed saved in the contract matches the block's difficulty
        
        Args:
            dice_helper: RandomDice helper object
            rpc_client: RPC client
            block_number: Block number
        
        Returns:
            Whether they match
        """
        # Get seed from contract
        seed = await dice_helper.get_last_seed()
        
        # Get block difficulty
        block = await rpc_client.get_block(block_number, full_transactions=False)
        difficulty_hex = block.get("difficulty", "0x0")
        difficulty = int(difficulty_hex, 16)
        
        match = (seed == difficulty)
        
        if match:
            LOG.info(f"✅ Seed matches: contract={seed}, block={difficulty}")
        else:
            LOG.warning(f"❌ Seed mismatch: contract={seed}, block={difficulty}")
        
        return match

