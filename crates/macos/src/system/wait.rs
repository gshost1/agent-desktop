use std::time::{Duration, Instant};

use agent_desktop_core::{AdapterError, Deadline, ErrorCode, MenuWait, ProcessIdentity};

use crate::system::process::process_identity::require_core;

/// Polls the menu state of `process` and feeds each whole read to core's
/// [`MenuWait`], which owns when the wait is satisfied. A read the deadline
/// cut short is not a read: at the deadline the wait says whether the menu
/// stayed closed for the whole wait, and only then is the timeout an error.
pub(crate) fn wait_for_menu(
    process: ProcessIdentity,
    mut wait: MenuWait,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    loop {
        let identity = require_core(&process)?;
        let read_started = Instant::now();
        match read_menu(identity.pid(), deadline) {
            Ok(open) if wait.observe(open, read_started) => {
                require_core(&process)?;
                return Ok(());
            }
            Ok(_) => {}
            Err(error) if error.code == ErrorCode::Timeout && deadline.is_expired() => {}
            Err(error) => return Err(error),
        }
        let pause = match deadline.remaining_slice(Duration::from_millis(50)) {
            Ok(pause) if !deadline.is_expired() => pause,
            _ if wait.satisfied_at_deadline(deadline) => return require_core(&process).map(|_| ()),
            _ => {
                let message = if wait.wants_open() {
                    "No context menu opened before the deadline"
                } else {
                    "Context menu did not close before the deadline"
                };
                return Err(deadline.timeout_error().with_platform_detail(message));
            }
        };
        std::thread::sleep(pause);
    }
}

/// One whole read for core's pre-action menu reading, between the same
/// identity checks the poll loop makes, so a process that exited, or a pid
/// reused by another process, never yields an open reading.
pub(crate) fn menu_open(
    process: &ProcessIdentity,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let identity = require_core(process)?;
    let open = read_menu(identity.pid(), deadline)?;
    require_core(process)?;
    Ok(open)
}

/// The one detector both the poll loop and the pre-action reading use: the
/// window server first, then the accessibility tree.
fn read_menu(pid: i32, deadline: Deadline) -> Result<bool, AdapterError> {
    #[cfg(test)]
    if let Some(read) = forced_reads::take() {
        return read.ok_or_else(|| {
            std::thread::sleep(deadline.remaining());
            deadline.timeout_error()
        });
    }
    let instant = crate::tree::locator_deadline::from_operation(deadline)?;
    crate::system::window::popup_menu_window::menu_shown(pid, || {
        crate::tree::surfaces::is_menu_open(pid, instant)
    })
}

/// Scripts the reads `read_menu` returns on this thread: `None` is a read
/// that runs to the deadline and fails with a timeout. The last scripted read
/// repeats, and `taken` counts the reads served.
#[cfg(test)]
mod forced_reads {
    use std::cell::RefCell;
    use std::collections::VecDeque;

    thread_local! {
        static READS: RefCell<(VecDeque<Option<bool>>, usize)> = const { RefCell::new((VecDeque::new(), 0)) };
    }

    pub(super) fn script(reads: &[Option<bool>]) {
        READS.with(|cell| *cell.borrow_mut() = (reads.iter().copied().collect(), 0));
    }

    pub(super) fn take() -> Option<Option<bool>> {
        READS.with(|cell| {
            let mut state = cell.borrow_mut();
            let read = if state.0.len() > 1 {
                state.0.pop_front()
            } else {
                state.0.front().copied()
            };
            if read.is_some() {
                state.1 += 1;
            }
            read
        })
    }

    pub(super) fn taken() -> usize {
        READS.with(|cell| cell.borrow().1)
    }
}

#[cfg(test)]
#[path = "wait_tests.rs"]
mod tests;
