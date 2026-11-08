use std::time::Instant;

/// Where a menu wait stands in the menu's lifecycle. Adapters only report
/// whole reads of "is a menu open"; this phase decides what a read means.
///
/// A closed-direction wait that starts right after the action that opens a
/// menu can read "closed" before the menu has appeared, so "not yet open"
/// and "closed" are different phases here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuWaitPhase {
    /// No read has shown the menu open yet. A closed read that began before
    /// `until` may be a menu that has not appeared yet, so it keeps this phase.
    AwaitingOpen { until: Instant },
    /// A read showed the menu open, or the caller saw it open before the action.
    Open,
    /// A read showed no menu after it was open, or after the opener grace ended.
    Closed,
}

impl MenuWaitPhase {
    /// The one transition. `open` is a whole read of the menu state and
    /// `read_started` the instant that read began, so a slow read that began
    /// inside the grace never counts as a closed read after it.
    pub fn observe(self, open: bool, read_started: Instant) -> Self {
        match (self, open) {
            (_, true) => Self::Open,
            (Self::AwaitingOpen { until }, false) if read_started < until => self,
            (_, false) => Self::Closed,
        }
    }
}

#[cfg(test)]
#[path = "menu_wait_phase_tests.rs"]
mod tests;
