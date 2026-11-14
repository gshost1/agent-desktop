use crate::{AccessibilityNode, NodePresentation, tree::accname::non_blank, tree::state::FOCUSED};

/// Every node of `tree` a diff reports, in the depth-first order refs are
/// allocated in, each without its children. A node with no ref, no text and
/// no state is structure only and is left out, so an unnamed group that
/// appears or disappears with its content adds no row of its own.
pub(crate) fn reported_rows(tree: &AccessibilityNode) -> Vec<AccessibilityNode> {
    let mut rows = Vec::new();
    collect_rows(tree, &mut rows);
    rows
}

/// The rows a baseline keeps: the reported rows without bounds or available
/// actions, which never identify a node and are not compared.
pub(crate) fn baseline_rows(tree: &AccessibilityNode) -> Vec<AccessibilityNode> {
    let mut rows = reported_rows(tree);
    for row in &mut rows {
        row.presentation.bounds = None;
        row.presentation.available_actions.clear();
    }
    rows
}

/// The deepest node of `tree` that reports keyboard focus, the first of them
/// in depth-first order when two share a depth.
pub(crate) fn focused_row(tree: &AccessibilityNode) -> Option<AccessibilityNode> {
    let mut best: Option<(usize, &AccessibilityNode)> = None;
    find_focused(tree, 0, &mut best);
    best.map(|(_, node)| row_of(node))
}

fn collect_rows(node: &AccessibilityNode, rows: &mut Vec<AccessibilityNode>) {
    if is_reported(node) {
        rows.push(row_of(node));
    }
    for child in &node.children {
        collect_rows(child, rows);
    }
}

fn find_focused<'a>(
    node: &'a AccessibilityNode,
    depth: usize,
    best: &mut Option<(usize, &'a AccessibilityNode)>,
) {
    let focused = node.presentation.states.iter().any(|state| state == FOCUSED);
    if focused && best.is_none_or(|(best_depth, _)| depth > best_depth) {
        *best = Some((depth, node));
    }
    for child in &node.children {
        find_focused(child, depth + 1, best);
    }
}

fn is_reported(node: &AccessibilityNode) -> bool {
    let presentation = &node.presentation;
    node.ref_id.is_some()
        || !presentation.states.is_empty()
        || [
            node.identity.name.as_deref(),
            node.identity.value.as_deref(),
            node.identity.description.as_deref(),
            presentation.descriptors.placeholder.as_deref(),
            presentation.hint.as_deref(),
        ]
        .into_iter()
        .any(|text| non_blank(text).is_some())
}

fn row_of(node: &AccessibilityNode) -> AccessibilityNode {
    AccessibilityNode {
        ref_id: node.ref_id.clone(),
        role: node.role.clone(),
        identity: node.identity.clone(),
        presentation: NodePresentation {
            hint: node.presentation.hint.clone(),
            states: node.presentation.states.clone(),
            available_actions: node.presentation.available_actions.clone(),
            bounds: node.presentation.bounds,
            ref_bounds: None,
            descriptors: node.presentation.descriptors.clone(),
        },
        children_count: node.children_count,
        subtree_truncated: node.subtree_truncated,
        children: Vec::new(),
    }
}

#[cfg(test)]
#[path = "diff_rows_tests.rs"]
mod tests;
