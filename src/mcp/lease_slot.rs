//! The one-holder slot through which tool calls take turns with an interaction lease the server inherited.

use std::collections::HashMap;

/// Whether a call holds the inherited lease, and the calls queued for it, each marked when it was cancelled.
#[derive(Default)]
pub(crate) struct LeaseSlot {
    held: bool,
    queued: HashMap<String, bool>,
}

/// What a queued call does next.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Turn {
    Wait,
    Take,
    Leave,
}

impl LeaseSlot {
    pub(crate) fn queue(&mut self, id: &str) {
        self.queued.insert(id.to_owned(), false);
    }

    /// The one transition for a queued call: it takes the free slot, leaves the queue once cancelled, or waits.
    pub(crate) fn advance(&mut self, id: &str) -> Turn {
        match self.queued.get(id) {
            Some(false) if self.held => Turn::Wait,
            Some(false) => {
                self.queued.remove(id);
                self.held = true;
                Turn::Take
            }
            Some(true) | None => {
                self.queued.remove(id);
                Turn::Leave
            }
        }
    }

    /// Marks queued `id` cancelled; false when it is not queued.
    pub(crate) fn cancel(&mut self, id: &str) -> bool {
        self.queued
            .get_mut(id)
            .map(|cancelled| *cancelled = true)
            .is_some()
    }

    pub(crate) fn release(&mut self) {
        self.held = false;
    }
}
