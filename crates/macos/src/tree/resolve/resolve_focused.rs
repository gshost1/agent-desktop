use agent_desktop_core::AdapterError;

use crate::tree::AXElement;

/// The application's focused surface as seen from the window a snapshot or a
/// saved ref names. With no window it is the live focused surface. With a
/// window it is that surface only while the window is the focused one, so a
/// snapshot or a ref never reads or acts in another window that took focus.
pub(in crate::tree) fn focused_surface_for_source(
    pid: i32,
    source_window_id: Option<&str>,
    deadline: std::time::Instant,
) -> Result<Option<AXElement>, AdapterError> {
    let Some(source_window_id) = source_window_id else {
        return crate::tree::surfaces::focused_surface_for_pid(pid, deadline);
    };
    let focused = match crate::tree::surfaces::focused_window_for_pid(pid, deadline)? {
        Some(window) => {
            let number = crate::system::window::window_resolve::ax_window_id_with_deadline(
                &window, deadline,
            )?;
            Some((window, number))
        }
        None => None,
    };
    focused_root_in_source_window(source_window_id, focused, |window| {
        crate::tree::surfaces::focused_surface_in_window(pid, window, deadline)
    })
}

/// The focused surface an observation of `window` reads: a process owner has
/// no window of its own, so it reads the live focused surface.
pub(in crate::tree) fn focused_surface_of(
    window: &agent_desktop_core::WindowInfo,
    pid: i32,
    deadline: std::time::Instant,
) -> Result<AXElement, AdapterError> {
    let source_window_id = (!window.is_process_owner()).then_some(window.id.as_str());
    focused_surface_for_source(pid, source_window_id, deadline)?
        .ok_or_else(|| AdapterError::element_not_found("No focused surface found"))
}

/// Accepts the focused surface for `source_window_id` only when the focused
/// window's number equals the one in that id. A focused window with no number,
/// or no focused window, cannot be proven to be the source window, so it is
/// refused like another window: a same-named control there belongs to another
/// document.
pub(in crate::tree) fn focused_root_in_source_window<W, R>(
    source_window_id: &str,
    focused_window: Option<(W, Option<i64>)>,
    surface_in: impl FnOnce(Option<W>) -> Result<Option<R>, AdapterError>,
) -> Result<Option<R>, AdapterError> {
    let Some(source) = super::resolve_roots::window_number_from_id(source_window_id) else {
        return Err(source_window_identity_invalid(source_window_id));
    };
    match focused_window {
        Some((window, Some(focused))) if focused == source => surface_in(Some(window)),
        focused_window => Err(focused_window_changed(
            source_window_id,
            focused_window.and_then(|(_, number)| number),
        )),
    }
}

fn source_window_identity_invalid(source_window_id: &str) -> AdapterError {
    AdapterError::element_not_found("saved source window").with_details(serde_json::json!({
        "kind": "source_window_identity_invalid",
        "source_window_id": source_window_id,
        "complete": true,
        "retryable": false,
    }))
}

fn focused_window_changed(source_window_id: &str, focused_number: Option<i64>) -> AdapterError {
    let mut details = serde_json::json!({
        "kind": "focused_window_changed",
        "source_window_id": source_window_id,
        "complete": true,
        "retryable": false,
    });
    if let (Some(number), Some(object)) = (focused_number, details.as_object_mut()) {
        object.insert(
            "focused_window_id".into(),
            serde_json::json!(format!("w-{number}")),
        );
    }
    AdapterError::stale_ref_because(format!(
        "Window {source_window_id} is no longer the application's focused window, so its focused surface is gone"
    ))
    .with_suggestion(format!(
        "Run 'focus-window --window-id {source_window_id}' and retry, or take a new snapshot of the window that should receive the action"
    ))
    .with_details(details)
}

#[cfg(test)]
#[path = "resolve_focused_tests.rs"]
mod tests;
