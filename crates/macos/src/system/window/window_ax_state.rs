use agent_desktop_core::{AdapterError, ErrorCode, WindowInfo};
use rustc_hash::{FxHashMap, FxHashSet};
use std::time::Instant;

type AxWindowIdentity = (Option<String>, Option<i64>);

#[derive(Clone, Debug)]
pub(crate) struct WindowAxState {
    pub(crate) focused: Option<AxWindowIdentity>,
    pub(crate) minimized_by_id: FxHashMap<i64, bool>,
    pub(crate) accessible_window_ids: FxHashSet<i64>,
    pub(crate) titles: crate::system::window::window_ax_titles::WindowAxTitles,
}

pub(crate) fn read_until(pid: i32, deadline: Instant) -> Result<WindowAxState, AdapterError> {
    let app = crate::tree::element_for_pid(pid);
    let focused = focused_identity(&app, pid, deadline)?;
    let mut minimized_by_id = FxHashMap::default();
    let mut accessible_window_ids = FxHashSet::default();
    let mut titles = crate::system::window::window_ax_titles::WindowAxTitles::default();
    for_each_window(pid, deadline, |window, window_id| {
        accessible_window_ids.insert(window_id);
        let (title, minimized) =
            crate::system::window::window_ax_attributes::read_title_and_minimized(
                window, deadline,
            )?;
        titles.record(window_id, title);
        if let Some(minimized) = minimized {
            minimized_by_id.insert(window_id, minimized);
        }
        Ok(())
    })?;
    Ok(WindowAxState {
        focused,
        minimized_by_id,
        accessible_window_ids,
        titles,
    })
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
        titles: Default::default(),
    })
}

/// Lists the CoreGraphics windows `pid` publishes through accessibility. A
/// title is read only for the window numbers in `wanted_titles`, the windows
/// whose CoreGraphics record carried no name, so a caller holding Screen
/// Recording permission pays for no title read.
pub(crate) fn accessible_window_titles_until(
    pid: i32,
    wanted_titles: &FxHashSet<i64>,
    deadline: Instant,
) -> Result<crate::system::window::window_ax_titles::WindowAxTitles, AdapterError> {
    let mut titles = crate::system::window::window_ax_titles::WindowAxTitles::default();
    for_each_window(pid, deadline, |window, id| {
        let title = if wanted_titles.contains(&id) {
            crate::tree::surface_read::string(window, "AXTitle", deadline)
                .ok()
                .flatten()
        } else {
            None
        };
        titles.record(id, title);
        Ok(())
    })?;
    Ok(titles)
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

/// Labels each window's accessibility and gives a window with an unknown title
/// the title of the AX window that bridged to it. A process whose windows
/// cannot be read keeps its windows selectable and their titles unknown.
pub(crate) fn label_accessibility(
    windows: &mut [WindowInfo],
    mut read: impl FnMut(
        i32,
        &FxHashSet<i64>,
    ) -> Result<
        crate::system::window::window_ax_titles::WindowAxTitles,
        AdapterError,
    >,
) {
    let mut pids = windows.iter().map(|window| window.pid).collect::<Vec<_>>();
    pids.sort_unstable();
    pids.dedup();
    for pid in pids {
        let wanted_titles = windows
            .iter()
            .filter(|window| window.pid == pid && window.title.is_empty())
            .filter_map(|window| {
                crate::system::window::window_resolve::parse_window_number(&window.id)
            })
            .collect::<FxHashSet<_>>();
        let Some(titles) = crate::system::process::process_identity::to_pid_t(pid)
            .ok()
            .and_then(|pid| read(pid, &wanted_titles).ok())
        else {
            continue;
        };
        for window in windows.iter_mut().filter(|window| window.pid == pid) {
            let number = crate::system::window::window_resolve::parse_window_number(&window.id);
            window.state.accessible = number.is_some_and(|id| titles.contains(id));
            if window.title.is_empty()
                && let Some(title) = number.and_then(|id| titles.title_for(id))
            {
                window.title = title.to_owned();
            }
        }
    }
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
    use super::{
        FocusedRoleKind, focused_role_kind, is_transient_surface_role, label_accessibility,
    };
    use crate::system::window::window_ax_titles::WindowAxTitles;
    use agent_desktop_core::{ProcessId, WindowInfo, WindowState};
    use rustc_hash::FxHashSet;

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

    #[test]
    fn failed_pid_probe_keeps_unknown_window_selectable() {
        let mut windows = vec![window(10, "w-1"), window(20, "w-2")];

        label_accessibility(&mut windows, |pid, _| {
            if pid == 10 {
                Ok(WindowAxTitles::default())
            } else {
                Err(agent_desktop_core::AdapterError::timeout("busy"))
            }
        });

        assert!(!windows[0].state.accessible);
        assert!(windows[1].state.accessible);
    }

    #[test]
    fn an_untitled_window_takes_its_bridged_ax_title_and_requests_only_its_own() {
        let mut windows = vec![window(10, "w-1"), titled(window(10, "w-2"), "Kept")];
        let mut requested = FxHashSet::default();

        label_accessibility(&mut windows, |_, wanted| {
            requested = wanted.clone();
            let mut titles = WindowAxTitles::default();
            titles.record(1, Some("Quarterly".into()));
            titles.record(2, Some("Accessibility title".into()));
            Ok(titles)
        });

        assert_eq!(requested, FxHashSet::from_iter([1]));
        assert_eq!(windows[0].title, "Quarterly");
        assert_eq!(windows[1].title, "Kept");
        assert!(windows[0].state.accessible);
    }

    #[test]
    fn an_unprovable_title_stays_unknown_and_never_becomes_the_app_name() {
        let mut windows = vec![window(10, "w-1"), window(10, "w-2"), window(20, "w-3")];

        label_accessibility(&mut windows, |pid, _| {
            let mut titles = WindowAxTitles::default();
            if pid == 10 {
                titles.record(1, Some("First".into()));
                titles.record(1, Some("Second".into()));
                return Ok(titles);
            }
            Err(agent_desktop_core::AdapterError::timeout("busy"))
        });

        assert_eq!(windows[0].title, "");
        assert!(windows[0].state.accessible);
        assert_eq!(windows[1].title, "");
        assert!(!windows[1].state.accessible);
        assert_eq!(windows[2].title, "");
    }

    fn titled(mut window: WindowInfo, title: &str) -> WindowInfo {
        window.title = title.into();
        window
    }

    fn window(pid: u32, id: &str) -> WindowInfo {
        WindowInfo {
            id: id.into(),
            title: String::new(),
            app: "Fixture".into(),
            pid: ProcessId::new(pid),
            process_instance: Some(format!("{pid}:100")),
            bounds: None,
            state: WindowState::default(),
        }
    }
}
