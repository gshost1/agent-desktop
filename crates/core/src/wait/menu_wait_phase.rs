use std::time::Instant;

/// Where a menu wait stands in the menu's lifecycle. Adapters only report
/// whole reads of "is a menu open"; this phase decides what a read means.
///
/// A closed-direction wait that starts right after the action that opens a
/// menu can read "closed" before the menu has appeared, so "not yet open"
/// and "closed" are different phases here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuWaitPhase {
    /// No read has shown the menu open yet. A closed read that began before
    /// `until` may be a menu that has not appeared yet, so it keeps this
    /// phase; `read_closed` records that at least one such read completed.
    AwaitingOpen { until: Instant, read_closed: bool },
    /// A read showed the menu open, or the caller saw it open before the action.
    Open,
    /// One closed read after `Open`. A second consecutive closed read is
    /// needed, so a menu swapped for another across one poll is not a close.
    Closing,
    /// The menu closed after it was open, or stayed closed through the grace.
    Closed,
}

impl MenuWaitPhase {
    /// The one transition per whole read. `open` is the read and
    /// `read_started` the instant it began, so a slow read that began inside
    /// the grace never counts as a closed read after it.
    pub(crate) fn observe(self, open: bool, read_started: Instant) -> Self {
        match (self, open) {
            (_, true) => Self::Open,
            (Self::AwaitingOpen { until, .. }, false) if read_started < until => {
                Self::AwaitingOpen {
                    until,
                    read_closed: true,
                }
            }
            (Self::Open, false) => Self::Closing,
            (_, false) => Self::Closed,
        }
    }

    /// The transition when the deadline arrives: a wait still awaiting the
    /// menu whose every completed read was closed ran out its grace with the
    /// deadline, so the menu stayed closed for the whole wait.
    pub(crate) fn expire(self) -> Self {
        match self {
            Self::AwaitingOpen {
                read_closed: true, ..
            } => Self::Closed,
            other => other,
        }
    }
}

#[cfg(test)]
#[path = "menu_wait_phase_tests.rs"]
mod tests;
