use gaptos::aptos_metrics_core::{register_histogram, Histogram};
use once_cell::sync::Lazy;
pub static TXN_TO_BLOCK_BUFFER_MANAGER: Lazy<Histogram> = Lazy::new(|| {
    register_histogram!(
        "txn_to_block_buffer_manager_ms",
        "Latency of txn to block buffer manager",
    )
    .unwrap()
});