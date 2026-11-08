use crate::system::x11::control::window_facts::WindowFacts;

/// What the read a confirmation pause after a matching read-back shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Confirmation {
    /// The rectangle is unchanged, read back inside the deadline.
    Stable(WindowFacts),
    /// The rectangle changed, read back inside the deadline.
    Changed(WindowFacts),
    /// The confirming read failed.
    Unreadable,
    /// The deadline left no room for the pause, or the confirming read came
    /// back after it; the latest read-back.
    OutOfTime(WindowFacts),
}
