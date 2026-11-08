use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use super::process_lease_state::ProcessLeaseState;
use crate::{AdapterError, Deadline, DeliverySemantics, ErrorCode};

static PROCESS_LEASE: Mutex<ProcessLeaseState> = Mutex::new(ProcessLeaseState::Free);

/// This process's share of the interaction lease. Another thread waits for it until its deadline; the
/// thread that already holds it is refused at once, because waiting on its own hold could only time out.
/// A guard moved to another thread still counts as held by the thread that took it until it is dropped.
pub struct ProcessLeaseGuard {
    contention_count: u64,
}

impl ProcessLeaseGuard {
    pub fn acquire(deadline: Deadline) -> Result<Self, AdapterError> {
        let caller = std::thread::current().id();
        let mut contention_count = 0_u64;
        loop {
            let attempt = lease_state().acquire(caller);
            match attempt {
                Ok(()) if deadline.is_expired() => {
                    lease_state().release();
                    return Err(timeout(deadline, contention_count));
                }
                Ok(()) => return Ok(Self { contention_count }),
                Err(owner) if owner == caller => return Err(reentrant()),
                Err(_) => {}
            }
            contention_count = contention_count.saturating_add(1);
            let remaining = deadline.remaining();
            if remaining.is_zero() {
                return Err(timeout(deadline, contention_count));
            }
            std::thread::sleep(remaining.min(Duration::from_millis(1)));
        }
    }

    pub fn contention_count(&self) -> u64 {
        self.contention_count
    }
}

impl Drop for ProcessLeaseGuard {
    fn drop(&mut self) {
        lease_state().release();
    }
}

fn lease_state() -> MutexGuard<'static, ProcessLeaseState> {
    PROCESS_LEASE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn reentrant() -> AdapterError {
    AdapterError::new(
        ErrorCode::PolicyDenied,
        "This thread already holds the interaction lease",
    )
    .with_suggestion(
        "Start the next interaction after the agent-desktop call running on this thread returns; \
         a log callback must not start one",
    )
    .with_details(serde_json::json!({
        "kind": "reentrant_interaction_lease",
        "retryable": false,
    }))
    .with_disposition(DeliverySemantics::not_delivered())
}

fn timeout(deadline: Deadline, contention_count: u64) -> AdapterError {
    deadline.timeout_error().with_details(serde_json::json!({
        "kind": "interaction_process_lock_timeout",
        "contention_count": contention_count,
    }))
}

#[cfg(test)]
static PROCESS_LEASE_TEST_LOCK: Mutex<()> = Mutex::new(());

/// Serializes the tests that take the process-wide lease, which would otherwise contend across test threads.
#[cfg(test)]
pub(crate) fn test_serialization() -> MutexGuard<'static, ()> {
    PROCESS_LEASE_TEST_LOCK
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
#[path = "process_lease_guard_tests.rs"]
mod tests;
