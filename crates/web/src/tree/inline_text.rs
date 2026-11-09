use agent_desktop_core::refs::ref_alloc::is_ref_able;
use agent_desktop_core::{AccessibilityNode, NameSource};

use super::inline_run::InlineRun;
use super::web_node::WebNode;

const INLINE_CONTROL_ROLES: &[&str] = &["link", "button"];

/// How the node whose children are merged is laid out: whether its DOM node
/// starts its own box, and whether it is a wrapper about to be spliced out.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BlockLayout {
    pub(crate) block_level: bool,
    pub(crate) transparent: bool,
}

/// Reads the static text among `children` into as few nodes as carry it.
/// A named node whose children are text runs and inline links or buttons,
/// and whose name already holds that text, loses the runs and keeps the
/// controls. An unnamed block-level node whose children are all inline is
/// named by its text in reading order, link text included, as a derived
/// name that never enters ref identity, and keeps only its controls. A
/// wrapper about to be spliced out takes that name only when it holds a
/// control, because only then does it need a node of its own. Anywhere
/// else, adjacent runs inside one block become one run.
pub(crate) fn merge_inline_text(
    built: &mut WebNode,
    children: &mut Vec<WebNode>,
    layout: BlockLayout,
) {
    if !children.iter().any(is_visible_run) || !children.iter().all(is_inline) {
        merge_adjacent_runs(children);
        return;
    }
    if let Some(name) = display_name(&built.node) {
        if squeeze(&reading_text(children)) == squeeze(name) {
            children.retain(|child| child.run.is_none());
        } else {
            merge_adjacent_runs(children);
        }
        return;
    }
    let crosses_block = children
        .iter()
        .any(|child| child.run == Some(InlineRun::BlockEdge));
    let holds_control = children.iter().any(|child| child.run.is_none());
    if !layout.block_level
        || crosses_block
        || is_ref_able(&built.node)
        || (layout.transparent && !holds_control)
    {
        merge_adjacent_runs(children);
        return;
    }
    let text = collapse_whitespace(&reading_text(children));
    children.retain(|child| child.run.is_none());
    let descriptors = &mut built.node.presentation.descriptors;
    descriptors.name_source = Some(NameSource::ContainedText);
    descriptors.derived_name = Some(text);
}

/// Marks the edges of a block-level node spliced out of the tree, so the
/// text before, inside and after it never merges into one run.
pub(crate) fn seal_block(mut children: Vec<WebNode>, block_level: bool) -> Vec<WebNode> {
    if block_level && !children.is_empty() {
        children.insert(0, marker(InlineRun::BlockEdge));
        children.push(marker(InlineRun::BlockEdge));
    }
    children
}

/// Removes the markers and blank text runs a node's children still carry,
/// once the node is emitted and its children are no longer merged.
pub(crate) fn drop_markers(children: &mut Vec<WebNode>) {
    children.retain(|child| match &child.run {
        Some(InlineRun::Text(text)) => !text.trim().is_empty(),
        Some(InlineRun::BlockEdge) => false,
        None => true,
    });
}

/// A line break, which reads as a space between the runs around it.
pub(crate) fn line_break() -> WebNode {
    marker(InlineRun::Text("\n".to_owned()))
}

fn marker(run: InlineRun) -> WebNode {
    WebNode {
        node: AccessibilityNode {
            ref_id: None,
            role: "statictext".to_owned(),
            identity: Default::default(),
            presentation: Default::default(),
            children_count: None,
            subtree_truncated: false,
            children: Vec::new(),
        },
        origin: None,
        children: Vec::new(),
        run: Some(run),
    }
}

fn merge_adjacent_runs(children: &mut Vec<WebNode>) {
    let mut merged: Vec<WebNode> = Vec::with_capacity(children.len());
    for child in children.drain(..) {
        let Some(InlineRun::Text(next)) = &child.run else {
            merged.push(child);
            continue;
        };
        match merged.last_mut() {
            Some(WebNode {
                node,
                run: Some(InlineRun::Text(text)),
                ..
            }) => {
                text.push_str(next);
                let name = collapse_whitespace(text);
                node.identity.name = (!name.is_empty()).then_some(name);
            }
            _ => merged.push(child),
        }
    }
    *children = merged;
}

fn is_visible_run(node: &WebNode) -> bool {
    matches!(&node.run, Some(InlineRun::Text(text)) if !text.trim().is_empty())
}

fn is_inline(node: &WebNode) -> bool {
    if node.run.is_some() {
        return true;
    }
    let inner = &node.node;
    node.children.is_empty()
        && (INLINE_CONTROL_ROLES.contains(&inner.role.as_str())
            || (inner.presentation.descriptors.name_source == Some(NameSource::ContainedText)
                && is_ref_able(inner)))
}

fn display_name(node: &AccessibilityNode) -> Option<&str> {
    node.identity
        .name
        .as_deref()
        .or(node.presentation.descriptors.derived_name.as_deref())
        .filter(|name| !name.trim().is_empty())
}

fn reading_text(children: &[WebNode]) -> String {
    let mut text = String::new();
    for child in children {
        match &child.run {
            Some(InlineRun::Text(run)) => text.push_str(run),
            Some(InlineRun::BlockEdge) => text.push(' '),
            None => text.push_str(display_name(&child.node).unwrap_or_default()),
        }
    }
    text
}

fn squeeze(text: &str) -> String {
    text.split_whitespace().collect()
}

pub(crate) fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
#[path = "inline_text_tests.rs"]
mod tests;
