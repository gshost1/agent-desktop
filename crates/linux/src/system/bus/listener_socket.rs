//! The accessibility bus's Unix socket, connected here instead of by zbus so
//! that a copy of it can be held after keys are sent. The connect never
//! blocks: a full backlog is retried on a timer inside the caller's deadline.

use std::os::fd::{FromRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

const BACKLOG_RETRY: Duration = Duration::from_millis(5);

/// Where a `unix:` bus address points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SocketTarget {
    Path(PathBuf),
    Abstract(Vec<u8>),
}

/// The socket a bus address names, or `None` for any transport this module
/// does not connect itself.
pub(crate) fn target_of(address: &str) -> Option<SocketTarget> {
    use zbus::address::transport::{Transport, UnixSocket};
    let parsed = zbus::Address::try_from(address).ok()?;
    let Transport::Unix(unix) = parsed.transport() else {
        return None;
    };
    match unix.path() {
        UnixSocket::File(path) => Some(SocketTarget::Path(path.clone())),
        UnixSocket::Abstract(name) => {
            use std::os::unix::ffi::OsStrExt;
            Some(SocketTarget::Abstract(name.as_bytes().to_vec()))
        }
        _ => None,
    }
}

pub(crate) async fn connect(target: &SocketTarget) -> std::io::Result<UnixStream> {
    let (address, length) = socket_address(target)?;
    loop {
        let fd = unsafe {
            libc::socket(
                libc::AF_UNIX,
                libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
                0,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let socket = unsafe { OwnedFd::from_raw_fd(fd) };
        let connected = unsafe {
            libc::connect(
                fd,
                std::ptr::from_ref(&address).cast::<libc::sockaddr>(),
                length,
            )
        };
        if connected == 0 {
            return Ok(UnixStream::from(socket));
        }
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::EAGAIN) {
            return Err(error);
        }
        drop(socket);
        async_io::Timer::after(BACKLOG_RETRY).await;
    }
}

fn socket_address(target: &SocketTarget) -> std::io::Result<(libc::sockaddr_un, libc::socklen_t)> {
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    let (bytes, start) = match target {
        SocketTarget::Path(path) => {
            use std::os::unix::ffi::OsStrExt;
            (path.as_os_str().as_bytes(), 0)
        }
        SocketTarget::Abstract(name) => (name.as_slice(), 1),
    };
    let end = start + bytes.len();
    let room = match target {
        SocketTarget::Path(_) => address.sun_path.len() - 1,
        SocketTarget::Abstract(_) => address.sun_path.len(),
    };
    if end > room {
        return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
    }
    for (slot, byte) in address.sun_path[start..end].iter_mut().zip(bytes) {
        *slot = *byte as libc::c_char;
    }
    let used = match target {
        SocketTarget::Path(_) => end + 1,
        SocketTarget::Abstract(_) => end,
    };
    let length = std::mem::offset_of!(libc::sockaddr_un, sun_path) + used;
    let length = libc::socklen_t::try_from(length)
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    Ok((address, length))
}

#[cfg(test)]
#[path = "listener_socket_tests.rs"]
mod tests;
