import asyncio
import json
import logging
import os
import time
from typing import Dict, Optional, Any
from pathlib import Path

import web3
from web3 import Web3
from eth_account import Account
from eth_utils import to_checksum_address, from_wei, to_wei

from ...helpers.test_helpers import RunHelper, TestResult, test_case

LOG = logging.getLogger(__name__)

# Default configuration path
DEFAULT_CONFIG_PATH = Path(__file__).parent.parent.parent.parent / "configs" / "cross_chain_config.json"


class CrossChainConfig:
    """Cross-chain test configuration manager"""
    
    def __init__(self, config_path: Optional[str] = None):
        config_path = config_path or os.getenv("CROSS_CHAIN_CONFIG_PATH", str(DEFAULT_CONFIG_PATH))
        with open(config_path, 'r') as f:
            self.config = json.load(f)
        
        # Override with environment variables if provided
        self._apply_env_overrides()
    
    def _apply_env_overrides(self):
        """Apply environment variable overrides"""
        if os.getenv("SEPOLIA_RPC_URL"):
            self.config["sepolia"]["rpc_url"] = os.getenv("SEPOLIA_RPC_URL")
        if os.getenv("SEPOLIA_PRIVATE_KEY"):
            self.config["sepolia"]["test_account"]["private_key"] = os.getenv("SEPOLIA_PRIVATE_KEY")
        if os.getenv("CROSS_CHAIN_TIMEOUT"):
            self.config["gravity"]["sync_settings"]["timeout_seconds"] = int(os.getenv("CROSS_CHAIN_TIMEOUT"))
    
    def get_sepolia_config(self) -> Dict:
        return self.config["sepolia"]
    
    def get_gravity_config(self) -> Dict:
        return self.config["gravity"]


