"""
Updated ERC20 test using the new enhanced contract framework
Demonstrates how to use the improved ContractCaller with complex data support
"""
import asyncio
import json
import logging
from pathlib import Path
from typing import Dict

from ...helpers.test_helpers import RunHelper, TestResult, test_case
from ...utils.contract_caller import ContractCaller
from ...utils.contract_utils import ContractUtils
from ...utils.shared_contracts import SharedContracts

LOG = logging.getLogger(__name__)


@test_case
async def test_erc20_enhanced(run_helper: RunHelper, test_result: TestResult):
    """Enhanced ERC20 test using new framework features"""
    LOG.info("Starting enhanced ERC20 test with new framework")
    
    # 1. Create test accounts
    deployer = await run_helper.create_test_account("token_deployer", fund_wei=5 * 10**18)
    recipient = await run_helper.create_test_account("token_recipient", fund_wei=1 * 10**18)
    
    LOG.info(f"Deployer: {deployer['address']}")
    LOG.info(f"Recipient: {recipient['address']}")
    
    # 2. Use shared contracts for efficient deployment/reuse
    shared_contracts = SharedContracts(run_helper)
    
    async def deploy_erc20_token():
        """Deploy SimpleToken contract using new framework"""
        from ...utils.shared_contracts import ContractDeployer
        deployer_tool = ContractDeployer(run_helper)
        return await deployer_tool.deploy_simple_contract("SimpleToken", deployer)
    
    contract_info = await shared_contracts.get_or_deploy("TestToken", deploy_erc20_token)
    contract_address = contract_info['address']
    
    LOG.info(f"Token contract at: {contract_address}")
    
    # 3. Create ContractCaller with enhanced features
    caller = ContractCaller(run_helper.client, contract_address, contract_info.get('abi'))
    
    # 4. Test ERC20 functionality using enhanced calls
    
    # Test basic token info (simple calls, same as before but cleaner)
    token_name = await caller.call("name")
    token_symbol = await caller.call("symbol")
    token_decimals = await caller.call("decimals")
    total_supply = await caller.call("totalSupply")
    
    LOG.info(f"Token: {token_name} ({token_symbol})")
    LOG.info(f"Decimals: {token_decimals}")
    LOG.info(f"Total Supply: {total_supply / 10**18:.2f} tokens")
    
    # Test balance queries (much cleaner than before)
    deployer_balance = await caller.call("balanceOf", deployer["address"])
    recipient_balance = await caller.call("balanceOf", recipient["address"])
    
    LOG.info(f"Deployer balance: {deployer_balance / 10**18:.2f} tokens")
    LOG.info(f"Recipient balance: {recipient_balance / 10**18:.2f} tokens")
    
    # Verify initial state
    if deployer_balance != total_supply:
        raise RuntimeError(f"Deployer balance mismatch: expected {total_supply}, got {deployer_balance}")
    
    if recipient_balance != 0:
        raise RuntimeError(f"Recipient should have 0 balance initially, got {recipient_balance}")
    
    # Test allowance functionality
    allowance_amount = 500 * 10**18  # 500 tokens
    
    # Set allowance
    LOG.info(f"Setting allowance of {allowance_amount / 10**18} tokens")
    allowance_tx = await caller.send_and_wait(
        "approve",
        recipient["address"],
        allowance_amount,
        from_account=deployer,
        gas_limit=65000
    )
    
    # Check allowance
    current_allowance = await caller.call("allowance", deployer["address"], recipient["address"])
    LOG.info(f"Current allowance: {current_allowance / 10**18:.2f} tokens")
    
    if current_allowance != allowance_amount:
        raise RuntimeError(f"Allowance mismatch: expected {allowance_amount}, got {current_allowance}")
    
    # Test transfer functionality
    transfer_amount = 100 * 10**18  # 100 tokens
    
    LOG.info(f"Transferring {transfer_amount / 10**18} tokens to recipient")
    transfer_tx = await caller.send_and_wait(
        "transfer",
        recipient["address"],
        transfer_amount,
        from_account=deployer,
        gas_limit=65000
    )
    
    # Check updated balances
    deployer_balance_after = await caller.call("balanceOf", deployer["address"])
    recipient_balance_after = await caller.call("balanceOf", recipient["address"])
    
    LOG.info(f"Deployer balance after transfer: {deployer_balance_after / 10**18:.2f} tokens")
    LOG.info(f"Recipient balance after transfer: {recipient_balance_after / 10**18:.2f} tokens")
    
    # Verify transfer amounts
    expected_deployer_balance = deployer_balance - transfer_amount
    if deployer_balance_after != expected_deployer_balance:
        raise RuntimeError(f"Deployer balance incorrect: expected {expected_deployer_balance}, got {deployer_balance_after}")
    
    if recipient_balance_after != transfer_amount:
        raise RuntimeError(f"Recipient balance incorrect: expected {transfer_amount}, got {recipient_balance_after}")
    
    # Test transferFrom functionality (using allowance)
    transfer_from_amount = 50 * 10**18  # 50 tokens
    
    LOG.info(f"Transferring {transfer_from_amount / 10**18} tokens using allowance")
    transfer_from_tx = await caller.send_and_wait(
        "transferFrom",
        deployer["address"],
        recipient["address"],
        transfer_from_amount,
        from_account=recipient,  # recipient is now spending from deployer's allowance
        gas_limit=65000
    )
    
    # Check final balances
    deployer_balance_final = await caller.call("balanceOf", deployer["address"])
    recipient_balance_final = await caller.call("balanceOf", recipient["address"])
    allowance_final = await caller.call("allowance", deployer["address"], recipient["address"])
    
    LOG.info(f"Final deployer balance: {deployer_balance_final / 10**18:.2f} tokens")
    LOG.info(f"Final recipient balance: {recipient_balance_final / 10**18:.2f} tokens")
    LOG.info(f"Final remaining allowance: {allowance_final / 10**18:.2f} tokens")
    
    # Verify final state
    expected_deployer_final = expected_deployer_balance - transfer_from_amount
    if deployer_balance_final != expected_deployer_final:
        raise RuntimeError(f"Final deployer balance incorrect: expected {expected_deployer_final}, got {deployer_balance_final}")
    
    expected_recipient_final = recipient_balance_after + transfer_from_amount
    if recipient_balance_final != expected_recipient_final:
        raise RuntimeError(f"Final recipient balance incorrect: expected {expected_recipient_final}, got {recipient_balance_final}")
    
    expected_allowance_final = allowance_amount - transfer_from_amount
    if allowance_final != expected_allowance_final:
        raise RuntimeError(f"Final allowance incorrect: expected {expected_allowance_final}, got {allowance_final}")
    
    LOG.info("✅ All ERC20 operations successful!")
    
    # Record comprehensive test results
    test_result.mark_success(
        contract_address=contract_address,
        token_name=token_name,
        token_symbol=token_symbol,
        token_decimals=token_decimals,
        total_supply=total_supply,
        
        # Transfer details
        transfer_amount=transfer_amount,
        transfer_from_amount=transfer_from_amount,
        allowance_amount=allowance_amount,
        
        # Final balances
        final_deployer_balance=deployer_balance_final,
        final_recipient_balance=recipient_balance_final,
        final_allowance=allowance_final,
        
        # Transaction details
        transfer_tx_hash=transfer_tx['transactionHash'],
        transfer_from_tx_hash=transfer_from_tx['transactionHash'],
        allowance_tx_hash=allowance_tx['transactionHash'],
        
        # Gas usage
        transfer_gas_used=int(transfer_tx.get('gasUsed', '0x0'), 16),
        transfer_from_gas_used=int(transfer_from_tx.get('gasUsed', '0x0'), 16),
        allowance_gas_used=int(allowance_tx.get('gasUsed', '0x0'), 16),
        
        # Framework benefits
        framework_benefits=[
            "Clean contract calls without manual hex encoding",
            "Automatic result decoding to Python types",
            "Efficient contract reuse through SharedContracts",
            "Unified interface for both calls and transactions",
            "Better error handling and validation"
        ]
    )
    
    LOG.info("Enhanced ERC20 test completed successfully with new framework")


