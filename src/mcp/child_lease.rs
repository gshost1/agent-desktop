//! What one tool-call child receives of an interaction lease the server inherited.

use std::process::Command;

use super::inherited_lease::InheritedLease;
use super::lease_slot::Ticket;

/// `Unchanged` when nothing was inherited, so the child takes the lease itself. `Withheld` names the lease
/// variable the child must not see. `Shared` lends the lease to a read beside the holder. `Lent` while this
/// call holds the server's one exclusive slot; dropping it ends the turn once the child is reaped.
pub(crate) enum ChildLease<'a> {
    Unchanged,
    Withheld(&'static str),
    Shared(&'a InheritedLease),
    Lent(&'a InheritedLease, Ticket),
}

impl ChildLease<'_> {
    pub(crate) fn apply(&self, command: &mut Command) {
        match self {
            Self::Unchanged => {}
            Self::Withheld(variable) => {
                command.env_remove(variable);
            }
            Self::Shared(lease) | Self::Lent(lease, _) => lease.lend(command),
        }
    }
}

impl Drop for ChildLease<'_> {
    fn drop(&mut self) {
        if let Self::Lent(lease, ticket) = self {
            lease.release(*ticket);
        }
    }
}
