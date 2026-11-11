//! The time a snapshot gives its out-of-process iframes. Each iframe target is
//! read in turn after the main page, so one that answers slowly would use up
//! the whole snapshot deadline and lose the page with it. A budgeted read
//! stops iframe reads a reserve short of the deadline and leaves out the
//! iframes that miss it, while the page is still returned.

use std::time::Duration;

use agent_desktop_core::{AdapterError, Deadline};

use crate::transport::transport_error::is_timeout;

const RESERVE_SHARE: u32 = 5;
const MINIMUM_RESERVE: Duration = Duration::from_millis(100);

#[derive(Clone, Copy)]
pub(crate) struct FrameBudget {
    snapshot: Deadline,
    iframes: Deadline,
    leaves_out_late_iframes: bool,
}

impl FrameBudget {
    /// Iframes share the snapshot deadline, and one that misses it fails the
    /// snapshot.
    pub(crate) fn whole(deadline: Deadline) -> Self {
        Self {
            snapshot: deadline,
            iframes: deadline,
            leaves_out_late_iframes: false,
        }
    }

    /// Iframes get what remains of `deadline` when the read starts, less a
    /// fifth of it (at least 100 ms) kept for detaching the iframe sessions
    /// and building the tree.
    pub(crate) fn reserved(deadline: Deadline) -> Self {
        let remaining = deadline.remaining();
        let reserve = (remaining / RESERVE_SHARE).max(MINIMUM_RESERVE);
        Self {
            snapshot: deadline,
            iframes: deadline.capped(remaining.saturating_sub(reserve)),
            leaves_out_late_iframes: true,
        }
    }

    pub(crate) fn snapshot(self) -> Deadline {
        self.snapshot
    }

    pub(crate) fn iframes(self) -> Deadline {
        self.iframes
    }

    /// Whether the iframe budget is spent, so no further iframe is read.
    pub(crate) fn is_spent(self) -> bool {
        self.leaves_out_late_iframes && self.iframes.is_expired()
    }

    /// Whether `error` is an iframe read that ran out of its budget while the
    /// snapshot itself still has time.
    pub(crate) fn missed(self, error: &AdapterError) -> bool {
        self.leaves_out_late_iframes && is_timeout(error) && !self.snapshot.is_expired()
    }
}

#[cfg(test)]
#[path = "frame_budget_tests.rs"]
mod tests;
