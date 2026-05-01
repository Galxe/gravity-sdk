{
    "reth_args": {
        "chain": "${GENESIS_PATH}",
        "http": "",
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
        "gravity_node_config": "${CONFIG_DIR}/public_full_node.yaml",
        "log.file.directory": "${LOG_DIR}/execution_logs/",
        "rpc.max-subscriptions-per-connection": 20000,
        "rpc.max-connections": 20000,
        "txpool.max-new-pending-txs-notifications": 1000000,
        "txpool.max-pending-txns": 1000000,
        "txpool.pending-max-count": 17592186044415,
        "txpool.pending-max-size": 17592186044415,
        "txpool.basefee-max-count": 17592186044415,
        "txpool.basefee-max-size": 17592186044415,
        "txpool.queued-max-count": 17592186044415,
        "txpool.queued-max-size": 17592186044415,
        "txpool.max-account-slots": ${TXPOOL_MAX_ACCOUNT_SLOTS},
        "ipcdisable": ""
    },
    "env_vars": {
        "BATCH_INSERT_TIME": 20
    }
}
