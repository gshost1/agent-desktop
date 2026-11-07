use std::collections::HashMap;

use agent_desktop_core::tree::state;
use agent_desktop_core::{AccessibilityNode, AdapterError, Deadline};
use serde_json::{Value, json};

use super::cdp_ax_node::CdpAxNode;
use super::frame_build::convert;
use super::frame_context::FrameContext;
use crate::act::remote_element::ensure_live;
use crate::transport::transport_error::transport_failed;
use crate::{CdpConnection, PageSession};

/// Re-reads one DOM node's accessibility node: its mapped role, name, value,
/// states and the capabilities this crate can deliver to it, in the form a
/// snapshot gave it. A node the page dropped is `STALE_REF`. A node the
/// browser leaves out of its accessibility tree, such as one hidden by CSS,
/// reads as hidden.
pub fn read_node(
    connection: &mut CdpConnection,
    session: &PageSession,
    backend_node_id: i64,
    deadline: Deadline,
) -> Result<AccessibilityNode, AdapterError> {
    ensure_live(connection, session, backend_node_id, deadline)?;
    let mut reply = connection.call(
        "Accessibility.getPartialAXTree",
        json!({ "backendNodeId": backend_node_id, "fetchRelatives": false }),
        Some(&session.session_id),
        deadline,
    )?;
    let nodes: Vec<CdpAxNode> = serde_json::from_value(
        reply.get_mut("nodes").map_or(Value::Null, Value::take),
    )
    .map_err(|error| transport_failed("Accessibility.getPartialAXTree", error.to_string()))?;
    let node = nodes
        .iter()
        .find(|node| node.backend_dom_node_id == Some(backend_node_id))
        .or_else(|| nodes.first())
        .ok_or_else(|| transport_failed("Accessibility.getPartialAXTree", "reply holds no node"))?;
    let dom_ids = HashMap::new();
    let context = FrameContext {
        target_id: &session.target_id,
        frame_id: "",
        dom_ids: &dom_ids,
    };
    let mut built = convert(node, &node.raw_role(), &context).node;
    if node.ignored && !state::has_state(&built.presentation.states, state::HIDDEN) {
        built.presentation.states.push(state::HIDDEN.to_owned());
    }
    Ok(built)
}

#[cfg(test)]
#[path = "node_read_tests.rs"]
mod tests;
