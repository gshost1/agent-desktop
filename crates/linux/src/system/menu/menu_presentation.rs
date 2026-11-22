use std::collections::HashMap;

use agent_desktop_core::Deadline;

use super::menu_popup::Node;
use crate::system::x11::{DisplayReader, PopupWindow};
use crate::tree::Extents;
use crate::tree::placement::visibility::is_placed;
use crate::tree::vocabulary::state_bits::{StateBits, VISIBLE};

fn covers(popup: Extents, item: Extents) -> bool {
    is_placed(popup)
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
        if !raw
            .traits
            .states
            .present()
            .is_some_and(|words| StateBits::new(words).has(VISIBLE))
        {
            return false;
        }
        let Some(rect) = raw
            .extents
            .present()
            .copied()
            .filter(|rect| is_placed(*rect))
        else {
            return false;
        };
        windows
            .entry(raw.element.pid())
            .or_insert_with(|| read(raw.element.pid()))
            .as_ref()
            .is_some_and(|popups| popups.iter().any(|popup| covers(popup.bounds, rect)))
    }
}

#[cfg(test)]
#[path = "menu_presentation_tests.rs"]
mod tests;
