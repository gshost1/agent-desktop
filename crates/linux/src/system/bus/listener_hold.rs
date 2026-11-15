//! Keeps this process's accessibility-bus connection, and with it the event
//! listener the registry counts, alive for a short time after the process
//! sent key events.
//!
//! at-spi2-atk answers every key event in its key snooper with a synchronous
//! `DeviceEventController.NotifyListenersSync` call and dispatches the bus
//! while it waits. When the last registered event listener leaves during that
//! wait, the bridge removes its key listener from inside the snooper, and GTK 3
//! applications and Chromium crash on the key. A short-lived CLI that sends
//! keys and exits is that last listener leaving. A child that holds a duplicate
//! of the connection's socket keeps the listener's bus name owned until the
//! applications are done with the keys, and costs the command no wait.

use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

/// How long the listener outlives the last key event sent.
pub(crate) const LISTENER_HOLD: Duration = Duration::from_millis(1_000);
const HELD_FD: i32 = 3;
const SLEEP_PROGRAMS: [&str; 2] = ["/usr/bin/sleep", "/bin/sleep"];

static LISTENER_SOCKET: Mutex<Option<OwnedFd>> = Mutex::new(None);

/// Records the socket of the connection that registered the event listener.
pub(crate) fn remember(stream: &UnixStream) {
    let Ok(copy) = stream.try_clone() else {
        return;
    };
    *LISTENER_SOCKET
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(OwnedFd::from(copy));
}

/// Starts a holder for the remembered connection after key events were sent.
/// Best effort: without a remembered socket or a `sleep` program nothing is
/// held, which is the behavior before this guard existed.
pub(crate) fn outlive_sent_keys() {
    let guard = LISTENER_SOCKET
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let Some(socket) = guard.as_ref() else {
        return;
    };
    if let Ok(child) = spawn_holder(socket.as_fd(), LISTENER_HOLD) {
        reap(child);
    }
}

/// A connection builder for the accessibility bus at `address`. A Unix socket
/// address is connected here, so the socket can be remembered for a holder;
/// any other address is left to zbus and is never held.
pub(crate) fn builder(address: &str) -> zbus::Result<zbus::connection::Builder<'static>> {
    match listener_stream(address) {
        Some(stream) => {
            remember(&stream);
            Ok(zbus::connection::Builder::async_io_unix_stream(stream))
        }
        None => zbus::connection::Builder::address(address),
    }
}

fn listener_stream(address: &str) -> Option<UnixStream> {
    use zbus::address::transport::{Transport, UnixSocket};
    let parsed = zbus::Address::try_from(address).ok()?;
    let Transport::Unix(unix) = parsed.transport() else {
        return None;
    };
    match unix.path() {
        UnixSocket::File(path) => UnixStream::connect(path).ok(),
        UnixSocket::Abstract(name) => {
            use std::os::linux::net::SocketAddrExt;
            use std::os::unix::ffi::OsStrExt;
            let socket =
                std::os::unix::net::SocketAddr::from_abstract_name(name.as_bytes()).ok()?;
            UnixStream::connect_addr(&socket).ok()
        }
        _ => None,
    }
}

/// Spawns a `sleep` that holds `fd` as its descriptor 3 for `hold` and no other
/// descriptor of this process beyond the null standard streams, so it keeps no
/// lock, pipe or lease of the caller open.
pub(crate) fn spawn_holder(fd: BorrowedFd<'_>, hold: Duration) -> std::io::Result<Child> {
    let program = SLEEP_PROGRAMS
        .iter()
        .find(|path| std::path::Path::new(path).exists())
        .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))?;
    let raw = fd.as_raw_fd();
    let mut command = Command::new(program);
    command
        .arg(format!("{:.3}", hold.as_secs_f64()))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    unsafe {
        command.pre_exec(move || keep_only(raw));
    }
    command.spawn()
}

fn keep_only(raw: i32) -> std::io::Result<()> {
    if raw != HELD_FD && unsafe { libc::dup2(raw, HELD_FD) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    if unsafe { libc::fcntl(HELD_FD, libc::F_SETFD, 0) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    if unsafe { libc::syscall(libc::SYS_close_range, HELD_FD + 1, u32::MAX, 0) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

fn reap(mut child: Child) {
    let _ = std::thread::Builder::new()
        .name("listener-hold".to_owned())
        .spawn(move || {
            let _ = child.wait();
        });
}

#[cfg(test)]
#[path = "listener_hold_tests.rs"]
mod tests;
