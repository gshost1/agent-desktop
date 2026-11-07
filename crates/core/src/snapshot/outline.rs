//! Indented text rendering of a serialized snapshot tree.
//!
//! The JSON tree repeats keys, punctuation and the snapshot id on every node,
//! and measured at about 2 bytes per model token. The outline carries the same
//! roles, names, values, states, refs and drill hints at roughly a third of the
//! tokens. Refs print in their local form (`[e12]`); the envelope's
//! `snapshot_id` qualifies them, and commands still accept only
//! `@<snapshot_id>:e12`.

use serde_json::Value;

const IMPLIED_ACTIONS: [&str; 4] = ["Click", "RightClick", "ScrollTo", "SetFocus"];
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

fn render_node(node: &Value, depth: usize, parent_scrolls: bool, out: &mut String) {
    if let Some(only_child) = collapsible_child(node) {
        render_node(only_child, depth, parent_scrolls, out);
        return;
    }
    let scrolls = has_action(node, "Scroll");
    for _ in 0..depth {
        out.push_str(INDENT);
    }
    out.push_str(&node_line(node, parent_scrolls));
    out.push('\n');
    for child in children(node) {
        render_node(child, depth + 1, scrolls || parent_scrolls, out);
    }
}

fn node_line(node: &Value, parent_scrolls: bool) -> String {
    let mut parts: Vec<String> = vec![text(node, "role").unwrap_or("?").to_string()];
    let name = text(node, "name");
    if let Some(name) = name {
        parts.push(quoted(name));
    }
    if let Some(local) = text(node, "ref_id").map(local_ref) {
        parts.push(format!("[{local}]"));
    }
    if let Some(value) = text(node, "value").filter(|value| Some(*value) != name) {
        parts.push(format!("= {}", quoted(value)));
    }
    if let Some(description) = text(node, "description").filter(|d| Some(*d) != name) {
        parts.push(format!("desc={}", quoted(description)));
    }
    for (key, label) in [("placeholder", "placeholder"), ("hint", "hint")] {
        if let Some(extra) = text(node, key) {
            parts.push(format!("{label}={}", quoted(extra)));
        }
    }
    if let Some(id) = node.pointer("/native_id/value").and_then(Value::as_str) {
        parts.push(format!("#{id}"));
    }
    let states = strings(node, "states");
    if !states.is_empty() {
        parts.push(format!("{{{}}}", states.join(",")));
    }
    let actions: Vec<&str> = strings(node, "available_actions")
        .into_iter()
        .filter(|action| !IMPLIED_ACTIONS.contains(action))
        .filter(|action| !(parent_scrolls && *action == "Scroll"))
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

/// A ref-less, unnamed, stateless group with exactly one child adds a line
/// and a level of indentation but no information the agent can use.
fn collapsible_child(node: &Value) -> Option<&Value> {
    let object = node.as_object()?;
    if text(node, "role") != Some("group") {
        return None;
    }
    let informative = object.keys().any(|key| {
        !matches!(
            key.as_str(),
            "role" | "children" | "available_actions" | "bounds"
        )
    });
    let only_implied_actions = strings(node, "available_actions")
        .iter()
        .all(|action| IMPLIED_ACTIONS.contains(action));
    match children(node) {
        [only] if !informative && only_implied_actions => Some(only),
        _ => None,
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
        .filter(|value| !value.is_empty())
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
