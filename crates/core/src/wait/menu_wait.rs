use std::time::{Duration, Instant};

use super::menu_wait_phase::MenuWaitPhase;

/// How long a closed-direction wait with no sign of an open menu keeps
/// reading before a closed read counts. A GTK 4 popover first reads open 157
/// to 999 ms after its opener returns (probes L18-2, L33-11, L33-51), and the
/// grace starts when the wait starts, which is after the opener returned.
pub const MENU_OPENER_GRACE: Duration = Duration::from_millis(1_000);

/// A menu wait: the state it waits for and its [`MenuWaitPhase`]. Core builds
/// it and every adapter feeds it each whole read, so the decision of when a
/// menu wait is satisfied is the same on every platform.
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

    /// Waits for a menu the caller saw open to close: the first closed read
    /// satisfies it.
    pub fn closed_after_open() -> Self {
        Self {
            open: false,
            phase: MenuWaitPhase::Open,
        }
    }

    /// Waits for a menu to close when nothing showed it open: a closed read
    /// that began before `until` does not satisfy it, an open read moves it
    /// to [`MenuWaitPhase::Open`].
    pub fn closed_awaiting_open(until: Instant) -> Self {
        Self {
            open: false,
            phase: MenuWaitPhase::AwaitingOpen { until },
        }
    }

    /// A closed wait that started at `started` with `budget` to spend: the
    /// opener grace is [`MENU_OPENER_GRACE`], cut to half the budget so a
    /// short wait still reads the menu after its grace ends.
    pub fn closed(started: Instant, budget: Duration) -> Self {
        let grace = MENU_OPENER_GRACE.min(budget / 2);
        Self::closed_awaiting_open(started.checked_add(grace).unwrap_or(started))
    }

    pub fn wants_open(&self) -> bool {
        self.open
    }

    pub fn phase(&self) -> MenuWaitPhase {
        self.phase
    }

    /// Applies one whole read that began at `read_started` and says whether
    /// the wait is satisfied. A read that could not tell is not a read: the
    /// adapter skips this call for it.
    pub fn observe(&mut self, open: bool, read_started: Instant) -> bool {
        self.phase = self.phase.observe(open, read_started);
        self.is_satisfied()
    }

    pub fn is_satisfied(&self) -> bool {
        match self.phase {
            MenuWaitPhase::Open => self.open,
            MenuWaitPhase::Closed => !self.open,
            MenuWaitPhase::AwaitingOpen { .. } => false,
        }
    }
}

#[cfg(test)]
#[path = "menu_wait_tests.rs"]
mod tests;
