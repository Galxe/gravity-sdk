"""
Example test file demonstrating the new contract testing utilities
"""
import asyncio
import logging
from typing import Dict

from ...helpers.test_helpers import RunHelper, TestResult, test_case
from ...utils.contract_utils import ContractUtils, encode_function_call, decode_result
from ...utils.contract_caller import ContractCaller
from ...utils.shared_contracts import SharedContracts, deploy_simple_storage

LOG = logging.getLogger(__name__)


@test_case
async def test_simple_storage_with_new_utils(run_helper: RunHelper, test_result: TestResult):
    """Test SimpleStorage using new utility functions"""
    LOG.info("Testing SimpleStorage with new utilities")
    
    # 1. Create test account
    deployer = await run_helper.create_test_account("deployer", fund_wei=5 * 10**18)
    
    # 2. Use shared contracts manager
    shared_contracts = SharedContracts(run_helper)
    
    # 3. Deploy or get existing contract
    contract_info = await shared_contracts.get_or_deploy(
        "SimpleStorage",
        lambda: deploy_simple_storage(run_helper, shared_contracts)
    )
    
    contract_address = contract_info['address']
    LOG.info(f"Using SimpleStorage at: {contract_address}")
    
    # 4. Create contract caller
    caller = ContractCaller(run_helper.client, contract_address, contract_info.get('abi'))
    
    # 5. Test contract functionality using caller
    
    # Read initial value
    initial_value = await caller.call("getValue")
    LOG.info(f"Initial value: {initial_value}")
    
    # Set new value using transaction
    new_value = 99999
    tx_receipt = await caller.send_and_wait(
        "setValue", 
        new_value, 
        from_account=deployer,
        gas_limit=50000
    )
    
    LOG.info(f"Set value transaction confirmed: {tx_receipt['transactionHash']}")
    
    # Read updated value
    updated_value = await caller.call("getValue")
    LOG.info(f"Updated value: {updated_value}")
    
    # Verify the value was updated correctly
    if updated_value == new_value:
        LOG.info("✅ Value update successful!")
    else:
        raise RuntimeError(f"❌ Value update failed: expected {new_value}, got {updated_value}")
    
    # Record test results
    test_result.mark_success(
        contract_address=contract_address,
        initial_value=initial_value,
        new_value=new_value,
        updated_value=updated_value,
        gas_used=int(tx_receipt.get('gasUsed', '0x0'), 16),
        contract_methods=caller.get_available_methods()
    )
    
    LOG.info("SimpleStorage test with new utilities completed successfully")


@test_case  
async def test_erc20_with_new_utils(run_helper: RunHelper, test_result: TestResult):
    """Test ERC20 token using new utility functions"""
    LOG.info("Testing ERC20 with new utilities")
    
    # 1. Create test accounts
    deployer = await run_helper.create_test_account("token_deployer", fund_wei=5 * 10**18)
    recipient = await run_helper.create_test_account("token_recipient", fund_wei=1 * 10**18)
    
    # 2. Deploy SimpleToken contract using shared contracts
    shared_contracts = SharedContracts(run_helper)
    
    async def deploy_token():
        """Deploy SimpleToken contract"""
        from ...utils.shared_contracts import ContractDeployer
        deployer_tool = ContractDeployer(run_helper)
        return await deployer_tool.deploy_simple_contract("SimpleToken", deployer)
    
    contract_info = await shared_contracts.get_or_deploy("TestToken", deploy_token)
    contract_address = contract_info['address']
    LOG.info(f"Using TestToken at: {contract_address}")
    
    # 3. Create contract caller
    caller = ContractCaller(run_helper.client, contract_address, contract_info.get('abi'))
    
    # 4. Test ERC20 functionality
    
    # Get token information
    name = await caller.call("name")
    symbol = await caller.call("symbol")
    decimals = await caller.call("decimals")
    total_supply = await caller.call("totalSupply")
    
    LOG.info(f"Token: {name} ({symbol}), Decimals: {decimals}")
    LOG.info(f"Total Supply: {total_supply / 10**18:.2f} tokens")
    
    # Check deployer balance (should have total supply initially)
    deployer_balance = await caller.call("balanceOf", deployer["address"])
    LOG.info(f"Deployer balance: {deployer_balance / 10**18:.2f} tokens")
    
    # Verify initial balances
    if deployer_balance != total_supply:
        raise RuntimeError(f"Initial balance mismatch: expected {total_supply}, got {deployer_balance}")
    
    # Transfer tokens to recipient
    transfer_amount = 100 * 10**18  # 100 tokens
    
    LOG.info(f"Transferring {transfer_amount / 10**18} tokens to {recipient['address']}")
    
    tx_receipt = await caller.send_and_wait(
        "transfer",
        recipient["address"], 
        transfer_amount,
        from_account=deployer,
        gas_limit=65000
    )
    
    LOG.info(f"Transfer transaction confirmed: {tx_receipt['transactionHash']}")
    
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
    
    LOG.info("✅ Transfer successful!")
    
    # Record test results
    test_result.mark_success(
        contract_address=contract_address,
        token_name=name,
        token_symbol=symbol,
        token_decimals=decimals,
        total_supply=total_supply,
        transfer_amount=transfer_amount,
        final_deployer_balance=deployer_balance_after,
        final_recipient_balance=recipient_balance_after,
        gas_used=int(tx_receipt.get('gasUsed', '0x0'), 16)
    )
    
    LOG.info("ERC20 test with new utilities completed successfully")


