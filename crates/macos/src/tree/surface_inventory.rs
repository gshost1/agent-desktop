use crate::tree::{AXElement, properties::element::element_for_pid};
use agent_desktop_core::{AdapterError, ErrorCode, SurfaceInfo};
use std::time::Instant;

pub(crate) fn list_surfaces_for_process(
    identity: crate::system::process::process_identity::ProcessIdentity,
    deadline: Instant,
) -> Result<Vec<SurfaceInfo>, AdapterError> {
    let pid = identity.pid();
    let windows = super::app_menu::observe(identity, deadline, || {
        focused_window_surfaces(pid, deadline)
    })?;
    let menus = super::app_menu::observe(identity, deadline, || open_menu_surfaces(pid, deadline));
    combine_surfaces(windows, menus)
}

fn focused_window_surfaces(pid: i32, deadline: Instant) -> Result<Vec<SurfaceInfo>, AdapterError> {
    let app = element_for_pid(pid);
    let mut surfaces = Vec::new();
    if let Some(window) = read_element(&app, "AXFocusedWindow", deadline)? {
        collect_window_surfaces(&window, "focused-window", deadline, &mut surfaces)?;
    }
    Ok(surfaces)
}

fn open_menu_surfaces(pid: i32, deadline: Instant) -> Result<Vec<SurfaceInfo>, AdapterError> {
    let mut surfaces = Vec::new();
    for (kind, id, menu) in super::app_menu::open_menus_for_pid(pid, deadline)? {
        push_menu_surface(&menu, kind, id, deadline, &mut surfaces)?;
    }
    Ok(surfaces)
}

/// Proving that no context menu is open takes a walk of the whole application
/// tree, which a large application does not finish inside a short budget. The
/// sheets and alerts of the focused window were already read, so an unfinished
/// menu scan keeps them and marks the menu kind unread on the application's
/// menu bar, the owner of every macOS menu. A missing `menu` entry is then
/// never mistaken for an observed absence.
fn combine_surfaces(
    windows: Vec<SurfaceInfo>,
    menus: Result<Vec<SurfaceInfo>, AdapterError>,
) -> Result<Vec<SurfaceInfo>, AdapterError> {
    match menus {
        Ok(mut surfaces) => {
            surfaces.extend(windows);
            Ok(surfaces)
        }
        Err(error) if menu_scan_unfinished(&error) => {
            let mut surfaces = windows;
            surfaces.push(SurfaceInfo {
                id: UNREAD_MENU_OWNER_ID.into(),
                kind: "menubar".into(),
                title: None,
                item_count: None,
                unclassified: vec!["menu".into()],
            });
            Ok(surfaces)
        }
        Err(error) => Err(error),
    }
}

const UNREAD_MENU_OWNER_ID: &str = "app/menubar";

fn menu_scan_unfinished(error: &AdapterError) -> bool {
    matches!(error.code, ErrorCode::AppUnresponsive | ErrorCode::Timeout)
}

fn push_menu_surface(
    menu: &AXElement,
    kind: &str,
    id: String,
    deadline: Instant,
    surfaces: &mut Vec<SurfaceInfo>,
) -> Result<(), AdapterError> {
    let title = read_title(menu, deadline)?;
    let item_count = read_array(menu, "AXChildren", deadline)?.map(|items| items.len());
    surfaces.push(SurfaceInfo {
        id,
        kind: kind.into(),
        title,
        item_count,
        unclassified: Vec::new(),
    });
    Ok(())
}

fn collect_window_surfaces(
    window: &AXElement,
    window_id: &str,
    deadline: Instant,
    surfaces: &mut Vec<SurfaceInfo>,
) -> Result<(), AdapterError> {
    if let Some(kind) = surface_kind(window, deadline)? {
        surfaces.push(SurfaceInfo {
            id: window_id.to_string(),
            kind: kind.into(),
            title: read_title(window, deadline)?,
            item_count: None,
            unclassified: Vec::new(),
        });
    }
    let Some(children) = read_array(window, "AXChildren", deadline)? else {
        return Ok(());
    };
    for (index, child) in children.iter().enumerate() {
        if let Some(kind) = surface_kind(child, deadline)? {
            surfaces.push(SurfaceInfo {
                id: format!("{window_id}/children/{index}"),
                kind: kind.into(),
                title: read_title(child, deadline)?,
                item_count: None,
                unclassified: Vec::new(),
            });
        }
    }
    Ok(())
}

fn surface_kind(
    element: &AXElement,
    deadline: Instant,
) -> Result<Option<&'static str>, AdapterError> {
    let role = read_string(element, "AXRole", deadline)?;
    let subrole = read_string(element, "AXSubrole", deadline)?;
    Ok(match subrole.as_deref() {
        Some("AXSheet") => Some("sheet"),
        Some("AXPopover") => Some("popover"),
        Some("AXDialog") | Some("AXAlert") => Some("alert"),
        _ => match role.as_deref() {
            Some("AXSheet") => Some("sheet"),
            Some("AXPopover") => Some("popover"),
            _ => None,
        },
    })
}

fn read_title(element: &AXElement, deadline: Instant) -> Result<Option<String>, AdapterError> {
    match read_string(element, "AXTitle", deadline)? {
        Some(title) => Ok(Some(title)),
        None => read_string(element, "AXDescription", deadline),
    }
}

fn read_string(
    element: &AXElement,
    attribute: &str,
    deadline: Instant,
) -> Result<Option<String>, AdapterError> {
    crate::tree::surface_read::string(element, attribute, deadline)
}

fn read_array(
    element: &AXElement,
    attribute: &str,
    deadline: Instant,
) -> Result<Option<Vec<AXElement>>, AdapterError> {
    crate::tree::surface_read::elements(element, attribute, deadline).map(Some)
}

fn read_element(
    element: &AXElement,
    attribute: &str,
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    crate::tree::surface_read::element(element, attribute, deadline)
}

#[cfg(test)]
#[path = "surface_inventory_tests.rs"]
mod tests;
