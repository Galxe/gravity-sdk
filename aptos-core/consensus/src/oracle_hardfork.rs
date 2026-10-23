// Copyright © Galxe
// SPDX-License-Identifier: Apache-2.0

//! Activation time for Oracle update signature verification.
//!
//! The rule is evaluated against the timestamp of the block being executed,
//! including during recovery. A node upgrade must not change the execution of
//! historical blocks before the coordinated activation time.

use std::sync::OnceLock;

static ORACLE_SIGNATURE_VERIFICATION_TIME_USECS: OnceLock<Option<u64>> = OnceLock::new();

/// Set the activation timestamp (microseconds) once during node startup.
/// `None` leaves the new rule disabled until a network activation is scheduled.
pub fn init_oracle_signature_verification_time(timestamp_usecs: Option<u64>) {
    ORACLE_SIGNATURE_VERIFICATION_TIME_USECS
        .set(timestamp_usecs)
        .expect("Oracle signature verification activation time already initialized");
}

/// Whether the rule is active for this block timestamp (microseconds).
/// Before initialization, the rule is inactive.
pub fn is_active(block_timestamp_usecs: u64) -> bool {
    is_active_at(
        ORACLE_SIGNATURE_VERIFICATION_TIME_USECS.get().copied().flatten(),
        block_timestamp_usecs,
    )
}

fn is_active_at(activation_time_usecs: Option<u64>, block_timestamp_usecs: u64) -> bool {
    activation_time_usecs.is_some_and(|activation| block_timestamp_usecs >= activation)
}

#[cfg(test)]
mod tests {
    use super::is_active_at;

    #[test]
    fn inactive_without_activation_time() {
        assert!(!is_active_at(None, 0));
        assert!(!is_active_at(None, u64::MAX));
    }

    #[test]
    fn activates_at_inclusive_block_timestamp_boundary() {
        const ACTIVATION: u64 = 1_000_000;
        assert!(!is_active_at(Some(ACTIVATION), ACTIVATION - 1));
        assert!(is_active_at(Some(ACTIVATION), ACTIVATION));
        assert!(is_active_at(Some(ACTIVATION), ACTIVATION + 1));
    }
}
