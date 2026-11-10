//! Wakes a wait for a process's exit when the kernel reports the exit, instead
//! of at the end of a fixed pause.
//!
//! A close polls the process identity, and between checks it slept a fixed
//! 25 ms, so a close that had already finished waited out the rest of that
//! pause. A kqueue exit filter ends the pause when the process exits. The wake
//! proves nothing by itself: the caller checks the identity again, so a
//! reused pid or a spurious wake reads the same as a timeout did.

use std::time::Duration;

/// Waits up to `pause` for `pid` to exit. Returns at once when the process is
/// already gone, and sleeps the whole pause when the kernel cannot watch it.
pub(crate) fn wait_for_exit_event(pid: i32, pause: Duration) {
    match watch_exit(pid, pause) {
        Watch::Ended => {}
        Watch::Unavailable => std::thread::sleep(pause),
    }
}

enum Watch {
    Ended,
    Unavailable,
}

fn watch_exit(pid: i32, pause: Duration) -> Watch {
    let Ok(ident) = usize::try_from(pid) else {
        return Watch::Unavailable;
    };
    let queue = unsafe { libc::kqueue() };
    if queue < 0 {
        return Watch::Unavailable;
    }
    let change = libc::kevent {
        ident,
        filter: libc::EVFILT_PROC,
        flags: libc::EV_ADD | libc::EV_ONESHOT | libc::EV_RECEIPT,
        fflags: libc::NOTE_EXIT,
        data: 0,
        udata: std::ptr::null_mut(),
    };
    let mut receipt = empty_event();
    let registered = unsafe { libc::kevent(queue, &change, 1, &mut receipt, 1, std::ptr::null()) };
    let watch = if registered != 1 || receipt.flags & libc::EV_ERROR == 0 {
        Watch::Unavailable
    } else if receipt.data == libc::ESRCH as isize {
        Watch::Ended
    } else if receipt.data != 0 {
        Watch::Unavailable
    } else {
        wait_for_event(queue, pause)
    };
    unsafe { libc::close(queue) };
    watch
}

fn wait_for_event(queue: i32, pause: Duration) -> Watch {
    let timeout = libc::timespec {
        tv_sec: libc::time_t::try_from(pause.as_secs()).unwrap_or(libc::time_t::MAX),
        tv_nsec: libc::c_long::from(pause.subsec_nanos()),
    };
    let mut event = empty_event();
    let woken = unsafe { libc::kevent(queue, std::ptr::null(), 0, &mut event, 1, &timeout) };
    if woken < 0 {
        return Watch::Unavailable;
    }
    Watch::Ended
}

fn empty_event() -> libc::kevent {
    libc::kevent {
        ident: 0,
        filter: 0,
        flags: 0,
        fflags: 0,
        data: 0,
        udata: std::ptr::null_mut(),
    }
}

#[cfg(test)]
#[path = "process_exit_event_tests.rs"]
mod tests;
