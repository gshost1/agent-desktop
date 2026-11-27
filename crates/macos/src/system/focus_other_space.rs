//! Brings a window on another Space on screen for `focus-window`.
//!
//! macOS gives a window on another Space no accessibility element, so the
//! raise that `focus-window` performs cannot start there. Activating the
//! window's application makes macOS switch to a Space that holds its windows,
//! unless the user turned that off in Desktop & Dock settings. The window then
//! joins `AXWindows`, and the raise proceeds. The wait for it is bounded by
//! [`SPACE_SWITCH_LIMIT`], which covers the Space switch animation.

use agent_desktop_core::{AdapterError, Deadline};
use std::time::{Duration, Instant};

const SPACE_SWITCH_LIMIT: Duration = Duration::from_millis(1500);
const POLL_INTERVAL: Duration = Duration::from_millis(25);

/// Whether `error` says the window is on another Space.
pub(crate) fn is_other_space_error(error: &AdapterError) -> bool {
    error_kind(error) == Some("window_on_other_space")
}

/// Looks the window up until it has an accessibility element, while macOS
/// switches Spaces after the activation. Any error other than the window
/// still being away ends the wait at once.
///
/// When the wait ends, the error names what was last seen: the command's own
/// deadline running out is a timeout; a window that reached the Space on
/// screen but has not published its accessibility element yet keeps that
/// error; only a window still placed on another Space says it stayed there.
pub(crate) fn element_after_activation<T>(
    mut lookup: impl FnMut() -> Result<T, AdapterError>,
    deadline: Deadline,
) -> Result<T, AdapterError> {
    let limit = Instant::now() + SPACE_SWITCH_LIMIT;
    loop {
        let error = match lookup() {
            Ok(element) => return Ok(element),
            Err(error) if is_still_away(&error) => error,
            Err(error) => return Err(error),
        };
        if deadline.is_expired() {
            return Err(activated(AdapterError::timeout(
                "Timed out waiting for the window to come on screen after its application was activated",
            )
            .with_details(serde_json::json!({
                "kind": "window_space_switch",
                "window_id": window_id(&error),
            }))));
        }
        if Instant::now() >= limit {
            return Err(if is_other_space_error(&error) {
                stayed_on_other_space(error)
            } else {
                activated(error)
            });
        }
        if let Ok(pause) = deadline.remaining_slice(POLL_INTERVAL) {
            std::thread::sleep(pause);
        }
    }
}

/// Adds `application_activated: true` to an error's details.
fn activated(mut error: AdapterError) -> AdapterError {
    let mut details = error
        .details
        .take()
        .filter(serde_json::Value::is_object)
        .unwrap_or_else(|| serde_json::json!({}));
    details["application_activated"] = serde_json::json!(true);
    error.with_details(details)
}

fn window_id(error: &AdapterError) -> String {
    error
        .details
        .as_ref()
        .and_then(|details| details.get("window_id"))
        .and_then(|id| id.as_str())
        .unwrap_or_default()
        .to_owned()
}

fn is_still_away(error: &AdapterError) -> bool {
    matches!(
        error_kind(error),
        Some("window_on_other_space" | "window_without_accessibility_element")
    )
}

fn error_kind(error: &AdapterError) -> Option<&str> {
    error.details.as_ref()?.get("kind")?.as_str()
}

fn stayed_on_other_space(error: AdapterError) -> AdapterError {
    let window_id = window_id(&error);
    AdapterError::new(
        error.code,
        format!("Window '{window_id}' stayed on another Space after its application was activated"),
    )
    .with_details(serde_json::json!({
        "kind": "window_on_other_space",
        "window_id": window_id,
        "application_activated": true,
    }))
    .with_suggestion(format!(
        "macOS did not switch Spaces. It switches on activation only when the application has \
         no window on the Space on screen and 'When switching to an application, switch to a \
         Space with open windows for the application' is on in System Settings > Desktop & \
         Dock. Switch to the window's Space in Mission Control, then retry. \
         'screenshot --window-id {window_id}' works without moving it."
    ))
}

#[cfg(test)]
#[path = "focus_other_space_tests.rs"]
mod tests;
