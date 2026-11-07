//! An interaction lease this server inherited from its parent, lent to one tool-call child at a time.
//!
//! Every child that adopts the inherited descriptor shares one open file description, and taking a lock
//! through a description that already holds it always succeeds, so the lock cannot tell siblings apart.
//! The slot does: a child receives the variable naming the lease only while its call holds the slot, and
//! the slot passes on only after that child has been reaped. On Unix the descriptor itself is close-on-exec
//! in this server and opened up only in the lent child. On Windows exclusivity rests on the variable alone:
//! the inherited handle stays inheritable, so every child holds a copy it never adopts.

use std::ffi::OsString;
use std::process::Command;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;

use super::child_lease::ChildLease;
use super::lease_slot::{LeaseSlot, Ticket};
use super::slot_step::SlotStep;

pub(crate) struct InheritedLease {
    variable: &'static str,
    value: OsString,
    #[cfg(unix)]
    descriptor: Option<std::os::fd::RawFd>,
    slot: Mutex<LeaseSlot>,
    changed: Condvar,
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
        }
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

    pub(crate) fn register(&self, id: &str) -> Ticket {
        self.lock().register(id)
    }

    /// Waits for the slot in arrival order and lends the lease to `ticket`; `None` when it was cancelled.
    pub(crate) fn lend_to(&self, ticket: Ticket) -> Option<ChildLease<'_>> {
        let mut slot = self.lock();
        loop {
            match slot.advance(ticket) {
                SlotStep::Take => return Some(ChildLease::Lent(self, ticket)),
                SlotStep::Wait => slot = self.wait(slot),
                _ => return None,
            }
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
