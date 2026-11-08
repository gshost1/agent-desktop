use agent_desktop_core::{AdapterError, Deadline, ErrorCode};
use std::collections::VecDeque;
use std::time::Instant;

use super::{
    menu_match::MenuMatch,
    menu_node::{MenuNode, menu_node_attribute_names, read_menu_node},
    menu_scan::MenuScan,
    shortcut_target::ShortcutTarget,
};
use crate::tree::AXElement;

const MAX_MENU_NODES: usize = 2_048;
const MAX_MENU_CHILDREN: usize = 256;
const MAX_MENU_DEPTH: u8 = 8;
const MENU_BAR_ITEM_DEPTH: u8 = 1;

/// Walks the application's menu bar breadth-first, one batched read and one
/// child read per element, and collects every item whose shortcut equals the
/// target. It stops as soon as two enabled items match, because the choice is
/// already ambiguous, and when a menu-bar title reports an open menu.
pub(crate) fn scan_menu_bar(
    app: &AXElement,
    target: &ShortcutTarget,
    deadline: Deadline,
) -> Result<MenuScan, AdapterError> {
    let Some(menu_bar) = crate::tree::surface_read::element(app, "AXMenuBar", instant(deadline)?)?
    else {
        return Ok(MenuScan::Complete(Vec::new()));
    };
    let names = menu_node_attribute_names();
    let mut queue = VecDeque::from([(menu_bar, 0_u8, None::<String>)]);
    let mut matches = Vec::new();
    let mut visited = 0_usize;
    while let Some((element, depth, menu)) = queue.pop_front() {
        deadline.ensure_remaining()?;
        visited += 1;
        if visited > MAX_MENU_NODES {
            return Err(incomplete_menu_error("node_limit"));
        }
        let node = read_menu_node(&element, &names, deadline)?;
        if depth == MENU_BAR_ITEM_DEPTH && node.selected == Some(true) {
            return Ok(MenuScan::MenuOpen);
        }
        let menu = if depth == MENU_BAR_ITEM_DEPTH {
            node.title.clone()
        } else {
            menu
        };
        if node.shortcut.matches(target) {
            matches.push(menu_match(&element, &node, menu.clone()));
            if matches.iter().filter(|found| found.enabled).count() > 1 {
                return Ok(MenuScan::Complete(matches));
            }
        }
        if !holds_menu_items(node.role.as_deref()) {
            continue;
        }
        let children = children(&element, depth, deadline)?;
        queue.extend(
            children
                .into_iter()
                .map(|child| (child, depth.saturating_add(1), menu.clone())),
        );
    }
    Ok(MenuScan::Complete(matches))
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
    let read = crate::tree::query::child_read::read_attribute_children(
        element,
        "AXChildren",
        limit,
        instant(deadline)?,
    );
    deadline.ensure_remaining()?;
    if read.status.api_disabled {
        return Err(crate::tree::surface_read::map_error(
            "AXChildren",
            accessibility_sys::kAXErrorAPIDisabled,
        ));
    }
    if read.status.invalid_element
        || read.status.health.cannot_complete > 0
        || read.status.health.deadline_exhausted > 0
    {
        return Err(incomplete_menu_error("child_read"));
    }
    if depth >= MAX_MENU_DEPTH {
        return if read.total_count > 0 {
            Err(incomplete_menu_error("depth_limit"))
        } else {
            Ok(Vec::new())
        };
    }
    if !read.complete || read.truncated() {
        return Err(incomplete_menu_error("child_limit"));
    }
    Ok(read.elements)
}

fn instant(deadline: Deadline) -> Result<Instant, AdapterError> {
    Instant::now()
        .checked_add(deadline.remaining())
        .ok_or_else(|| AdapterError::new(ErrorCode::InvalidArgs, "Deadline is out of range"))
}

fn incomplete_menu_error(kind: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Menu shortcut lookup exceeded its bounded accessibility search",
    )
    .with_details(serde_json::json!({ "kind": kind, "complete": false }))
}
