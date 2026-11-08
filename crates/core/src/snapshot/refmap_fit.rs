//! Fits a full snapshot under the refmap size cap.
//!
//! A refmap is stored whole or not at all, and the store refuses one above
//! `MAX_REFMAP_BYTES`. When a full snapshot allocates more refs than fit, the
//! presented tree is cut in the order refs were allocated (depth first, a node
//! before its children): every node up to the first ref that would overflow is
//! kept with its ref, and that node and everything after it are dropped. Every
//! kept ancestor of the cut carries `subtree_truncated`, the same marker an
//! exhausted observation budget leaves, so the result is an ordinary partial
//! snapshot. The cut is a pure function of the tree, so the same tree always
//! cuts at the same node.
//!
//! Capture proof is unaffected: a kept ref was counted against the whole
//! observation, and dropping other refs changes neither that count nor what a
//! later resolve compares against.

use crate::refs::{MAX_REFMAP_BYTES, RefMap};
use crate::{AccessibilityNode, AppError};
use std::collections::HashSet;

/// The refs kept so far in allocation order and the bytes still free for more.
struct RefBudget<'a> {
    refmap: &'a RefMap,
    remaining: usize,
    kept: HashSet<String>,
    exhausted: bool,
}

impl RefBudget<'_> {
    fn admit(&mut self, ref_id: &str) -> Result<bool, AppError> {
        let Some(entry_len) = self.refmap.serialized_entry_len(ref_id)? else {
            return Ok(true);
        };
        let separator = usize::from(!self.kept.is_empty());
        let cost = entry_len + separator;
        if cost > self.remaining {
            self.exhausted = true;
            return Ok(false);
        }
        self.remaining -= cost;
        self.kept.insert(ref_id.to_string());
        Ok(true)
    }
}

/// Cuts `tree` and `refmap` so the refmap fits `MAX_REFMAP_BYTES`. Returns
/// whether anything was cut; a refmap that already fits is left untouched.
pub(crate) fn fit_refmap_cap(
    tree: &mut AccessibilityNode,
    refmap: &mut RefMap,
) -> Result<bool, AppError> {
    let cap = usize::try_from(MAX_REFMAP_BYTES).unwrap_or(usize::MAX);
    fit_refmap_within(tree, refmap, cap)
}

pub(crate) fn fit_refmap_within(
    tree: &mut AccessibilityNode,
    refmap: &mut RefMap,
    cap: usize,
) -> Result<bool, AppError> {
    if refmap.serialized_len()? <= cap {
        return Ok(false);
    }
    let fixed = refmap.serialized_len_without_entries()?;
    let mut budget = RefBudget {
        refmap,
        remaining: cap.saturating_sub(fixed),
        kept: HashSet::new(),
        exhausted: false,
    };
    if !keep_allocated_prefix(tree, &mut budget)? {
        tree.ref_id = None;
        tree.children.clear();
        tree.subtree_truncated = true;
    }
    let kept = budget.kept;
    refmap.retain_refs(&kept);
    Ok(true)
}

/// Walks `node` in allocation order. Returns `false` when `node` itself does
/// not fit, so its parent drops it and every later sibling. A kept node whose
/// walk ran the budget out is an ancestor of the cut and is marked, as an
/// exhausted observation marks every ancestor of the node it stopped at.
fn keep_allocated_prefix(
    node: &mut AccessibilityNode,
    budget: &mut RefBudget<'_>,
) -> Result<bool, AppError> {
    if budget.exhausted {
        return Ok(false);
    }
    if let Some(ref_id) = node.ref_id.as_deref()
        && !budget.admit(ref_id)?
    {
        return Ok(false);
    }
    let mut kept_children = node.children.len();
    for (index, child) in node.children.iter_mut().enumerate() {
        if !keep_allocated_prefix(child, budget)? {
            kept_children = index;
            break;
        }
    }
    node.children.truncate(kept_children);
    if budget.exhausted {
        node.subtree_truncated = true;
    }
    Ok(true)
}

#[cfg(test)]
#[path = "refmap_fit_tests.rs"]
mod tests;
