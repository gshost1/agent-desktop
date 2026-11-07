//! The one-holder slot through which tool calls take turns, in arrival order, with an inherited lease.

use std::collections::{HashMap, HashSet, VecDeque};
use std::thread::JoinHandle;

use super::slot_holder::SlotHolder;
use super::slot_step::SlotStep;

/// One admitted call's place in line; unique for the server's lifetime, unlike a JSON-RPC id.
pub(crate) type Ticket = u64;

/// The holder, the calls in line in arrival order, the ones cancelled before their turn, and the ticket
/// each in-flight request id holds, so a cancel finds the call even before its thread has run.
#[derive(Default)]
pub(crate) struct LeaseSlot {
    issued: Ticket,
    line: VecDeque<Ticket>,
    cancelled: HashSet<Ticket>,
    tickets: HashMap<String, Ticket>,
    holder: SlotHolder,
}

impl LeaseSlot {
    /// Puts request `id` in line when it is admitted, before its call thread starts.
    pub(crate) fn register(&mut self, id: &str) -> Ticket {
        self.issued += 1;
        self.line.push_back(self.issued);
        self.tickets.insert(id.to_owned(), self.issued);
        self.issued
    }

    /// The transition for a call in line: it leaves once cancelled, takes the slot when it is free and the
    /// call is first in line, and otherwise waits.
    pub(crate) fn advance(&mut self, ticket: Ticket) -> SlotStep {
        if self.cancelled.remove(&ticket) || !self.line.contains(&ticket) {
            self.line.retain(|queued| *queued != ticket);
            return SlotStep::Leave;
        }
        if matches!(self.holder, SlotHolder::Free) && self.line.front() == Some(&ticket) {
            self.line.pop_front();
            self.holder = SlotHolder::Lent(ticket);
            return SlotStep::Take;
        }
        SlotStep::Wait
    }

    /// Takes `ticket` out of line for good: its call does not need the lease, or has ended.
    pub(crate) fn forget(&mut self, ticket: Ticket) {
        self.line.retain(|queued| *queued != ticket);
        self.cancelled.remove(&ticket);
        self.tickets.retain(|_, held| *held != ticket);
    }

    /// Cancels request `id`: a call still in line will leave, and a holder waits for its child to be reaped.
    /// Returns the holder's ticket when `id` holds the slot.
    pub(crate) fn cancel(&mut self, id: &str) -> Option<Ticket> {
        let ticket = self.tickets.remove(id)?;
        if self.line.contains(&ticket) {
            self.cancelled.insert(ticket);
            return None;
        }
        if matches!(self.holder, SlotHolder::Lent(held) if held == ticket) {
            self.holder = SlotHolder::Cancelling(ticket);
            return Some(ticket);
        }
        None
    }

    /// Hands cancelled holder `ticket` the thread reaping its child, `None` when it never started one.
    pub(crate) fn hand_reaper(&mut self, ticket: Ticket, reaper: Option<JoinHandle<()>>) {
        if matches!(self.holder, SlotHolder::Cancelling(held) if held == ticket) {
            self.holder = SlotHolder::Reaping(ticket, reaper);
        }
    }

    /// The transition for holder `ticket` ending its turn; the slot is freed only once no child is running.
    pub(crate) fn end(&mut self, ticket: Ticket) -> SlotStep {
        match &mut self.holder {
            SlotHolder::Cancelling(held) if *held == ticket => SlotStep::Wait,
            SlotHolder::Reaping(held, reaper) if *held == ticket => match reaper.take() {
                Some(reaper) => SlotStep::Join(reaper),
                None => self.free(),
            },
            SlotHolder::Lent(held) if *held == ticket => self.free(),
            _ => SlotStep::Freed,
        }
    }

    fn free(&mut self) -> SlotStep {
        self.holder = SlotHolder::Free;
        SlotStep::Freed
    }
}
