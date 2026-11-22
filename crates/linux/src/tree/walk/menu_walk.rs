use agent_desktop_core::{AdapterError, Deadline};
use futures_util::stream::{self, StreamExt};
use zbus::Connection;

use super::walker::walk_levels_with;
use super::walker_expand::UNKNOWN_PID;
use super::wrapper::classify;
use crate::system::{ACCESSIBLE_INTERFACE, within};
use crate::tree::properties::node_read::{call, deserialize};
use crate::tree::vocabulary::role_numbers::{MENU, MENU_BAR, POPUP_MENU, is_menu_item};
use crate::tree::vocabulary::state_bits::{EXPANDED, SHOWING, StateBits};
use crate::tree::{AtspiElement, Expansion, ObservedRead, RawNode, Walk, WalkLimits};

fn is_unpresented_menubar(node: &RawNode) -> bool {
    node.traits.role.present() == Some(&MENU_BAR)
        && node.extents.present().is_some_and(|bounds| {
            bounds.width >= 0 && bounds.height >= 0 && (bounds.width == 0 || bounds.height == 0)
        })
}

fn is_presented_menubar(node: &RawNode) -> bool {
    node.traits.role.present() == Some(&MENU_BAR)
        && node
            .extents
            .present()
            .is_some_and(|bounds| bounds.width > 0 && bounds.height > 0)
}

fn can_probe_children<K>(
    parent: Option<&ObservedRead>,
    read: &Expansion<ObservedRead, K>,
    limits: &WalkLimits,
) -> bool {
    let traits = &read.payload.node.traits;
    parent.is_some_and(|parent| parent.node.traits.role.present() == Some(&MENU_BAR))
        && traits
            .role
            .present()
            .is_some_and(|role| matches!(*role, MENU | POPUP_MENU) || is_menu_item(*role))
        && traits
            .states
            .present()
            .is_some_and(|words| !words.is_empty() && !StateBits::new(words).has(EXPANDED))
        && !read.children_unread
        && !read.gone
        && !read.children.is_empty()
        && read.children.len() <= limits.children.max_per_node.min(limits.children.max_edges)
}

pub(super) fn walk_menu_levels<K: Clone + PartialEq>(
    root: K,
    limits: &WalkLimits,
    deadline: Deadline,
    mut expand: impl FnMut(&[K]) -> Result<Vec<Expansion<ObservedRead, K>>, AdapterError>,
    mut children_are_hidden: impl FnMut(&[K]) -> bool,
) -> Result<Walk<ObservedRead>, AdapterError> {
    let mut presented = false;
    let mut deferred = false;
    let mut cached = Vec::new();
    let walk = walk_levels_with(
        root.clone(),
        limits,
        deadline,
        |level| {
            let reads = expand(level)?;
            presented |= reads
                .iter()
                .any(|read| is_presented_menubar(&read.payload.node));
            cached.extend(level.iter().cloned().zip(reads.iter().cloned()));
            Ok(reads)
        },
        |parent, read| {
            if parent.is_some() && is_unpresented_menubar(&read.payload.node) {
                deferred = true;
                return None;
            }
            if can_probe_children(parent, read, limits) && children_are_hidden(&read.children) {
                read.children.clear();
            }
            Some(classify(&read.payload))
        },
    )?;
    if presented || !deferred {
        return Ok(walk);
    }
    walk_levels_with(
        root,
        limits,
        deadline,
        |level| expand_cached(level, &mut cached, &mut expand),
        |parent, read| {
            if can_probe_children(parent, read, limits) && children_are_hidden(&read.children) {
                read.children.clear();
            }
            Some(classify(&read.payload))
        },
    )
}

fn expand_cached<K: Clone + PartialEq>(
    level: &[K],
    cached: &mut Vec<(K, Expansion<ObservedRead, K>)>,
    expand: &mut impl FnMut(&[K]) -> Result<Vec<Expansion<ObservedRead, K>>, AdapterError>,
) -> Result<Vec<Expansion<ObservedRead, K>>, AdapterError> {
    let mut reads: Vec<_> = level
        .iter()
        .map(|key| {
            cached
                .iter()
                .position(|(saved, _)| saved == key)
                .map(|index| cached.swap_remove(index).1)
        })
        .collect();
    let missing: Vec<_> = level
        .iter()
        .zip(&reads)
        .filter(|(_, read)| read.is_none())
        .map(|(key, _)| key.clone())
        .collect();
    if !missing.is_empty() {
        let mut fresh = expand(&missing)?.into_iter();
        for read in &mut reads {
            if read.is_none() {
                *read = fresh.next();
            }
        }
    }
    Ok(reads.into_iter().flatten().collect())
}

fn is_hidden_state(words: Option<&[u32]>) -> bool {
    words.is_some_and(|words| {
        let bits = StateBits::new(words);
        !words.is_empty() && !bits.has(SHOWING) && !bits.has(EXPANDED)
    })
}

/// Menubar openers can remain showing while the dropdown is closed. Probe
/// the dropdown children's states without reading their full nodes.
pub(super) fn children_are_hidden(
    connection: &Connection,
    children: &[AtspiElement],
    deadline: Deadline,
) -> bool {
    if children.iter().any(|child| child.pid() == UNKNOWN_PID) {
        return false;
    }
    within(deadline, async {
        let mut states = stream::iter(children.iter().map(|child| async move {
            let reply = call(connection, child, ACCESSIBLE_INTERFACE, "GetState", &()).await;
            is_hidden_state(deserialize::<Vec<u32>>(reply).ok().as_deref())
        }))
        .buffer_unordered(32);
        while let Some(hidden) = states.next().await {
            if !hidden {
                return Ok(false);
            }
        }
        Ok(true)
    })
    .unwrap_or(false)
}

#[cfg(test)]
#[path = "menu_walk_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "menu_state_tests.rs"]
mod state_tests;

#[cfg(test)]
#[path = "menu_presentation_tests.rs"]
mod presentation_tests;
