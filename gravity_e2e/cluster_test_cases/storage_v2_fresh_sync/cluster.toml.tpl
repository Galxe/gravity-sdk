# Gravity Cluster Configuration - storage_v2_fresh_sync (template)
#
# Rendered to cluster.toml by render_config.py using test_params.toml:
# - {{SOURCE}}    -> the OLD (v1.7.5) binary for the four legacy nodes,
#                    which the case rolling-upgrades to the new binary;
# - {{SF_SOURCE}} -> the NEW (merge v2.3.0) binary for the five SF nodes,
#                    which never run anything else.
# The rendered cluster.toml is untracked (see .gitignore). See README.md.
#
# Topology (storage-v2 TC9): the SF x non-SF upstream/downstream matrix.
#
#   node1 (genesis) <── vfn1 <── pfn1          <- tx entry (ALL load)
#         ^                └───── sf_pfn2      SF pfn  <- legacy vfn
#         │
#   node2 (genesis) <── sf_vfn1 <── sf_pfn1    SF pfn  <- SF vfn
#   sf_val1 (joins)  <── sf_vfn2                SF vfn  <- SF validator
#
# vfn1/pfn1 are the legacy controls (upgraded v1.7.5 datadirs); the five
# sf_* nodes are SF-enabled fresh nodes that sync from block 0 AFTER the
# legacy core is upgraded. legacy-downstream-of-SF-upstream cells are
# deliberately absent: the sync wire protocol is transparent to the
# downstream's storage format, and the SF serving path is covered by the
# SF <- SF cells; wiring a control node under an SF upstream would only
# contaminate the control group.
#
# Role-specific ports: pfn nodes carry ONLY public_port (no
# validator_port/vfn_port); sf_val1 carries vfn_port because it is
# sf_vfn2's upstream (VFN network listener). Every vfn/pfn pins its
# upstream via static seeds + discovery_method = "none" so the matrix
# edges are exclusive (on-chain discovery would fan out to every
# validator and dissolve the per-edge coverage).

[cluster]
name = "gravity-devnet-storage-v2-fresh-sync"
base_dir = "/tmp/gravity-cluster-storage-v2-fresh-sync"

[genesis_source]
genesis_path = "./artifacts/genesis.json"
waypoint_path = "./artifacts/waypoint.txt"

# ============ Legacy core (v1.7.5 -> rolling upgrade) ============

[[nodes]]
id = "node1"
host = "127.0.0.1"
role = "genesis"
source = {{SOURCE}}
validator_port = 6680
vfn_port = 6690
rpc_port = 18945
metrics_port = 9501
inspection_port = 10510
https_port = 1530
authrpc_port = 8951
reth_p2p_port = 12530

[[nodes]]
id = "node2"
host = "127.0.0.1"
role = "genesis"
source = {{SOURCE}}
validator_port = 6681
vfn_port = 6691
rpc_port = 18946
metrics_port = 9502
inspection_port = 10511
https_port = 1531
authrpc_port = 8952
reth_p2p_port = 12531

[[nodes]]
id = "vfn1"
host = "127.0.0.1"
role = "vfn"
source = {{SOURCE}}
seeds = [
    { from = "node1" },     # Vfn seeds -> node1's Vfn listener (6690)
]
# Pin the upstream edge (vfn1 <- node1): static seeds already reach it,
# and on-chain discovery would also dial node2 / sf_val1.
discovery_method = "none"
vfn_port = 6692
public_port = 6700          # Public listener so pfn1 and sf_pfn2 can dial in
rpc_port = 18947
metrics_port = 9503
inspection_port = 10512
https_port = 1532
authrpc_port = 8953
reth_p2p_port = 12532

[[nodes]]
id = "pfn1"
host = "127.0.0.1"
role = "pfn"
source = {{SOURCE}}
seeds = [
    { from = "vfn1" },      # upstream — auto-infers ValidatorFullNode
]
public_port = 6701
rpc_port = 18948
metrics_port = 9504
inspection_port = 10513
authrpc_port = 8954
reth_p2p_port = 12533

# ============ SF nodes (merge v2.3.0 from birth, sync from 0) ============

[[nodes]]
id = "sf_val1"
host = "127.0.0.1"
role = "validator"
source = {{SF_SOURCE}}
validator_port = 6682
vfn_port = 6693             # sf_vfn2's upstream listener
rpc_port = 18949
metrics_port = 9505
inspection_port = 10514
https_port = 1534
authrpc_port = 8955
reth_p2p_port = 12534

[[nodes]]
id = "sf_vfn1"
host = "127.0.0.1"
role = "vfn"
source = {{SF_SOURCE}}
seeds = [
    { from = "node2" },     # SF vfn <- legacy validator
]
discovery_method = "none"
vfn_port = 6694
public_port = 6702          # Public listener so sf_pfn1 can dial in
rpc_port = 18950
metrics_port = 9506
inspection_port = 10515
https_port = 1535
authrpc_port = 8956
reth_p2p_port = 12535

[[nodes]]
id = "sf_vfn2"
host = "127.0.0.1"
role = "vfn"
source = {{SF_SOURCE}}
seeds = [
    { from = "sf_val1" },   # SF vfn <- SF validator (start only after sf_val1)
]
discovery_method = "none"
vfn_port = 6695
rpc_port = 18951
metrics_port = 9507
inspection_port = 10516
https_port = 1536
authrpc_port = 8957
reth_p2p_port = 12536

[[nodes]]
id = "sf_pfn1"
host = "127.0.0.1"
role = "pfn"
source = {{SF_SOURCE}}
seeds = [
    { from = "sf_vfn1" },   # SF pfn <- SF vfn (start only after sf_vfn1)
]
public_port = 6703
rpc_port = 18952
metrics_port = 9508
inspection_port = 10517
authrpc_port = 8958
reth_p2p_port = 12537

[[nodes]]
id = "sf_pfn2"
host = "127.0.0.1"
role = "pfn"
source = {{SF_SOURCE}}
seeds = [
    { from = "vfn1" },      # SF pfn <- legacy vfn
]
public_port = 6704
rpc_port = 18953
metrics_port = 9509
inspection_port = 10518
authrpc_port = 8959
reth_p2p_port = 12538

# [faucet_init] is REQUIRED here (unlike storage_v2_upgrade): sf_val1's
# validator_join draws its EVM account from the accounts.csv this
# generates (manager._ensure_evm_account -> get_bench_accounts), and the
# 2 ETH equal-power stake is paid from that account's init balance. One
# account: sf_val1 is the only VALIDATOR-role node. The funding happens
# at suite init — before pytest, before the background sender exists —
# so it cannot race anything (account-discipline constraint (4) in the
# test module).
[faucet_init]
num_accounts = 1
private_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
eth_balance = "10000000000000000000000"
