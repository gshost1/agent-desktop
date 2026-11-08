use crate::system::x11::control::window_facts::WindowFacts;

/// How the read-back of a write ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Settled {
    /// The window holds what was asked, as this read-back shows.
    Held(WindowFacts),
    /// The window did not hold what was asked; the latest read-back, if any.
    Unheld(Option<WindowFacts>),
    /// A read-back matched, but the read meant to confirm it failed, so the
    /// effect is unknown; the read that matched.
    Unreadable(WindowFacts),
    /// A read-back matched, but the command deadline left no time to confirm
    /// that it held.
    Unconfirmed(WindowFacts),
}

impl Settled {
    /// The latest read-back of the window, if any.
    pub(super) fn observed(self) -> Option<WindowFacts> {
        match self {
            Self::Held(facts) | Self::Unreadable(facts) | Self::Unconfirmed(facts) => Some(facts),
            Self::Unheld(observed) => observed,
        }
    }
}
