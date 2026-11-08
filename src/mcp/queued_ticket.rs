//! A call's place in line for the inherited lease, given up however its call thread ends.

use std::ops::Deref;

use super::lease_inheritance::LeaseInheritance;
use super::lease_slot::Ticket;

/// Holds `ticket` for the life of one call thread and forgets it on drop, unwinding included, so a call
/// that ends before its turn never blocks the calls behind it.
pub(crate) struct QueuedTicket<L: Deref<Target = LeaseInheritance>> {
    lease: L,
    ticket: Option<Ticket>,
}

impl<L: Deref<Target = LeaseInheritance>> QueuedTicket<L> {
    pub(crate) fn new(lease: L, ticket: Option<Ticket>) -> Self {
        Self { lease, ticket }
    }

    pub(crate) fn ticket(&self) -> Option<Ticket> {
        self.ticket
    }
}

impl<L: Deref<Target = LeaseInheritance> + Send + 'static> QueuedTicket<L> {
    /// Runs `call` with this place in line on the thread `spawn` starts. The place moves into that thread's
    /// body before it starts, so it is given up however the thread ends, and also when it never starts.
    pub(crate) fn run_on<T>(
        self,
        spawn: impl FnOnce(Box<dyn FnOnce() + Send>) -> std::io::Result<T>,
        call: impl FnOnce(Option<Ticket>) + Send + 'static,
    ) -> std::io::Result<T> {
        spawn(Box::new(move || {
            call(self.ticket());
            drop(self);
        }))
    }
}

impl<L: Deref<Target = LeaseInheritance>> Drop for QueuedTicket<L> {
    fn drop(&mut self) {
        self.lease.forget(self.ticket);
    }
}

#[cfg(test)]
#[path = "queued_ticket_tests.rs"]
mod tests;
