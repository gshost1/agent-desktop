use agent_desktop_core::{AdapterError, RefEntry};

#[cfg(target_os = "macos")]
use crate::tree::AXElement;

/// The live focused surface of the application, accepted for `entry` only
/// under [`focused_root_for_entry`].
#[cfg(target_os = "macos")]
pub(in crate::tree) fn focused_source_root(
    pid: i32,
    entry: &RefEntry,
    deadline: std::time::Instant,
) -> Result<Option<AXElement>, AdapterError> {
    let focused = match crate::tree::surfaces::focused_window_for_pid(pid, deadline)? {
        Some(window) => {
            let number = crate::system::window::window_resolve::ax_window_id_with_deadline(
                &window, deadline,
            )?;
            Some((window, number))
        }
        None => None,
    };
    focused_root_for_entry(entry, focused, |window| {
        crate::tree::surfaces::focused_surface_in_window(pid, window, deadline)
    })
}

/// The focused surface a saved `focused` ref resolves in. The surface is read
/// live, so a ref with a recorded window resolves only while that window is
/// still the application's focused window; once focus has moved to another
/// window the ref is refused rather than re-rooted there, where a same-named
/// control belongs to another document. A ref with no recorded window keeps
/// the live focused surface, the only scope it was captured with.
pub(in crate::tree) fn focused_root_for_entry<W, R>(
    entry: &RefEntry,
    focused_window: Option<(W, Option<i64>)>,
    surface_in: impl FnOnce(Option<W>) -> Result<Option<R>, AdapterError>,
) -> Result<Option<R>, AdapterError> {
    let Some(recorded_id) = entry.source.source_window_id.as_deref() else {
        return surface_in(focused_window.map(|(window, _)| window));
    };
    let recorded = super::resolve_roots::source_window_number(entry);
    match (recorded, focused_window) {
        (Some(recorded), Some((window, Some(focused)))) if focused == recorded => {
            surface_in(Some(window))
        }
        (_, focused_window) => Err(focused_window_changed(
            recorded_id,
            focused_window.and_then(|(_, number)| number),
        )),
    }
}

fn focused_window_changed(recorded_id: &str, focused_number: Option<i64>) -> AdapterError {
    let mut details = serde_json::json!({
        "kind": "focused_window_changed",
        "source_window_id": recorded_id,
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
        "The ref was captured from the focused surface of window {recorded_id}, which is no longer the focused window"
    ))
    .with_suggestion(format!(
        "Run 'focus-window --window-id {recorded_id}' and retry, or take a new snapshot of the window that should receive the action"
    ))
    .with_details(details)
}

#[cfg(test)]
#[path = "resolve_focused_tests.rs"]
mod tests;
