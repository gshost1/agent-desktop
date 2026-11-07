//! An interaction lease this server inherited from its parent, lent to one tool-call child at a time.
//!
//! Every child that adopts the inherited descriptor shares one open file description, and taking a lock
//! through a description that already holds it always succeeds, so the lock cannot tell siblings apart.
//! The slot does: a child receives the descriptor and its variable only while it holds the slot.

use std::ffi::OsString;
use std::process::Command;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};

use super::child_lease::ChildLease;
use super::lease_slot::{LeaseSlot, Turn};

pub(crate) struct InheritedLease {
    variable: &'static str,
    value: OsString,
    #[cfg(unix)]
    descriptor: Option<std::os::fd::RawFd>,
    slot: Mutex<LeaseSlot>,
    changed: Condvar,
}

impl InheritedLease {
    /// The lease named by `variable` with `value`. On Unix the descriptor it names is marked close-on-exec
    /// here, so only a child the lease is lent to inherits it.
    pub(crate) fn new(variable: &'static str, value: OsString) -> Self {
        Self {
            variable,
            #[cfg(unix)]
            descriptor: keep_from_children(&value),
            value,
            slot: Mutex::default(),
            changed: Condvar::new(),
        }
    }

    /// Waits for the slot and lends the lease to call `id`; `None` when `id` was cancelled while it waited.
    pub(crate) fn lend_to(&self, id: &str) -> Option<ChildLease<'_>> {
        let mut slot = self.lock();
        slot.queue(id);
        loop {
            match slot.advance(id) {
                Turn::Take => return Some(ChildLease::Lent(self)),
                Turn::Leave => return None,
                Turn::Wait => {
                    slot = self
                        .changed
                        .wait(slot)
                        .unwrap_or_else(PoisonError::into_inner);
                }
            }
        }
    }

    /// Stops call `id` from waiting for the slot; a call not waiting is left alone.
    pub(crate) fn cancel(&self, id: &str) {
        if self.lock().cancel(id) {
            self.changed.notify_all();
        }
    }

    pub(crate) fn release(&self) {
        self.lock().release();
        self.changed.notify_all();
    }

    pub(crate) fn withhold(&self, command: &mut Command) {
        command.env_remove(self.variable);
    }

    pub(crate) fn lend(&self, command: &mut Command) {
        command.env(self.variable, &self.value);
        #[cfg(unix)]
        lend_descriptor(command, self.descriptor);
    }

    fn lock(&self) -> MutexGuard<'_, LeaseSlot> {
        self.slot.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(unix)]
fn keep_from_children(value: &OsString) -> Option<std::os::fd::RawFd> {
    value
        .to_str()?
        .parse::<std::os::fd::RawFd>()
        .ok()
        .filter(|descriptor| *descriptor >= 0)
        .filter(|descriptor| set_close_on_exec(*descriptor, true).is_ok())
}

#[cfg(unix)]
fn lend_descriptor(command: &mut Command, descriptor: Option<std::os::fd::RawFd>) {
    use std::os::unix::process::CommandExt;

    if let Some(descriptor) = descriptor {
        unsafe { command.pre_exec(move || set_close_on_exec(descriptor, false)) };
    }
}

#[cfg(unix)]
fn set_close_on_exec(descriptor: std::os::fd::RawFd, close: bool) -> std::io::Result<()> {
    let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFD) };
    if flags < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let flags = if close {
        flags | libc::FD_CLOEXEC
    } else {
        flags & !libc::FD_CLOEXEC
    };
    if unsafe { libc::fcntl(descriptor, libc::F_SETFD, flags) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}
