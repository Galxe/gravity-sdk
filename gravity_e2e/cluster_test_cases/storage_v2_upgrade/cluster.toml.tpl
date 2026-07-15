# Gravity Cluster Configuration - storage_v2_upgrade (template)
#
# Rendered to cluster.toml by render_config.py using test_params.toml —
# {{SOURCE}} becomes the OLD (pre-upgrade) node binary source for every
# node (bin_path / project_path / github+rev). The rendered cluster.toml
# is untracked (see .gitignore), so the v1.7.5 binary can be pinned
# without modifying any tracked file. See README.md.
#
# Topology: rolling_upgrade's 6 nodes (node1-node4 genesis, node5
# validator, vfn1 full node), with a dedicated port block and base_dir so
# a live run cannot collide with other suites or with any manually
# started local cluster.

[cluster]
name = "gravity-devnet-storage-v2-upgrade"
base_dir = "/tmp/gravity-cluster-storage-v2-upgrade"

[genesis_source]
genesis_path = "./artifacts/genesis.json"
waypoint_path = "./artifacts/waypoint.txt"

# ============ Genesis Nodes (node1-node4) ============

[[nodes]]
id = "node1"
host = "127.0.0.1"
role = "genesis"
source = {{SOURCE}}
validator_port = 6580
vfn_port = 6590
rpc_port = 18845
metrics_port = 9401
inspection_port = 10410
https_port = 1430
authrpc_port = 8871
reth_p2p_port = 12430

[[nodes]]
id = "node2"
host = "127.0.0.1"
role = "genesis"
source = {{SOURCE}}
validator_port = 6581
vfn_port = 6591
rpc_port = 18846
metrics_port = 9402
inspection_port = 10411
https_port = 1431
authrpc_port = 8872
reth_p2p_port = 12431

[[nodes]]
id = "node3"
host = "127.0.0.1"
role = "genesis"
source = {{SOURCE}}
validator_port = 6582
vfn_port = 6592
rpc_port = 18847
metrics_port = 9403
inspection_port = 10412
https_port = 1432
authrpc_port = 8873
reth_p2p_port = 12432

[[nodes]]
id = "node4"
host = "127.0.0.1"
role = "genesis"
source = {{SOURCE}}
validator_port = 6583
vfn_port = 6593
rpc_port = 18848
metrics_port = 9404
inspection_port = 10413
https_port = 1433
authrpc_port = 8874
reth_p2p_port = 12433

# ============ Validator Node (node5) ============

[[nodes]]
id = "node5"
host = "127.0.0.1"
role = "validator"
source = {{SOURCE}}
validator_port = 6584
vfn_port = 6594
rpc_port = 18849
metrics_port = 9405
inspection_port = 10414
https_port = 1434
authrpc_port = 8875
reth_p2p_port = 12434

# ============ Full Node (vfn1) ============

[[nodes]]
id = "vfn1"
host = "127.0.0.1"
role = "vfn"
source = {{SOURCE}}
vfn_port = 6595
rpc_port = 18850
metrics_port = 9406
inspection_port = 10415
https_port = 1435
authrpc_port = 8876
reth_p2p_port = 12435

# No [faucet_init]: the case only needs the genesis faucet account (known
# dev key) for history-building and the background tx load — skipping it
# keeps the suite start fast and avoids the gravity_bench build dependency.
