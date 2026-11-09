//! Bounded readback for a selection write the application applies later.

use std::time::Duration;

use agent_desktop_core::Deadline;

/// How long a selection write may take to show up in its readback.
pub(crate) const SELECTION_SETTLE_MS: u64 = 300;
/// How often the readback is repeated while it waits.
pub(crate) const SELECTION_POLL_MS: u64 = 25;

/// A selection write is applied asynchronously: the application answers the
/// write at once and updates the attribute a moment later. A single read
/// therefore calls a selection the user can already see a failure, whichever
/// path wrote it. This polls `holds` until it is true, the settle window ends,
/// or the operation deadline expires, and it reads at least once.
pub(crate) fn selection_settles(deadline: Deadline, mut holds: impl FnMut() -> bool) -> bool {
    let window = deadline.capped(Duration::from_millis(SELECTION_SETTLE_MS));
    loop {
        if holds() {
            return true;
        }
        if window.is_expired() {
            return false;
        }
        std::thread::sleep(
            window
                .remaining()
                .min(Duration::from_millis(SELECTION_POLL_MS)),
        );
    }
}

/// A member that owns its own `AXSelected` is verified the same way a
/// container write is: by a settled readback, never by one immediate read.
pub(crate) fn direct_selection_holds(
    deadline: Deadline,
    mut read_selected: impl FnMut() -> Option<bool>,
) -> bool {
    selection_settles(deadline, || read_selected() == Some(true))
}

#[cfg(test)]
#[path = "selection_settle_tests.rs"]
mod tests;
