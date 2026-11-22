use agent_desktop_core::{AdapterError, Deadline};
use futures_util::stream::{self, StreamExt, TryStreamExt};

use super::menu_popup::Node;
use crate::system::{SharedBus, within};
use crate::tree::vocabulary::role_numbers::{MENU, POPUP_MENU, is_menu_item};
use crate::tree::vocabulary::state_bits::{SHOWING, StateBits, VISIBLE};
use crate::tree::{ObservedRead, ReadPurpose, read_node_for};

fn collect<'a>(node: &'a mut Node, menu_showing: bool, items: &mut Vec<&'a mut ObservedRead>) {
    if node.in_web_document {
        return;
    }
    let raw = &node.payload.node;
    let child_menu_showing =
        if matches!(raw.traits.role.present().copied(), Some(MENU | POPUP_MENU)) {
            raw.traits
                .states
                .present()
                .is_some_and(|words| StateBits::new(words).has(SHOWING))
        } else {
            menu_showing
        };
    if menu_showing
        && raw
            .traits
            .role
            .present()
            .is_some_and(|role| is_menu_item(*role))
        && raw.extents.is_failed()
        && raw.traits.states.present().is_some_and(|words| {
            let bits = StateBits::new(words);
            bits.has(VISIBLE) && !bits.has(SHOWING)
        })
    {
        items.push(&mut node.payload);
    }
    for child in &mut node.children {
        collect(child, child_menu_showing, items);
    }
}

pub(super) fn fill(
    bus: &SharedBus,
    root: &mut Node,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let mut items = Vec::new();
    collect(root, false, &mut items);
    if items.is_empty() {
        return Ok(());
    }
    let connection = bus.get(deadline)?;
    within(deadline, async {
        stream::iter(items.into_iter().map(|item| {
            let connection = connection.inner();
            async move {
                let read = read_node_for(connection, &item.node.element, ReadPurpose::Bounds).await;
                apply_bounds(item, read.map(|(node, _)| node.extents))
            }
        }))
        .buffer_unordered(32)
        .try_collect::<Vec<_>>()
        .await?;
        Ok(())
    })
}

fn apply_bounds(
    item: &mut ObservedRead,
    read: Result<crate::tree::Field<crate::tree::Extents>, AdapterError>,
) -> Result<(), AdapterError> {
    if let Ok(bounds) = read {
        item.node.extents = bounds;
    }
    Ok(())
}

#[cfg(test)]
#[path = "menu_geometry_tests.rs"]
mod tests;
