use agent_desktop_core::{AdapterError, Deadline};
use serde_json::{Value, json};

use super::cdp_ax_node::CdpAxNode;
use super::dom_facts::DomFacts;
use super::dom_ids::read_dom_facts;
use super::frame_budget::FrameBudget;
use super::frame_build::build_frame;
use super::frame_context::FrameContext;
use super::frame_graft::FrameNodes;
use super::frame_tree::list_frames;
use crate::cdp::protocol_error::{is_protocol_error, is_target_gone};
use crate::transport::transport_error::transport_failed;
use crate::{CdpConnection, PageSession};

/// How one target's frames are read: the page's own target reads its main
/// frame within the snapshot deadline and its in-process iframes within the
/// iframe budget; an iframe target reads all of its frames within that budget.
#[derive(Clone, Copy)]
pub(crate) struct TargetReads {
    pub(crate) budget: FrameBudget,
    pub(crate) is_page: bool,
}

impl TargetReads {
    fn target_deadline(self) -> Deadline {
        if self.is_page {
            self.budget.snapshot()
        } else {
            self.budget.iframes()
        }
    }
}

/// Builds the nodes of every frame one target renders, in pre-order, and
/// lists the frames whose tree the target refused to read. A page's main
/// frame must be readable; any other frame that is not is skipped. When an
/// iframe target's own root frame is unreadable, every frame of that target
/// is listed and none is built.
pub(crate) fn read_target(
    connection: &mut CdpConnection,
    session: &PageSession,
    reads: TargetReads,
) -> Result<(Vec<FrameNodes>, Vec<String>), AdapterError> {
    let deadline = reads.target_deadline();
    let frames = list_frames(connection, session, deadline)?;
    let dom = match read_dom_facts(connection, session, deadline) {
        Ok(facts) => facts,
        Err(error) if is_protocol_error(&error) => DomFacts::default(),
        Err(error) => return Err(error),
    };
    let session_id = session.session_id.as_str();
    let mut built = Vec::with_capacity(frames.len());
    let mut unreadable = Vec::new();
    for (index, entry) in frames.iter().enumerate() {
        let in_page_iframe = reads.is_page && index > 0;
        if in_page_iframe && reads.budget.is_spent() {
            unreadable.push(entry.frame_id.clone());
            continue;
        }
        let frame_deadline = if in_page_iframe {
            reads.budget.iframes()
        } else {
            deadline
        };
        let reply = connection.call(
            "Accessibility.getFullAXTree",
            json!({ "frameId": entry.frame_id }),
            Some(session_id),
            frame_deadline,
        );
        let nodes = match reply {
            Ok(reply) => parse_nodes(reply)?,
            Err(error) if entry.parent_id.is_some() && is_protocol_error(&error) => {
                if index == 0 {
                    let all = frames.iter().map(|frame| frame.frame_id.clone()).collect();
                    return Ok((Vec::new(), all));
                }
                unreadable.push(entry.frame_id.clone());
                continue;
            }
            Err(error) if in_page_iframe && reads.budget.missed(&error) => {
                unreadable.push(entry.frame_id.clone());
                continue;
            }
            Err(error) => return Err(error),
        };
        let owner_backend_node_id = match index {
            0 => None,
            _ => match frame_owner(connection, session, &entry.frame_id, frame_deadline) {
                Ok(owner) => owner,
                Err(error) if in_page_iframe && reads.budget.missed(&error) => {
                    unreadable.push(entry.frame_id.clone());
                    continue;
                }
                Err(error) => return Err(error),
            },
        };
        let context = FrameContext {
            target_id: &session.target_id,
            frame_id: &entry.frame_id,
            loader_id: &entry.loader_id,
            dom: &dom,
        };
        built.push(FrameNodes {
            frame_id: entry.frame_id.clone(),
            parent_id: entry.parent_id.clone(),
            owner_backend_node_id,
            nodes: build_frame(nodes, &context),
        });
    }
    Ok((built, unreadable))
}

/// The backend node id of the element that holds `frame_id`, asked of the
/// target that renders that element. `None` when the target cannot say.
pub(crate) fn frame_owner(
    connection: &mut CdpConnection,
    session: &PageSession,
    frame_id: &str,
    deadline: Deadline,
) -> Result<Option<i64>, AdapterError> {
    match connection.call(
        "DOM.getFrameOwner",
        json!({ "frameId": frame_id }),
        Some(&session.session_id),
        deadline,
    ) {
        Ok(reply) => Ok(reply.get("backendNodeId").and_then(Value::as_i64)),
        Err(error) if is_protocol_error(&error) || is_target_gone(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

fn parse_nodes(mut reply: Value) -> Result<Vec<CdpAxNode>, AdapterError> {
    let nodes = reply.get_mut("nodes").map_or(Value::Null, Value::take);
    serde_json::from_value(nodes)
        .map_err(|error| transport_failed("Accessibility.getFullAXTree", error.to_string()))
}