@test_case
async def test_contract_utils_directly(run_helper: RunHelper, test_result: TestResult):
    """Test contract utility functions directly"""
    LOG.info("Testing contract utility functions")
    
    # 1. Load contract data
    contract_data = ContractUtils.load_contract_data("SimpleStorage")
    LOG.info(f"Loaded SimpleStorage contract with {len(contract_data.get('abi', []))} ABI items")
    
    # 2. Test function encoding
    test_value = 12345
    encoded_call = encode_function_call("getValue")
    LOG.info(f"Encoded getValue(): {encoded_call}")
    
    encoded_set_call = encode_function_call("setValue", [test_value])
    LOG.info(f"Encoded setValue({test_value}): {encoded_set_call[:50]}...")
    
    # 3. Deploy a contract for testing
    deployer = await run_helper.create_test_account("utils_test_deployer", fund_wei=3 * 10**18)
    
    # Deploy using manual transaction
    nonce = await run_helper.client.get_transaction_count(deployer["address"])
    gas_price = await run_helper.client.get_gas_price()
    
    deploy_tx = {
        "data": contract_data["bytecode"],
        "gas": hex(200000),
        "gasPrice": hex(gas_price),
        "nonce": hex(nonce),
        "chainId": hex(await run_helper.client.get_chain_id())
    }
    
    # Sign and send
    from eth_account import Account
    private_key = deployer["private_key"]
    if private_key.startswith("0x"):
        private_key = private_key[2:]
    
    signed_tx = Account.sign_transaction(deploy_tx, private_key)
    tx_hash = await run_helper.client.send_raw_transaction(signed_tx.raw_transaction)
    
    receipt = await run_helper.client.wait_for_transaction_receipt(tx_hash, timeout=60)
    
    if receipt.get("status") != "0x1":
        raise RuntimeError(f"Contract deployment failed: {receipt}")
    
    contract_address = receipt.get("contractAddress")
    LOG.info(f"Contract deployed at: {contract_address}")
    
    # 4. Test calls using utility functions
    # Read initial value
    initial_result = await run_helper.client.call(to=contract_address, data=encoded_call)
    initial_value = decode_result("getValue", initial_result)
    LOG.info(f"Initial value: {initial_value}")
    
    # Set new value
    set_result = await run_helper.client.send_raw_transaction(
        Account.sign_transaction({
            "to": contract_address,
            "data": encoded_set_call,
            "gas": hex(50000),
            "gasPrice": hex(gas_price),
            "nonce": hex(nonce + 1),
            "chainId": hex(await run_helper.client.get_chain_id()),
            "value": "0x0"
        }, private_key).raw_transaction
    )
    
    set_receipt = await run_helper.client.wait_for_transaction_receipt(set_result, timeout=60)
    
    if set_receipt.get("status") != "0x1":
        raise RuntimeError(f"Set value transaction failed: {set_receipt}")
    
    # Read updated value
    updated_result = await run_helper.client.call(to=contract_address, data=encoded_call)
    updated_value = decode_result("getValue", updated_result)
    LOG.info(f"Updated value: {updated_value}")
    
    # Test results
    test_result.mark_success(
        contract_address=contract_address,
        initial_value=initial_value,
        test_value=test_value,
        final_value=updated_value,
        deployment_gas_used=int(receipt.get('gasUsed', '0x0'), 16),
        set_gas_used=int(set_receipt.get('gasUsed', '0x0'), 16),
        utilities_tested=["encode_function_call", "decode_result", "load_contract_data"]
    )
    
    LOG.info("Contract utilities test completed successfully")