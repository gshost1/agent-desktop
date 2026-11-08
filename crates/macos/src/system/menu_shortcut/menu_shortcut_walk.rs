use agent_desktop_core::{AdapterError, Deadline, ErrorCode};
use std::collections::VecDeque;
use std::time::Instant;

use super::{
    menu_match::MenuMatch,
    menu_node::{MenuNode, menu_node_attribute_names, read_menu_node},
    menu_scan::MenuScan,
    menu_walk::MenuWalk,
    shortcut_target::ShortcutTarget,
};
use crate::tree::AXElement;

const MAX_MENU_NODES: usize = 2_048;
const MAX_MENU_CHILDREN: usize = 256;
const MAX_MENU_DEPTH: u8 = 8;
const MENU_BAR_ITEM_DEPTH: u8 = 1;
const SUBMENU_PARENT_DEPTH: u8 = 3;

struct Pending {
    element: AXElement,
    depth: u8,
    menu: Option<String>,
}

/// Walks the application's menu bar breadth-first, one batched read and one
/// child read per element, and collects every item whose shortcut equals the
/// target. The menu bar and its top-level menus must read completely. Below
/// them, a submenu the application cannot hand over, which system-provided
/// submenus filled on demand often cannot, is recorded in `unread_menus` and
/// stepped over rather than discarding the whole walk. It stops as soon as two
/// matches can press, and when a menu-bar title reports an open menu.
pub(crate) fn scan_menu_bar(
    app: &AXElement,
    target: &ShortcutTarget,
    enabled_is_current: bool,
    deadline: Deadline,
) -> Result<MenuScan, AdapterError> {
    let Some(menu_bar) = crate::tree::surface_read::element(app, "AXMenuBar", instant(deadline)?)?
    else {
        return Ok(MenuScan::Complete(MenuWalk::default()));
    };
    let names = menu_node_attribute_names();
    let mut queue = VecDeque::from([Pending {
        element: menu_bar,
        depth: 0,
        menu: None,
    }]);
    let mut walk = MenuWalk::default();
    while let Some(pending) = queue.pop_front() {
        deadline.ensure_remaining()?;
        walk.visited += 1;
        if walk.visited > MAX_MENU_NODES {
            return Err(incomplete_menu_error("node_limit", &pending, &walk));
        }
        let node = match read_menu_node(&pending.element, &names, deadline) {
            Ok(node) => node,
            Err(error) => {
                step_over(error, &pending, None, &mut walk)?;
                continue;
            }
        };
        if pending.depth == MENU_BAR_ITEM_DEPTH && node.selected == Some(true) {
            return Ok(MenuScan::MenuOpen);
        }
        let menu = if pending.depth == MENU_BAR_ITEM_DEPTH {
            node.title.clone()
        } else {
            pending.menu.clone()
        };
        if node.shortcut.matches(target) {
            walk.matches
                .push(menu_match(&pending.element, &node, menu.clone()));
            let pressable = walk
                .matches
                .iter()
                .filter(|found| found.enabled || !enabled_is_current)
                .count();
            if pressable > 1 {
                return Ok(MenuScan::Complete(walk));
            }
        }
        if !holds_menu_items(node.role.as_deref()) {
            continue;
        }
        let here = Pending { menu, ..pending };
        let children = match children(&here.element, here.depth, deadline) {
            Ok(children) => children,
            Err(error) => {
                step_over(error, &here, node.title.as_deref(), &mut walk)?;
                continue;
            }
        };
        queue.extend(children.into_iter().map(|child| Pending {
            element: child,
            depth: here.depth.saturating_add(1),
            menu: here.menu.clone(),
        }));
    }
    Ok(MenuScan::Complete(walk))
}

/// Records an unreadable element inside a top-level menu and continues. A
/// failure on the menu bar or a top-level menu, a permission failure and an
/// exhausted deadline still end the walk.
fn step_over(
    error: AdapterError,
    pending: &Pending,
    title: Option<&str>,
    walk: &mut MenuWalk,
) -> Result<(), AdapterError> {
    if error.code == ErrorCode::PermDenied {
        return Err(error);
    }
    if pending.depth < SUBMENU_PARENT_DEPTH {
        let kind = error
            .details
            .as_ref()
            .and_then(|details| details.get("kind"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("read_failed")
            .to_string();
        return Err(incomplete_menu_error(&kind, pending, walk));
    }
    walk.unread_menus
        .push(unread_label(pending.menu.as_deref(), title));
    Ok(())
}

pub(crate) fn unread_label(menu: Option<&str>, item: Option<&str>) -> String {
    format!("{} > {}", menu.unwrap_or("?"), item.unwrap_or("?"))
}

fn menu_match(element: &AXElement, node: &MenuNode, menu: Option<String>) -> MenuMatch {
    MenuMatch {
        element: element.clone(),
        menu,
        item: node.title.clone(),
        enabled: node.enabled != Some(false),
    }
}

fn holds_menu_items(role: Option<&str>) -> bool {
    matches!(
        role,
        Some("AXMenuBar" | "AXMenuBarItem" | "AXMenu" | "AXMenuItem")
    )
}

fn children(
    element: &AXElement,
    depth: u8,
    deadline: Deadline,
) -> Result<Vec<AXElement>, AdapterError> {
    let limit = if depth >= MAX_MENU_DEPTH {
        0
    } else {
        MAX_MENU_CHILDREN
    };
    let end = instant(deadline)?;
    let read = crate::tree::query::child_source::read_resilient(&["AXChildren"], end, |name| {
        crate::tree::query::child_read::read_attribute_children(element, name, limit, end)
    });
    deadline.ensure_remaining()?;
    if read.status.api_disabled {
        return Err(crate::tree::surface_read::map_error(
            "AXChildren",
            accessibility_sys::kAXErrorAPIDisabled,
        ));
    }
    if depth >= MAX_MENU_DEPTH {
        return if read.total_count > 0 {
            Err(child_error("depth_limit"))
        } else {
            Ok(Vec::new())
        };
    }
    if read.status.invalid_element {
        return Err(child_error("child_invalid"));
    }
    if !read.complete {
        return Err(child_error("child_read"));
    }
    if read.truncated() {
        return Err(child_error("child_limit"));
    }
    Ok(read.elements)
}

fn instant(deadline: Deadline) -> Result<Instant, AdapterError> {
    Instant::now()
        .checked_add(deadline.remaining())
        .ok_or_else(|| AdapterError::new(ErrorCode::InvalidArgs, "Deadline is out of range"))
}

fn child_error(kind: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Menu children could not be read",
    )
    .with_details(serde_json::json!({ "kind": kind }))
}

fn incomplete_menu_error(kind: &str, pending: &Pending, walk: &MenuWalk) -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Menu shortcut lookup could not read the menu bar within its bounds",
    )
    .with_details(serde_json::json!({
        "kind": kind,
        "complete": false,
        "depth": pending.depth,
        "menu": pending.menu,
        "visited": walk.visited,
    }))
}

#[cfg(test)]
#[path = "menu_shortcut_walk_tests.rs"]
mod tests;
