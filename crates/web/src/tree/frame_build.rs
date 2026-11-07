use std::collections::{HashMap, HashSet};

use agent_desktop_core::actions::capability;
use agent_desktop_core::tree::roles::is_interactive_role;
use agent_desktop_core::{
    AccessibilityNode, ElementIdentifier, IdentifierKind, NodeIdentity, NodePresentation,
};

use super::ax_value::AxValue;
use super::cdp_ax_node::CdpAxNode;
use super::frame_context::FrameContext;
use super::node_actions::actions_of;
use super::node_states::states_of;
use super::web_node::WebNode;
use super::web_roles::{is_frame_owner_role, is_text_fragment_role, map_cdp_role};
use crate::NodeOrigin;

/// Builds one frame's `Accessibility.getFullAXTree` result into nodes.
/// Ignored nodes and transparent wrappers are spliced out, keeping their
/// children; text fragments are dropped with their subtrees; a static-text
/// child that only repeats its parent's name is dropped. Static text and
/// text fields are leaves, as the native AX path exposes them: their
/// children are inline text boxes and the field's inner editor.
pub(crate) fn build_frame(nodes: Vec<CdpAxNode>, context: &FrameContext<'_>) -> Vec<WebNode> {
    let children: HashSet<&str> = nodes
        .iter()
        .flat_map(|node| node.child_ids.iter().map(String::as_str))
        .collect();
    let Some(root_id) = nodes
        .iter()
        .find(|node| !children.contains(node.node_id.as_str()))
        .map(|node| node.node_id.clone())
    else {
        return Vec::new();
    };
    let by_id: HashMap<&str, &CdpAxNode> = nodes
        .iter()
        .map(|node| (node.node_id.as_str(), node))
        .collect();
    let mut ancestors = HashSet::new();
    build_node(&root_id, &by_id, context, &mut ancestors)
}

type NodeIndex<'a> = HashMap<&'a str, &'a CdpAxNode>;

fn build_node(
    node_id: &str,
    by_id: &NodeIndex<'_>,
    context: &FrameContext<'_>,
    ancestors: &mut HashSet<String>,
) -> Vec<WebNode> {
    let Some(node) = by_id.get(node_id).copied() else {
        return Vec::new();
    };
    if ancestors.contains(node_id) {
        return Vec::new();
    }
    let raw_role = node.raw_role();
    if is_text_fragment_role(&raw_role) {
        return Vec::new();
    }
    ancestors.insert(node_id.to_owned());
    let mut children: Vec<WebNode> = node
        .child_ids
        .iter()
        .flat_map(|child| build_node(child, by_id, context, ancestors))
        .collect();
    ancestors.remove(node_id);
    if node.ignored {
        return children;
    }
    let built = convert(node, &raw_role, context);
    if matches!(built.node.role.as_str(), "statictext" | "textfield") {
        children.clear();
    } else if let Some(name) = built.node.identity.name.as_deref() {
        children.retain(|child| {
            !(child.node.role == "statictext"
                && child.node.identity.name.as_deref() == Some(name)
                && child.children.is_empty())
        });
    }
    if is_transparent(&built, &raw_role) {
        return children;
    }
    vec![WebNode { children, ..built }]
}

fn convert(node: &CdpAxNode, raw_role: &str, context: &FrameContext<'_>) -> WebNode {
    let role = map_cdp_role(raw_role);
    let text = |value: &Option<AxValue>| value.as_ref().and_then(AxValue::as_text);
    let origin = node.backend_dom_node_id.map(|backend_node_id| NodeOrigin {
        target_id: context.target_id.to_owned(),
        frame_id: context.frame_id.to_owned(),
        backend_node_id,
    });
    let available_actions = actions_of(role, node);
    let retained_object = origin
        .as_ref()
        .filter(|_| is_interactive_role(role) || !available_actions.is_empty())
        .map(|origin| origin.element_token().to_string());
    let native_id = node
        .backend_dom_node_id
        .and_then(|backend| context.dom_ids.get(&backend))
        .map(|value| ElementIdentifier {
            kind: IdentifierKind::AxDomIdentifier,
            value: value.clone(),
        });
    let identity = NodeIdentity {
        retained_object,
        name: text(&node.text.name),
        value: text(&node.text.value),
        description: text(&node.text.description),
        native_id,
    };
    let presentation = NodePresentation {
        states: states_of(node),
        available_actions,
        ..NodePresentation::default()
    };
    WebNode {
        node: AccessibilityNode {
            ref_id: None,
            role: role.to_owned(),
            identity,
            presentation,
            children_count: None,
            subtree_truncated: false,
            children: Vec::new(),
        },
        origin,
        children: Vec::new(),
    }
}

fn is_transparent(built: &WebNode, raw_role: &str) -> bool {
    let identity = &built.node.identity;
    let presentation = &built.node.presentation;
    built.node.role == "group"
        && !is_frame_owner_role(raw_role)
        && identity.name.is_none()
        && identity.value.is_none()
        && identity.description.is_none()
        && identity.native_id.is_none()
        && presentation.states.is_empty()
        && presentation
            .available_actions
            .iter()
            .all(|action| action == capability::SET_FOCUS)
}

#[cfg(test)]
#[path = "frame_build_tests.rs"]
mod tests;
