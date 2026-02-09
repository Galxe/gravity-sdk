base:
  role: "validator"
  data_dir: "${DATA_DIR}/data"
  waypoint:
    from_file: "${CONFIG_DIR}/waypoint.txt"

consensus:
  safety_rules:
    backend:
      type: "on_disk_storage"
      path: ${DATA_DIR}/data/secure_storage.json
    initial_safety_rules_config:
      from_file:
        waypoint:
          from_file: ${CONFIG_DIR}/waypoint.txt
        identity_blob_path: ${CONFIG_DIR}/identity.yaml
  enable_pipeline: true
  max_sending_block_txns_after_filtering: 5000
  max_sending_block_txns: 5000
  max_receiving_block_txns: 5000
  max_sending_block_bytes: 31457280
  max_receiving_block_bytes: 31457280
  quorum_store:
    receiver_max_total_txns: 7000
    sender_max_total_txns: 7000
    receiver_max_batch_bytes: 1048736
    sender_max_batch_bytes: 1048736
    sender_max_total_bytes: 1073741824
    receiver_max_total_bytes: 1073741824
    memory_quota: 1073741824
    db_quota: 1073741824
    back_pressure:
      dynamic_max_txn_per_s: 30000
      backlog_txn_limit_count: 50000
      backlog_per_validator_batch_limit_count: 2000

validator_network:
  network_id: validator
  listen_address: "/ip4/0.0.0.0/tcp/${P2P_PORT}"
  discovery_method:
    onchain
  mutual_authentication: true
  identity:
    type: "from_file"
    path: ${CONFIG_DIR}/identity.yaml

full_node_networks:
  - network_id:
      private: "vfn"
    listen_address: "/ip4/0.0.0.0/tcp/${VFN_PORT}"
    identity:
      type: "from_file"
      path: ${CONFIG_DIR}/identity.yaml
    discovery_method:
      onchain
    mutual_authentication: false

storage:
  dir: "${DATA_DIR}/data"

log_file_path: "${DATA_DIR}/consensus_log/validator.log"

inspection_service:
  port: ${INSPECTION_PORT}
  address: 0.0.0.0

mempool:
  capacity_per_user: 20000

https_server_address: 0.0.0.0:${HTTPS_PORT}
