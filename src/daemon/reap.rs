//! A one-shot CLI leaves the helpers it starts to be reaped by init when it exits. A daemon
//! lives on, so between commands it collects every child that has already exited; nothing waits
//! on a child then, because no command is running.

/// Collects exited children without blocking; returns how many were collected.
pub(crate) fn reap_exited_children() -> usize {
    let mut reaped = 0;
    loop {
        let mut status = 0;
        let pid = unsafe { libc::waitpid(-1, &raw mut status, libc::WNOHANG) };
        if pid <= 0 {
            return reaped;
        }
        reaped += 1;
    }
}

#[cfg(test)]
#[path = "reap_tests.rs"]
mod tests;
