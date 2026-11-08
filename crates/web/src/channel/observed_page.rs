use agent_desktop_core::{
    AccessibilityNode, AdapterError, IdentifierEvidence, LocatorEvidence, LocatorField,
    LocatorRefEvidence, LocatorStats, ObservationRoot, ObservationSource, ObservedSubtree,
    ObservedTree, SnapshotSurface,
};

/// Builds the observation core allocates refs from out of a page tree read
/// over CDP. Every field CDP reported is known; the page tree has no screen
/// bounds. A node at `max_depth` keeps its child count and no children, the
/// way a platform walk cuts at its depth limit.
pub fn observed_page(
    root: &ObservationRoot<'_>,
    tree: AccessibilityNode,
    max_depth: u8,
) -> Result<ObservedTree, AdapterError> {
    ObservedTree::from_roots(
        vec![subtree(tree, 0, max_depth)],
        ObservationSource::from_root(root, SnapshotSurface::Web),
        LocatorStats::default(),
        true,
    )
}

/// The observed evidence of one page node, without its children.
pub fn node_evidence(node: &AccessibilityNode) -> LocatorEvidence {
    let identity = &node.identity;
    let identifiers = IdentifierEvidence::typed(
        identity.native_id.clone(),
        identity.native_id.as_ref().map(|_| 0),
        true,
    );
    LocatorEvidence {
        role: LocatorField::Known(node.role.clone()),
        name: field(identity.name.clone()),
        description: field(identity.description.clone()),
        value: field(identity.value.clone()),
        identifiers: match identity.retained_object.clone() {
            Some(token) => identifiers.with_retained_object(token),
            None => identifiers,
        },
        states: LocatorField::Known(node.presentation.states.clone()),
        ref_evidence: LocatorRefEvidence {
            bounds: LocatorField::Absent,
            window_bounds: LocatorField::Absent,
            available_actions: LocatorField::Known(node.presentation.available_actions.clone()),
            descriptors: node.presentation.descriptors.clone(),
        },
    }
}

fn subtree(node: AccessibilityNode, depth: u8, max_depth: u8) -> ObservedSubtree {
    let evidence = node_evidence(&node);
    if depth >= max_depth && !node.children.is_empty() {
        let count = u32::try_from(node.children.len()).unwrap_or(u32::MAX);
        return ObservedSubtree::new(evidence, Vec::new(), true, Some(count));
    }
    let children = node
        .children
        .into_iter()
        .enumerate()
        .map(|(index, child)| {
            subtree(child, depth.saturating_add(1), max_depth).with_source_child_index(index)
        })
        .collect();
    ObservedSubtree::new(evidence, children, true, node.children_count)
}

fn field(value: Option<String>) -> LocatorField<String> {
    value.map_or(LocatorField::Absent, LocatorField::Known)
}

#[cfg(test)]
#[path = "observed_page_tests.rs"]
mod tests;
