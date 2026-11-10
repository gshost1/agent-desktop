use std::time::Duration;

use agent_desktop_core::system::process_state::ProcessState;
use agent_desktop_core::{
    AdapterError, Deadline, DeliverySemantics, ErrorCode, ProcessId, WindowInfo,
};

use super::super::process_state::state_from_exit_code;

const FIRST_POLL: Duration = Duration::from_millis(50);
const POLL_LIMIT: Duration = Duration::from_millis(250);

/// Reads for the launch's window until one appears, the launched program is
/// gone, or the wait ends. `read` is one window observation. `gone` answers
/// the error to end on once the launched process has exited, and is asked
/// only after a read found no window, so a program that showed its window
/// and then exited still reports that window. `polls` false reads once.
pub(crate) fn watch_window(
    mut read: impl FnMut() -> Result<Option<WindowInfo>, AdapterError>,
    mut gone: impl FnMut() -> Option<AdapterError>,
    polls: bool,
    deadline: Deadline,
) -> Result<Option<WindowInfo>, AdapterError> {
    let mut poll_interval = FIRST_POLL;
    loop {
        if let Some(window) = read()? {
            return Ok(Some(window));
        }
        if let Some(error) = gone() {
            return Err(error);
        }
        if !polls || deadline.remaining().is_zero() {
            return Ok(None);
        }
        std::thread::sleep(poll_interval.min(deadline.remaining()));
        poll_interval = (poll_interval * 3 / 2).min(POLL_LIMIT);
    }
}

/// The refusal for a launched program that exited before any window of its
/// process appeared, with the code the shared launch contract gives that case. A stub that starts another program and exits ends
/// here too: the window it caused belongs to a process this launch cannot
/// attribute, so reporting the exited pid as the launch would be wrong.
#[allow(dead_code)]
pub(crate) fn exited_before_window(image: &str, pid: ProcessId, code: u32) -> AdapterError {
    let (kind, message) = match state_from_exit_code(code) {
        ProcessState::Crashed { .. } => (
            "crashed",
            format!(
                "'{image}' (pid {pid}) crashed with code {code:#010x} before it showed a window"
            ),
        ),
        _ => (
            "exited",
            format!("'{image}' (pid {pid}) exited with status {code} before it showed a window"),
        ),
    };
    AdapterError::new(ErrorCode::AppUnresponsive, message)
        .with_details(serde_json::json!({ "kind": kind, "status": code, "pid": pid }))
        .with_suggestion(
            "The program may have started another process and exited, as a launcher stub does. Run 'list-windows' to see whether a window opened before you launch again.",
        )
        .with_disposition(DeliverySemantics::delivered_unverified())
}

#[cfg(test)]
#[path = "launch_observe_tests.rs"]
mod tests;
