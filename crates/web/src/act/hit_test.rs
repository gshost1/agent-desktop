use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode, Point};
use serde_json::{Value, json};

use super::element_errors::{node_error, script_failed};
use crate::cdp::protocol_error::is_protocol_error;
use crate::{CdpConnection, ElementToken, PageSession};

const OBJECT_GROUP: &str = "agent-desktop-hit";

const CONTAINS_HIT: &str = r"function(hit) {
  for (let node = hit; node; node = node.parentNode || node.host) {
    if (node === this) return true;
  }
  return false;
}";

/// Proves the page would deliver a press at `point` to the token's node:
/// the node `DOM.getNodeForLocation` reports there must be the node itself
/// or one of its descendants, across shadow roots. A point an overlay, a
/// banner or another element covers is `ACTION_FAILED` and nothing is sent.
pub(crate) fn confirm_hit(
    connection: &mut CdpConnection,
    session: &PageSession,
    token: &ElementToken,
    point: &Point,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let located = connection.call(
        "DOM.getNodeForLocation",
        json!({
            "x": point.x.round() as i64,
            "y": point.y.round() as i64,
            "includeUserAgentShadowDOM": true,
            "ignorePointerEventsNone": false,
        }),
        Some(&session.session_id),
        deadline,
    );
    let hit = match located {
        Ok(reply) => reply.get("backendNodeId").and_then(Value::as_i64),
        Err(error) if is_protocol_error(&error) => None,
        Err(error) => return Err(error),
    };
    let Some(hit) = hit else {
        return Err(intercepted(token, point, None));
    };
    if hit == token.backend_node_id || hit_is_inside(connection, session, token, hit, deadline)? {
        return Ok(());
    }
    Err(intercepted(token, point, Some(hit)))
}

fn hit_is_inside(
    connection: &mut CdpConnection,
    session: &PageSession,
    token: &ElementToken,
    hit: i64,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    let target = resolve(connection, session, token.backend_node_id, deadline)
        .map_err(|error| node_error(error, token))?;
    let inside = resolve(connection, session, hit, deadline).and_then(|hit_object| {
        let reply = connection.call(
            "Runtime.callFunctionOn",
            json!({
                "objectId": target,
                "functionDeclaration": CONTAINS_HIT,
                "arguments": [{ "objectId": hit_object }],
                "returnByValue": true,
            }),
            Some(&session.session_id),
            deadline,
        );
        release(connection, session, &hit_object, deadline);
        reply
    });
    release(connection, session, &target, deadline);
    let reply = inside?;
    if let Some(exception) = reply.get("exceptionDetails") {
        return Err(script_failed(exception));
    }
    Ok(reply.pointer("/result/value").and_then(Value::as_bool) == Some(true))
}

fn resolve(
    connection: &mut CdpConnection,
    session: &PageSession,
    backend_node_id: i64,
    deadline: Deadline,
) -> Result<String, AdapterError> {
    let reply = connection.call(
        "DOM.resolveNode",
        json!({ "backendNodeId": backend_node_id, "objectGroup": OBJECT_GROUP }),
        Some(&session.session_id),
        deadline,
    )?;
    reply
        .pointer("/object/objectId")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| AdapterError::internal("DOM.resolveNode returned no object"))
}

fn release(connection: &mut CdpConnection, session: &PageSession, object_id: &str, deadline: Deadline) {
    let _ = connection.call(
        "Runtime.releaseObject",
        json!({ "objectId": object_id }),
        Some(&session.session_id),
        deadline,
    );
}

fn intercepted(token: &ElementToken, point: &Point, hit: Option<i64>) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        format!(
            "Another element covers DOM node {} at the point the click would land",
            token.backend_node_id
        ),
    )
    .with_suggestion(
        "Close the overlay or dialog that covers the element, or scroll it clear, then snapshot again",
    )
    .with_details(json!({
        "kind": "cdp_click_intercepted",
        "backend_node_id": token.backend_node_id,
        "hit_backend_node_id": hit,
        "point": { "x": point.x, "y": point.y },
    }))
    .with_disposition(DeliverySemantics::not_delivered())
}

#[cfg(test)]
#[path = "hit_test_tests.rs"]
mod tests;
