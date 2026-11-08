use std::thread::ThreadId;

/// Who holds this process's interaction lease: nobody, or exactly one thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProcessLeaseState {
    Free,
    Held { owner: ThreadId },
}

impl ProcessLeaseState {
    /// Takes the lease for `caller` when it is free. A held lease is left unchanged and its owner returned,
    /// so the caller can tell a hold of its own from another thread's.
    pub(crate) fn acquire(&mut self, caller: ThreadId) -> Result<(), ThreadId> {
        match *self {
            Self::Free => {
                *self = Self::Held { owner: caller };
                Ok(())
            }
            Self::Held { owner } => Err(owner),
        }
    }

    /// Frees the lease whichever thread releases it, since a guard may be dropped on a thread other than
    /// the one that took it.
    pub(crate) fn release(&mut self) {
        *self = Self::Free;
    }
}

#[cfg(test)]
#[path = "process_lease_state_tests.rs"]
mod tests;
