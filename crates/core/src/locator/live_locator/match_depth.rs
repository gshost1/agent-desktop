//! How deep a snapshot of the same root must reach for a match to still
//! match there. Only a resolution that asks for
//! [`super::LocatorMaterialization::MatchDepth`] computes it.

use super::{
    ObservedTree, compiled_clause::CompiledClause, match_verdict::MatchVerdict,
    predicate::self_text_verdict,
};

/// The clauses an evaluation compiled and the verdicts it recorded for them,
/// one cell per node and clause.
pub(super) struct MatchVerdicts<'a, 'q> {
    pub(super) clauses: &'a [CompiledClause<'q>],
    pub(super) matches: &'a [MatchVerdict],
    pub(super) subtree_matches: &'a [MatchVerdict],
    pub(super) subtree_text: &'a [MatchVerdict],
}

impl MatchVerdicts<'_, '_> {
    fn is(&self, verdicts: &[MatchVerdict], node: usize, clause: usize) -> bool {
        verdicts.get(node * self.clauses.len() + clause) == Some(&MatchVerdict::Match)
    }
}

/// The least depth among `matched` that a snapshot cut must reach to keep
/// one of them matching. A match never needs less than its own depth, so a
/// match no shallower than the best found is passed over unread.
pub(super) fn shallowest_match_depth(
    tree: &ObservedTree,
    matched: &[usize],
    clause: usize,
    verdicts: &MatchVerdicts<'_, '_>,
) -> Option<u8> {
    let mut best: Option<u8> = None;
    for &index in matched {
        let own = tree.logical_depths.get(index).copied()?;
        if best.is_some_and(|best| own >= best) {
            continue;
        }
        if let Some(depth) = match_depth(tree, index, clause, verdicts) {
            best = Some(best.map_or(depth, |best| best.min(depth)));
        }
    }
    best
}

/// The depth a cut must reach to keep the node at `index` matching `clause`:
/// its own depth, the shallowest descendant whose text a text part matched,
/// and the shallowest descendant that satisfies a containment part, at the
/// depth that one needs in turn. `None` where the observation recorded no
/// depth.
fn match_depth(
    tree: &ObservedTree,
    index: usize,
    clause: usize,
    verdicts: &MatchVerdicts<'_, '_>,
) -> Option<u8> {
    let own = tree.logical_depths.get(index).copied()?;
    let compiled = verdicts.clauses.get(clause)?;
    let text = match compiled.query.has_text.as_deref() {
        Some(_) => shallowest_text(tree, index, clause, verdicts)?,
        None => own,
    };
    let contained = match compiled.has {
        Some(nested) => shallowest_contained(tree, index, nested, verdicts)?,
        None => own,
    };
    Some(own.max(text).max(contained))
}

fn shallowest_text(
    tree: &ObservedTree,
    index: usize,
    clause: usize,
    verdicts: &MatchVerdicts<'_, '_>,
) -> Option<u8> {
    let query = verdicts.clauses.get(clause)?.query;
    let mut shallowest: Option<u8> = None;
    let mut pending = vec![index];
    while let Some(node_index) = pending.pop() {
        let node = tree.nodes.get(node_index)?;
        let depth = tree.logical_depths.get(node_index).copied()?;
        if shallowest.is_some_and(|best| depth >= best) {
            continue;
        }
        if self_text_verdict(query.has_text.as_deref(), &node.evidence, query.exact)
            == MatchVerdict::Match
        {
            shallowest = Some(depth);
            continue;
        }
        pending.extend(
            node.children
                .iter()
                .map(|child| *child as usize)
                .filter(|child| verdicts.is(verdicts.subtree_text, *child, clause)),
        );
    }
    shallowest
}

/// The least depth that keeps a descendant of `index` matching `nested`,
/// read only through children whose subtrees hold such a match.
fn shallowest_contained(
    tree: &ObservedTree,
    index: usize,
    nested: usize,
    verdicts: &MatchVerdicts<'_, '_>,
) -> Option<u8> {
    let below = |node_index: usize| -> Option<Vec<usize>> {
        Some(
            tree.nodes
                .get(node_index)?
                .children
                .iter()
                .map(|child| *child as usize)
                .filter(|child| verdicts.is(verdicts.subtree_matches, *child, nested))
                .collect(),
        )
    };
    let mut best: Option<u8> = None;
    let mut pending = below(index)?;
    while let Some(node_index) = pending.pop() {
        let depth = tree.logical_depths.get(node_index).copied()?;
        if best.is_some_and(|best| depth >= best) {
            continue;
        }
        if verdicts.is(verdicts.matches, node_index, nested)
            && let Some(needed) = match_depth(tree, node_index, nested, verdicts)
        {
            best = Some(best.map_or(needed, |best| best.min(needed)));
        }
        pending.extend(below(node_index)?);
    }
    best
}

#[cfg(test)]
#[path = "match_depth_tests.rs"]
mod tests;
