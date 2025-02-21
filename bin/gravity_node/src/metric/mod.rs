use greth::reth_metrics::{
    metrics::{Counter, Histogram},
    Metrics,
};

#[derive(Metrics)]
#[metrics(scope = "reth_coordinator")]
pub(crate) struct RethCliMetric {
    /// recv txns rate from reth to aptos
    pub(crate) recv_txns_rate: Histogram,
}
