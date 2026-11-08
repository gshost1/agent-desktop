//! A call's place in line for the inherited lease, given up however its call thread ends.

use super::lease_inheritance::LeaseInheritance;
use super::lease_slot::Ticket;

/// Holds `ticket` for the life of one call thread and forgets it on drop, unwinding included, so a call
/// that ends before its turn never blocks the calls behind it.
pub(crate) struct QueuedTicket<'a> {
    lease: &'a LeaseInheritance,
    ticket: Option<Ticket>,
}

impl<'a> QueuedTicket<'a> {
    pub(crate) fn new(lease: &'a LeaseInheritance, ticket: Option<Ticket>) -> Self {
        Self { lease, ticket }
    }

    pub(crate) fn ticket(&self) -> Option<Ticket> {
        self.ticket
    }
}

impl Drop for QueuedTicket<'_> {
    fn drop(&mut self) {
        self.lease.forget(self.ticket);
    }
}
