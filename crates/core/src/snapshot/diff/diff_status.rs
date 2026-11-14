/// What a diff can claim about the window between two snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiffStatus {
    /// Both snapshots read their tree whole and no reported row differs.
    Unchanged,
    /// Both snapshots read their tree whole and the rows say what differs.
    Changed,
    /// One or both snapshots were cut short, so a row beyond a cut may be
    /// missing from either side; the rows are what the two reads could pair,
    /// and no change is never claimed.
    Incomplete { baseline: bool, snapshot: bool },
    /// The snapshots observed a different owner or shaped the tree
    /// differently, so the full tree is returned instead of rows.
    NotComparable(&'static str),
}

impl DiffStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Unchanged => "unchanged",
            Self::Changed => "changed",
            Self::Incomplete { .. } => "incomplete",
            Self::NotComparable(_) => "not_comparable",
        }
    }
}