class SepoliaClient:
    """Sepolia testnet client wrapper"""
    
    def __init__(self, config: Dict):
        self.config = config
        self.w3 = Web3(Web3.HTTPProvider(config["rpc_url"]))
        self.account = Account.from_key(config["test_account"]["private_key"])
        
        # Initialize contracts
        self.gravity_bridge = self.w3.eth.contract(
            address=config["contracts"]["gravity_bridge"]["address"],
            abi=config["contracts"]["gravity_bridge"]["abi"]
        )
        self.g_token = self.w3.eth.contract(
            address=config["contracts"]["g_token"]["address"],
            abi=config["contracts"]["g_token"]["abi"]
        )
        
        LOG.info(f"Sepolia client initialized for account: {self.account.address}")
    
    async def check_balances(self) -> Dict[str, Any]:
        """Check account balances"""
        try:
            # Check ETH balance
            eth_balance_wei = self.w3.eth.get_balance(self.account.address)
            eth_balance = from_wei(eth_balance_wei, 'ether')
            
            # Check G Token balance
            g_balance = self.g_token.functions.balanceOf(self.account.address).call()
            
            # Get G Token decimals
            try:
                decimals = self.g_token.functions.decimals().call()
                g_balance_formatted = g_balance / (10 ** decimals)
            except:
                g_balance_formatted = from_wei(g_balance, 'ether')
            
            LOG.info(f"ETH balance: {eth_balance:.6f} ETH")
            LOG.info(f"G Token balance: {g_balance_formatted:.6f} G")
            
            return {
                "eth_balance": eth_balance_wei,
                "g_balance": g_balance,
                "eth_balance_formatted": eth_balance,
                "g_balance_formatted": g_balance_formatted
            }
        except Exception as e:
            LOG.error(f"Failed to check balances: {e}")
            raise
    
    async def approve_g_token(self, amount: int) -> str:
        """Approve Gravity Bridge contract to spend G Tokens"""
        try:
            # Build approval transaction
            nonce = self.w3.eth.get_transaction_count(self.account.address)
            gas_price = self.w3.eth.gas_price
            
            tx = self.g_token.functions.approve(
                self.config["contracts"]["gravity_bridge"]["address"],
                amount
            ).build_transaction({
                'from': self.account.address,
                'nonce': nonce,
                'gas': 100000,
                'gasPrice': gas_price
            })
            
            # Sign and send transaction
            signed_tx = self.w3.eth.account.sign_transaction(tx, self.account.key)
            tx_hash = self.w3.eth.send_raw_transaction(signed_tx.raw_transaction)
            
            LOG.info(f"Approval transaction sent: {tx_hash.hex()}")
            
            # Wait for confirmation
            receipt = self.w3.eth.wait_for_transaction_receipt(tx_hash, timeout=60)
            
            if receipt.status != 1:
                raise RuntimeError(f"Approval transaction failed: {receipt}")
            
            LOG.info(f"Approval confirmed in block: {receipt.blockNumber}")
            return tx_hash.hex()
            
        except Exception as e:
            LOG.error(f"Failed to approve G Token: {e}")
            raise
    
    async def deposit_gravity(self, amount: int, target_address: Optional[str] = None) -> Dict[str, Any]:
        """Execute depositGravity function"""
        try:
            if not target_address:
                target_address = self.account.address
            
            # Build deposit transaction
            nonce = self.w3.eth.get_transaction_count(self.account.address)
            gas_price = self.w3.eth.gas_price
            
            tx = self.gravity_bridge.functions.depositGravity(
                to_checksum_address(target_address),
                amount
            ).build_transaction({
                'from': self.account.address,
                'nonce': nonce,
                'gas': 200000,
                'gasPrice': gas_price
            })
            
            # Sign and send transaction
            signed_tx = self.w3.eth.account.sign_transaction(tx, self.account.key)
            tx_hash = self.w3.eth.send_raw_transaction(signed_tx.raw_transaction)
            
            LOG.info(f"Deposit transaction sent: {tx_hash.hex()}")
            
            # Wait for confirmation
            receipt = self.w3.eth.wait_for_transaction_receipt(tx_hash, timeout=60)
            
            if receipt.status != 1:
                raise RuntimeError(f"Deposit transaction failed: {receipt}")
            
            # Parse DepositGravityEvent
            deposit_event = None
            LOG.info(f"Transaction receipt has {len(receipt.logs)} logs")
            
            for i, log in enumerate(receipt.logs):
                LOG.info(f"Processing log {i+1}: address={log.address}")
                try:
                    # Check if this log is from our contract
                    if log.address.lower() != self.config["contracts"]["gravity_bridge"]["address"].lower():
                        LOG.debug(f"Log from different contract: {log.address}")
                        continue
                    
                    LOG.info(f"Found log from gravity bridge contract!")
                    LOG.info(f"Data length: {len(log.data)} bytes")
                    
                    # Parse the event from our contract
                    # Based on the log data structure from the logs:
                    # user(32 bytes) + amount(32 bytes) + targetAddress(32 bytes) + blockNumber(32 bytes)
                    data_hex = log.data.hex()
                    LOG.info(f"Raw data: {data_hex}")
                    
                    # Extract values from 128 bytes (256 hex chars)
                    # Structure: user(32) + amount(32) + targetAddress(32) + blockNumber(32)
                    deposit_event = {
                        "user": "0x" + data_hex[24:64],  # User address (32 bytes, last 20 bytes are the address)
                        "amount": int(data_hex[64:128], 16),  # Amount (32 bytes)
                        "targetAddress": "0x" + data_hex[152:192],  # Target address (last 20 bytes of 32 bytes)
                        "blockNumber": int(data_hex[192:256], 16)  # Block number (32 bytes)
                    }
                    
                    LOG.info(f"✅ DepositGravityEvent detected:")
                    LOG.info(f"   User: {deposit_event['user']}")
                    LOG.info(f"   Amount: {deposit_event['amount']}")
                    LOG.info(f"   Target: {deposit_event['targetAddress']}")
                    LOG.info(f"   Block: {deposit_event['blockNumber']}")
                    break
                except Exception as e:
                    LOG.error(f"Error parsing log {i+1}: {e}")
                    import traceback
                    traceback.print_exc()
                    continue
            
            if not deposit_event:
                LOG.error(f"No DepositGravityEvent found in receipt. Logs:")
                for i, log in enumerate(receipt.logs):
                    LOG.error(f"  Log {i+1}: address={log.address}, topics={log.topics}, data={log.data}")
                raise RuntimeError("DepositGravityEvent not found in transaction receipt")
            
            LOG.info(f"Deposit confirmed in block: {receipt.blockNumber}")
            return {
                "tx_hash": tx_hash.hex(),
                "block_number": receipt.blockNumber,
                "gas_used": receipt.gasUsed,
                "event": deposit_event
            }
            
        except Exception as e:
            LOG.error(f"Failed to deposit gravity: {e}")
            raise


