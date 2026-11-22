use agent_desktop_core::{AdapterError, Deadline, ErrorCode, RefEntry};
use zbus::Connection;

use crate::system::within;
use crate::tree::element::bounded::collect_bounded;
use crate::tree::properties::read_purpose::ReadPurpose;
use crate::tree::properties::wrapper_description::listing_parent;
use crate::tree::properties::wrapper_parent::WrapperParent;
use crate::tree::vocabulary::roles::may_resolve_to;
use crate::tree::walk::walker_expand::read_observed_with_parent;
use crate::tree::{AtspiElement, ObservedRead, Walk, WalkLimits, WalkedNode};

const NODES_IN_FLIGHT: usize = 32;

fn may_be_target(node: &WalkedNode<ObservedRead>, entry: &RefEntry) -> bool {
    node.payload
        .node
        .traits
        .role
        .present()
        .is_some_and(|number| may_resolve_to(*number, &entry.identity.role))
}

fn targets(
    node: &WalkedNode<ObservedRead>,
    entry: &RefEntry,
    found: &mut Vec<(AtspiElement, Option<WrapperParent>)>,
) {
    targets_with_parent(node, entry, found, None);
}

fn targets_with_parent(
    node: &WalkedNode<ObservedRead>,
    entry: &RefEntry,
    found: &mut Vec<(AtspiElement, Option<WrapperParent>)>,
    parent: Option<&crate::tree::RawNode>,
) {
    if may_be_target(node, entry) {
        let element = &node.payload.node.element;
        found.push((
            element.clone(),
            parent.map(|parent| listing_parent(parent, element)),
        ));
    }
    for child in &node.children {
        targets_with_parent(child, entry, found, Some(&node.payload.node));
    }
}

fn fill(
    node: &mut WalkedNode<ObservedRead>,
    entry: &RefEntry,
    reads: &mut impl Iterator<Item = ObservedRead>,
) {
    if may_be_target(node, entry) {
        if let Some(read) = reads.next() {
            node.payload = read;
        }
    }
    for child in &mut node.children {
        fill(child, entry, reads);
    }
}

/// Reads in full only the nodes of a shallow walk whose role number can read
/// as the stored role: every other node is refuted by its role alone, so
/// nothing else about it is needed to decide the search. A node that left
/// between the two reads leaves the walk incomplete.
pub(in crate::tree) fn read_candidates(
    connection: &Connection,
    walk: &mut Walk<ObservedRead>,
    entry: &RefEntry,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let mut elements = Vec::new();
    targets(&walk.root, entry, &mut elements);
    let max_bytes = WalkLimits::default().max_field_bytes;
    let outcomes = within(deadline, async {
        Ok(collect_bounded(
            NODES_IN_FLIGHT,
            elements.iter().map(|(element, parent)| async move {
                read_observed_with_parent(
                    connection,
                    element,
                    max_bytes,
                    ReadPurpose::Resolve,
                    parent.as_ref(),
                )
                .await
                .map(|(read, _)| read)
            }),
        )
        .await)
    })?;
    let mut reads = Vec::with_capacity(outcomes.len());
    for ((element, _), outcome) in elements.iter().zip(outcomes) {
        match outcome {
            Ok(read) => reads.push(read),
            Err(error)
                if matches!(
                    error.code,
                    ErrorCode::ElementNotFound | ErrorCode::AppNotFound
                ) =>
            {
                walk.complete = false;
                reads.push(ObservedRead::unreadable(element.clone(), &error.message));
            }
            Err(error) => return Err(error),
        }
    }
    fill(&mut walk.root, entry, &mut reads.into_iter());
    Ok(())
}

#[cfg(test)]
#[path = "resolve_candidates_tests.rs"]
mod tests;
