//! Starts server mode only with explicit opt-in and a launch frame on an inherited socket.

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd};
use std::os::unix::net::UnixDatagram;

use super::{OPT_IN_ENV, SERVE_ENV};

const LAUNCH_FRAME: &[u8] = b"agent-desktop daemon startup";

pub(super) fn launch_socket() -> io::Result<UnixDatagram> {
    let (sender, receiver) = UnixDatagram::pair()?;
    sender.send(LAUNCH_FRAME)?;
    Ok(receiver)
}

pub(super) fn take_socket_path() -> Option<OsString> {
    let socket_path = std::env::var_os(SERVE_ENV)?;
    let opt_in = std::env::var_os(OPT_IN_ENV);
    if !is_daemon_launch(opt_in.as_deref(), io::stdin().as_fd()) {
        return None;
    }
    let null = File::open("/dev/null").ok()?;
    if unsafe { libc::dup2(null.as_raw_fd(), libc::STDIN_FILENO) } < 0 {
        return None;
    }
    unsafe { std::env::remove_var(SERVE_ENV) };
    Some(socket_path)
}

fn is_daemon_launch(opt_in: Option<&OsStr>, fd: BorrowedFd<'_>) -> bool {
    opt_in == Some(OsStr::new("1")) && has_launch_frame(fd)
}

fn has_launch_frame(fd: BorrowedFd<'_>) -> bool {
    let mut frame = [0_u8; LAUNCH_FRAME.len() + 1];
    let count = unsafe {
        libc::recv(
            fd.as_raw_fd(),
            frame.as_mut_ptr().cast(),
            frame.len(),
            libc::MSG_DONTWAIT | libc::MSG_PEEK,
        )
    };
    usize::try_from(count)
        .ok()
        .and_then(|count| frame.get(..count))
        == Some(LAUNCH_FRAME)
}

#[cfg(test)]
#[path = "startup_tests.rs"]
mod tests;