async def poll_gravity_for_event(run_helper: RunHelper, expected_sender: str, 
                                expected_amount: int, expected_target: str,
                                gravity_config: Dict) -> Optional[Dict[str, Any]]:
    """Poll Gravity Chain for CrossChainDepositProcessed event"""
    timeout = gravity_config["sync_settings"]["timeout_seconds"]
    poll_interval = gravity_config["sync_settings"]["poll_interval_seconds"]
    max_blocks_per_query = gravity_config["sync_settings"]["max_blocks_per_query"]
    
    # Calculate event topic for CrossChainDepositProcessed
    event_signature = web3.Web3.keccak(text="CrossChainDepositProcessed(address,address,uint256,uint256,bool,string,string,uint256)")
    event_topic = "0x" + event_signature.hex()
    
    start_time = time.time()
    last_checked_block = 0
    
    LOG.info(f"Starting to poll Gravity Chain for event from sender={expected_sender}")
    LOG.info(f"Poll settings: timeout={timeout}s, interval={poll_interval}s")
    
    while time.time() - start_time < timeout:
        try:
            # Get current block number
            current_block = await run_helper.client.get_block_number()
            
            # Only query new blocks
            if current_block > last_checked_block:
                # Calculate query range
                from_block = max(last_checked_block + 1, current_block - max_blocks_per_query)
                to_block = current_block
                
                LOG.debug(f"Querying blocks {from_block} to {to_block}")
                
                # Query for events
                logs = await run_helper.client.get_logs(
                    from_block=from_block,
                    to_block=to_block,
                    address=gravity_config["monitor_contract"]["address"],
                    topics=[[event_topic]]
                )
                
                # Parse and match events
                for log in logs:
                    if parse_and_match_event(log, expected_sender, expected_amount, expected_target):
                        LOG.info(f"✅ CrossChainDepositProcessed found after {time.time() - start_time:.1f} seconds")
                        return log
                
                last_checked_block = current_block
                LOG.info(f"Still waiting... Checked up to block {current_block}")
            
            # Wait before next poll
            await asyncio.sleep(poll_interval)
            
        except Exception as e:
            LOG.warning(f"Error during polling: {e}")
            await asyncio.sleep(poll_interval)
    
    LOG.error(f"❌ Timeout: CrossChainDepositProcessed not found after {timeout} seconds")
    return None


def parse_and_match_event(log: Dict, expected_sender: str, expected_amount: int, 
                         expected_target: str) -> bool:
    """Parse and match CrossChainDepositProcessed event"""
    try:
        # Extract data from log
        topics = log.get("topics", [])
        data = log.get("data", "")
        
        if len(topics) < 3:
            return False
        
        # Extract indexed parameters (sender and targetAddress)
        sender = "0x" + topics[1][-40:]  # Last 20 bytes
        target_address = "0x" + topics[2][-40:]  # Last 20 bytes
        
        # Remove "0x" prefix and convert to lowercase for comparison
        sender = to_checksum_address(sender).lower()
        target_address = to_checksum_address(target_address).lower()
        expected_sender = expected_sender.lower()
        expected_target = expected_target.lower()
        
        # Extract non-indexed parameters from data
        # Data layout: amount(32) + blockNumber(32) + success(32) + errorMessageOffset(32) + errorMessageLength(32) + issuerOffset(32) + onchainBlockNumber(32)
        amount_hex = data[2:66]  # First 32 bytes
        amount = int(amount_hex, 16)
        
        block_number_hex = data[66:130]  # Second 32 bytes
        block_number = int(block_number_hex, 16)
        
        success_hex = data[130:194]  # Third 32 bytes
        success = int(success_hex, 16) > 0
        
        onchain_block_hex = data[258:322]  # Last 32 bytes
        onchain_block_number = int(onchain_block_hex, 16)
        
        LOG.debug(f"Event parsed: sender={sender}, target={target_address}, amount={amount}, "
                 f"success={success}, block={block_number}, onchain={onchain_block_number}")
        
        # Match conditions
        if (sender == expected_sender and 
            target_address == expected_target and 
            amount == expected_amount and 
            success):
            
            LOG.info(f"✅ Event matched successfully!")
            LOG.info(f"   Sender: {sender}")
            LOG.info(f"   Target: {target_address}")
            LOG.info(f"   Amount: {amount}")
            LOG.info(f"   Success: {success}")
            LOG.info(f"   Block: {block_number}")
            LOG.info(f"   Onchain Block: {onchain_block_number}")
            
            return True
        
    except Exception as e:
        LOG.debug(f"Failed to parse event: {e}")
    
    return False


