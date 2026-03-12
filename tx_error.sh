RECEIPT_DATA='{
    "jsonrpc": "2.0",
    "method": "eth_getTransactionReceipt",
    "params": ["0x915bdb39dab3bb175a0f505193737133d29775a79d149da03dbe1a26b20920cc"],
    "id": 2
}'

curl -X POST \
     -H "Content-Type: application/json" \
     --data "$RECEIPT_DATA" \
     http://127.0.0.1:8545
