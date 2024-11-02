import json
import argparse
from itertools import combinations
import concurrent.futures
import time
from web3 import Web3
from eth_account import Account

parser = argparse.ArgumentParser(description='Process account path.')
parser.add_argument('--account_path', type=str, default='genesis_accounts.json',
                    help='Path to the account file (default: genesis_accounts.json)')
parser.add_argument('--rpc_port', type=str, default='http://127.0.0.1:8545',
                    help='RPC port (default: http://127.0.0.1:8545)')
parser.add_argument('--chain_id', type=str, default='1',
                    help='Chain id (default: 1(Mainnet)')
args = parser.parse_args()

class Account:
    def __init__(self, address, private_key, nonce: 0):
        self.address = address
        self.private_key = private_key
        self.balance = 100000
        self.nonce = nonce

    def __repr__(self):
        return f"Account(address={self.address}, private_key={self.private_key}, balance={self.balance}, nonce={self.nonce})"
    
    def set_nonce(self, nonce):
        self.nonce = nonce

    def inc_nonce(self):
        nonce = self.nonce
        self.nonce = nonce + 1
        return nonce

def load_accounts_from_json(file_path):
    with open(file_path, 'r') as file:
        accounts_data = json.load(file)

    accounts = []
    for account in accounts_data:
        addr = account['address']
        priv_key = account['private_key']
        accounts.append(Account(addr, priv_key, 0))
    
    return accounts

def init_nonce_for_accounts(w3, accounts):
    for account in accounts:
        nonce = w3.eth.get_transaction_count(account.address)
        account.set_nonce(nonce)

def single_transaction_request(w3, tx, private_key):
    try:
        print('Signing transaction...')
        signed_tx = w3.eth.account.sign_transaction(tx, private_key)

        print('Sending transaction...')
        tx_hash = w3.eth.send_raw_transaction(signed_tx.raw_transaction)
        print('Transaction hash:', tx_hash.hex())

        receipt = w3.eth.wait_for_transaction_receipt(tx_hash)
        print('Transaction receipt:', receipt.hex())

    except Exception as e:
        print('An error occurred:', str(e))

def generate_batch_task(w3, accounts):
    pairs = []
    for from_account, to_account in combinations(accounts, 2):
        private_key = from_account.private_key
        value = w3.to_wei(0.1, 'ether')
        from_nonce = from_account.inc_nonce()
        print(f"{from_account.address} to {to_account.address}, nonce {from_nonce}")

        tx = {
            'from': from_account.address,
            'nonce': from_nonce,
            'to': to_account.address,
            'value': value,
            'gas': 21000,
            'gasPrice': w3.to_wei(1, 'gwei'),
            'chainId': args.chain_id,
        }
        pairs.append((tx, private_key))
    return pairs

def request_process(accounts, addr):
    w3 = Web3(Web3.HTTPProvider(addr))
    init_nonce_for_accounts(w3, accounts)
    while True:
        tasks = generate_batch_task(w3, accounts)
        print(f"Going to process {len(tasks)} request")
        
        with concurrent.futures.ThreadPoolExecutor() as executor:
            futures = [executor.submit(single_transaction_request, w3, tx, private_key) 
                      for tx, private_key in tasks]

            for future in concurrent.futures.as_completed(futures):
                result = future.result()
                print(result)

def main():
    print(f'Using account path: {args.account_path}')
    accounts = load_accounts_from_json(args.account_path)
    request_process(accounts, args.rpc_port)

if __name__ == "__main__":
    main()