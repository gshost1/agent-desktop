//! An interaction lease this server inherited from its parent, lent to one tool-call child at a time.
//!
//! Every child that adopts the inherited descriptor shares one open file description, and taking a lock
//! through a description that already holds it always succeeds, so the lock cannot tell siblings apart.
//! The slot does: a call that can inject input or steal focus receives the variable naming the lease only
//! while it holds the slot, and the slot passes on only after its child has been reaped. Reads that may
//! take the lease share it beside the holder; they send no input under it, because the stale-input sweep
//! runs only under a lease its process owns, never under an adopted one. Commands that never take the
//! lease get nothing, and an unusable inherited value refuses only exclusive calls. On Unix the
//! descriptor itself is close-on-exec in this server and opened up only in a child the lease is lent to.
//! An exclusive call waits for its turn at most `QUEUE_WAIT_BOUND` and is then refused with TIMEOUT.
//!
//! Known limits. On Windows exclusivity rests on the variable alone: the inherited handle stays
//! inheritable, so every child holds a copy it never adopts, and any integer value is taken for a handle.
//! A call waiting for its turn counts against the server's `MAX_CONCURRENT_CALLS`. Each call parses its
//! command line once more to find its `LeaseRole`.

use agent_desktop_core::{AdapterError, AppError, DeliverySemantics, ErrorCode};
use std::ffi::OsString;
use std::process::Command;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::child_lease::ChildLease;
use super::lease_refusal::LeaseRefusal;
use super::lease_slot::{LeaseSlot, Ticket};
use super::slot_step::SlotStep;

/// How long an exclusive call waits for its turn before it is refused; the CLI's default wait budget.
pub(crate) const QUEUE_WAIT_BOUND: Duration = Duration::from_secs(30);

pub(crate) struct InheritedLease {
    variable: &'static str,
    value: OsString,
    #[cfg(unix)]
    descriptor: Option<std::os::fd::RawFd>,
    slot: Mutex<LeaseSlot>,
    changed: Condvar,
    queue_bound: Duration,
}

impl InheritedLease {
    pub(crate) fn new(variable: &'static str, value: OsString) -> Self {
        Self {
            variable,
            value,
            #[cfg(unix)]
            descriptor: None,
            slot: Mutex::default(),
            changed: Condvar::new(),
            queue_bound: QUEUE_WAIT_BOUND,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_queue_bound(mut self, queue_bound: Duration) -> Self {
        self.queue_bound = queue_bound;
        self
    }

    /// Lends `descriptor`, already close-on-exec in this server, to each child the lease is lent to.
    #[cfg(unix)]
    pub(crate) fn with_descriptor(mut self, descriptor: std::os::fd::RawFd) -> Self {
        self.descriptor = Some(descriptor);
        self
    }

    pub(crate) fn variable(&self) -> &'static str {
        self.variable
    }

    pub(crate) fn register(&self, id: Option<&str>) -> Ticket {
        self.lock().register(id)
    }

    /// Waits for the slot in arrival order and lends the lease to `ticket`. Refused when it was cancelled
    /// first, and with TIMEOUT, out of line, when its turn has not come within the queue bound.
    pub(crate) fn lend_to(&self, ticket: Ticket) -> Result<ChildLease<'_>, LeaseRefusal> {
        let started = Instant::now();
        let mut slot = self.lock();
        loop {
            match slot.advance(ticket) {
                SlotStep::Take => return Ok(ChildLease::Lent(self, ticket)),
                SlotStep::Wait => {}
                _ => return Err(LeaseRefusal::Cancelled),
            }
            let remaining = self.queue_bound.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                slot.forget(ticket);
                drop(slot);
                self.changed.notify_all();
                return Err(LeaseRefusal::Refused(queue_timeout(self.queue_bound)));
            }
            slot = self
                .changed
                .wait_timeout(slot, remaining)
                .map_or_else(|poisoned| poisoned.into_inner().0, |(slot, _)| slot);
        }
    }

    pub(crate) fn forget(&self, ticket: Ticket) {
        self.lock().forget(ticket);
        self.changed.notify_all();
    }

    /// Cancels request `id` through `cancel_call`, which returns the thread reaping its child. When `id`
    /// holds the slot, the slot stays taken until that reaper has joined.
    pub(crate) fn cancel(&self, id: &str, cancel_call: impl FnOnce() -> Option<JoinHandle<()>>) {
        let holder = self.lock().cancel(id);
        let reaper = cancel_call();
        if let Some(ticket) = holder {
            self.lock().hand_reaper(ticket, reaper);
        }
        self.changed.notify_all();
    }

    /// Ends `ticket`'s turn once its child is reaped, by its call thread or by the cancel's reaper.
    pub(crate) fn release(&self, ticket: Ticket) {
        let mut slot = self.lock();
        loop {
            match slot.end(ticket) {
                SlotStep::Wait => slot = self.wait(slot),
                SlotStep::Join(reaper) => {
                    drop(slot);
                    let _ = reaper.join();
                    slot = self.lock();
                }
                _ => break,
            }
        }
        drop(slot);
        self.changed.notify_all();
    }

    pub(crate) fn lend(&self, command: &mut Command) {
        command.env(self.variable, &self.value);
        #[cfg(unix)]
        lend_descriptor(command, self.descriptor);
    }

    fn lock(&self) -> MutexGuard<'_, LeaseSlot> {
        self.slot.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn wait<'a>(&self, slot: MutexGuard<'a, LeaseSlot>) -> MutexGuard<'a, LeaseSlot> {
        self.changed
            .wait(slot)
            .unwrap_or_else(PoisonError::into_inner)
    }
}

fn queue_timeout(bound: Duration) -> AppError {
    AdapterError::new(
        ErrorCode::Timeout,
        format!(
            "The inherited interaction lease is held by an earlier call; this call waited {} ms for its turn and did not run",
            bound.as_millis()
        ),
    )
    .with_suggestion("Retry once the earlier call has finished")
    .with_disposition(DeliverySemantics::not_delivered())
    .into()
}

#[cfg(unix)]
fn lend_descriptor(command: &mut Command, descriptor: Option<std::os::fd::RawFd>) {
    use std::os::unix::process::CommandExt;

    if let Some(descriptor) = descriptor {
        unsafe { command.pre_exec(move || open_to_child(descriptor)) };
    }
}

#[cfg(unix)]
fn open_to_child(descriptor: std::os::fd::RawFd) -> std::io::Result<()> {
    let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFD) };
    if flags < 0
        || unsafe { libc::fcntl(descriptor, libc::F_SETFD, flags & !libc::FD_CLOEXEC) } != 0
    {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
#[path = "inherited_lease_tests.rs"]
mod tests;
