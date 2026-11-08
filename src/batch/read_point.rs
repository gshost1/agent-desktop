//! When an entry takes the read a later seeded wait diffs against.

/// The read an entry owes the seeded wait at `wait_index`: before the entry
/// runs, or after it finishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReadPoint {
    Before { wait_index: usize },
    After { wait_index: usize },
}

impl ReadPoint {
    pub(super) fn wait_index(self) -> usize {
        match self {
            Self::Before { wait_index } | Self::After { wait_index } => wait_index,
        }
    }
}
