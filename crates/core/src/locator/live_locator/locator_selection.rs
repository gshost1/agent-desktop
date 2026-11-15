#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocatorSelection {
    Strict,
    All { limit: Option<u32> },
    Count,
    First,
    /// The first match whose whole name equals a substring name query, and
    /// the first match when none does. Where the platform reads list and table
    /// rows last, a match outside the rows still comes before any inside them.
    FirstPreferringExactName,
    Last,
    Nth(u32),
}

impl LocatorSelection {
    /// Whether the selection returns one match by its place in traversal,
    /// which lets a read leave list and table rows for later.
    pub(crate) fn is_first(self) -> bool {
        matches!(self, Self::First | Self::FirstPreferringExactName)
    }
}
