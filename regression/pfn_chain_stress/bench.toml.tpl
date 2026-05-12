# Bench config for pfn_chain Docker stress test.
# Rendered by render.sh from env vars (see envsubst block in render.sh).
#
# contract_config_path is a relative path; bench will deploy fresh ERC20s on
# first run and write the address map there. Keep it inside /tmp/bench so it
# persists across `docker compose run --rm bench` invocations of the same
# cluster (the ./bench volume mount on the host side preserves it).

contract_config_path = "/tmp/bench/deploy.json"
target_tps = ${BENCH_TARGET_TPS}

nodes = [
    { rpc_url = "${TARGET_RPC_URL}", chain_id = ${TARGET_CHAIN_ID} },
]

num_tokens = 2
enable_swap_token = false
address_pool_type = "random"

[faucet]
private_key = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
faucet_level = 10
wait_duration_secs = 30
fauce_eth_balance = "100000000000000000000"

[accounts]
num_accounts = ${BENCH_NUM_ACCOUNTS}

[performance]
num_senders = ${BENCH_NUM_SENDERS}
max_pool_size = 100000
duration_secs = ${BENCH_DURATION_SECS}
sampling = 20

log_path = "/tmp/bench/bench.log"
