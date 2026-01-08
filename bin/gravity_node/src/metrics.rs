use std::sync::OnceLock;

use greth::reth_metrics::{metrics::Histogram, Metrics};

use greth::reth_metrics::metrics;

#[derive(Metrics)]
#[metrics(scope = "reth.mempook")]
pub(crate) struct RethTxnMetrics {
    /// Histogram of state root duration
    pub(crate) txn_time: Histogram,
}

static RETH_TXN_METRICS: OnceLock<RethTxnMetrics> = OnceLock::new();

pub fn fetch_reth_txn_metrics() -> &'static RethTxnMetrics {
    RETH_TXN_METRICS.get_or_init(|| RethTxnMetrics::default())
}
