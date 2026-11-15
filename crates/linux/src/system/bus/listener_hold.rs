//! Keeps this process's accessibility-bus connection, and with it the event
//! listener the registry counts, alive for a short time after the process
//! sent key events.
//!
//! at-spi2-atk answers every key event in its key snooper with a synchronous
//! `DeviceEventController.NotifyListenersSync` call and dispatches the bus
//! while it waits. When the last registered event listener leaves during that
//! wait, the bridge removes its key listener from inside the snooper, and GTK 3
//! applications and Chromium crash on the key. A short-lived CLI that sends
//! keys and exits is that last listener leaving. A forked child that holds a
//! duplicate of the connection's socket keeps the listener's bus name owned
//! until the applications are done with the keys, and costs the command no
//! wait. One holder lives at a time: a new hold starts its child before it
//! kills and reaps the previous one.

use std::os::fd::{AsRawFd, OwnedFd, RawFd};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use super::listener_child::fork_holder;
use super::listener_socket;

const LISTENER_HOLD: Duration = Duration::from_millis(1_000);
const SOCKET_FLOOR: RawFd = 10;

/// The remembered listener socket, the live holder child and when keys were
/// last sent.
#[derive(Default)]
pub(crate) struct ListenerHold {
    socket: Option<OwnedFd>,
    child: Option<libc::pid_t>,
    keys_sent: Option<Instant>,
}

static HOLD: Mutex<ListenerHold> = Mutex::new(ListenerHold {
    socket: None,
    child: None,
    keys_sent: None,
});

fn shared() -> std::sync::MutexGuard<'static, ListenerHold> {
    HOLD.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Starts a holder for the remembered connection after key events were sent.
/// Best effort: without a remembered socket nothing is held, which is the
/// behavior before this guard existed.
pub(crate) fn outlive_sent_keys() {
    shared().after_keys(LISTENER_HOLD);
}

/// Runs `send` and then `hold` when `sends_keys`, whatever `send` returned:
/// a call that failed after some keys left still has applications answering
/// them.
pub(crate) fn hold_after<T>(sends_keys: bool, send: impl FnOnce() -> T, hold: impl FnOnce()) -> T {
    let sent = send();
    if sends_keys {
        hold();
    }
    sent
}

/// A connection builder for the accessibility bus at `address`. A Unix socket
/// address is connected here without blocking the deadline, so the socket can
/// be remembered for a holder; any other address is left to zbus, never held,
/// and forgets an earlier socket.
pub(crate) async fn builder(address: &str) -> zbus::Result<zbus::connection::Builder<'static>> {
    let Some(target) = listener_socket::target_of(address) else {
        shared().forget();
        return zbus::connection::Builder::address(address);
    };
    let stream = listener_socket::connect(&target).await?;
    shared().remember(&stream, LISTENER_HOLD);
    Ok(zbus::connection::Builder::async_io_unix_stream(stream))
}

impl ListenerHold {
    /// Keeps a copy of the listener socket above the standard descriptors. A
    /// listener registered right after keys were sent is held at once: it
    /// would otherwise leave with the process while the keys are answered.
    pub(crate) fn remember(&mut self, stream: &std::os::unix::net::UnixStream, hold: Duration) {
        let copy = unsafe { libc::fcntl(stream.as_raw_fd(), libc::F_DUPFD_CLOEXEC, SOCKET_FLOOR) };
        if copy < 0 {
            return;
        }
        self.socket = Some(unsafe { std::os::fd::FromRawFd::from_raw_fd(copy) });
        if self.keys_sent.is_some_and(|sent| sent.elapsed() < hold) {
            self.after_keys(hold);
        }
    }

    pub(crate) fn forget(&mut self) {
        self.socket = None;
    }

    /// Starts the new holder, then kills and reaps the previous one, so at
    /// most one child of this process holds the socket.
    pub(crate) fn after_keys(&mut self, hold: Duration) -> Option<libc::pid_t> {
        self.keys_sent = Some(Instant::now());
        let socket = self.socket.as_ref()?.as_raw_fd();
        let started = fork_holder(socket, hold).ok()?;
        if let Some(previous) = self.child.replace(started) {
            unsafe {
                libc::kill(previous, libc::SIGKILL);
                libc::waitpid(previous, std::ptr::null_mut(), 0);
            }
        }
        Some(started)
    }
}

#[cfg(test)]
#[path = "listener_hold_tests.rs"]
mod tests;
