use crate::{
    AppError, ErrorCode, ProcessIdentity, WindowInfo,
    adapter::{PlatformAdapter, WindowFilter},
};
use serde_json::json;

pub(crate) fn find_window_for_process(
    process: ProcessIdentity,
    adapter: &dyn PlatformAdapter,
    deadline: crate::Deadline,
) -> Result<WindowInfo, AppError> {
    let filter = WindowFilter {
        focused_only: false,
        app: None,
    };
    find_window_in(process, &filter, adapter, deadline)
}

/// Like [`find_window_for_process`], but chooses among the windows the
/// app-scoped inventory lists, the set `list-windows --app` shows. On macOS
/// that set holds the app's windows on other Spaces and leaves out the
/// off-screen windows it ordered out.
pub(crate) fn find_window_for_app(
    app: &crate::AppInfo,
    adapter: &dyn PlatformAdapter,
    deadline: crate::Deadline,
) -> Result<WindowInfo, AppError> {
    let process = crate::system::app_lookup::process_identity(app)?;
    let filter = WindowFilter {
        focused_only: false,
        app: Some(app.name.clone()),
    };
    find_window_in(process, &filter, adapter, deadline)
}

fn find_window_in(
    process: ProcessIdentity,
    filter: &WindowFilter,
    adapter: &dyn PlatformAdapter,
    deadline: crate::Deadline,
) -> Result<WindowInfo, AppError> {
    let same_pid: Vec<_> = adapter
        .list_windows(filter, deadline)?
        .into_iter()
        .filter(|window| window.pid == process.pid)
        .collect();
    let incomplete_count = same_pid
        .iter()
        .filter(|window| window.process_instance.as_deref().is_none_or(str::is_empty))
        .count();
    if incomplete_count > 0 {
        return Err(crate::AdapterError::new(
            ErrorCode::ActionNotSupported,
            "Window inventory has incomplete process identity",
        )
        .with_details(json!({
            "candidate_count": same_pid.len(),
            "incomplete_identity_count": incomplete_count,
        }))
        .into());
    }
    let candidates: Vec<_> = same_pid
        .into_iter()
        .filter(|window| window.process_instance.as_deref() == Some(process.instance.as_str()))
        .collect();
    select_app_window(
        adapter,
        candidates,
        crate::AdapterError::new(
            ErrorCode::WindowNotFound,
            "No window found for the selected process instance",
        ),
        "Multiple windows matched the selected process instance",
        deadline,
    )
}

/// Chooses one of an application's windows, as every `--app` command does.
/// When the rules of [`select_window`] leave several windows, the ones the
/// adapter reports as auxiliary, such as a floating panel or a text field's
/// suggestion list, are set aside and the choice is made among the rest.
/// Several windows still left, as for an application in the background,
/// which focuses none, resolve to the one the adapter names the
/// application's main window. Without that, they stay ambiguous, and a set
/// that is all auxiliary or has none is judged as it stands.
pub(crate) fn select_app_window(
    adapter: &dyn PlatformAdapter,
    candidates: Vec<WindowInfo>,
    empty_error: crate::AdapterError,
    ambiguous_message: &str,
    deadline: crate::Deadline,
) -> Result<WindowInfo, AppError> {
    let remaining = match narrow_window(candidates.clone()) {
        Ok(chosen) => return Ok(chosen),
        Err(remaining) if remaining.is_empty() => return Err(empty_error.unsent().into()),
        Err(remaining) => remaining,
    };
    let mut remaining = match standard_windows(adapter, candidates, deadline) {
        Some(standard) => match narrow_window(standard) {
            Ok(chosen) => return Ok(chosen),
            Err(remaining) => remaining,
        },
        None => remaining,
    };
    let main = adapter.main_window(&remaining, deadline);
    match remaining
        .iter()
        .position(|window| main.as_deref() == Some(window.id.as_str()))
    {
        Some(index) => Ok(remaining.swap_remove(index)),
        None => Err(ambiguous_error(&remaining, ambiguous_message)),
    }
}

