//! `--compact` presentation: which single-child wrappers a snapshot drops.

use crate::AccessibilityNode;

/// A wrapper is collapsible when it carries nothing a caller reads: a
/// transparent role, no ref, no identity text, no hint, state, action, bounds
/// or truncation marker, and exactly one child. Callers judge a node after
/// its own presentation is shaped, so bounds hidden by the snapshot and
/// children pruned by `-i` no longer count against it.
pub(crate) fn is_collapsible(node: &AccessibilityNode) -> bool {
    crate::Role::from_token(&node.role).is_transparent_wrapper()
        && node.ref_id.is_none()
        && node
            .identity
            .name
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        && node
            .identity
            .value
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        && node
            .identity
            .description
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        && node.identity.native_id.is_none()
        && node
            .presentation
            .hint
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        && node.presentation.states.is_empty()
        && node.presentation.available_actions.is_empty()
        && node.presentation.bounds.is_none()
        && node.children_count.is_none()
        && node.children.len() == 1
}

#[cfg(test)]
#[path = "ref_alloc_compact_tests.rs"]
mod tests;
