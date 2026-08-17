{
  "reth_args": {
    "chain": "${GENESIS_PATH}",
    "http": "",
    "relayer_config": "${CONFIG_DIR}/relayer_config.json",
    "http.port": ${RPC_PORT},
    "http.corsdomain": "${RPC_HTTP_CORSDOMAIN}",
    "http.api": "${RPC_HTTP_API}",
    "http.addr": "0.0.0.0",
    "dev": "",
    "port": ${P2P_PORT_RETH},
    "authrpc.port": ${AUTHRPC_PORT},
    "authrpc.addr": "0.0.0.0",
    "metrics": "0.0.0.0:${METRICS_PORT}",
    "log.file.filter": "info",
    "log.stdout.filter": "error",
    "datadir": "${STORAGE_DIR}/reth",
    "datadir.static-files": "${STORAGE_DIR}/reth",
    "gravity_node_config": "${CONFIG_DIR}/validator.yaml",
    "log.file.directory": "${LOG_DIR}/execution_logs/",
    "rpc.eth-proof-window": 128,
    "ipcdisable": ""
  },
  "env_vars": {
    "BATCH_INSERT_TIME": 20
  }
}