@test_case
async def test_cross_chain_gravity_deposit(run_helper: RunHelper, test_result: TestResult):
    """Test cross-chain Gravity Deposit functionality"""
    LOG.info("=" * 60)
    LOG.info("Starting Cross-Chain Gravity Deposit Test")
    LOG.info("=" * 60)
    
    try:
        # Load configuration
        config = CrossChainConfig()
        sepolia_config = config.get_sepolia_config()
        gravity_config = config.get_gravity_config()
        
        # Initialize Sepolia client
        LOG.info("Initializing Sepolia client...")
        sepolia_client = SepoliaClient(sepolia_config)
        
        # Check balances
        LOG.info("Checking account balances...")
        balances = await sepolia_client.check_balances()
        
        # Get deposit parameters
        deposit_amount = int(sepolia_config["deposit_params"]["amount"])
        target_address = sepolia_config["deposit_params"]["target_address"]
        if not target_address:
            target_address = sepolia_client.account.address
        
        # Validate balances
        if balances["eth_balance"] < to_wei(0.01, 'ether'):
            raise RuntimeError(f"Insufficient ETH balance: {balances['eth_balance_formatted']} ETH")
        
        if balances["g_balance"] < deposit_amount:
            raise RuntimeError(f"Insufficient G Token balance: {balances['g_balance_formatted']} G "
                             f"(required: {deposit_amount / 10**18:.6f} G)")
        
        # Approve G Token spending
        LOG.info(f"Approving {deposit_amount / 10**18:.6f} G Tokens...")
        await sepolia_client.approve_g_token(deposit_amount)
        
        # Execute depositGravity
        LOG.info(f"Executing depositGravity to {target_address}...")
        deposit_result = await sepolia_client.deposit_gravity(deposit_amount, target_address)
        
        # Extract event data
        sepolia_event = deposit_result["event"]
        LOG.info(f"Sepolia event received:")
        LOG.info(f"  User: {sepolia_event['user']}")
        LOG.info(f"  Amount: {sepolia_event['amount']}")
        LOG.info(f"  Target: {sepolia_event['targetAddress']}")
        LOG.info(f"  Block: {sepolia_event['blockNumber']}")
        
        # Wait for cross-chain sync
        # NOTE: This may take up to 12+ minutes as we wait for Sepolia block finalization
        # (2 epochs * 6.4 minutes per epoch on Sepolia)
        LOG.info("\nWaiting for cross-chain synchronization...")
        LOG.info("⚠️  This may take 12+ minutes for Sepolia block finalization")
        sync_start_time = time.time()
        
        gravity_event = await poll_gravity_for_event(
            run_helper=run_helper,
            expected_sender=sepolia_event["user"],
            expected_amount=sepolia_event["amount"],
            expected_target=sepolia_event["targetAddress"],
            gravity_config=gravity_config
        )
        
        sync_duration = time.time() - sync_start_time
        
        if not gravity_event:
            raise RuntimeError("Cross-chain sync timeout - event not found on Gravity Chain")
        
        # Record test results
        test_result.mark_success(
            sepolia_tx_hash=deposit_result["tx_hash"],
            sepolia_block_number=deposit_result["block_number"],
            sepolia_gas_used=deposit_result["gas_used"],
            gravity_tx_hash=gravity_event.get("transactionHash", "unknown"),
            cross_chain_sync_time=sync_duration,
            deposit_amount=deposit_amount,
            sender_address=sepolia_event["user"],
            target_address=sepolia_event["targetAddress"]
        )
        
        LOG.info("\n" + "=" * 60)
        LOG.info("✅ Cross-Chain Gravity Deposit Test PASSED")
        LOG.info(f"   Sepolia TX: {deposit_result['tx_hash']}")
        LOG.info(f"   Gravity TX: {gravity_event.get('transactionHash', 'unknown')}")
        LOG.info(f"   Sync Time: {sync_duration:.1f} seconds")
        LOG.info(f"   Amount: {deposit_amount / 10**18:.6f} G")
        LOG.info("=" * 60)
        
    except Exception as e:
        LOG.error(f"\n❌ Cross-Chain Gravity Deposit Test FAILED: {e}")
        test_result.mark_failure(str(e))
        raise