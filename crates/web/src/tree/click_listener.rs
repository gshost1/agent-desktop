use agent_desktop_core::{AdapterError, Deadline};
use serde_json::{Value, json};

use crate::cdp::protocol_error::is_protocol_error;
use crate::{CdpConnection, PageSession};

const OBJECT_GROUP: &str = "agent-desktop-listeners";
const MOUSE_BUTTON_EVENTS: [&str; 3] = ["click", "mousedown", "mouseup"];

/// Whether the DOM node has a `click`, `mousedown` or `mouseup` listener of
/// its own, the listeners Chromium's DOM snapshot counts when it reports a
/// node as clickable. A node the page cannot resolve, or a listener read the
/// browser refuses, counts as having none, so the node advertises no click.
pub(crate) fn listens_for_clicks(
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
