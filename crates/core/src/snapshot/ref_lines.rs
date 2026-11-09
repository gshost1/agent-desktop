//! The `--lines` reading form of a snapshot tree: one line per node, the ref
//! first, then role and name, so the ref an agent copies sits beside the name
//! it reads. `docs/json-output.md` holds the grammar.

use std::fmt::Write as _;

use serde::Deserialize;
use serde_json::Value;

use crate::{AccessibilityNode, AppError, Rect, tree::search_text::is_bidirectional_control};

const INDENT: &str = "  ";

/// Renders `tree` as one line per node in depth-first order, the order refs
/// are allocated in. Every node gets a line, with or without a ref.
pub fn render_ref_lines(tree: &AccessibilityNode) -> Vec<String> {
    let mut lines = Vec::new();
    write_node(tree, 0, &mut lines);
    lines
}

/// Replaces `data.tree` with `data.lines` and leaves every other key as it
/// was. Returns `false`, changing nothing, when `data` holds no tree; a
/// tree that does not read back leaves `data` unchanged and is an error.
pub fn replace_tree_with_lines(data: &mut Value) -> Result<bool, AppError> {
    let Some(body) = data.as_object_mut() else {
        return Ok(false);
    };
    let Some(tree) = body.get("tree") else {
        return Ok(false);
    };
    let tree = AccessibilityNode::deserialize(tree)?;
    body.remove("tree");
    body.insert("lines".into(), Value::from(render_ref_lines(&tree)));
    Ok(true)
}

fn write_node(node: &AccessibilityNode, depth: usize, lines: &mut Vec<String>) {
    lines.push(node_line(node, depth));
    for child in &node.children {
        write_node(child, depth + 1, lines);
    }
}

fn node_line(node: &AccessibilityNode, depth: usize) -> String {
    let mut line = INDENT.repeat(depth);
    if let Some(ref_id) = &node.ref_id {
        line.push_str(ref_id);
        line.push(' ');
    }
    line.push_str(&node.role);
    if let Some(name) = &node.identity.name {
        line.push(' ');
        line.push_str(&quote_text(name));
    }
    push_field(&mut line, "value", node.identity.value.as_deref());
    push_field(&mut line, "desc", distinct_description(node));
    let descriptors = &node.presentation.descriptors;
    push_field(&mut line, "placeholder", descriptors.placeholder.as_deref());
    push_field(&mut line, "hint", node.presentation.hint.as_deref());
    if let Some(bounds) = node.presentation.bounds {
        push_bounds(&mut line, bounds);
    }
    if !node.presentation.states.is_empty() {
        let _ = write!(line, " [{}]", node.presentation.states.join(", "));
    }
    if let Some(count) = node.children_count {
        let _ = write!(line, " children_count={count}");
    }
    if node.subtree_truncated {
        line.push_str(" subtree_truncated");
    }
    line
}

/// The description when it says something the name does not: a description
/// equal to the name, as on links whose title repeats their text, is left out.
fn distinct_description(node: &AccessibilityNode) -> Option<&str> {
    let description = node.identity.description.as_deref()?;
    (node.identity.name.as_deref() != Some(description)).then_some(description)
}

fn push_field(line: &mut String, key: &str, text: Option<&str>) {
    if let Some(text) = text {
        let _ = write!(line, " {key}={}", quote_text(text));
    }
}

fn push_bounds(line: &mut String, bounds: Rect) {
    let _ = write!(
        line,
        " bounds={},{},{}x{}",
        bounds.x, bounds.y, bounds.width, bounds.height
    );
}

/// Quotes `text` on one line: `"` and `\` are escaped, line breaks, tabs,
/// other control characters and the Unicode line and paragraph separators
/// become escapes, and the bidirectional controls
/// that text matching already ignores are dropped.
pub(crate) fn quote_text(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');
    for character in text.chars().filter(|c| !is_bidirectional_control(*c)) {
        match character {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            c if c.is_control() || matches!(c, '\u{2028}' | '\u{2029}') => {
                let _ = write!(quoted, "\\u{:04x}", u32::from(c));
            }
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}

#[cfg(test)]
#[path = "ref_lines_tests.rs"]
mod tests;
