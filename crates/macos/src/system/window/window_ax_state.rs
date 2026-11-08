use agent_desktop_core::{AdapterError, ErrorCode};
use rustc_hash::{FxHashMap, FxHashSet};
use std::time::Instant;

type AxWindowIdentity = (Option<String>, Option<i64>);

#[derive(Clone, Debug)]
pub(crate) struct WindowAxState {
    pub(crate) focused: Option<AxWindowIdentity>,
    pub(crate) minimized_by_id: FxHashMap<i64, bool>,
    pub(crate) accessible_window_ids: FxHashSet<i64>,
    pub(crate) title_by_id: FxHashMap<i64, String>,
}

pub(crate) fn read_until(pid: i32, deadline: Instant) -> Result<WindowAxState, AdapterError> {
    let app = crate::tree::element_for_pid(pid);
    let focused = focused_identity(&app, pid, deadline)?;
    let mut state = read_windows_until(pid, deadline, |_| true)?;
    state.focused = focused;
    Ok(state)
}

pub(crate) fn read_frontmost_until(
    pid: i32,
    deadline: Instant,
) -> Result<WindowAxState, AdapterError> {
    let app = crate::tree::element_for_pid(pid);
    Ok(WindowAxState {
        focused: focused_identity(&app, pid, deadline)?,
        minimized_by_id: FxHashMap::default(),
        accessible_window_ids: FxHashSet::default(),
        title_by_id: FxHashMap::default(),
    })
}

/// One pass over the accessibility windows of `pid`. Every window whose id is
/// read and that `read_fields` selects has `AXMinimized` and `AXTitle` read in
/// the same visit, so ids and titles come from one walk. A walk that fails or
/// runs out of budget returns an error rather than a partial state, so a caller
/// that tolerates the failure keeps every window of that owner on its
/// CoreGraphics title or the application name.
pub(crate) fn read_windows_until(
    pid: i32,
    deadline: Instant,
    read_fields: impl Fn(i64) -> bool,
) -> Result<WindowAxState, AdapterError> {
    let mut minimized_by_id = FxHashMap::default();
    let mut accessible_window_ids = FxHashSet::default();
    let mut title_by_id = FxHashMap::default();
    for_each_window(pid, deadline, |window, window_id| {
        accessible_window_ids.insert(window_id);
        if !read_fields(window_id) {
            return Ok(());
        }
        let fields = crate::system::window::window_ax_fields::read_window_fields(window, deadline)?;
        if let Some(minimized) = fields.minimized {
            minimized_by_id.insert(window_id, minimized);
        }
        if let Some(title) = fields.title {
            title_by_id.insert(window_id, title);
        }
        Ok(())
    })?;
    Ok(WindowAxState {
        focused: None,
        minimized_by_id,
        accessible_window_ids,
        title_by_id,
    })
}

fn for_each_window(
    pid: i32,
    deadline: Instant,
    mut visit: impl FnMut(&crate::tree::AXElement, i64) -> Result<(), AdapterError>,
) -> Result<(), AdapterError> {
    let app = crate::tree::element_for_pid(pid);
    for window in crate::tree::surface_read::elements(&app, "AXWindows", deadline)? {
        match crate::system::window::window_resolve::ax_window_id_with_deadline(&window, deadline) {
            Ok(Some(id)) => visit(&window, id)?,
            Ok(None) => {}
            Err(error) if crate::system::window::window_bridge::is_unavailable(&error) => break,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

/// Frontmost-ness only selects whether a focused window is reported, and a busy
/// application answering "unknown" is not a reason to fail the command that
/// asked. Permission and API failures still propagate, because those describe
/// the caller's access rather than the application's state.
fn is_frontmost(app: &crate::tree::AXElement, deadline: Instant) -> Result<bool, AdapterError> {
    match crate::tree::surface_read::boolean(app, "AXFrontmost", deadline) {
        Ok(frontmost) => Ok(frontmost == Some(true)),
        Err(error) if error.code == ErrorCode::PermDenied => Err(error),
        Err(_) => Ok(false),
    }
}

fn focused_identity(
    app: &crate::tree::AXElement,
    pid: i32,
    deadline: Instant,
) -> Result<Option<AxWindowIdentity>, AdapterError> {
    if !is_frontmost(app, deadline)? {
        return Ok(None);
    }
    let Some(focused) = crate::tree::surface_read::element(app, "AXFocusedWindow", deadline)?
    else {
        return Ok(None);
    };
    let role = crate::tree::surface_read::string(&focused, "AXRole", deadline)?;
    let window = match focused_role_kind(role.as_deref()) {
        FocusedRoleKind::Window => focused,
        FocusedRoleKind::TransientSurface => {
            let Some(window) = crate::tree::surface_read::element(&focused, "AXWindow", deadline)?
            else {
                return Ok(None);
            };
            if crate::tree::surface_read::string(&window, "AXRole", deadline)?.as_deref()
                != Some("AXWindow")
            {
                return Ok(None);
            }
            window
        }
        FocusedRoleKind::NotAWindow => {
            tracing::debug!(pid, ?role, "focused accessibility object is not a window");
            return Ok(None);
        }
    };
    let title = crate::tree::surface_read::string(&window, "AXTitle", deadline)?;
    let number = match crate::system::window::window_resolve::ax_window_id_with_deadline(
        &window, deadline,
    ) {
        Ok(number) => number,
        Err(error) if crate::system::window::window_bridge::is_unavailable(&error) => None,
        Err(error) => return Err(error),
    };
    Ok(Some((title, number)))
}

fn is_transient_surface_role(role: &str) -> bool {
    matches!(role, "AXSheet" | "AXDialog" | "AXAlert" | "AXPopover")
}

#[derive(Debug, PartialEq)]
enum FocusedRoleKind {
    Window,
    TransientSurface,
    NotAWindow,
}

fn focused_role_kind(role: Option<&str>) -> FocusedRoleKind {
    match role {
        Some("AXWindow") => FocusedRoleKind::Window,
        Some(role) if is_transient_surface_role(role) => FocusedRoleKind::TransientSurface,
        _ => FocusedRoleKind::NotAWindow,
    }
}

#[cfg(test)]
mod tests {
    use super::{FocusedRoleKind, focused_role_kind, is_transient_surface_role};

    #[test]
    fn modal_surfaces_can_temporarily_own_focused_window() {
        for role in ["AXSheet", "AXDialog", "AXAlert", "AXPopover"] {
            assert!(is_transient_surface_role(role));
        }
        assert!(!is_transient_surface_role("AXButton"));
    }

    #[test]
    fn a_focused_desktop_is_no_focused_window_rather_than_an_error() {
        assert_eq!(focused_role_kind(Some("AXWindow")), FocusedRoleKind::Window);
        assert_eq!(
            focused_role_kind(Some("AXSheet")),
            FocusedRoleKind::TransientSurface
        );
        assert_eq!(
            focused_role_kind(Some("AXScrollArea")),
            FocusedRoleKind::NotAWindow
        );
        assert_eq!(focused_role_kind(None), FocusedRoleKind::NotAWindow);
    }
}
