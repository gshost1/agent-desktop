//! The time a snapshot gives its out-of-process iframes. Each iframe target is
//! read in turn after the main page, so one that answers slowly would use up
//! the whole snapshot deadline and lose the page with it. Iframe reads stop
//! a reserve short of the deadline, and an iframe that misses that budget is
//! left out as unreadable while the page is still returned.

use std::time::Duration;

use agent_desktop_core::{AdapterError, Deadline};

use crate::transport::transport_error::is_timeout;

const RESERVE_SHARE: u32 = 5;
const MINIMUM_RESERVE: Duration = Duration::from_millis(100);

/// The deadline for iframe reads: what remains of `deadline` after the main
/// page, less a fifth of it (at least 100 ms) kept for detaching the iframe
/// sessions and building the tree.
pub(crate) fn iframe_deadline(deadline: Deadline) -> Deadline {
    let remaining = deadline.remaining();
    let reserve = (remaining / RESERVE_SHARE).max(MINIMUM_RESERVE);
    deadline.capped(remaining.saturating_sub(reserve))
}

/// Whether `error` is an iframe read that ran out of its budget while the
/// snapshot itself still has time.
pub(crate) fn missed_budget(error: &AdapterError, deadline: Deadline) -> bool {
    is_timeout(error) && !deadline.is_expired()
}

#[cfg(test)]
#[path = "frame_budget_tests.rs"]
mod tests;
