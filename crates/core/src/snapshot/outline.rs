//! Indented text rendering of a serialized snapshot tree.
//!
//! The JSON tree repeats keys, punctuation and the snapshot id on every node,
//! and measured at about 2 bytes per model token. The outline carries the same
//! roles, names, values, states, refs and drill hints at roughly a third of the
//! tokens. Refs print in their local form (`[e12]`); the envelope's
//! `snapshot_id` qualifies them, and commands still accept only
//! `@<snapshot_id>:e12`.

use serde_json::Value;

/// Actions macOS and web runtimes advertise on nearly every node. They carry
/// no information on their own, so the outline leaves them out.
const UBIQUITOUS_ACTIONS: [&str; 3] = ["RightClick", "ScrollTo", "SetFocus"];
const INDENT: &str = "  ";

/// Replaces `data.tree` in a snapshot result body with `data.outline`.
/// A body without a tree is left unchanged.
pub fn replace_tree_with_outline(body: &mut Value) {
    let Some(object) = body.as_object_mut() else {
        return;
    };
    let Some(tree) = object.remove("tree") else {
        return;
    };
    object.insert("outline".to_string(), Value::String(render_outline(&tree)));
}

/// Renders one serialized `AccessibilityNode` and its descendants, one node
/// per line, indented two spaces per level.
pub fn render_outline(tree: &Value) -> String {
    let mut out = String::new();
    render_node(tree, 0, false, &mut out);
    out
}

fn render_node(node: &Value, depth: usize, inside_scroller: bool, out: &mut String) {
    if let Some(only_child) = collapsible_child(node) {
        render_node(only_child, depth, inside_scroller, out);
        return;
    }
    for _ in 0..depth {
        out.push_str(INDENT);
    }
    out.push_str(&node_line(node, inside_scroller));
    out.push('\n');
    let scrolls = inside_scroller || has_action(node, "Scroll");
    for child in children(node) {
        render_node(child, depth + 1, scrolls, out);
    }
}

fn node_line(node: &Value, inside_scroller: bool) -> String {
    let mut parts: Vec<String> = vec![role_text(node)];
    let name = text(node, "name");
    if let Some(name) = name {
        parts.push(quoted(name));
    }
    let has_ref = text(node, "ref_id").is_some();
    if let Some(local) = text(node, "ref_id").map(local_ref) {
        parts.push(format!("[{local}]"));
    }
    if let Some(value) = text(node, "value").filter(|value| Some(*value) != name) {
        parts.push(format!("= {}", quoted(value)));
    }
    for (key, label) in [
        ("description", "desc"),
        ("placeholder", "placeholder"),
        ("hint", "hint"),
        ("role_description", "roledesc"),
    ] {
        if let Some(extra) = text(node, key).filter(|extra| Some(*extra) != name) {
            parts.push(format!("{label}={}", quoted(extra)));
        }
    }
    if let Some(id) = node.pointer("/native_id/value").and_then(Value::as_str) {
        parts.push(format!("#{}", bare_or_quoted(id)));
    }
    let classes = strings(node, "dom_classes");
    if !classes.is_empty() {
        parts.push(format!(".{}", classes.join(".")));
    }
    let states = strings(node, "states");
    if !states.is_empty() {
        parts.push(format!("{{{}}}", states.join(",")));
    }
    let actions: Vec<&str> = strings(node, "available_actions")
        .into_iter()
        .filter(|action| shown_action(action, has_ref, inside_scroller, node))
        .collect();
    if !actions.is_empty() {
        parts.push(format!("({})", actions.join(",")));
    }
    if let Some(bounds) = node.get("bounds") {
        parts.push(bounds_text(bounds));
    }
    if let Some(count) = node.get("children_count").and_then(Value::as_u64) {
        parts.push(format!("+{count}"));
    } else if node.get("subtree_truncated").and_then(Value::as_bool) == Some(true) {
        parts.push("+…".to_string());
    }
    parts.join(" ")
}

/// A ref already says the node is clickable, so `Click` is shown only on a
/// node without one. macOS repeats a scroll area's `Scroll` on every leaf
/// inside it; a leaf inside a scroller drops it, while a nested container that
/// scrolls on its own keeps it.
fn shown_action(action: &str, has_ref: bool, inside_scroller: bool, node: &Value) -> bool {
    if UBIQUITOUS_ACTIONS.contains(&action) {
        return false;
    }
    match action {
        "Click" => !has_ref,
        "Scroll" => !inside_scroller || is_container(node),
        _ => true,
    }
}

fn is_container(node: &Value) -> bool {
    !children(node).is_empty() || node.get("children_count").is_some()
}

fn role_text(node: &Value) -> String {
    let role = text(node, "role").unwrap_or("?");
    match text(node, "subrole") {
        Some(subrole) => format!("{role}/{subrole}"),
        None => role.to_string(),
    }
}

/// The same wrapper the ref allocator collapses: a transparent-wrapper role
/// with blank identity, no ref and no states. Unlike the allocator it also
/// collapses a wrapper that advertises only ubiquitous actions, because macOS
/// groups carry those on every level and would otherwise never collapse.
fn collapsible_child(node: &Value) -> Option<&Value> {
    let role = text(node, "role")?;
    if !crate::Role::from_token(role).is_transparent_wrapper() {
        return None;
    }
    let informative = node
        .as_object()?
        .iter()
        .any(|(key, value)| match key.as_str() {
            "role" | "children" | "available_actions" | "bounds" => false,
            _ => !is_blank(value),
        });
    let only_ubiquitous_actions = strings(node, "available_actions")
        .iter()
        .all(|action| UBIQUITOUS_ACTIONS.contains(action));
    match children(node) {
        [only] if !informative && only_ubiquitous_actions => Some(only),
        _ => None,
    }
}

fn is_blank(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(text) => text.trim().is_empty(),
        Value::Array(items) => items.is_empty(),
        Value::Bool(flag) => !flag,
        _ => false,
    }
}

fn children(node: &Value) -> &[Value] {
    node.get("children")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn has_action(node: &Value, action: &str) -> bool {
    strings(node, "available_actions").contains(&action)
}

fn text<'a>(node: &'a Value, key: &str) -> Option<&'a str> {
    node.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
}

fn strings<'a>(node: &'a Value, key: &str) -> Vec<&'a str> {
    node.get(key)
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

fn local_ref(qualified: &str) -> &str {
    qualified.rsplit(':').next().unwrap_or(qualified)
}

fn quoted(raw: &str) -> String {
    Value::String(raw.to_string()).to_string()
}

/// An identifier made only of word characters, dots, colons and dashes prints
/// bare; anything else is quoted so it cannot read as a ref, state or action.
fn bare_or_quoted(raw: &str) -> String {
    let bare = raw
        .chars()
        .all(|ch| ch.is_alphanumeric() || matches!(ch, '_' | '.' | ':' | '-'));
    if bare { raw.to_string() } else { quoted(raw) }
}

fn bounds_text(bounds: &Value) -> String {
    let field = |key: &str| {
        bounds
            .get(key)
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            .round() as i64
    };
    format!(
        "@{},{} {}x{}",
        field("x"),
        field("y"),
        field("width"),
        field("height")
    )
}

#[cfg(test)]
#[path = "outline_tests.rs"]
mod tests;
