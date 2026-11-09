use agent_desktop_core::{AdapterError, Deadline};
use serde_json::{Value, json};

use crate::transport::transport_error::transport_failed;
use crate::{CdpConnection, PageSession};

const WORLD_NAME: &str = "agent-desktop-visibility";

/// Reads the page's `document.visibilityState` (`visible` or `hidden`). The
/// expression runs in an isolated world created for the main frame, so page
/// scripts cannot observe or spoof it, and the runtime domain is never
/// enabled.
pub fn page_visibility(
    connection: &mut CdpConnection,
    session: &PageSession,
    deadline: Deadline,
) -> Result<String, AdapterError> {
    let session_id = Some(session.session_id.as_str());
    let tree = connection.call("Page.getFrameTree", json!({}), session_id, deadline)?;
    let frame_id = tree
        .pointer("/frameTree/frame/id")
        .and_then(Value::as_str)
        .ok_or_else(|| transport_failed("Page.getFrameTree", "reply names no main frame"))?;
    let world = connection.call(
        "Page.createIsolatedWorld",
        json!({ "frameId": frame_id, "worldName": WORLD_NAME }),
        session_id,
        deadline,
    )?;
    let context_id = world
        .get("executionContextId")
        .and_then(Value::as_i64)
        .ok_or_else(|| {
            transport_failed(
                "Page.createIsolatedWorld",
                "reply names no execution context",
            )
        })?;
    let state = connection.call(
        "Runtime.evaluate",
        json!({
            "expression": "document.visibilityState",
            "contextId": context_id,
            "returnByValue": true,
        }),
        session_id,
        deadline,
    )?;
    state
        .pointer("/result/value")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| transport_failed("Runtime.evaluate", "visibilityState was not a string"))
}

#[cfg(test)]
#[path = "page_visibility_tests.rs"]
mod tests;
