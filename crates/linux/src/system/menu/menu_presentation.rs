use std::collections::HashMap;

use agent_desktop_core::Deadline;

use super::menu_popup::Node;
use crate::system::x11::{DisplayReader, PopupWindow};
use crate::tree::Extents;
use crate::tree::placement::visibility::is_placed;
use crate::tree::vocabulary::role_numbers::{MENU, POPUP_MENU, is_menu_item};
use crate::tree::vocabulary::state_bits::{SHOWING, StateBits, VISIBLE};

pub(in crate::system) fn covers(popup: Extents, item: Extents) -> bool {
    is_placed(item)
        && is_placed(popup)
        && item.x >= popup.x
        && item.y >= popup.y
        && i64::from(item.x) + i64::from(item.width) <= i64::from(popup.x) + i64::from(popup.width)
        && i64::from(item.y) + i64::from(item.height)
            <= i64::from(popup.y) + i64::from(popup.height)
}

pub(super) fn corroborate(deadline: Deadline) -> impl FnMut(&Node) -> bool {
    corroborate_with(move |pid| DisplayReader::menu_popups(pid, deadline))
}

fn corroborate_with(
    mut read: impl FnMut(u32) -> Option<Vec<PopupWindow>>,
) -> impl FnMut(&Node) -> bool {
    let mut windows = HashMap::new();
    move |node| {
        let raw = &node.payload.node;
        let mut rects = Vec::new();
        item_rects(node, &mut rects);
        if rects.is_empty() {
            return false;
        }
        windows
            .entry(raw.element.pid())
            .or_insert_with(|| read(raw.element.pid()))
            .as_ref()
            .is_some_and(|popups| {
                popups
                    .iter()
                    .any(|popup| rects.iter().all(|rect| covers(popup.bounds, *rect)))
            })
    }
}

fn item_rects(node: &Node, rects: &mut Vec<Extents>) {
    if node.in_web_document {
        return;
    }
    if let Some(rect) = item_rect(node) {
        rects.push(rect);
    }
    for child in &node.children {
        if !matches!(
            child.payload.node.traits.role.present(),
            Some(&MENU | &POPUP_MENU)
        ) {
            item_rects(child, rects);
        }
    }
}

pub(super) fn needs_corroboration(node: &Node) -> bool {
    !node.in_web_document
        && node.children.iter().any(|child| {
            (item_rect(child).is_some()
                && child
                    .payload
                    .node
                    .traits
                    .states
                    .present()
                    .is_some_and(|words| !StateBits::new(words).has(SHOWING)))
                || (!matches!(
                    child.payload.node.traits.role.present(),
                    Some(&MENU | &POPUP_MENU)
                ) && needs_corroboration(child))
        })
}

pub(super) fn item_rect(node: &Node) -> Option<Extents> {
    let raw = &node.payload.node;
    if !node.in_web_document
        && raw
            .traits
            .role
            .present()
            .is_some_and(|role| is_menu_item(*role))
        && raw
            .traits
            .states
            .present()
            .is_some_and(|words| StateBits::new(words).has(VISIBLE))
        && let Some(rect) = raw
            .extents
            .present()
            .copied()
            .filter(|rect| is_placed(*rect))
    {
        return Some(rect);
    }
    None
}

#[cfg(test)]
#[path = "menu_presentation_tests.rs"]
mod tests;
