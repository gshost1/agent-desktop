//! How deep a snapshot of the same root must reach for a match to still
//! match there.

use super::{
    ObservedTree, compiled_clause::CompiledClause, match_verdict::MatchVerdict,
    predicate::self_text_verdict,
};

/// The clause a match answered and the subtree text verdicts the evaluation
/// recorded for it, one per node.
pub(super) struct MatchedClause<'a> {
    pub(super) clause: &'a CompiledClause<'a>,
    pub(super) index: usize,
    pub(super) count: usize,
    pub(super) subtree_text: &'a [MatchVerdict],
}

/// The logical depth a snapshot cut must reach to keep the node at `index`
/// matching: its own depth, the depth of the shallowest descendant whose
/// text a text part matched, and, for a containment part, the deepest node
/// below it. `None` where the observation recorded no depth.
pub(super) fn match_depth(
    tree: &ObservedTree,
    index: usize,
    matched: &MatchedClause<'_>,
) -> Option<u8> {
    let own = tree.logical_depths.get(index).copied()?;
    let text = match matched.clause.query.has_text.as_deref() {
        Some(text) => shallowest_text(tree, index, text, matched)?,
        None => own,
    };
    let contained = match matched.clause.has {
        Some(_) => deepest_below(tree, index)?,
        None => own,
    };
    Some(own.max(text).max(contained))
}

fn shallowest_text(
    tree: &ObservedTree,
    index: usize,
    text: &str,
    matched: &MatchedClause<'_>,
) -> Option<u8> {
    let exact = matched.clause.query.exact;
    let mut shallowest: Option<u8> = None;
    let mut pending = vec![index];
    while let Some(node_index) = pending.pop() {
        let (Some(node), Some(depth)) = (
            tree.nodes.get(node_index),
            tree.logical_depths.get(node_index).copied(),
        ) else {
            return None;
        };
        if shallowest.is_some_and(|best| depth >= best) {
            continue;
        }
        if self_text_verdict(Some(text), &node.evidence, exact) == MatchVerdict::Match {
            shallowest = Some(depth);
            continue;
        }
        pending.extend(
            node.children
                .iter()
                .map(|child| *child as usize)
                .filter(|child| {
                    matched
                        .subtree_text
                        .get(child * matched.count + matched.index)
                        == Some(&MatchVerdict::Match)
                }),
        );
    }
    shallowest
}

fn deepest_below(tree: &ObservedTree, index: usize) -> Option<u8> {
    let mut deepest = None;
    let mut pending = vec![index];
    while let Some(node_index) = pending.pop() {
        let depth = tree.logical_depths.get(node_index).copied()?;
        deepest = deepest.max(Some(depth));
        pending.extend(
            tree.nodes
                .get(node_index)?
                .children
                .iter()
                .map(|child| *child as usize),
        );
    }
    deepest
}

#[cfg(test)]
#[path = "match_depth_tests.rs"]
mod tests;
