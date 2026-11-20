//! Who is on the other end of a daemon connection. The daemon serves only peers running as its
//! own effective user; on Linux the peer must also run the same executable file.

use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;

/// Whether `stream`'s peer may use this daemon.
pub(crate) fn is_trusted_peer(stream: &UnixStream) -> bool {
    let own_uid = unsafe { libc::geteuid() };
    match peer_identity(stream) {
        Some((uid, pid)) => uid == own_uid && runs_own_executable(pid),
        None => false,
    }
}

#[cfg(target_os = "linux")]
fn peer_identity(stream: &UnixStream) -> Option<(libc::uid_t, Option<libc::pid_t>)> {
    let mut credentials = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut len = libc::socklen_t::try_from(std::mem::size_of::<libc::ucred>()).ok()?;
    let status = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            std::ptr::from_mut(&mut credentials).cast(),
            &raw mut len,
        )
    };
    (status == 0).then_some((credentials.uid, Some(credentials.pid)))
}

#[cfg(not(target_os = "linux"))]
fn peer_identity(stream: &UnixStream) -> Option<(libc::uid_t, Option<libc::pid_t>)> {
    let mut uid: libc::uid_t = 0;
    let mut gid: libc::gid_t = 0;
    let status = unsafe { libc::getpeereid(stream.as_raw_fd(), &raw mut uid, &raw mut gid) };
    (status == 0).then_some((uid, None))
}

#[cfg(target_os = "linux")]
fn runs_own_executable(pid: Option<libc::pid_t>) -> bool {
    use std::os::unix::fs::MetadataExt;
    let Some(pid) = pid.filter(|pid| *pid > 0) else {
        return false;
    };
    let file_id = |path: &str| std::fs::metadata(path).ok().map(|m| (m.dev(), m.ino()));
    let own = file_id("/proc/self/exe");
    own.is_some() && own == file_id(&format!("/proc/{pid}/exe"))
}

#[cfg(not(target_os = "linux"))]
fn runs_own_executable(_pid: Option<libc::pid_t>) -> bool {
    true
}

#[cfg(test)]
#[path = "peer_tests.rs"]
mod tests;
