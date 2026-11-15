use agent_desktop_core::{AdapterError, Deadline};

use super::launch_spawn::before_spawn;

/// Refuses a spawn when the session has no display server that answers,
/// including an X server that refuses this client. Every read a launch makes
/// after the spawn needs one, so a spawn would start a program whose launch
/// can only fail, and leave it running unreported.
pub(super) fn require_display(deadline: Deadline) -> Result<(), AdapterError> {
    crate::system::display::display_server::require(deadline)
        .map(|_| ())
        .map_err(before_spawn)
}

/// A failure after the spawn names the spawned process in
/// `details.spawned_pid`, so the caller can find what the launch started.
/// Details that are not an object are kept under `cause`.
pub(super) fn with_spawned_pid(mut error: AdapterError, pid: u32) -> AdapterError {
    let details = match error.details.take() {
        Some(serde_json::Value::Object(mut map)) => {
            map.insert("spawned_pid".to_owned(), pid.into());
            serde_json::Value::Object(map)
        }
        None => serde_json::json!({ "spawned_pid": pid }),
        Some(other) => serde_json::json!({ "spawned_pid": pid, "cause": other }),
    };
    error.with_details(details)
}

#[cfg(test)]
#[path = "launch_preflight_tests.rs"]
mod tests;
