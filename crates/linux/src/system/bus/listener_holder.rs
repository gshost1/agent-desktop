//! A started holder child, addressed through a pidfd so a kill can never reach
//! another process that reused its pid after a host reaped the holder
//! (`SIGCHLD` ignored, or a `waitpid(-1, ...)` reaper).

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

/// The holder's pid and, where the kernel has pidfds (Linux 5.3 and later), a
/// pidfd opened right after the fork, while the child is still asleep.
pub(crate) struct HolderHandle {
    pid: libc::pid_t,
    pidfd: Option<OwnedFd>,
}

impl HolderHandle {
    pub(crate) fn of(pid: libc::pid_t) -> Self {
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
        let pidfd = i32::try_from(fd)
            .ok()
            .filter(|fd| *fd >= 0)
            .map(|fd| unsafe { OwnedFd::from_raw_fd(fd) });
        Self { pid, pidfd }
    }

    pub(crate) fn pid(&self) -> libc::pid_t {
        self.pid
    }

    /// Ends the holder if it still runs and reaps it if it is still ours.
    /// Returns whether a kill was sent. A holder that already exited and was
    /// reaped by the host gets no signal, and neither does whatever process
    /// now has its pid.
    pub(crate) fn retire(self) -> bool {
        match &self.pidfd {
            Some(pidfd) => retire_by_pidfd(pidfd),
            None => retire_by_pid(self.pid),
        }
    }
}

fn retire_by_pidfd(pidfd: &OwnedFd) -> bool {
    let fd = pidfd.as_raw_fd();
    let sent = unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            fd,
            libc::SIGKILL,
            std::ptr::null::<libc::siginfo_t>(),
            0,
        )
    } == 0;
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    let flags = if sent {
        libc::WEXITED
    } else {
        libc::WEXITED | libc::WNOHANG
    };
    unsafe {
        libc::waitid(libc::P_PIDFD, fd as libc::id_t, &mut info, flags);
    }
    sent
}

fn retire_by_pid(pid: libc::pid_t) -> bool {
    match unsafe { libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG) } {
        0 => unsafe {
            libc::kill(pid, libc::SIGKILL);
            libc::waitpid(pid, std::ptr::null_mut(), 0);
            true
        },
        _ => false,
    }
}

#[cfg(test)]
#[path = "listener_holder_tests.rs"]
mod tests;
