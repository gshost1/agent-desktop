//! The pause an inventory stabilizer takes between two captures.
//!
//! A stabilizer accepts an inventory once two captures in a row agree. The
//! second capture follows the first at once: it is a separate read of the
//! live system, so agreement still shows nothing changed between the two
//! reads. Only a capture that differed from the one before it, or that failed,
//! is followed by a pause, so a system that is still changing gets time to
//! settle instead of being read back to back.

use std::time::{Duration, Instant};

const CHURN_PAUSE: Duration = Duration::from_millis(5);

/// Pauses before the next capture when the last one changed or failed.
pub(crate) fn pause_before_next_capture(attempts: u64, failed: bool, deadline: Instant) {
    if !needs_pause(attempts, failed) {
        return;
    }
    let remaining = deadline.saturating_duration_since(Instant::now());
    if !remaining.is_zero() {
        std::thread::sleep(remaining.min(CHURN_PAUSE));
    }
}

fn needs_pause(attempts: u64, failed: bool) -> bool {
    attempts > 1 || failed
}

#[cfg(test)]
#[path = "inventory_recapture_tests.rs"]
mod tests;
