//! Bounds the text replacements still waiting for an edit control to read
//! their text.
//!
//! A replacement the edit did not answer in time stays queued for the edit's
//! thread, which reads the text buffer when it gets to the message. The
//! worker thread that sent it owns that buffer and holds a permit from this
//! pool until its send returns, so the buffers and threads kept alive for
//! unresponsive edits never outnumber the pool's capacity.

use std::sync::atomic::{AtomicUsize, Ordering};

/// The most replacements one process keeps waiting on unresponsive edits.
pub(crate) const MAX_PENDING_REPLACEMENTS: usize = 8;

/// A fixed number of slots for text replacements in flight.
#[derive(Debug)]
pub(crate) struct PendingReplacements {
    capacity: usize,
    in_flight: AtomicUsize,
}

impl PendingReplacements {
    pub(crate) const fn new(capacity: usize) -> Self {
        Self {
            capacity,
            in_flight: AtomicUsize::new(0),
        }
    }

    /// Takes a slot, or `None` when every slot is held.
    pub(crate) fn try_acquire(&'static self) -> Option<PendingPermit> {
        self.in_flight
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |held| {
                (held < self.capacity).then_some(held + 1)
            })
            .ok()?;
        Some(PendingPermit { pool: self })
    }

    #[cfg(test)]
    pub(crate) fn in_flight(&self) -> usize {
        self.in_flight.load(Ordering::Acquire)
    }

    pub(crate) fn capacity(&self) -> usize {
        self.capacity
    }
}

/// A held slot, given back when dropped.
#[derive(Debug)]
pub(crate) struct PendingPermit {
    pool: &'static PendingReplacements,
}

impl Drop for PendingPermit {
    fn drop(&mut self) {
        self.pool.in_flight.fetch_sub(1, Ordering::AcqRel);
    }
}

#[cfg(test)]
#[path = "edit_pending_tests.rs"]
mod tests;
