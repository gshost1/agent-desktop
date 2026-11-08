use agent_desktop_core::{AdapterError, Deadline, MenuWait, ProcessIdentity};

#[cfg(target_os = "macos")]
pub(crate) fn wait_for_menu(
    process: ProcessIdentity,
    mut wait: MenuWait,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    use std::time::{Duration, Instant};

    loop {
        let identity = crate::system::process::process_identity::require_core(&process)?;
        let instant = match crate::tree::locator_deadline::from_operation(deadline) {
            Ok(instant) => instant,
            Err(_) if wait.satisfied_at_deadline() => {
                crate::system::process::process_identity::require_core(&process)?;
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        let read_started = Instant::now();
        if wait.observe(read_menu(identity.pid(), instant)?, read_started) {
            crate::system::process::process_identity::require_core(&process)?;
            return Ok(());
        }
        if deadline.is_expired() {
            if wait.satisfied_at_deadline() {
                crate::system::process::process_identity::require_core(&process)?;
                return Ok(());
            }
            let message = if wait.wants_open() {
                "No context menu opened before the deadline"
            } else {
                "Context menu did not close before the deadline"
            };
            return Err(deadline.timeout_error().with_platform_detail(message));
        }
        let pause = match deadline.remaining_slice(Duration::from_millis(50)) {
            Ok(pause) => pause,
            Err(_) if wait.satisfied_at_deadline() => {
                crate::system::process::process_identity::require_core(&process)?;
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        std::thread::sleep(pause);
    }
}

/// Routes every poll read through one call so a test can script the reads
/// the real loop sees without a live menu.
#[cfg(target_os = "macos")]
fn read_menu(pid: i32, deadline: std::time::Instant) -> Result<bool, AdapterError> {
    #[cfg(test)]
    if let Some(open) = forced_reads::take() {
        return Ok(open);
    }
    crate::tree::surfaces::is_menu_open(pid, deadline)
}

#[cfg(target_os = "macos")]
pub(crate) fn menu_open(
    process: &ProcessIdentity,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let identity = crate::system::process::process_identity::require_core(process)?;
    let instant = crate::tree::locator_deadline::from_operation(deadline)?;
    crate::tree::surfaces::is_menu_open(identity.pid(), instant)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn wait_for_menu(
    _process: ProcessIdentity,
    _wait: MenuWait,
    _deadline: Deadline,
) -> Result<(), AdapterError> {
    Err(AdapterError::not_supported("wait_for_menu"))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn menu_open(
    _process: &ProcessIdentity,
    _deadline: Deadline,
) -> Result<bool, AdapterError> {
    Err(AdapterError::not_supported("menu_open"))
}

/// Scripts the reads `read_menu` returns on this thread; the last scripted
/// read repeats, and `taken` counts the reads served.
#[cfg(all(test, target_os = "macos"))]
mod forced_reads {
    use std::cell::RefCell;
    use std::collections::VecDeque;

    thread_local! {
        static READS: RefCell<(VecDeque<bool>, usize)> = const { RefCell::new((VecDeque::new(), 0)) };
    }

    pub(super) fn script(reads: &[bool]) {
        READS.with(|cell| *cell.borrow_mut() = (reads.iter().copied().collect(), 0));
    }

    pub(super) fn take() -> Option<bool> {
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

#[cfg(all(test, target_os = "macos"))]
#[path = "wait_tests.rs"]
mod tests;
