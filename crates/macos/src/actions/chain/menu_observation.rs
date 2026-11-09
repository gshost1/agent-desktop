/// One read of whether an application shows a menu. `Unreadable` means the
/// application did not answer the read, which is the only evidence this
/// adapter has that it stopped responding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MenuObservation {
    Open,
    Closed,
    Unreadable,
}
