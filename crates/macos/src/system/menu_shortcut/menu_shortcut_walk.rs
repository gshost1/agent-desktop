use agent_desktop_core::{AdapterError, Deadline, ErrorCode};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

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
const TOP_LEVEL_MENU_DEPTH: u8 = 2;
const SUBMENU_READ_SLICE: Duration = Duration::from_millis(100);

struct Pending {
    element: AXElement,
    depth: u8,
    menu: Option<String>,
    parent: Option<String>,
    position: usize,
}

/// Walks the application's menu bar breadth-first, one batched read and one
/// child read per element, and collects every item whose shortcut equals the
/// target. The menu bar and its top-level menus must read completely. Below
/// them each read gets its own short slice, because a submenu the system fills
/// on demand can hold the application's reply for longer than the whole
/// budget. A read that misses its slice is recorded in `unread_menus` and
/// stepped over, and a budget that runs out below the top-level menus ends the
/// walk with what it read and the count it did not. It stops as soon as two
/// enabled items match, and when a menu-bar title reports an open menu.
pub(crate) fn scan_menu_bar(
    app: &AXElement,
    target: &ShortcutTarget,
    lookup: Deadline,
) -> Result<MenuScan, AdapterError> {
    let Some(menu_bar) = crate::tree::surface_read::element(app, "AXMenuBar", instant(lookup)?)?
    else {
        return Ok(MenuScan::Complete(MenuWalk::default()));
    };
    let names = menu_node_attribute_names();
    let mut queue = VecDeque::from([Pending {
        element: menu_bar,
        depth: 0,
        menu: None,
        parent: None,
        position: 1,
    }]);
    let mut walk = MenuWalk::default();
    while let Some(pending) = queue.pop_front() {
        if lookup.is_expired() {
            if pending.depth > TOP_LEVEL_MENU_DEPTH {
                walk.unread_after_budget = queue.len() + 1;
                break;
            }
            return Err(incomplete_menu_error("deadline", &pending, &walk));
        }
        walk.visited += 1;
        if walk.visited > MAX_MENU_NODES {
            return Err(incomplete_menu_error("node_limit", &pending, &walk));
        }
        let started = Instant::now();
        let read = read_menu_node(&pending.element, &names, read_slice(lookup, pending.depth));
        walk.note_read(started, || read_label(&pending, None));
        let node = match read {
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
            if walk.matches.iter().filter(|found| found.enabled).count() > 1 {
                return Ok(MenuScan::Complete(walk));
            }
        }
        if !holds_menu_items(node.role.as_deref()) {
            continue;
        }
        let here = Pending { menu, ..pending };
        let started = Instant::now();
        let read = children(&here.element, here.depth, read_slice(lookup, here.depth));
        walk.note_read(started, || read_label(&here, node.title.as_deref()));
        let children = match read {
            Ok(children) => children,
            Err(error) => {
                step_over(error, &here, node.title.as_deref(), &mut walk)?;
                continue;
            }
        };
        let parent = node.title.filter(|title| !title.is_empty());
        queue.extend(
            children
                .into_iter()
                .enumerate()
                .map(|(index, child)| Pending {
                    element: child,
                    depth: here.depth.saturating_add(1),
                    menu: here.menu.clone(),
                    parent: parent.clone(),
                    position: index + 1,
                }),
        );
    }
    Ok(MenuScan::Complete(walk))
}

fn read_slice(lookup: Deadline, depth: u8) -> Deadline {
    if depth > TOP_LEVEL_MENU_DEPTH {
        lookup.capped(SUBMENU_READ_SLICE)
    } else {
        lookup
    }
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
    if pending.depth <= TOP_LEVEL_MENU_DEPTH {
        let kind = error
            .details
            .as_ref()
            .and_then(|details| details.get("kind"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("read_failed")
            .to_string();
        return Err(incomplete_menu_error(&kind, pending, walk));
    }
    walk.unread_menus.push(read_label(pending, title));
    Ok(())
}

/// Names a read by its top-level menu, the item that holds it when that is
/// not the menu itself, and its own title, or its position among its siblings
/// when the read that would return the title is the one that failed.
fn read_label(pending: &Pending, title: Option<&str>) -> String {
    let menu = pending.menu.as_deref().unwrap_or("menu bar");
    let item = title
        .filter(|title| !title.is_empty())
        .map_or_else(|| format!("item {}", pending.position), str::to_string);
    match pending.parent.as_deref() {
        Some(parent) if parent != menu => format!("{menu} > {parent} > {item}"),
        _ => format!("{menu} > {item}"),
    }
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
    .with_details({
        let mut fields = walk.report_fields();
        fields.insert("kind".into(), kind.into());
        fields.insert("complete".into(), false.into());
        fields.insert("depth".into(), pending.depth.into());
        fields.insert("menu".into(), serde_json::json!(pending.menu));
        serde_json::Value::Object(fields)
    })
}

#[cfg(test)]
#[path = "menu_shortcut_walk_tests.rs"]
mod tests;
