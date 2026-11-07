//! What one tool-call child receives of the interaction lease the server inherited.

use std::process::Command;

use super::inherited_lease::InheritedLease;

/// `Unchanged` when nothing was inherited, so the child takes the lease itself. `Withheld` when the call
/// does not need the lease: the child gets neither the descriptor nor the variable naming it. `Lent` while
/// this child holds the server's one slot; dropping it frees the slot for the next call.
pub(crate) enum ChildLease<'a> {
    Unchanged,
    Withheld(&'a InheritedLease),
    Lent(&'a InheritedLease),
}

impl ChildLease<'_> {
    pub(crate) fn apply(&self, command: &mut Command) {
        match self {
            Self::Unchanged => {}
            Self::Withheld(lease) => lease.withhold(command),
            Self::Lent(lease) => lease.lend(command),
        }
    }
}

impl Drop for ChildLease<'_> {
    fn drop(&mut self) {
        if let Self::Lent(lease) = self {
            lease.release();
        }
    }
}
