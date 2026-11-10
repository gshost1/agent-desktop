use agent_desktop_core::AdapterError;

use super::launch_spawn::before_spawn;
use crate::system::display::display_server::DisplayServer;

/// Refuses a launch before anything is spawned when the session has no
/// display server that answers. Every read a launch makes after the spawn
/// needs one, so a spawn would start a program whose launch can only fail,
/// and leave it running unreported.
pub(super) fn require_display(server: DisplayServer) -> Result<(), AdapterError> {
    match server {
        DisplayServer::Unavailable(why) => {
            Err(before_spawn(crate::system::display::unreachable(why)))
        }
        DisplayServer::Wayland | DisplayServer::X11(_) => Ok(()),
    }
}

/// A failure after the spawn names the spawned process in
/// `details.spawned_pid`, so the caller can find what the launch started.
pub(super) fn with_spawned_pid(error: AdapterError, pid: u32) -> AdapterError {
    let mut details = match error.details.clone() {
        Some(serde_json::Value::Object(map)) => serde_json::Value::Object(map),
        None => serde_json::json!({}),
        Some(_) => return error,
    };
    details["spawned_pid"] = pid.into();
    error.with_details(details)
}

#[cfg(test)]
#[path = "launch_preflight_tests.rs"]
mod tests;