/// The windows among `windows` that are not auxiliary, when the adapter
/// reports at least one auxiliary window and at least one other.
pub(crate) fn standard_windows(
    adapter: &dyn PlatformAdapter,
    windows: Vec<WindowInfo>,
    deadline: crate::Deadline,
) -> Option<Vec<WindowInfo>> {
    let count = windows.len();
    let standard: Vec<_> = windows
        .into_iter()
        .filter(|window| !adapter.is_auxiliary_window(window, deadline))
        .collect();
    (!standard.is_empty() && standard.len() < count).then_some(standard)
}

/// A menu bar, menu, or alert belongs to the application, not to one of its
/// windows, so several open windows are not an ambiguity for those surfaces —
/// any window of the app names the same process. Prefers a focused or visible
/// window so the caller still gets the most relevant identity.
pub(crate) fn select_surface_owner(
    candidates: Vec<WindowInfo>,
    empty_error: crate::AdapterError,
) -> Result<WindowInfo, AppError> {
    if candidates.is_empty() {
        return Err(empty_error.unsent().into());
    }
    let best = candidates
        .iter()
        .position(|window| window.state.is_focused)
        .or_else(|| {
            candidates
                .iter()
                .position(|window| window.state.visible == Some(true))
        })
        .unwrap_or(0);
    let mut candidates = candidates;
    Ok(candidates.swap_remove(best))
}

/// Orders an app's windows the way a surface owner is chosen: focused first,
/// then visible, then the rest, each group in the order the platform listed.
pub(crate) fn surface_owner_order(mut candidates: Vec<WindowInfo>) -> Vec<WindowInfo> {
    candidates.sort_by_key(|window| (!window.state.is_focused, window.state.visible != Some(true)));
    candidates
}

pub(crate) fn select_window(
    candidates: Vec<WindowInfo>,
    empty_error: crate::AdapterError,
    ambiguous_message: &str,
) -> Result<WindowInfo, AppError> {
    match narrow_window(candidates) {
        Ok(chosen) => Ok(chosen),
        Err(remaining) if remaining.is_empty() => Err(empty_error.unsent().into()),
        Err(remaining) => Err(ambiguous_error(&remaining, ambiguous_message)),
    }
}

/// The one window the visibility, focus and accessibility rules leave, or
/// every window still in the running when they leave none or several.
fn narrow_window(mut candidates: Vec<WindowInfo>) -> Result<WindowInfo, Vec<WindowInfo>> {
    if candidates.len() == 1 {
        return Ok(candidates.swap_remove(0));
    }
    if candidates.is_empty() {
        return Err(candidates);
    }
    if candidates
        .iter()
        .any(|window| window.state.visible == Some(true))
    {
        candidates.retain(|window| window.state.visible == Some(true));
        if candidates.len() == 1 {
            return Ok(candidates.swap_remove(0));
        }
    }
    let focused = candidates
        .iter()
        .position(|window| window.state.is_focused)
        .filter(|first| {
            !candidates[*first + 1..]
                .iter()
                .any(|window| window.state.is_focused)
        });
    if let Some(index) = focused {
        return Ok(candidates.swap_remove(index));
    }
    if candidates.iter().any(|window| window.state.accessible) {
        candidates.retain(|window| window.state.accessible);
        if candidates.len() == 1 {
            return Ok(candidates.swap_remove(0));
        }
    }
    Err(candidates)
}

/// Window titles are left out: in a browser they are the user's open tabs,
/// and an error must not carry them into the caller's context.
fn ambiguous_error(candidates: &[WindowInfo], ambiguous_message: &str) -> AppError {
    let summaries = candidates
        .iter()
        .take(10)
        .map(|window| {
            let mut summary = json!({
                "id": window.id,
                "is_focused": window.state.is_focused,
                "visible": window.state.visible,
            });
            if let Some(on_other_space) = window.state.on_other_space {
                summary["on_other_space"] = json!(on_other_space);
            }
            summary
        })
        .collect::<Vec<_>>();
    crate::AdapterError::ambiguous_target(ambiguous_message)
        .with_suggestion("Run 'list-windows' and retry with --window-id <id>.")
        .with_details(json!({
            "candidate_count": candidates.len(),
            "candidate_summaries_truncated": candidates.len() > summaries.len(),
            "candidates": summaries,
        }))
        .into()
}

#[cfg(test)]
#[path = "window_lookup_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "window_lookup_auxiliary_tests.rs"]
mod auxiliary_tests;

#[cfg(test)]
#[path = "window_lookup_main_tests.rs"]
mod main_tests;
