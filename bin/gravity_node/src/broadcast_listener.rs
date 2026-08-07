//! Consumer-side admit batch policy for the broadcast listener.
//!
//! Pure flush decision (CAP + hard time limit). Async drain loop lands in a later task.

use std::time::Duration;

/// Max number of txs to batch before flushing to admit.
pub const ADMIT_BATCH_CAP: usize = 128;

/// Max time to wait after the first item before flushing even if under CAP.
pub const ADMIT_BATCH_MAX_WAIT: Duration = Duration::from_millis(1);

/// Drain decision: flush when CAP hit, max wait elapsed, or no more pending work.
///
/// Actual async loop must still hard-timeout the wait for the next item
/// (`tokio::time::timeout` on `recv`), not only check elapsed after a blocking recv.
pub fn should_flush(len: usize, elapsed: Duration, more_pending: bool) -> bool {
    len >= ADMIT_BATCH_CAP || elapsed >= ADMIT_BATCH_MAX_WAIT || (!more_pending && len > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flush_on_cap() {
        assert!(should_flush(128, Duration::from_micros(10), true));
    }

    #[test]
    fn flush_on_max_wait_even_if_under_cap() {
        assert!(should_flush(1, Duration::from_millis(1), true));
    }

    #[test]
    fn no_flush_mid_batch_before_wait() {
        assert!(!should_flush(3, Duration::from_micros(100), true));
    }

    #[test]
    fn flush_when_no_more_pending() {
        assert!(should_flush(1, Duration::from_micros(1), false));
    }
}
