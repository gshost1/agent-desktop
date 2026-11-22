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
