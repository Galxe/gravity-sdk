# Gravity Genesis Configuration - storage_v2_fresh_sync (template)
# Rendered to genesis.toml by render_config.py — do not edit the rendered
# file directly. Skeleton: storage_v2_upgrade's tpl with this case's ports
# (kept consistent with cluster.toml.tpl by a unit test), plus the
# fuzzy_cluster governance recipe so the case can flip permissionless
# validator join via the full proposal lifecycle.

[dependencies.genesis_contracts]
repo = "{{GENESIS_CONTRACTS_REPO}}"
ref = "{{GENESIS_CONTRACTS_REF}}"

[genesis.hardforks]
{{HARDFORKS}}

# Two genesis validators only (the user-pinned matrix topology): f=0
# until sf_val1 joins — every validator swap window freezes the chain,
# which the case treats as expected (constraint (3) in the test module).
#
# node1.address == faucet so pool[0].voter is the faucet, letting the
# case drive the one-time governance proposal that enables permissionless
# join (the fuzzy_cluster recipe).
[[genesis_validators]]
id = "node1"
address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
host = "127.0.0.1"
validator_port = 6680
vfn_port = 6690
stake_amount = "2000000000000000000"
voting_power = "2000000000000000000"
consensus_pop = "0x000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"

[[genesis_validators]]
id = "node2"
address = "0x7b254Bd44F6CE45e00a912b2460D47F3Be56fAD7"
host = "127.0.0.1"
validator_port = 6681
vfn_port = 6691
stake_amount = "2000000000000000000"
voting_power = "2000000000000000000"
consensus_pop = "0x000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"

[genesis]
chain_id = 1337
# 2 min (fuzzy_cluster's value): the case needs epoch crossings for the
# A-batch history, the join activation and the L2 window; 5 min would
# add ~10 min of pure waiting.
epoch_interval_micros = 120000000
major_version = 1
consensus_config = "0x0301010a00000000000000280000000000000001010000000a000000000000000100010200000000000000000020000000000000"
execution_config = "0x00"
initial_locked_until_micros = 1798848000000000

# Governance owner: faucet — addExecutor() authority for the
# permissionless-join setup (helpers/governance.py).
governance_owner = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"

[genesis.faucet]
address = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
private_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
balance = "1000000000000000000000000"  # 1M ETH

[genesis.validator_config]
minimum_bond = "1000000000000000000"
maximum_bond = "1000000000000000000000000"
unbonding_delay_micros = 604800000000
allow_validator_set_change = true
# sf_val1 joins with 2 ETH on a 4 ETH total = +50%; the default limit of
# 50 sits exactly on the boundary, so give explicit headroom — the L3
# necessity probe REQUIRES the equal-power join to land.
voting_power_increase_limit_pct = 100
max_validator_set_size = "100"
auto_evict_enabled = false
auto_evict_threshold_pct = 0

[genesis.staking_config]
minimum_stake = "1000000000000000000"
lockup_duration_micros = 86400000000
unbonding_delay_micros = 86400000000

# Short voting window; pool[0]'s 2e18 VP clears both thresholds in one
# vote (fuzzy_cluster recipe).
[genesis.governance_config]
min_voting_threshold = "1000000000000000000"
required_proposer_stake = "1000000000000000000"
voting_duration_micros = 5000000

[genesis.randomness_config]
variant = 1
secrecy_threshold = 9223372036854775808
reconstruction_threshold = 12297829382473033728
fast_path_secrecy_threshold = 12297829382473033728

[genesis.oracle_config]
source_types = [1]
callbacks = ["0x00000000000000000000000000000001625F4001"]

[genesis.oracle_config.bridge_config]
deploy = true
trusted_bridge = "0xcbEAF3BDe82155F56486Fb5a1072cb8baAf547cc"
trusted_source_id = 11155111

[[genesis.oracle_config.tasks]]
source_type = 0
source_id = 11155111
task_name = "sepolia"
config = "gravity://0/11155111/events?contract=0x0f761B1B3c1aC9232C9015A7276692560aD6a05F&eventSignature=0x5646e682c7d994bf11f5a2c8addb60d03c83cda3b65025a826346589df43406e&fromBlock=10201260"

# JWK config - Google OIDC provider
[genesis.jwk_config]
issuers = ["0x68747470733a2f2f6163636f756e74732e676f6f676c652e636f6d"]

[[genesis.jwk_config.jwks]]
kid = "f5f4c0ae6e6090a65ab0a694d6ba6f19d5d0b4e6"
kty = "RSA"
alg = "RS256"
e = "AQAB"
n = "2K7epoJWl_aBoYGpXmDBBiEnwQ0QdVRU1gsbGXNrEbrZEQdY5KjH5P5gZMq3d3KvT1j5KsD2tF_9jFMDLqV4VWDNJRLgSNJxhJuO_oLO2BXUSL9a7fLHxnZCUfJvT2K-O8AXjT3_ZM8UuL8d4jBn_fZLzdEI4MHrZLVSaHDvvKqL_mExQo6cFD-qyLZ-T6aHv2x8R7L_3X7E1nGMjKVVZMveQ_HMeXvnGxKf5yfEP0hIQlC_kFm4L_1kV1S0UPmMptZL2qI4VnXqmqI6TZJyE-3VXHgNn1Z1O_9QZlPC0fF0spLHf2S3nNqI0v3k2E7q3DkqxVf5xvn7q_X-gPqzVE9Jw"
