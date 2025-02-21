use std::{cell::OnceCell, sync::OnceLock};

use greth::reth_metrics::{
    metrics::{Counter, Histogram},
    Metrics,
};

#[derive(Metrics)]
#[metrics(scope = "reth_coordinator")]
pub(crate) struct RethCliMetric {
    /// recv txns rate from aptos
    pub(crate) recv_txns_count: Counter,
    /// recv txns size from reth 
    pub(crate)  reth_notify_count: Counter,
}


pub(crate) static METRICS: OnceLock<RethCliMetric> = OnceLock::new();