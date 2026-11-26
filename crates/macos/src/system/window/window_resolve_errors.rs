use agent_desktop_core::{AdapterError, ErrorCode};

pub(crate) fn invalid_window_id(id: &str) -> AdapterError {
    AdapterError::new(ErrorCode::InvalidArgs, format!("Invalid window id: '{id}'"))
        .with_suggestion("Window ids come from 'list-windows' (format w-<number>).")
}

/// The window exists in the CoreGraphics inventory but the owning application
/// does not publish it through accessibility, and nothing proves it closed: it
/// may sit on another Space, behind a full-screen Space, or be a window the
/// application never publishes. Screenshots and coordinate input address
/// CoreGraphics windows; only the semantic path needs an accessibility element.
pub(crate) fn window_not_accessible(id: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionNotSupported,
        format!("Window '{id}' exists but is not exposed through accessibility"),
    )
    .with_details(serde_json::json!({
        "kind": "window_without_accessibility_element",
        "window_id": id,
    }))
    .with_suggestion(
        "This window supports screenshots and coordinate input only. Bring it on screen, \
         or target another window from 'list-windows'.",
    )
}

pub(crate) fn window_not_found(id: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::WindowNotFound,
        format!("Window '{id}' not found"),
    )
    .with_suggestion("Run 'list-windows' to see available windows and their IDs.")
}

pub(crate) fn window_identity_mismatch(id: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::WindowNotFound,
        format!("Window '{id}' identity mismatch"),
    )
    .with_suggestion("Run 'list-windows' to refresh window IDs, then retry.")
}

/// The window exists, the window server places it on a Space other than the
/// one on screen, and macOS gives windows there no accessibility tree. Only a
/// screenshot reaches it where it is; a tree read needs it on screen first.
pub(crate) fn window_on_other_space(id: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionNotSupported,
        format!("Window '{id}' is on another Space, where macOS exposes no accessibility tree"),
    )
    .with_details(serde_json::json!({
        "kind": "window_on_other_space",
        "window_id": id,
    }))
    .with_suggestion(format!(
        "Run 'focus-window --window-id {id}' to bring its Space on screen, then retry. \
         'screenshot --window-id {id}' works without moving it."
    ))
    .with_disposition(agent_desktop_core::DeliverySemantics::not_delivered())
}
