base:
  role: "full_node"
  data_dir: "${STORAGE_DIR}"
  waypoint:
    from_file: "${CONFIG_DIR}/waypoint.txt"

consensus:
  safety_rules:
    backend:
      type: "on_disk_storage"
      path: ${STORAGE_DIR}/secure_storage.json
    initial_safety_rules_config:
      ${SAFETY_RULES_IDENTITY_VARIANT}:
        waypoint:
          from_file: ${CONFIG_DIR}/waypoint.txt
        ${SAFETY_RULES_IDENTITY_KEY}: ${SAFETY_RULES_IDENTITY_VALUE}
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

full_node_networks:
  - network_id: public
    listen_address: "/ip4/0.0.0.0/tcp/${PUBLIC_PORT}"
    identity:
      type: "${NETWORK_IDENTITY_TYPE}"
      ${NETWORK_IDENTITY_FIELD}: ${NETWORK_IDENTITY_VALUE}
${DISCOVERY_METHOD_FULLNODE_BLOCK}
${PFN_SEEDS_BLOCK}

storage:
  dir: "${STORAGE_DIR}"

log_file_path: "${LOG_DIR}/consensus_log/vfn.log"

inspection_service:
  port: ${INSPECTION_PORT}
  address: 0.0.0.0

mempool:
  capacity_per_user: 20000

logger:
  level: INFO
