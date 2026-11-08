use agent_desktop_core::{AdapterError, Deadline, MenuWait, ProcessIdentity};

#[cfg(target_os = "macos")]
pub(crate) fn wait_for_menu(
    process: ProcessIdentity,
    mut wait: MenuWait,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    use crate::tree::surfaces::is_menu_open;
    use std::time::{Duration, Instant};

    loop {
        let identity = crate::system::process::process_identity::require_core(&process)?;
        let instant = crate::tree::locator_deadline::from_operation(deadline)?;
        let read_started = Instant::now();
        if wait.observe(is_menu_open(identity.pid(), instant)?, read_started) {
            crate::system::process::process_identity::require_core(&process)?;
            return Ok(());
        }
        if deadline.is_expired() {
            let message = if wait.wants_open() {
                "No context menu opened before the deadline"
            } else {
                "Context menu did not close before the deadline"
            };
            return Err(deadline.timeout_error().with_platform_detail(message));
        }
        let pause = deadline.remaining_slice(Duration::from_millis(50))?;
        std::thread::sleep(pause);
    }
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
