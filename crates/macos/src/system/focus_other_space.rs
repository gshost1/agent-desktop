//! Brings a window on another Space on screen for `focus-window`.
//!
//! macOS gives a window on another Space no accessibility element, so the
//! raise that `focus-window` performs cannot start there. Activating the
//! window's application makes macOS switch to a Space that holds its windows,
//! unless the user turned that off in Desktop & Dock settings. The window then
//! joins `AXWindows`, and the raise proceeds. The wait for it is bounded by
//! [`SPACE_SWITCH_LIMIT`], which covers the Space switch animation.

use agent_desktop_core::{AdapterError, Deadline, ErrorCode};
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
/// When the wait ends, the error names what was seen: the command's own
/// deadline running out is a `TIMEOUT` (`window_space_switch`), even when the
/// lookup itself is what reports the deadline; a window seen on another Space
/// during the wait stayed there; a window only ever seen without its
/// accessibility element keeps that error. `application_activated` records
/// whether the activation started.
pub(crate) fn element_after_activation<T>(
    mut lookup: impl FnMut() -> Result<T, AdapterError>,
    application_activated: bool,
    deadline: Deadline,
) -> Result<T, AdapterError> {
    let limit = Instant::now() + SPACE_SWITCH_LIMIT;
    let mut last_away: Option<AdapterError> = None;
    let mut seen_on_other_space = false;
    loop {
        if let Some(away) = last_away.as_ref().filter(|_| deadline.is_expired()) {
            return Err(space_switch_timeout(
                away,
                seen_on_other_space,
                application_activated,
            ));
        }
        match lookup() {
            Ok(element) => return Ok(element),
            Err(error) if is_still_away(&error) => {
                seen_on_other_space |= is_other_space_error(&error);
                last_away = Some(error);
            }
            Err(error) if error.code == ErrorCode::Timeout && deadline.is_expired() => {
                let away = last_away.unwrap_or(error);
                return Err(space_switch_timeout(
                    &away,
                    seen_on_other_space,
                    application_activated,
                ));
            }
            Err(error) => return Err(error),
        }
        if Instant::now() >= limit {
            let away = last_away.take().expect("a still-away lookup was recorded");
            return Err(if seen_on_other_space {
                stayed_on_other_space(&away, application_activated)
            } else {
                with_activation(away, application_activated)
            });
        }
        if let Ok(pause) = deadline.remaining_slice(POLL_INTERVAL) {
            std::thread::sleep(pause);
        }
    }
}

fn space_switch_timeout(
    away: &AdapterError,
    seen_on_other_space: bool,
    application_activated: bool,
) -> AdapterError {
    let window_id = window_id(away);
    let message = if seen_on_other_space {
        format!("Window '{window_id}' did not come on screen before the command's deadline")
    } else {
        format!(
            "Window '{window_id}' did not publish its accessibility element before the command's deadline"
        )
    };
    AdapterError::timeout(message)
        .with_details(serde_json::json!({
            "kind": "window_space_switch",
            "window_id": window_id,
            "application_activated": application_activated,
        }))
        .with_suggestion(format!(
            "Retry with a longer --timeout; the Space switch can take a moment. If it still \
             fails, switch to the window's Space in Mission Control. 'screenshot --window-id \
             {window_id}' works without moving it."
        ))
}

/// Adds `application_activated` to an error's details.
fn with_activation(mut error: AdapterError, application_activated: bool) -> AdapterError {
    let mut details = error
        .details
        .take()
        .filter(serde_json::Value::is_object)
        .unwrap_or_else(|| serde_json::json!({}));
    details["application_activated"] = serde_json::json!(application_activated);
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

fn stayed_on_other_space(error: &AdapterError, application_activated: bool) -> AdapterError {
    let window_id = window_id(error);
    AdapterError::new(
        error.code.clone(),
        format!("Window '{window_id}' stayed on another Space after its application was activated"),
    )
    .with_details(serde_json::json!({
        "kind": "window_on_other_space",
        "window_id": window_id,
        "application_activated": application_activated,
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
