use std::time::{Duration, Instant};

use super::menu_wait_phase::MenuWaitPhase;

/// How long a closed-direction wait with no sign of an open menu keeps
/// reading before a closed read counts. A GTK 4 popover first reads open 157
/// to 999 ms after its opener returns (probes L18-2, L33-11, L33-51), and the
/// grace starts when the wait starts, which is after the opener returned.
pub const MENU_OPENER_GRACE: Duration = Duration::from_millis(1_500);

/// A menu wait: the state it waits for and its phase. Core builds it and
/// every adapter feeds it each whole read, so when a menu wait is satisfied
/// is decided the same way on every platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuWait {
    open: bool,
    phase: MenuWaitPhase,
}

impl MenuWait {
    /// Waits for a menu to show: only an open read satisfies it.
    pub fn opened() -> Self {
        Self {
            open: true,
            phase: MenuWaitPhase::Closed,
        }
    }

    /// Waits for a menu the caller saw open to close: two consecutive closed
    /// reads satisfy it.
    pub fn closed_after_open() -> Self {
        Self {
            open: false,
            phase: MenuWaitPhase::Open,
        }
    }

    /// Waits for a menu to close when nothing showed it open, in a wait that
    /// started at `started`: a closed read that began inside the
    /// [`MENU_OPENER_GRACE`] does not satisfy it, an open read means the menu
    /// must then close.
    pub fn closed(started: Instant) -> Self {
        Self::closed_awaiting_open(started.checked_add(MENU_OPENER_GRACE).unwrap_or(started))
    }

    pub(crate) fn closed_awaiting_open(until: Instant) -> Self {
        Self {
            open: false,
            phase: MenuWaitPhase::AwaitingOpen {
                until,
                read_closed: false,
            },
        }
    }

    pub fn wants_open(&self) -> bool {
        self.open
    }

    #[cfg(test)]
    pub(crate) fn phase(&self) -> MenuWaitPhase {
        self.phase
    }

    /// Applies one whole read that began at `read_started` and says whether
    /// the wait is satisfied. A read that could not tell is not a read: the
    /// adapter skips this call for it.
    pub fn observe(&mut self, open: bool, read_started: Instant) -> bool {
        self.phase = self.phase.observe(open, read_started);
        self.is_satisfied()
    }

    /// Called when the deadline arrives before the wait was satisfied. A
    /// closed wait still inside its grace whose completed reads were all
    /// closed is satisfied: the menu stayed closed for the whole wait.
    pub fn satisfied_at_deadline(&mut self) -> bool {
        self.phase = self.phase.expire();
        self.is_satisfied()
    }

    pub fn is_satisfied(&self) -> bool {
        match self.phase {
            MenuWaitPhase::Open => self.open,
            MenuWaitPhase::Closed => !self.open,
            MenuWaitPhase::AwaitingOpen { .. } | MenuWaitPhase::Closing => false,
        }
    }
}

#[cfg(test)]
#[path = "menu_wait_tests.rs"]
mod tests;
