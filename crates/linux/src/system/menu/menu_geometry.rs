use agent_desktop_core::{AdapterError, Deadline};
use futures_util::stream::{self, StreamExt, TryStreamExt};

use super::menu_popup::Node;
use crate::system::{SharedBus, within};
use crate::tree::vocabulary::role_numbers::is_menu_item;
use crate::tree::vocabulary::state_bits::{SHOWING, StateBits, VISIBLE};
use crate::tree::{ObservedRead, ReadPurpose, read_node_for};

fn collect<'a>(node: &'a mut Node, items: &mut Vec<&'a mut ObservedRead>) {
    if node.in_web_document {
        return;
    }
    let raw = &node.payload.node;
    if raw
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
        collect(child, items);
    }
}

pub(super) fn fill(
    bus: &SharedBus,
    root: &mut Node,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let mut items = Vec::new();
    collect(root, &mut items);
    if items.is_empty() {
        return Ok(());
    }
    let connection = bus.get(deadline)?;
    within(deadline, async {
        stream::iter(items.into_iter().map(|item| {
            let connection = connection.inner();
            async move {
                let (read, _) =
                    read_node_for(connection, &item.node.element, ReadPurpose::Bounds).await?;
                item.node.extents = read.extents;
                Ok::<_, AdapterError>(())
            }
        }))
        .buffer_unordered(32)
        .try_collect::<Vec<_>>()
        .await?;
        Ok(())
    })
}

#[cfg(test)]
#[path = "menu_geometry_tests.rs"]
mod tests;
