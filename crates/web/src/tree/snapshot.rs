use std::collections::HashMap;

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode};
use serde_json::{Value, json};

use super::cdp_ax_node::CdpAxNode;
use super::dom_ids::read_dom_ids;
use super::flatten::flatten;
use super::frame_build::build_frame;
use super::frame_context::FrameContext;
use super::frame_graft::{FrameNodes, graft_frames};
use super::frame_tree::list_frames;
use crate::cdp::protocol_error::is_protocol_error;
use crate::transport::transport_error::transport_failed;
use crate::{CdpConnection, PageSession, WebSnapshot};

/// Reads the full accessibility tree of every frame in the page, grafts
/// each child frame beneath its owner element, and returns it in core form.
pub fn snapshot_page(
    connection: &mut CdpConnection,
    session: &PageSession,
    deadline: Deadline,
) -> Result<WebSnapshot, AdapterError> {
    let session_id = session.session_id.as_str();
    let frames = list_frames(connection, session, deadline)?;
    let dom_ids = match read_dom_ids(connection, session, deadline) {
        Ok(ids) => ids,
        Err(error) if is_protocol_error(&error) => HashMap::new(),
        Err(error) => return Err(error),
    };
    let mut built = Vec::with_capacity(frames.len());
    let mut unreadable_frames = Vec::new();
    for (frame_id, parent_id) in frames {
        let reply = connection.call(
            "Accessibility.getFullAXTree",
            json!({ "frameId": frame_id }),
            Some(session_id),
            deadline,
        );
        let nodes = match reply {
            Ok(reply) => parse_nodes(reply)?,
            Err(error) if parent_id.is_some() && is_protocol_error(&error) => {
                unreadable_frames.push(frame_id);
                continue;
            }
            Err(error) => return Err(error),
        };
        let owner_backend_node_id = match parent_id {
            Some(_) => frame_owner(connection, session_id, &frame_id, deadline)?,
            None => None,
        };
        let context = FrameContext {
            target_id: &session.target_id,
            frame_id: &frame_id,
            dom_ids: &dom_ids,
        };
        let nodes = build_frame(nodes, &context);
        built.push(FrameNodes {
            frame_id,
            parent_id,
            owner_backend_node_id,
            nodes,
        });
    }
    let mut roots = graft_frames(built).into_iter();
    let (Some(root), None) = (roots.next(), roots.next()) else {
        return Err(no_tree(&session.target_id));
    };
    let mut origins = Vec::new();
    let tree = flatten(root, &mut origins);
    Ok(WebSnapshot {
        tree,
        origins,
        unreadable_frames,
    })
}

fn parse_nodes(mut reply: Value) -> Result<Vec<CdpAxNode>, AdapterError> {
    let nodes = reply.get_mut("nodes").map_or(Value::Null, Value::take);
    serde_json::from_value(nodes)
        .map_err(|error| transport_failed("Accessibility.getFullAXTree", error.to_string()))
}

fn frame_owner(
    connection: &mut CdpConnection,
    session_id: &str,
    frame_id: &str,
    deadline: Deadline,
) -> Result<Option<i64>, AdapterError> {
    match connection.call(
        "DOM.getFrameOwner",
        json!({ "frameId": frame_id }),
        Some(session_id),
        deadline,
    ) {
        Ok(reply) => Ok(reply.get("backendNodeId").and_then(Value::as_i64)),
        Err(error) if is_protocol_error(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

fn no_tree(target_id: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        format!("Page target {target_id} exposed no single accessibility tree root"),
    )
    .with_details(json!({ "kind": "cdp_tree_empty", "target_id": target_id }))
    .with_disposition(DeliverySemantics::not_delivered())
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