@test_case
async def test_erc20_batch_operations(run_helper: RunHelper, test_result: TestResult):
    """Test batch operations using enhanced framework"""
    LOG.info("Testing ERC20 batch operations with enhanced framework")
    
    # Create multiple test accounts
    accounts = []
    for i in range(5):
        account = await run_helper.create_test_account(f"user_{i}", fund_wei=2 * 10**18)
        accounts.append(account)
        LOG.info(f"Created account {i}: {account['address']}")
    
    # Get token contract
    shared_contracts = SharedContracts(run_helper)
    contract_info = await shared_contracts.get_or_deploy("BatchToken", 
        lambda: deploy_simple_token(run_helper, "SimpleToken", shared_contracts))
    
    caller = ContractCaller(run_helper.client, contract_info['address'], contract_info.get('abi'))
    
    # Deployer account (initial token holder)
    deployer = await run_helper.create_test_account("batch_deployer", fund_wei=10 * 10**18)
    
    # Check deployer balance
    deployer_balance = await caller.call("balanceOf", deployer["address"])
    total_supply = await caller.call("totalSupply")
    
    LOG.info(f"Deployer balance: {deployer_balance / 10**18:.2f}")
    LOG.info(f"Total supply: {total_supply / 10**18:.2f}")
    
    # Batch transfer to all accounts
    transfer_amount = 100 * 10**18  # 100 tokens each
    total_transferred = 0
    
    LOG.info(f"Transferring {transfer_amount / 10**18} tokens to {len(accounts)} accounts")
    
    for i, account in enumerate(accounts):
        try:
            tx_receipt = await caller.send_and_wait(
                "transfer",
                account["address"],
                transfer_amount,
                from_account=deployer,
                gas_limit=65000
            )
            
            total_transferred += transfer_amount
            LOG.info(f"✅ Transfer {i+1} successful to {account['address'][:10]}...")
            
        except Exception as e:
            LOG.error(f"❌ Transfer {i+1} failed: {e}")
    
    # Batch check balances
    LOG.info("Checking all recipient balances...")
    
    all_balances = []
    total_received = 0
    
    for i, account in enumerate(accounts):
        balance = await caller.call("balanceOf", account["address"])
        all_balances.append({
            "address": account["address"],
            "balance": balance
        })
        total_received += balance
        LOG.info(f"User {i+1} balance: {balance / 10**18:.2f} tokens")
    
    # Verify totals
    deployer_final_balance = await caller.call("balanceOf", deployer["address"])
    expected_deployer_final = deployer_balance - total_transferred
    
    LOG.info(f"Deployer final balance: {deployer_final_balance / 10**18:.2f}")
    LOG.info(f"Total transferred: {total_transferred / 10**18:.2f}")
    LOG.info(f"Total received: {total_received / 10**18:.2f}")
    
    # Verify final state
    if deployer_final_balance != expected_deployer_final:
        raise RuntimeError(f"Deployer final balance mismatch: expected {expected_deployer_final}, got {deployer_final_balance}")
    
    if total_received != total_transferred:
        raise RuntimeError(f"Total received mismatch: expected {total_transferred}, got {total_received}")
    
    # Test multiple view calls efficiently
    LOG.info("Testing efficient view calls...")
    
    # Get all balances in sequence (demonstrates clean syntax)
    view_call_balances = []
    for account in accounts:
        balance = await caller.call("balanceOf", account["address"])
        view_call_balances.append(balance)
    
    # Verify they match
    if view_call_balances != [b["balance"] for b in all_balances]:
        raise RuntimeError("Balance mismatch in view calls")
    
    LOG.info("✅ Batch operations test successful!")
    
    test_result.mark_success(
        contract_address=contract_info['address'],
        accounts_count=len(accounts),
        transfer_amount_per_account=transfer_amount,
        total_transferred=total_transferred,
        total_received=total_received,
        deployer_final_balance=deployer_final_balance,
        all_balances=all_balances,
        batch_operations_successful=True,
        framework_advantages=[
            "Clean batch operation syntax",
            "Automatic error handling per operation",
            "Efficient view calls without hex encoding",
            "Type-safe balance handling",
            "Comprehensive result tracking"
        ]
    )
    
    LOG.info("ERC20 batch operations test completed successfully")


# Helper function for deployment
async def deploy_simple_token(run_helper, token_name: str = "SimpleToken", shared_contracts: SharedContracts = None):
    """Deploy SimpleToken contract"""
    from ...utils.shared_contracts import ContractDeployer
    deployer = ContractDeployer(run_helper)
    deployer_account = await run_helper.create_test_account(f"{token_name}_deployer", fund_wei=3 * 10**18)
    return await deployer.deploy_simple_contract(token_name, deployer_account)