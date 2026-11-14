//! Where one accepted connection's command stands, shared by the executor that runs it and the
//! thread that notices when its client goes away.

use std::sync::atomic::{AtomicU8, Ordering};

const QUEUED: u8 = 0;
const RUNNING: u8 = 1;
const FINISHED: u8 = 2;
const ABANDONED: u8 = 3;

/// Queued, running, finished, or abandoned by a client that left before it started.
#[derive(Debug, Default)]
pub(crate) struct JobState(AtomicU8);

impl JobState {
    /// Moves a queued command to running; `false` when its client already left.
    pub(crate) fn start(&self) -> bool {
        self.0
            .compare_exchange(QUEUED, RUNNING, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub(crate) fn finish(&self) {
        self.0.store(FINISHED, Ordering::Release);
    }

    /// Records that the client is gone. Returns `true` when its command is running, which the
    /// daemon answers by stopping, as a one-shot CLI would stop with its caller.
    pub(crate) fn client_left(&self) -> bool {
        match self
            .0
            .compare_exchange(QUEUED, ABANDONED, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => false,
            Err(current) => current == RUNNING,
        }
    }
}

#[cfg(test)]
#[path = "job_state_tests.rs"]
mod tests;
