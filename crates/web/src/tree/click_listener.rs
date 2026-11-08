use agent_desktop_core::{AdapterError, Deadline};
use serde_json::{Value, json};

use super::dom_ids::collect_clickable;
use crate::cdp::protocol_error::is_protocol_error;
use crate::{CdpConnection, PageSession};

const OBJECT_GROUP: &str = "agent-desktop-listeners";
const MOUSE_BUTTON_EVENTS: [&str; 3] = ["click", "mousedown", "mouseup"];

/// Whether Chromium reports the DOM node as clickable, the signal a snapshot
/// promotes a node by. A `click`, `mousedown` or `mouseup` listener of the
/// node's own answers yes at the cost of one listener read; any other node is
/// looked up in the `isClickable` list of a fresh `DOMSnapshot.captureSnapshot`,
/// read through the same filter a snapshot uses. A node the page cannot
/// resolve, or a read the browser refuses, counts as not clickable, so the
/// node advertises no click.
pub(crate) fn responds_to_clicks(
    connection: &mut CdpConnection,
    session: &PageSession,
    backend_node_id: i64,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    if listens_for_clicks(connection, session, backend_node_id, deadline)? {
        return Ok(true);
    }
    match connection.call(
        "DOMSnapshot.captureSnapshot",
        json!({ "computedStyles": [] }),
        Some(&session.session_id),
        deadline,
    ) {
        Ok(capture) => Ok(collect_clickable(&capture).contains(&backend_node_id)),
        Err(error) if is_protocol_error(&error) => Ok(false),
        Err(error) => Err(error),
    }
}

fn listens_for_clicks(
    connection: &mut CdpConnection,
    session: &PageSession,
    backend_node_id: i64,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let session_id = Some(session.session_id.as_str());
    let resolved = connection.call(
        "DOM.resolveNode",
        json!({ "backendNodeId": backend_node_id, "objectGroup": OBJECT_GROUP }),
        session_id,
        deadline,
    );
    let object_id = match resolved {
        Ok(reply) => reply
            .pointer("/object/objectId")
            .and_then(Value::as_str)
            .map(str::to_owned),
        Err(error) if is_protocol_error(&error) => None,
        Err(error) => return Err(error),
    };
    let Some(object_id) = object_id else {
        return Ok(false);
    };
    let listeners = connection.call(
        "DOMDebugger.getEventListeners",
        json!({ "objectId": object_id }),
        session_id,
        deadline,
    );
    let _ = connection.call(
        "Runtime.releaseObjectGroup",
        json!({ "objectGroup": OBJECT_GROUP }),
        session_id,
        deadline,
    );
    match listeners {
        Ok(reply) => Ok(has_mouse_button_listener(&reply)),
        Err(error) if is_protocol_error(&error) => Ok(false),
        Err(error) => Err(error),
    }
}

fn has_mouse_button_listener(reply: &Value) -> bool {
    reply
        .get("listeners")
        .and_then(Value::as_array)
        .is_some_and(|listeners| {
            listeners.iter().any(|listener| {
                listener
                    .get("type")
                    .and_then(Value::as_str)
                    .is_some_and(|kind| MOUSE_BUTTON_EVENTS.contains(&kind))
            })
        })
}

#[cfg(test)]
#[path = "click_listener_tests.rs"]
mod tests;
