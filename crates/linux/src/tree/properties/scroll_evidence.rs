use futures_util::future::join;
use zbus::{Connection, zvariant::OwnedValue};

use super::node_read::{call, deserialize};
use crate::system::PROPERTIES_INTERFACE;
use crate::tree::vocabulary::role_numbers::{
    LIST_ITEM, SCROLL_BAR, TABLE_CELL, TABLE_ROW, TREE_ITEM,
};
use crate::tree::vocabulary::state_bits::{SHOWING, StateBits, VERTICAL};
use crate::tree::{AtspiElement, Extents, RawNode};

const VALUE_INTERFACE: &str = "org.a11y.atspi.Value";

async fn number(connection: &Connection, bar: &AtspiElement, key: &str) -> Option<f64> {
    let value = deserialize::<OwnedValue>(
        call(
            connection,
            bar,
            PROPERTIES_INTERFACE,
            "Get",
            &(VALUE_INTERFACE, key),
        )
        .await,
    )
    .ok()?;
    value
        .downcast_ref::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn decide_bar(vertical: bool, range: Option<(f64, f64)>) -> Option<Result<bool, ()>> {
    vertical.then(|| match range {
        Some((min, max)) if min.is_finite() && max.is_finite() => Ok(max > min),
        _ => Err(()),
    })
}

pub(crate) async fn read_bar(connection: &Connection, node: &RawNode) -> Option<Result<bool, ()>> {
    if node.traits.role.present() != Some(&SCROLL_BAR) {
        return None;
    }
    let vertical = StateBits::new(node.traits.states.present()?).has(VERTICAL);
    if !vertical {
        return None;
    }
    let (min, max) = join(
        number(connection, &node.element, "MinimumValue"),
        number(connection, &node.element, "MaximumValue"),
    )
    .await;
    decide_bar(vertical, min.zip(max))
}

pub(crate) fn with_child_visibility<'a>(
    bar: Option<bool>,
    role: Option<&str>,
    states: Option<&[u32]>,
    own_viewport: bool,
    children: impl Iterator<Item = (Option<u32>, Option<&'a [u32]>)>,
) -> Option<bool> {
    bar.or_else(|| {
        if !own_viewport
            || !matches!(
                role,
                Some("list" | "listbox" | "table" | "outline" | "grid" | "browser")
            )
            || !states.is_some_and(|words| StateBits::new(words).has(SHOWING))
        {
            return None;
        }
        let (mut showing, mut not_showing) = (false, false);
        for words in children.filter_map(|(role, states)| {
            matches!(role, Some(LIST_ITEM | TABLE_CELL | TABLE_ROW | TREE_ITEM))
                .then_some(states)
                .flatten()
        }) {
            if StateBits::new(words).has(SHOWING) {
                showing = true;
            } else {
                not_showing = true;
            }
        }
        (showing && not_showing).then_some(true)
    })
}

pub(crate) fn is_own_viewport(bounds: Option<&Extents>, area: Option<Option<&Extents>>) -> bool {
    match area {
        None => true,
        Some(Some(area)) => {
            bounds.is_some_and(|bounds| bounds == area && bounds.width > 0 && bounds.height > 0)
        }
        Some(None) => false,
    }
}

pub(crate) fn of_children(bars: impl Iterator<Item = Option<Result<bool, ()>>>) -> Option<bool> {
    let mut found = false;
    let mut unknown = false;
    for bar in bars.flatten() {
        match bar {
            Ok(true) => return Some(true),
            Ok(false) => found = true,
            Err(()) => unknown = true,
        }
    }
    (found && !unknown).then_some(false)
}

#[cfg(test)]
#[path = "scroll_evidence_tests.rs"]
mod tests;
