# Gravity Cluster Configuration - storage_v2_baseline (template)
#
# Rendered to cluster.toml by render_config.py using test_params.toml —
# {{SOURCE}} becomes the node binary source (bin_path / project_path /
# github+rev). The rendered cluster.toml is untracked (see .gitignore), so
# an external gravity_node binary can be pinned without modifying any
# tracked file. See README.md.
#
# Skeleton: single_node/cluster.toml, with a dedicated port block and
# base_dir so a live run cannot collide with other suites or with any
# manually started local cluster.

[cluster]
name = "gravity-devnet-storage-v2-baseline"
base_dir = "/tmp/gravity-cluster-storage-v2-baseline"

[genesis_source]
genesis_path = "./artifacts/genesis.json"
waypoint_path = "./artifacts/waypoint.txt"

[[nodes]]
id = "node1"
role = "genesis"
source = {{SOURCE}}
host = "127.0.0.1"
validator_port = 6480
vfn_port = 6490
rpc_port = 18745
metrics_port = 9301
inspection_port = 10310
https_port = 1330
authrpc_port = 8861
reth_p2p_port = 12330

# No [faucet_init]: the case only needs the genesis faucet account (known
# dev key), not gravity_bench-funded bench accounts — skipping it keeps the
# suite start fast and avoids the gravity_bench build dependency.
