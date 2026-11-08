use agent_desktop_core::actions::capability;
use agent_desktop_core::refs::ref_alloc::{is_ref_able, is_ref_able_role_actions};
use agent_desktop_core::{AccessibilityNode, NameSource};

use super::web_node::WebNode;
use super::web_roles::is_frame_owner_role;

const CONTAINED_TEXT_CAP: usize = 200;

const CONTAINER_ROLES: &[&str] = &[
    "webarea",
    "statictext",
    "list",
    "table",
    "grid",
    "outline",
    "tablist",
    "menu",
    "toolbar",
    "radiogroup",
    "form",
    "main",
    "navigation",
    "banner",
    "contentinfo",
    "complementary",
    "region",
    "search",
    "article",
    "document",
    "dialog",
    "alertdialog",
];

/// Whether a node of `role` may take a click it does not advertise because
/// the page listens for clicks on its DOM node. Nodes that already earn a
/// ref keep what they advertise. Text runs, frame owners, and containers of
/// items or regions are never click targets of their own: a list or a
/// landmark that listens for clicks is delegating them to its items, and a
/// click at its centre would land on whichever item sits there.
pub(crate) fn may_take_click(role: &str, raw_role: &str, available_actions: &[String]) -> bool {
    !CONTAINER_ROLES.contains(&role)
        && !is_frame_owner_role(raw_role)
        && !is_ref_able_role_actions(role, available_actions)
}

/// Advertises `Click` on a node whose DOM node responds to clicks while its
/// accessibility node exposes no control, as a `<div>` or `<span>` with a
/// click listener reads, and gives it the element token every web ref
/// carries. A node with a ref-able descendant is a container whose controls
/// are the targets, so it stays as it is. A promoted node with no accessible
/// name is named by the text it contains, marked as derived so that ref
/// identity never reads it; a lone static-text child repeating that text is
/// dropped.
pub(crate) fn promote_clickable(built: &mut WebNode, children: &mut Vec<WebNode>, raw_role: &str) {
    let node = &built.node;
    if !may_take_click(&node.role, raw_role, &node.presentation.available_actions)
        || children.iter().any(holds_ref_able)
    {
        return;
    }
    let Some(token) = built.origin.as_ref().map(|origin| origin.element_token()) else {
        return;
    };
    grant_click(&mut built.node, token.to_string());
    let node = &mut built.node;
    if node
        .identity
        .name
        .as_deref()
        .is_some_and(|name| !name.trim().is_empty())
    {
        return;
    }
    let Some(text) = contained_text(children) else {
        return;
    };
    if let [only] = children.as_slice()
        && only.node.role == "statictext"
        && only.children.is_empty()
        && only
            .node
            .identity
            .name
            .as_deref()
            .is_some_and(|name| collapse_whitespace(name) == text)
    {
        children.clear();
    }
    node.identity.name = None;
    node.presentation.descriptors.name_source = Some(NameSource::ContainedText);
    node.presentation.descriptors.derived_name = Some(text);
}

/// Adds `Click` ahead of the node's other actions and records its token.
pub(crate) fn grant_click(node: &mut AccessibilityNode, token: String) {
    node.presentation
        .available_actions
        .insert(0, capability::CLICK.to_owned());
    node.identity.retained_object = Some(token);
}

fn holds_ref_able(node: &WebNode) -> bool {
    is_ref_able(&node.node) || node.children.iter().any(holds_ref_able)
}

fn contained_text(children: &[WebNode]) -> Option<String> {
    let mut parts = Vec::new();
    let mut length = 0;
    for child in children {
        collect_leaf_text(child, &mut parts, &mut length);
    }
    let text: String = collapse_whitespace(&parts.join(" "))
        .chars()
        .take(CONTAINED_TEXT_CAP)
        .collect();
    let text = text.trim_end().to_owned();
    (!text.is_empty()).then_some(text)
}

fn collect_leaf_text<'a>(node: &'a WebNode, parts: &mut Vec<&'a str>, length: &mut usize) {
    if *length > CONTAINED_TEXT_CAP {
        return;
    }
    if node.children.is_empty() {
        if let Some(name) = node.node.identity.name.as_deref() {
            *length += name.len();
            parts.push(name);
        }
        return;
    }
    for child in &node.children {
        collect_leaf_text(child, parts, length);
    }
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
#[path = "click_promotion_tests.rs"]
mod tests;
