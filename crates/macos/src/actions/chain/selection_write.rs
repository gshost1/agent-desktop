/// What a right-click fallback's selection write left behind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SelectionWrite {
    /// No candidate took the write, so the application is unchanged.
    NotWritten,
    /// The write was accepted but the selection did not read back as set.
    Unconfirmed,
    /// The write was accepted and the selection reads back as set.
    Selected,
}
