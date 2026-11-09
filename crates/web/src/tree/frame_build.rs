use std::collections::{HashMap, HashSet};

use agent_desktop_core::actions::capability;
use agent_desktop_core::tree::roles::is_interactive_role;
use agent_desktop_core::{
    AccessibilityNode, ElementIdentifier, IdentifierKind, NodeIdentity, NodePresentation,
};

use super::ax_value::AxValue;
use super::cdp_ax_node::CdpAxNode;
use super::click_promotion::promote_clickable;
use super::frame_context::FrameContext;
use super::inline_run::InlineRun;
use super::inline_text::{BlockLayout, drop_markers, line_break, merge_inline_text, seal_block};
use super::node_actions::actions_of;
use super::node_states::states_of;
use super::web_node::WebNode;
use super::web_roles::{
    is_frame_owner_role, is_line_break_role, is_text_fragment_role, map_cdp_role,
};
use crate::NodeOrigin;

/// Builds one frame's `Accessibility.getFullAXTree` result into nodes.
/// Ignored nodes and transparent wrappers are spliced out, keeping their
/// children; text fragments are dropped with their subtrees; a static-text
/// child that only repeats its parent's name is dropped. Static text and
/// text fields are leaves, as the native AX path exposes them: their
/// children are inline text boxes and the field's inner editor. A node the
/// DOM snapshot reports as clickable, whose accessibility node exposes no
/// control, takes a click (see [`promote_clickable`]). The static text inside
/// one block is read into as few nodes as carry it, never across the edge
/// of a block-level element (see [`merge_inline_text`]).
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
    let mut roots = build_node(&root_id, &by_id, context, &mut ancestors);
    drop_markers(&mut roots);
    roots
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
    if is_line_break_role(&raw_role) {
        return vec![line_break()];
    }
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
        return seal_block(children, lays_out_block(node.backend_dom_node_id, context));
    }
    let mut built = convert(node, &raw_role, context);
    if built
        .origin
        .as_ref()
        .is_some_and(|origin| context.dom.clickable.contains(&origin.backend_node_id))
    {
        let owns_frame = holds_frame_owner(node, by_id);
        promote_clickable(&mut built, &mut children, &raw_role, owns_frame);
    }
    if matches!(built.node.role.as_str(), "statictext" | "textfield") {
        children.clear();
    } else {
        if let Some(name) = built.node.identity.name.as_deref() {
            children.retain(|child| {
                !(child.node.role == "statictext"
                    && child.node.identity.name.as_deref() == Some(name)
                    && child.children.is_empty())
            });
        }
        let block_level = lays_out_block(node.backend_dom_node_id, context);
        let layout = BlockLayout {
            block_level,
            transparent: is_transparent(&built, &raw_role, block_level),
        };
        merge_inline_text(&mut built, &mut children, layout);
        if is_transparent(&built, &raw_role, block_level) {
            return seal_block(children, layout.block_level);
        }
    }
    drop_markers(&mut children);
    vec![WebNode { children, ..built }]
}

/// Whether the DOM node starts its own box rather than sitting inside a line
/// of text. A node the DOM capture did not lay out inline counts as a block,
/// so text merges only where the capture shows it may.
fn lays_out_block(backend_node_id: Option<i64>, context: &FrameContext<'_>) -> bool {
    backend_node_id.is_none_or(|backend| !context.dom.inline.contains(&backend))
}

/// Whether a frame-owner element sits anywhere below `node` in the frame's
/// accessibility tree. A child frame's tree is grafted beneath its owner only
/// after every frame is built, so the built children cannot show its controls.
fn holds_frame_owner(node: &CdpAxNode, by_id: &NodeIndex<'_>) -> bool {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut pending: Vec<&str> = node.child_ids.iter().map(String::as_str).collect();
    while let Some(id) = pending.pop() {
        if !seen.insert(id) {
            continue;
        }
        let Some(child) = by_id.get(id) else {
            continue;
        };
        if is_frame_owner_role(&child.raw_role()) {
            return true;
        }
        pending.extend(child.child_ids.iter().map(String::as_str));
    }
    false
}

pub(crate) fn convert(node: &CdpAxNode, raw_role: &str, context: &FrameContext<'_>) -> WebNode {
    let role = map_cdp_role(raw_role);
    let text = |value: &Option<AxValue>| value.as_ref().and_then(AxValue::as_text);
    let origin = node.backend_dom_node_id.map(|backend_node_id| NodeOrigin {
        target_id: context.target_id.to_owned(),
        frame_id: context.frame_id.to_owned(),
        loader_id: context.loader_id.to_owned(),
        backend_node_id,
    });
    let available_actions = actions_of(role, node);
    let retained_object = origin
        .as_ref()
        .filter(|_| is_interactive_role(role) || !available_actions.is_empty())
        .map(|origin| origin.element_token().to_string());
    let native_id = node
        .backend_dom_node_id
        .and_then(|backend| context.dom.ids.get(&backend))
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
    let run = (role == "statictext").then(|| {
        InlineRun::Text(
            node.text
                .name
                .as_ref()
                .and_then(AxValue::as_untrimmed_text)
                .unwrap_or_default(),
        )
    });
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
        run,
    }
}

fn is_transparent(built: &WebNode, raw_role: &str, block_level: bool) -> bool {
    let identity = &built.node.identity;
    let presentation = &built.node.presentation;
    built.node.role == "group"
        && !is_frame_owner_role(raw_role)
        && identity.name.is_none()
        && identity.value.is_none()
        && identity.description.is_none()
        && (identity.native_id.is_none() || !block_level)
        && presentation.descriptors.derived_name.is_none()
        && presentation.states.is_empty()
        && presentation
            .available_actions
            .iter()
            .all(|action| action == capability::SET_FOCUS)
}

#[cfg(test)]
#[path = "frame_build_tests.rs"]
mod tests;
