//! The holder child: forked without exec, so it needs no `sleep` program on
//! the system, and between `fork` and `_exit` it only makes async-signal-safe
//! calls.

use std::os::fd::{AsRawFd, OwnedFd, RawFd};
use std::time::Duration;

use super::listener_holder::HolderHandle;

const HELD_FD: RawFd = 3;
const SLEEP_ROUNDS: usize = 64;
const CLOSE_CEILING: RawFd = 1 << 20;
const LAST_SIGNAL: libc::c_int = 64;

/// What the child needs, opened before the fork so the child itself only
/// makes async-signal-safe calls.
struct ChildPlan {
    null: OwnedFd,
    socket: RawFd,
    last_fd: RawFd,
    hold: libc::timespec,
}

pub(super) fn fork_holder(socket: RawFd, hold: Duration) -> std::io::Result<HolderHandle> {
    let null = std::fs::File::open("/dev/null")?;
    let plan = ChildPlan {
        null: OwnedFd::from(null),
        socket,
        last_fd: descriptor_limit(),
        hold: libc::timespec {
            tv_sec: libc::time_t::try_from(hold.as_secs()).unwrap_or(libc::time_t::MAX),
            tv_nsec: libc::c_long::from(hold.subsec_nanos()),
        },
    };
    match unsafe { libc::fork() } {
        -1 => Err(std::io::Error::last_os_error()),
        0 => unsafe { run_child(&plan) },
        child => Ok(HolderHandle::of(child)),
    }
}

fn descriptor_limit() -> RawFd {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } != 0 {
        return 1024;
    }
    close_ceiling(limit.rlim_cur)
}

/// The last descriptor the fallback close loop reaches: the soft limit, but
/// never more than 2^20 closes, also when the limit is `RLIM_INFINITY`.
fn close_ceiling(soft_limit: libc::rlim_t) -> RawFd {
    RawFd::try_from(soft_limit)
        .unwrap_or(CLOSE_CEILING)
        .min(CLOSE_CEILING)
}

/// The child: default signal dispositions and an empty signal mask, so none
/// of the parent's handlers runs in it and `SIGTERM` ends it; null standard
/// streams, the socket as descriptor 3, every other descriptor closed, then a
/// sleep and `_exit`. It never returns and runs no destructor or exit handler
/// of the parent.
unsafe fn run_child(plan: &ChildPlan) -> ! {
    unsafe {
        let mut default: libc::sigaction = std::mem::zeroed();
        default.sa_sigaction = libc::SIG_DFL;
        for signal in 1..=LAST_SIGNAL {
            libc::sigaction(signal, &default, std::ptr::null_mut());
        }
        let mut unblocked: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut unblocked);
        libc::sigprocmask(libc::SIG_SETMASK, &unblocked, std::ptr::null_mut());
        libc::setpgid(0, 0);
        let null = plan.null.as_raw_fd();
        for standard in 0..HELD_FD {
            libc::dup2(null, standard);
        }
        if plan.socket != HELD_FD {
            libc::dup2(plan.socket, HELD_FD);
        }
        if libc::syscall(libc::SYS_close_range, HELD_FD + 1, u32::MAX, 0) != 0 {
            for fd in HELD_FD + 1..plan.last_fd {
                libc::close(fd);
            }
        }
        let mut asked = plan.hold;
        for _ in 0..SLEEP_ROUNDS {
            let mut left = libc::timespec {
                tv_sec: 0,
                tv_nsec: 0,
            };
            if libc::nanosleep(&asked, &mut left) == 0 {
                break;
            }
            asked = left;
        }
        libc::_exit(0)
    }
}

#[cfg(test)]
#[path = "listener_child_tests.rs"]
mod tests;
