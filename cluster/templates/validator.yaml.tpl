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

validator_network:
  network_id: validator
  listen_address: "/ip4/0.0.0.0/tcp/${VALIDATOR_PORT}"
${DISCOVERY_METHOD_NETWORK_BLOCK}
  mutual_authentication: true
  identity:
    type: "${NETWORK_IDENTITY_TYPE}"
    ${NETWORK_IDENTITY_FIELD}: ${NETWORK_IDENTITY_VALUE}

full_node_networks:
  - network_id:
      private: "vfn"
    listen_address: "/ip4/0.0.0.0/tcp/${VFN_PORT}"
    identity:
      type: "${NETWORK_IDENTITY_TYPE}"
      ${NETWORK_IDENTITY_FIELD}: ${NETWORK_IDENTITY_VALUE}
${DISCOVERY_METHOD_FULLNODE_BLOCK}
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
